//! Essais de conversion réels (FFmpeg requis ; ignorés sinon).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::*;
use crate::media::wav::{read_info, write_test_wav};

fn ffmpeg_available() -> bool {
    tools::locate("ffmpeg").is_some() && tools::locate("ffprobe").is_some()
}

fn ffmpeg(args: &[&str]) {
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
}

/// Vidéo de 2 s avec timecode 10:00:00:00 et deux pistes son mono.
fn clip(dir: &Path) -> PathBuf {
    let p = dir.join("A001C001.mov");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=640x360:rate=25:duration=2",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=440:sample_rate=48000:duration=2",
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=880:sample_rate=48000:duration=2",
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-map",
        "2:a",
        "-c:v",
        "mjpeg",
        "-c:a",
        "pcm_s24le",
        "-timecode",
        "10:00:00:00",
        p.to_str().unwrap(),
    ]);
    p
}

fn request(sources: Vec<PathBuf>, preset: &str, dest: &Path) -> Request {
    Request {
        sources,
        settings: Settings {
            preset: preset.into(),
            software: true,
            ..Default::default()
        },
        dest: Some(dest.to_path_buf()),
        suffix: String::new(),
        existing: Existing::Rename,
    }
}

fn run(req: &Request) -> Summary {
    let cancel = AtomicBool::new(false);
    let mut last = 0.0;
    let summary = execute(req, &cancel, |e| {
        if let Event::Progress { fraction, .. } = e {
            last = fraction;
        }
    })
    .unwrap();
    for f in &summary.files {
        assert_eq!(f.status, Status::Done, "{:?}", f.message);
    }
    assert!(
        last > 0.99 || req.settings.preset == "analyze",
        "progression finale {last}"
    );
    summary
}

#[test]
fn video_presets_produce_playable_files_with_timecode() {
    if !ffmpeg_available() {
        eprintln!("FFmpeg absent : test ignoré");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = clip(dir.path());
    let out = dir.path().join("out");
    for (preset, codec) in [
        ("prores_proxy", "prores"),
        ("dnxhr_lb", "dnxhd"),
        ("h264", "h264"),
        ("rewrap_mkv", "mjpeg"),
    ] {
        let s = run(&request(vec![src.clone()], preset, &out));
        let file = s.files[0].output.clone().unwrap();
        let info = probe(&file).unwrap();
        assert_eq!(info.video.as_ref().unwrap().codec, codec, "{preset}");
        assert_eq!(info.audio.len(), 2, "{preset} : deux pistes son");
        if preset != "rewrap_mkv" {
            assert_eq!(
                info.start_timecode.as_deref(),
                Some("10:00:00:00"),
                "{preset}"
            );
        }
        assert!(!file
            .with_extension(format!(
                "{}.part",
                file.extension().unwrap().to_string_lossy()
            ))
            .exists());
    }
    // Proxy : moitié de la taille.
    let s = run(&request(vec![src.clone()], "proxy_dnxhr", &out));
    let f = s.files[0].output.clone().unwrap();
    assert!(f.file_name().unwrap().to_string_lossy().ends_with(".mov"));
    let v = probe(&f).unwrap().video.unwrap();
    assert_eq!((v.width, v.height), (320, 180));
    // ProRes logiciel signalé non certifié.
    let s = run(&request(vec![src], "prores_hq", &out));
    assert!(s.files[0].encoder.as_ref().unwrap().uncertified_prores);
}

#[test]
fn extracted_sound_is_a_poly_bwf_at_the_video_timecode() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = clip(dir.path());
    let s = run(&request(vec![src], "extract_audio", dir.path()));
    let wav = read_info(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!((wav.channels, wav.sample_rate, wav.bits), (2, 48_000, 24));
    assert_eq!(wav.time_reference, Some(1_728_000_000));
}

#[test]
fn wav_conversion_keeps_ixml_and_rescales_bext() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // WAV d'enregistreur : iXML (scène, prise, noms de pistes).
    let ixml_src = dir.path().join("12A_T3.WAV");
    let ixml = "<BWFXML><SCENE>12A</SCENE><TAKE>3</TAKE><SPEED><FILE_SAMPLE_RATE>48000</FILE_SAMPLE_RATE></SPEED><TRACK_LIST><TRACK><CHANNEL_INDEX>1</CHANNEL_INDEX><NAME>Perche</NAME></TRACK><TRACK><CHANNEL_INDEX>2</CHANNEL_INDEX><NAME>HF</NAME></TRACK></TRACK_LIST></BWFXML>";
    write_test_wav(
        &ixml_src,
        2,
        48_000,
        24,
        false,
        48_000,
        Some(ixml),
        |n, c| ((n as f64 * (c as f64 + 1.0) * 0.05).sin()) * 0.25,
    )
    .unwrap();
    // WAV avec bext (référence temporelle 10:00:00 à 48 kHz).
    let bext_src = dir.path().join("bext.wav");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=1000:sample_rate=48000:duration=1",
        "-c:a",
        "pcm_s24le",
        "-write_bext",
        "1",
        "-metadata",
        "time_reference=1728000000",
        bext_src.to_str().unwrap(),
    ]);
    let out = dir.path().join("out");
    let mut req = request(vec![ixml_src, bext_src], "wav", &out);
    req.settings.sample_rate = Some(96_000);
    req.settings.bit_depth = Some(BitDepth::S16);
    let s = run(&req);
    let a = read_info(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!((a.sample_rate, a.bits, a.channels), (96_000, 16, 2));
    assert_eq!(a.ixml.scene.as_deref(), Some("12A"));
    assert_eq!(a.ixml.take.as_deref(), Some("3"));
    assert_eq!(a.track_name(1), "HF");
    let b = read_info(s.files[1].output.as_ref().unwrap()).unwrap();
    assert_eq!(b.time_reference, Some(3_456_000_000));
}

#[test]
fn normalization_reaches_target_and_analysis_writes_nothing() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("tone.wav");
    // Sinus 1 kHz : -18 dBFS (niveau du générateur) + 8 dB = -10 dBFS, environ -13 LUFS.
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=1000:sample_rate=48000:duration=6,volume=8dB",
        "-c:a",
        "pcm_s24le",
        src.to_str().unwrap(),
    ]);
    let ebu = LoudnessTarget {
        integrated: -23.0,
        true_peak: -1.0,
    };
    let mut req = request(vec![src.clone()], "flac", dir.path());
    req.settings.loudness = Some(ebu);
    let s = run(&req);
    let gain = s.files[0].loudness.as_ref().unwrap().gain.unwrap();
    assert!(gain < 0.0, "gain {gain}");
    let mut check = request(
        vec![s.files[0].output.clone().unwrap()],
        "analyze",
        dir.path(),
    );
    check.settings.loudness = Some(ebu);
    let m = run(&check);
    assert!(m.files[0].output.is_none());
    let l = m.files[0].loudness.as_ref().unwrap();
    assert!((l.integrated + 23.0).abs() < 0.3, "{}", l.integrated);
    assert_eq!(l.gain, None, "une analyse n'applique pas de gain");
}

#[test]
fn lossy_formats_from_poly_wav() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("poly.wav");
    write_test_wav(&src, 4, 96_000, 24, false, 96_000, None, |n, c| {
        ((n as f64 * (c as f64 + 1.0) * 0.02).sin()) * 0.25
    })
    .unwrap();
    for preset in ["mp3", "aac", "opus", "aiff", "alac"] {
        if preset == "opus" && !encoders::compiled().contains("libopus") {
            continue;
        }
        let s = run(&request(vec![src.clone()], preset, dir.path()));
        let info = probe(s.files[0].output.as_ref().unwrap()).unwrap();
        let a = &info.audio[0];
        let lossy = ["mp3", "aac", "opus"].contains(&preset);
        assert_eq!(a.channels, if lossy { 2 } else { 4 }, "{preset}");
        if preset == "mp3" || preset == "opus" {
            assert_eq!(a.sample_rate, 48_000, "{preset}");
        }
    }
}

#[test]
fn cancel_stops_and_leaves_no_partial_file() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("long.mov");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=size=1280x720:rate=25:duration=60",
        "-c:v",
        "mjpeg",
        "-q:v",
        "5",
        src.to_str().unwrap(),
    ]);
    let req = request(vec![src.clone(), src], "ffv1", &dir.path().join("out"));
    let cancel = AtomicBool::new(false);
    let s = execute(&req, &cancel, |e| {
        if let Event::Progress { fraction, .. } = e {
            if fraction > 0.02 {
                cancel.store(true, Ordering::Relaxed);
            }
        }
    })
    .unwrap();
    assert!(s.cancelled);
    assert_eq!(s.files.len(), 1, "le deuxième fichier n'est pas commencé");
    assert_eq!(s.files[0].status, Status::Cancelled);
    let left: Vec<_> = std::fs::read_dir(dir.path().join("out")).unwrap().collect();
    assert!(left.is_empty(), "aucun fichier laissé : {left:?}");
}

#[test]
fn output_names_never_overwrite_the_source() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("clip.mov");
    std::fs::write(&src, b"x").unwrap();
    let taken = HashSet::new();
    // Même dossier, même extension, sans suffixe : jamais la source.
    for mode in [Existing::Rename, Existing::Overwrite] {
        let p = output_path(&src, None, "", "mov", mode, &taken).unwrap();
        assert_eq!(p, dir.path().join("clip_1.mov"));
    }
    assert_eq!(
        output_path(&src, None, "", "mov", Existing::Skip, &taken),
        None
    );
    assert_eq!(
        output_path(&src, None, "_proxy", "mov", Existing::Rename, &taken).unwrap(),
        dir.path().join("clip_proxy.mov")
    );
    // Deux sources du même nom dans un lot : deux sorties distinctes.
    let mut taken = HashSet::new();
    let out = dir.path().join("out");
    let a = output_path(&src, Some(&out), "", "mp4", Existing::Overwrite, &taken).unwrap();
    taken.insert(a.clone());
    let b = output_path(&src, Some(&out), "", "mp4", Existing::Overwrite, &taken).unwrap();
    assert_ne!(a, b);
}
