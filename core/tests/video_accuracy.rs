//! Vérifie la précision à l'image du lecteur vidéo sur des clips générés.
//! Ignoré automatiquement si FFmpeg n'est pas installé.

use std::path::Path;
use std::process::Command;

use veriflow_core::player::video::{FrameFormat, VideoPlayer};
use veriflow_core::tools;

fn make_clip(dir: &Path, name: &str, rate: &str, codec: &[&str]) -> std::path::PathBuf {
    let path = dir.join(name);
    let status = Command::new(tools::locate("ffmpeg").unwrap())
        .args(["-v", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=size=320x180:rate={rate}:duration=4"))
        .args(codec)
        .arg(&path)
        .status()
        .unwrap();
    assert!(status.success());
    path
}

fn check_seek_accuracy(path: &Path) {
    // Référence : lecture continue de toutes les images.
    let mut reference = VideoPlayer::open(path, 320, 180, FrameFormat::Rgba).unwrap();
    let count = reference.clip().frame_count;
    assert!(count > 90, "clip trop court : {count}");
    let frames: Vec<_> = (0..count)
        .map(|i| reference.frame(i).unwrap().unwrap())
        .collect();

    // Sauts aléatoires (y compris en arrière et hors des images clés).
    for &target in &[37, 3, 88, 12, 13, 11, 60, 0, count - 1] {
        let mut player = VideoPlayer::open(path, 320, 180, FrameFormat::Rgba).unwrap();
        let got = player.frame(target).unwrap().unwrap();
        assert!(
            got == frames[target as usize],
            "{} : image {target} incorrecte après saut",
            path.display()
        );
    }

    // Pas à pas arrière sur un même lecteur (cache), puis avant.
    let mut player = VideoPlayer::open(path, 320, 180, FrameFormat::Rgba).unwrap();
    for i in (40..=50).rev() {
        assert!(player.frame(i).unwrap().unwrap() == frames[i as usize]);
    }
    for i in 50..=70 {
        assert!(player.frame(i).unwrap().unwrap() == frames[i as usize]);
    }
}

#[test]
fn frame_accurate_seeks() {
    if tools::locate("ffmpeg").is_none() || tools::locate("ffprobe").is_none() {
        eprintln!("FFmpeg absent : test ignoré");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // GOP long (image clé toutes les 12 images) : cas le plus difficile.
    let long_gop = make_clip(
        dir.path(),
        "gop.mp4",
        "25",
        &["-c:v", "mpeg4", "-g", "12", "-bf", "2", "-q:v", "4"],
    );
    check_seek_accuracy(&long_gop);
    // MPEG-2 en flux programme (horodatage de départ non nul).
    let long_gop = make_clip(
        dir.path(),
        "gop.mpg",
        "25",
        &["-c:v", "mpeg2video", "-g", "12", "-q:v", "4"],
    );
    check_seek_accuracy(&long_gop);
    // Cadence NTSC 29.97 en tout-intra.
    let ntsc = make_clip(
        dir.path(),
        "ntsc.mov",
        "30000/1001",
        &["-c:v", "mjpeg", "-q:v", "3"],
    );
    check_seek_accuracy(&ntsc);
}

#[test]
fn display_lut_is_applied_and_removed() {
    if tools::locate("ffmpeg").is_none() || tools::locate("ffprobe").is_none() {
        eprintln!("FFmpeg absent : test ignoré");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let clip = make_clip(dir.path(), "lut.mov", "25", &["-c:v", "mjpeg", "-q:v", "2"]);
    // LUT 2x2x2 qui inverse les couleurs (rouge varie le plus vite).
    let mut cube = String::from("TITLE \"inverse\"\nLUT_3D_SIZE 2\n");
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                cube += &format!("{} {} {}\n", 1 - r, 1 - g, 1 - b);
            }
        }
    }
    // Chemin avec espace et apostrophe : vérifie l'échappement du filtre.
    let lut = dir.path().join("L'inverse 1.cube");
    std::fs::write(&lut, cube).unwrap();

    let mut p = VideoPlayer::open(&clip, 320, 180, FrameFormat::Rgba).unwrap();
    let plain = p.frame(10).unwrap().unwrap();
    p.set_lut(Some(&lut)).unwrap();
    let graded = p.frame(10).unwrap().unwrap();
    // Chaque composante devient environ 255 - composante (tolérance de compression).
    let mut err = 0u64;
    for (a, b) in plain.chunks(4).zip(graded.chunks(4)) {
        for c in 0..3 {
            err += (255 - a[c] as i64 - b[c] as i64).unsigned_abs();
        }
    }
    let mean = err as f64 / (plain.len() / 4 * 3) as f64;
    assert!(mean < 6.0, "écart moyen à l'inverse : {mean}");
    p.set_lut(None).unwrap();
    assert_eq!(p.frame(10).unwrap().unwrap(), plain);

    let bad = dir.path().join("faux.cube");
    std::fs::write(&bad, "pas une LUT").unwrap();
    assert!(p.set_lut(Some(&bad)).is_err());
}
