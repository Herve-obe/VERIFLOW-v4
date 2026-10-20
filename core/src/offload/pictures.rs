//! Images du rapport PDF : logo de la production et vignettes des vidéos,
//! préparées par FFmpeg en JPEG (inséré tel quel dans le PDF, sans
//! décodage ni bibliothèque d'image).

use std::path::Path;
use std::process::Stdio;

use crate::media::probe::probe;
use crate::tools;

/// Image JPEG prête à insérer.
pub struct Jpeg {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Nombre de composantes (1 : gris, 3 : couleur).
    pub components: u8,
}

impl Jpeg {
    fn new(data: Vec<u8>) -> Option<Self> {
        let (width, height, components) = jpeg_size(&data)?;
        Some(Self {
            data,
            width,
            height,
            components,
        })
    }
}

/// Lance FFmpeg et récupère le JPEG écrit sur sa sortie.
fn ffmpeg_jpeg(before: &[String], input: &Path, filter: &str) -> Option<Jpeg> {
    let out = tools::command("ffmpeg")
        .ok()?
        .args(["-v", "error", "-nostdin"])
        .args(before)
        .arg("-i")
        .arg(input)
        .args(["-frames:v", "1", "-vf", filter, "-q:v", "3"])
        .args(["-f", "image2pipe", "-c:v", "mjpeg", "-"])
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Jpeg::new(out.stdout)
}

/// Logo (PNG, JPEG, TIFF...) réduit à 800 pixels de large au plus ; la
/// transparence devient du blanc (le fond de la page).
pub fn logo(path: &Path) -> Option<Jpeg> {
    ffmpeg_jpeg(
        &[],
        path,
        "scale='min(800,iw)':-2,format=rgba,split[a][b];[a]drawbox=c=white:t=fill[bg];\
         [bg][b]overlay,format=yuvj444p",
    )
}

/// Vignette d'une vidéo (image à 10 % de la durée, 10 s au plus), 240
/// pixels de large.
pub fn thumbnail(path: &Path) -> Option<Jpeg> {
    let d = probe(path).ok()?.duration;
    let at = (d * 0.1).min(10.0).min((d - 0.1).max(0.0));
    ffmpeg_jpeg(
        &["-ss".into(), format!("{at:.3}")],
        path,
        "scale=240:-2,format=yuvj420p",
    )
}

/// Largeur, hauteur et nombre de composantes d'un JPEG (segment SOF).
pub fn jpeg_size(data: &[u8]) -> Option<(u32, u32, u8)> {
    if data.get(..2)? != [0xFF, 0xD8] {
        return None;
    }
    let mut i = 2;
    while i + 4 <= data.len() {
        if data[i] != 0xFF {
            return None;
        }
        let marker = data[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let len = u16::from_be_bytes([data[i + 2], data[i + 3]]) as usize;
        // SOF0 à SOF15, sauf DHT (C4), JPG (C8) et DAC (CC).
        if (0xC0..=0xCF).contains(&marker) && ![0xC4, 0xC8, 0xCC].contains(&marker) {
            let s = data.get(i + 4..i + 10)?;
            let h = u16::from_be_bytes([s[1], s[2]]) as u32;
            let w = u16::from_be_bytes([s[3], s[4]]) as u32;
            return (w > 0 && h > 0).then_some((w, h, s[5]));
        }
        i += 2 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ffmpeg_available() -> bool {
        tools::locate("ffmpeg").is_some() && tools::locate("ffprobe").is_some()
    }

    #[test]
    fn reads_jpeg_size() {
        // En-tête minimal : SOI, APP0 vide, SOF0 320×180 en 3 composantes.
        let mut j = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x02];
        j.extend([0xFF, 0xC0, 0x00, 0x11, 8, 0x00, 180, 0x01, 0x40, 3]);
        assert_eq!(jpeg_size(&j), Some((320, 180, 3)));
        assert_eq!(jpeg_size(b"PNG"), None);
    }

    #[test]
    fn logo_and_thumbnail() {
        if !ffmpeg_available() {
            eprintln!("FFmpeg absent : test ignoré");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let run = |args: &[&str]| {
            let out = tools::command("ffmpeg")
                .unwrap()
                .args(["-v", "error", "-y"])
                .args(args)
                .output()
                .unwrap();
            assert!(
                out.status.success(),
                "{}",
                String::from_utf8_lossy(&out.stderr)
            );
        };
        // Logo transparent 1200×300 avec un carré bleu opaque au centre.
        let png = dir.path().join("logo.png");
        run(&[
            "-f",
            "lavfi",
            "-i",
            "color=c=blue:s=400x100,format=rgba",
            "-f",
            "lavfi",
            "-i",
            "color=c=black@0:s=1200x300,format=rgba",
            "-filter_complex",
            "[1][0]overlay=400:100",
            "-frames:v",
            "1",
            png.to_str().unwrap(),
        ]);
        let l = logo(&png).expect("logo converti");
        assert_eq!((l.width, l.height), (800, 200), "réduit à 800 px");
        // Coin (transparent) blanc, centre bleu : relu par FFmpeg.
        let jpg = dir.path().join("logo.jpg");
        std::fs::write(&jpg, &l.data).unwrap();
        let pixel = |x: u32, y: u32| -> Vec<u8> {
            tools::command("ffmpeg")
                .unwrap()
                .args(["-v", "error", "-i"])
                .arg(&jpg)
                .args(["-vf", &format!("crop=1:1:{x}:{y},format=rgb24")])
                .args(["-f", "rawvideo", "-"])
                .output()
                .unwrap()
                .stdout
        };
        assert!(pixel(10, 10).iter().all(|&v| v > 245), "fond blanc");
        let c = pixel(400, 100);
        assert!(c[2] > 200 && c[0] < 40 && c[1] < 40, "centre bleu : {c:?}");

        let clip = dir.path().join("A001C001.mov");
        run(&[
            "-f",
            "lavfi",
            "-i",
            "testsrc2=size=1920x1080:rate=25:duration=2",
            "-c:v",
            "mjpeg",
            clip.to_str().unwrap(),
        ]);
        let t = thumbnail(&clip).expect("vignette");
        assert_eq!((t.width, t.height), (240, 136));
        assert_eq!(t.components, 3);
        assert!(thumbnail(&png.with_extension("absent")).is_none());
    }
}
