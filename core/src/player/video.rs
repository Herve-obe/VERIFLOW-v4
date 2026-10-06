//! Décodage vidéo image par image via FFmpeg (charte §7.3).
//!
//! Principe : un processus FFmpeg décode à partir d'une image donnée et
//! fournit des images RGBA à la taille d'affichage. Un fil de lecture anticipée
//! garde quelques images d'avance pour une lecture fluide, et un cache des
//! images récentes rend le retour en arrière instantané.

use std::collections::VecDeque;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::mpsc::{sync_channel, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;

use crate::media::probe::{probe, MediaInfo};
use crate::{tools, Error, Result};

/// Nombre d'images lues d'avance.
const PREFETCH: usize = 8;
/// Nombre d'images récentes conservées (retour arrière instantané).
const CACHE: usize = 24;
/// Au-delà de cet écart vers l'avant, on relance le décodage par un saut.
const MAX_SKIP: i64 = 12;
/// Marge de recul avant la cible lors d'un saut (couvre le réordonnancement
/// des images B des codecs à GOP long).
const PREROLL_SECONDS: f64 = 1.0;

/// Codecs où chaque image est une image clé : le saut est direct.
const INTRA_CODECS: &[&str] = &[
    "prores", "dnxhd", "mjpeg", "jpeg2000", "ffv1", "cfhd", "rawvideo", "v210", "v410", "r10k",
    "r210", "png", "tiff", "dpx", "exr", "hap", "qtrle", "utvideo",
];

/// Repères temporels du flux vidéo.
#[derive(Clone, Copy)]
struct Timing {
    start_time: f64,
    frame_duration: f64,
    preroll: f64,
}

/// Image décodée (RGBA brut ou JPEG selon `FrameFormat`).
pub type FrameData = Arc<Vec<u8>>;

/// Format des images produites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameFormat {
    /// RGBA 8 bits brut : exact, mais lourd à transférer (3,6 Mo en 1280x720).
    Rgba,
    /// JPEG qualité maximale en 4:4:4 : environ 20 fois plus léger, pour l'aperçu.
    Jpeg,
}

/// Lit une image JPEG complète (de SOI à EOI) dans un flux MJPEG.
/// Dans les données compressées, l'octet 0xFF est toujours suivi de 0x00 :
/// le marqueur de fin 0xFFD9 ne peut donc apparaître qu'en fin d'image.
fn read_jpeg(reader: &mut impl Read) -> std::io::Result<Option<Vec<u8>>> {
    let mut out = Vec::with_capacity(256 * 1024);
    let mut byte = [0u8; 1];
    let mut prev = 0u8;
    loop {
        match reader.read(&mut byte) {
            Ok(0) => return Ok(None),
            Ok(_) => {
                out.push(byte[0]);
                if prev == 0xFF && byte[0] == 0xD9 && out.len() > 4 {
                    return Ok(Some(out));
                }
                prev = byte[0];
            }
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        }
    }
}

/// Calcule une taille d'affichage contenue dans `max_w` x `max_h`,
/// en conservant les proportions et avec des dimensions paires.
pub fn fit_size(width: u32, height: u32, max_w: u32, max_h: u32) -> (u32, u32) {
    if width == 0 || height == 0 {
        return (max_w & !1, max_h & !1);
    }
    let scale = (max_w as f64 / width as f64)
        .min(max_h as f64 / height as f64)
        .min(1.0);
    // Arrondi au nombre pair le plus proche (exigé par la mise à l'échelle).
    let even = |v: f64| (((v / 2.0).round() as u32) * 2).max(2);
    let w = even(width as f64 * scale);
    let h = even(height as f64 * scale);
    (w, h)
}

/// Processus FFmpeg qui produit les images à partir de `start`.
struct Decoder {
    child: Arc<Mutex<Child>>,
    rx: Receiver<Result<FrameData>>,
    next: i64,
}

impl Decoder {
    fn spawn(
        path: &Path,
        start: i64,
        timing: Timing,
        size: (u32, u32),
        format: FrameFormat,
        lut: Option<&Path>,
    ) -> Result<Self> {
        // Saut précis à l'image, y compris en GOP long avec images B :
        // 1. on se place une seconde avant la cible (FFmpeg repart de l'image
        //    clé précédente) en gardant les horodatages d'origine (-copyts) ;
        // 2. le filtre `select` ne laisse passer que les images dont
        //    l'horodatage atteint la cible. On vise le milieu de l'image
        //    précédente pour être insensible aux arrondis.
        let target = timing.start_time + (start as f64 - 0.5) * timing.frame_duration;
        let seek = (target - timing.preroll).max(0.0);
        let mut filter = format!(
            "select=gte(t\\,{target:.6}),scale={}:{}:flags=bilinear",
            size.0, size.1
        );
        // LUT d'affichage appliquée sur l'image réduite (rapide), jamais au fichier.
        if let Some(lut) = lut {
            filter += &lut_filter(lut);
        }
        let mut child = tools::command("ffmpeg")?
            .args(["-v", "error", "-nostdin", "-copyts", "-noaccurate_seek"])
            .args(["-ss", &format!("{seek:.6}"), "-i"])
            .arg(path)
            .args(["-map", "0:v:0", "-an", "-sn", "-vf", &filter])
            .args(match format {
                FrameFormat::Rgba => &["-f", "rawvideo", "-pix_fmt", "rgba", "-"][..],
                FrameFormat::Jpeg => &[
                    "-c:v",
                    "mjpeg",
                    "-q:v",
                    "2",
                    "-pix_fmt",
                    "yuvj444p",
                    "-f",
                    "image2pipe",
                    "-",
                ][..],
            })
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let stdout = child.stdout.take().expect("stdout redirigé");
        let frame_size = (size.0 * size.1 * 4) as usize;
        let (tx, rx) = sync_channel(PREFETCH);
        thread::spawn(move || {
            let mut stdout = std::io::BufReader::with_capacity(1 << 20, stdout);
            loop {
                let frame = match format {
                    FrameFormat::Rgba => {
                        let mut buf = vec![0u8; frame_size];
                        match stdout.read_exact(&mut buf) {
                            Ok(()) => Ok(Some(buf)),
                            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => Ok(None),
                            Err(e) => Err(e),
                        }
                    }
                    FrameFormat::Jpeg => read_jpeg(&mut stdout),
                };
                match frame {
                    Ok(Some(buf)) => {
                        if tx.send(Ok(Arc::new(buf))).is_err() {
                            break; // décodeur abandonné
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = tx.send(Err(e.into()));
                        break;
                    }
                }
            }
        });
        Ok(Self {
            child: Arc::new(Mutex::new(child)),
            rx,
            next: start,
        })
    }

    /// Image suivante, ou `None` en fin de fichier.
    fn read(&mut self) -> Result<Option<FrameData>> {
        match self.rx.recv() {
            Ok(Ok(frame)) => {
                self.next += 1;
                Ok(Some(frame))
            }
            Ok(Err(e)) => Err(e),
            Err(_) => Ok(None),
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

/// Filtre `lut3d` pour un fichier LUT (.cube, .3dl...). FFmpeg demande deux
/// niveaux d'échappement : valeur de l'option (`\` `'` `:`), puis graphe de
/// filtres (`\` `'` `[` `]` `,` `;`). Les barres obliques inverses de Windows
/// sont remplacées par des barres obliques, acceptées par FFmpeg.
fn lut_filter(lut: &Path) -> String {
    let path = lut.display().to_string().replace('\\', "/");
    let mut option = String::new();
    for c in path.chars() {
        if matches!(c, '\\' | '\'' | ':') {
            option.push('\\');
        }
        option.push(c);
    }
    let mut graph = String::new();
    for c in option.chars() {
        if matches!(c, '\\' | '\'' | '[' | ']' | ',' | ';') {
            graph.push('\\');
        }
        graph.push(c);
    }
    format!(",lut3d=file={graph}:interp=tetrahedral")
}

/// Vérifie qu'une LUT est lisible par FFmpeg (message d'erreur explicite sinon).
pub fn check_lut(lut: &Path) -> Result<()> {
    if !lut.is_file() {
        return Err(Error::NotFound(lut.display().to_string()));
    }
    let out = tools::command("ffmpeg")?
        .args([
            "-v",
            "error",
            "-nostdin",
            "-f",
            "lavfi",
            "-i",
            "color=size=16x16:duration=0.04",
        ])
        .args(["-frames:v", "1", "-vf"])
        .arg(format!("scale=16:16{}", lut_filter(lut)))
        .args(["-f", "null", "-"])
        .stdin(Stdio::null())
        .output()?;
    if out.status.success() {
        Ok(())
    } else {
        Err(Error::Tool {
            tool: "ffmpeg".into(),
            message: format!(
                "LUT illisible ({}) : {}",
                lut.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        })
    }
}

/// Informations envoyées à l'interface à l'ouverture d'un clip.
#[derive(Debug, Clone, Serialize)]
pub struct VideoClip {
    pub info: MediaInfo,
    pub display_width: u32,
    pub display_height: u32,
    pub frame_count: i64,
    pub start_frame: i64,
}

/// Lecteur vidéo d'un clip.
pub struct VideoPlayer {
    path: PathBuf,
    clip: VideoClip,
    timing: Timing,
    format: FrameFormat,
    decoder: Option<Decoder>,
    cache: VecDeque<(i64, FrameData)>,
    lut: Option<PathBuf>,
}

impl VideoPlayer {
    /// Ouvre un clip ; les images seront mises à l'échelle dans `max_w` x `max_h`.
    pub fn open(path: &Path, max_w: u32, max_h: u32, format: FrameFormat) -> Result<Self> {
        let info = probe(path)?;
        let video = info.video.clone().ok_or_else(|| {
            Error::Unsupported(format!("{} : aucune piste vidéo", path.display()))
        })?;
        let (display_width, display_height) = fit_size(video.width, video.height, max_w, max_h);
        let start_frame = info.start_tc().map(|tc| tc.frames).unwrap_or(0);
        Ok(Self {
            path: path.to_path_buf(),
            timing: Timing {
                start_time: video.start_time,
                frame_duration: video.rate.frame_duration(),
                preroll: if INTRA_CODECS.contains(&video.codec.as_str()) {
                    0.0
                } else {
                    PREROLL_SECONDS
                },
            },
            clip: VideoClip {
                frame_count: video.frame_count,
                info,
                display_width,
                display_height,
                start_frame,
            },
            format,
            decoder: None,
            cache: VecDeque::with_capacity(CACHE),
            lut: None,
        })
    }

    pub fn clip(&self) -> &VideoClip {
        &self.clip
    }

    /// Applique (ou retire avec `None`) une LUT d'affichage. Les images déjà
    /// décodées sont oubliées.
    pub fn set_lut(&mut self, lut: Option<&Path>) -> Result<()> {
        if let Some(l) = lut {
            check_lut(l)?;
        }
        self.lut = lut.map(Path::to_path_buf);
        self.cache.clear();
        self.decoder = None;
        Ok(())
    }

    pub fn lut(&self) -> Option<&Path> {
        self.lut.as_deref()
    }

    fn remember(&mut self, index: i64, frame: FrameData) {
        if self.cache.len() == CACHE {
            self.cache.pop_front();
        }
        self.cache.push_back((index, frame));
    }

    /// Renvoie l'image `index` (0 = première image du clip).
    pub fn frame(&mut self, index: i64) -> Result<Option<FrameData>> {
        let last = self.clip.frame_count.saturating_sub(1).max(0);
        let index = index.clamp(0, last);
        if let Some((_, f)) = self.cache.iter().rev().find(|(i, _)| *i == index) {
            return Ok(Some(f.clone()));
        }
        let reuse =
            matches!(&self.decoder, Some(d) if index >= d.next && index - d.next <= MAX_SKIP);
        if !reuse {
            self.decoder = Some(Decoder::spawn(
                &self.path,
                index,
                self.timing,
                (self.clip.display_width, self.clip.display_height),
                self.format,
                self.lut.as_deref(),
            )?);
        }
        loop {
            let decoder = self.decoder.as_mut().expect("décodeur actif");
            let current = decoder.next;
            match decoder.read()? {
                Some(frame) => {
                    self.remember(current, frame.clone());
                    if current == index {
                        return Ok(Some(frame));
                    }
                }
                None => {
                    // Fin de flux avant l'image demandée (nombre d'images estimé
                    // trop grand) : on renvoie la dernière image disponible.
                    self.decoder = None;
                    return Ok(self.cache.back().map(|(_, f)| f.clone()));
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_mjpeg_stream() {
        let a = [0xFF, 0xD8, 1, 0xFF, 0x00, 2, 0xFF, 0xD9];
        let b = [0xFF, 0xD8, 3, 4, 0xFF, 0xD9];
        let stream: Vec<u8> = a.iter().chain(b.iter()).copied().collect();
        let mut r = std::io::Cursor::new(stream);
        assert_eq!(read_jpeg(&mut r).unwrap().unwrap(), a);
        assert_eq!(read_jpeg(&mut r).unwrap().unwrap(), b);
        assert!(read_jpeg(&mut r).unwrap().is_none());
    }

    #[test]
    fn lut_paths_are_escaped_for_ffmpeg() {
        assert_eq!(
            lut_filter(Path::new("C:\\LUTs\\a b.cube")),
            ",lut3d=file=C\\\\:/LUTs/a b.cube:interp=tetrahedral"
        );
        if tools::locate("ffmpeg").is_none() {
            return;
        }
        // Nom réel contenant « : » et « , » (possible sous Linux et macOS).
        let dir = tempfile::tempdir().unwrap();
        let lut = dir.path().join("Rec709:LogC,v2.cube");
        let mut cube = String::from("LUT_3D_SIZE 2\n");
        for b in 0..2 {
            for g in 0..2 {
                for r in 0..2 {
                    cube += &format!("{r} {g} {b}\n");
                }
            }
        }
        std::fs::write(&lut, cube).unwrap();
        check_lut(&lut).unwrap();
    }

    #[test]
    fn fit_keeps_aspect_and_even_sizes() {
        assert_eq!(fit_size(1920, 1080, 1280, 720), (1280, 720));
        assert_eq!(fit_size(4096, 2160, 1280, 720), (1280, 676));
        assert_eq!(fit_size(720, 576, 1280, 720), (720, 576));
        assert_eq!(fit_size(1080, 1920, 1280, 720), (406, 720));
    }
}
