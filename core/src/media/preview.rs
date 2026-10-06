//! Aperçus pour l'onglet MEDIA (charte §7.2) : vignette, bande d'images
//! (filmstrip) et forme d'onde. Résultats mis en cache sur disque, indexés
//! par chemin, taille et date du fichier : un média modifié est recalculé.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::catalog::{kind_of, MediaKind};
use super::probe::probe;
use super::wav::WavReader;
use crate::offload::hash::{hash_data, HashAlgo};
use crate::{tools, Error, Result};

/// Nombre d'images de la bande d'aperçu.
pub const FILMSTRIP_FRAMES: usize = 8;
/// Nombre de points de la forme d'onde.
pub const WAVEFORM_POINTS: usize = 600;

/// Clé de cache d'un fichier (change si le fichier est modifié).
pub fn cache_key(path: &Path) -> String {
    let meta = fs::metadata(path).ok();
    let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
    let mtime = meta
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    hash_data(
        HashAlgo::Xxh128,
        format!("{}\n{size}\n{mtime}", path.display()).as_bytes(),
    )
}

fn run_ffmpeg(args: &[String]) -> Result<()> {
    let out = tools::command("ffmpeg")?
        .args(["-v", "error", "-nostdin", "-y"])
        .args(args)
        .stdin(Stdio::null())
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Tool {
            tool: "ffmpeg".into(),
            message: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        })
    }
}

/// Extrait une image JPEG à `seconds` (saut rapide), largeur `width`.
fn grab(path: &Path, seconds: f64, width: u32, out: &Path) -> Result<()> {
    let tmp = out.with_extension("tmp.jpg");
    run_ffmpeg(&[
        "-ss".into(),
        format!("{:.3}", seconds.max(0.0)),
        "-i".into(),
        path.display().to_string(),
        "-frames:v".into(),
        "1".into(),
        "-vf".into(),
        format!("scale={width}:-2"),
        "-q:v".into(),
        "4".into(),
        tmp.display().to_string(),
    ])?;
    fs::rename(&tmp, out)?;
    Ok(())
}

fn duration_of(path: &Path) -> f64 {
    probe(path).map(|p| p.duration).unwrap_or(0.0)
}

/// Vignette d'un média vidéo ou image (image à 10 % de la durée).
pub fn thumbnail(path: &Path, cache: &Path) -> Result<PathBuf> {
    let out = cache.join(format!("{}_thumb.jpg", cache_key(path)));
    if out.exists() {
        return Ok(out);
    }
    fs::create_dir_all(cache)?;
    let at = match kind_of(path) {
        Some(MediaKind::Image) => 0.0,
        _ => {
            let d = duration_of(path);
            (d * 0.1).min(10.0).min((d - 0.1).max(0.0))
        }
    };
    grab(path, at, 320, &out)?;
    Ok(out)
}

/// Bande d'images réparties sur la durée (aperçu au survol).
pub fn filmstrip(path: &Path, cache: &Path) -> Result<Vec<PathBuf>> {
    fs::create_dir_all(cache)?;
    let key = cache_key(path);
    let files: Vec<PathBuf> = (0..FILMSTRIP_FRAMES)
        .map(|i| cache.join(format!("{key}_strip{i}.jpg")))
        .collect();
    if files.iter().all(|f| f.exists()) {
        return Ok(files);
    }
    let d = duration_of(path);
    if d <= 0.0 {
        return Err(Error::Unsupported(format!(
            "{} : durée inconnue",
            path.display()
        )));
    }
    for (i, f) in files.iter().enumerate() {
        if !f.exists() {
            let t = d * (i as f64 + 0.5) / FILMSTRIP_FRAMES as f64;
            grab(path, t, 200, f)?;
        }
    }
    Ok(files)
}

/// Forme d'onde : crête absolue (0 à 1) de chaque segment, toutes pistes confondues.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Waveform {
    pub peaks: Vec<f32>,
    pub duration: f64,
}

fn peaks_from<F: FnMut(&mut Vec<f32>) -> Result<usize>>(
    total: u64,
    channels: usize,
    mut next: F,
) -> Result<Vec<f32>> {
    let mut peaks = vec![0f32; WAVEFORM_POINTS];
    let per = (total as f64 / WAVEFORM_POINTS as f64).max(1.0);
    let mut buf = Vec::new();
    let mut frame = 0u64;
    loop {
        let n = next(&mut buf)?;
        if n == 0 {
            break;
        }
        for chunk in buf.chunks_exact(channels.max(1)) {
            let idx = ((frame as f64 / per) as usize).min(WAVEFORM_POINTS - 1);
            let m = chunk.iter().fold(0f32, |a, s| a.max(s.abs()));
            if m > peaks[idx] {
                peaks[idx] = m;
            }
            frame += 1;
        }
    }
    Ok(peaks)
}

/// Calcule (ou relit en cache) la forme d'onde d'un média audio.
pub fn waveform(path: &Path, cache: &Path) -> Result<Waveform> {
    let out = cache.join(format!("{}_wave.json", cache_key(path)));
    if let Some(w) = fs::read(&out)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
    {
        return Ok(w);
    }
    fs::create_dir_all(cache)?;
    let wave = match WavReader::open(path) {
        // WAV / BWF / RF64 : lecture directe, rapide.
        Ok(mut r) => {
            let info = r.info().clone();
            let mut pos = 0u64;
            let peaks = peaks_from(info.frames, info.channels as usize, |buf| {
                let n = r.read(pos, 65_536, buf)?;
                pos += n as u64;
                Ok(n)
            })?;
            Waveform {
                peaks,
                duration: info.duration(),
            }
        }
        // Autres formats : décodage FFmpeg en mono 8 kHz.
        Err(_) => {
            let duration = duration_of(path);
            let mut child = tools::command("ffmpeg")?
                .args(["-v", "error", "-nostdin", "-i"])
                .arg(path)
                .args(["-vn", "-ac", "1", "-ar", "8000", "-f", "f32le", "-"])
                .stdin(Stdio::null())
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()?;
            let mut stdout = child.stdout.take().expect("stdout redirigé");
            let total = (duration * 8000.0) as u64;
            // Une lecture de tuyau peut couper un échantillon : le reste est conservé.
            let mut carry: Vec<u8> = Vec::new();
            let peaks = peaks_from(total.max(1), 1, |buf| {
                buf.clear();
                let mut raw = vec![0u8; 65_536 * 4];
                loop {
                    let n = stdout.read(&mut raw)?;
                    carry.extend_from_slice(&raw[..n]);
                    if n == 0 || carry.len() >= 4 {
                        break;
                    }
                }
                let whole = carry.len() - carry.len() % 4;
                buf.extend(
                    carry[..whole]
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .map(|c| f32::from_le_bytes(*c)),
                );
                carry.drain(..whole);
                Ok(buf.len())
            })?;
            let _ = child.wait();
            Waveform { peaks, duration }
        }
    };
    fs::write(&out, serde_json::to_vec(&wave).unwrap_or_default())?;
    Ok(wave)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::wav::write_test_wav;

    #[test]
    fn waveform_of_wav_is_cached_and_follows_signal() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("a.wav");
        // Première moitié à 0.5, seconde moitié silencieuse.
        write_test_wav(&wav, 2, 48_000, 24, false, 96_000, None, |n, _| {
            if n < 48_000 {
                0.5
            } else {
                0.0
            }
        })
        .unwrap();
        let cache = dir.path().join("cache");
        let w = waveform(&wav, &cache).unwrap();
        assert_eq!(w.peaks.len(), WAVEFORM_POINTS);
        assert!((w.duration - 2.0).abs() < 1e-6);
        assert!((w.peaks[10] - 0.5).abs() < 1e-3);
        assert_eq!(w.peaks[WAVEFORM_POINTS - 10], 0.0);
        assert_eq!(waveform(&wav, &cache).unwrap(), w, "relu en cache");
        assert_eq!(fs::read_dir(&cache).unwrap().count(), 1);
    }

    #[test]
    fn thumbnails_and_filmstrip_with_ffmpeg() {
        if tools::locate("ffmpeg").is_none() || tools::locate("ffprobe").is_none() {
            eprintln!("FFmpeg absent : test ignoré");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("clip.mov");
        run_ffmpeg(&[
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "testsrc2=size=640x360:rate=25:duration=4".into(),
            "-c:v".into(),
            "mjpeg".into(),
            clip.display().to_string(),
        ])
        .unwrap();
        let cache = dir.path().join("cache");
        // Forme d'onde d'un format décodé par FFmpeg : 1 s de sinus puis 1 s de silence.
        let flac = dir.path().join("son.flac");
        run_ffmpeg(&[
            "-f".into(),
            "lavfi".into(),
            "-i".into(),
            "sine=frequency=440:sample_rate=48000:duration=1,apad=whole_dur=2".into(),
            flac.display().to_string(),
        ])
        .unwrap();
        let w = waveform(&flac, &cache).unwrap();
        assert!((w.duration - 2.0).abs() < 0.05, "{}", w.duration);
        assert!(w.peaks[50] > 0.05, "début audible");
        assert!(w.peaks[WAVEFORM_POINTS - 20] < 1e-3, "fin silencieuse");
        let t = thumbnail(&clip, &cache).unwrap();
        assert!(fs::read(&t).unwrap().starts_with(&[0xFF, 0xD8]));
        let strip = filmstrip(&clip, &cache).unwrap();
        assert_eq!(strip.len(), FILMSTRIP_FRAMES);
        assert!(strip.iter().all(|f| f.exists()));
        // Clé de cache stable, changée si le fichier change.
        let k = cache_key(&clip);
        assert_eq!(k, cache_key(&clip));
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&clip, b"autre").unwrap();
        assert_ne!(k, cache_key(&clip));
    }
}
