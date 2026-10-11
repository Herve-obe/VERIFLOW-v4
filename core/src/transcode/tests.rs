//! Essais de conversion réels (FFmpeg requis ; ignorés sinon). Pour essayer
//! avec le FFmpeg livré : `VERIFLOW_FFMPEG_DIR=<dossier> cargo test`.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::settings::{LogoStyle, SequenceOptions, SubtitleOptions, TextStyle, Trim};
use super::*;
use crate::media::wav::{read_info, write_test_wav};
use crate::tools;

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

/// Valeur d'un champ FFprobe (`stream=codec_name` par exemple) de chaque flux.
fn ffprobe(path: &Path, entries: &str) -> Vec<String> {
    let out = tools::command("ffprobe")
        .unwrap()
        .args(["-v", "error", "-show_entries", entries, "-of", "csv=p=0"])
        .arg(path)
        .output()
        .unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(|l| l.trim().to_owned())
        .filter(|l| !l.is_empty())
        .collect()
}

/// Vidéo avec timecode 10:00:00:00 et deux pistes son mono.
fn clip_sized(dir: &Path, name: &str, size: &str, seconds: u32) -> PathBuf {
    let p = dir.join(name);
    let video = format!("testsrc2=size={size}:rate=25:duration={seconds}");
    let a = format!("sine=frequency=440:sample_rate=48000:duration={seconds}");
    let b = format!("sine=frequency=880:sample_rate=48000:duration={seconds}");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &video,
        "-f",
        "lavfi",
        "-i",
        &a,
        "-f",
        "lavfi",
        "-i",
        &b,
        "-map",
        "0:v",
        "-map",
        "1:a",
        "-map",
        "2:a",
        "-c:v",
        "mjpeg",
        "-q:v",
        "3",
        "-c:a",
        "pcm_s24le",
        "-timecode",
        "10:00:00:00",
        p.to_str().unwrap(),
    ]);
    p
}

fn clip(dir: &Path) -> PathBuf {
    clip_sized(dir, "A001C001.mov", "640x360", 2)
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
        ..Default::default()
    }
}

fn run(req: &Request) -> Summary {
    let cancel = AtomicBool::new(false);
    let summary = execute(req, &cancel, |_| {}).unwrap();
    for f in &summary.files {
        assert_eq!(
            f.status,
            Status::Done,
            "{} : {:?}",
            req.settings.preset,
            f.message
        );
    }
    summary
}

fn available(id: &str) -> bool {
    let ok = find(id).unwrap().unavailable().is_none();
    if !ok {
        eprintln!("{id} : indisponible dans ce FFmpeg, essai ignoré");
    }
    ok
}

#[test]
fn every_video_codec_produces_a_file() {
    if !ffmpeg_available() {
        eprintln!("FFmpeg absent : test ignoré");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = clip_sized(dir.path(), "HD.mov", "1920x1080", 1);
    let out = dir.path().join("out");
    #[rustfmt::skip]
    let cases = [
        ("prores_proxy", "prores", true), ("dnxhr_lb", "dnxhd", true), ("dnxhd_sq", "dnxhd", true),
        ("cineform", "cfhd", true), ("ffv1", "ffv1", false), ("h264", "h264", true), ("hevc", "hevc", true),
        ("av1", "av1", true), ("vvc", "vvc", true), ("vp9", "vp9", false), ("vp8", "vp8", false),
        ("xdcam_hd422", "mpeg2video", true), ("xdcam_hd35", "mpeg2video", true), ("avc_intra100", "h264", true),
        ("xavc_intra", "h264", true), ("xavc_longgop", "h264", true), ("hap_q", "hap", true),
        ("mpeg2", "mpeg2video", false), ("mpeg1", "mpeg1video", false), ("mjpeg", "mjpeg", true),
        ("dv", "dvvideo", false), ("xvid", "mpeg4", false), ("wmv", "wmv2", false), ("theora", "theora", false),
    ];
    for (preset, codec, tc) in cases {
        if !available(preset) {
            continue;
        }
        let s = run(&request(vec![src.clone()], preset, &out));
        let file = s.files[0].output.clone().unwrap();
        let info = crate::media::probe::probe(&file).unwrap();
        assert_eq!(info.video.as_ref().unwrap().codec, codec, "{preset}");
        assert!(!info.audio.is_empty(), "{preset} : son présent");
        if tc {
            assert_eq!(
                info.start_timecode.as_deref(),
                Some("10:00:00:00"),
                "{preset}"
            );
        }
    }
    // XDCAM : une piste mono par canal, 1920×1080 imposé.
    let s = run(&request(vec![src.clone()], "xdcam_hd422", &out));
    let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!(info.audio.len(), 2);
    assert!(info.audio.iter().all(|a| a.channels == 1));
    // DV : 720×576, son stéréo.
    let s = run(&request(vec![src], "dv", &out));
    let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!(
        info.video.as_ref().map(|v| (v.width, v.height)),
        Some((720, 576))
    );
}

#[test]
fn web_presets_reframe_and_mix_to_stereo() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = clip(dir.path());
    let s = run(&request(vec![src], "social_vertical", dir.path()));
    let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
    let v = info.video.unwrap();
    assert_eq!((v.width, v.height), (1080, 1920));
    assert_eq!(info.audio.len(), 1);
    assert_eq!(info.audio[0].channels, 2);
}

#[test]
fn images_single_every_and_tc_numbered() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = clip(dir.path());
    let out = dir.path().join("img");
    let mut req = request(vec![src.clone()], "image_jpeg", &out);
    req.settings.sequence = SequenceOptions {
        single: true,
        ..Default::default()
    };
    let s = run(&req);
    let f = s.files[0].output.clone().unwrap();
    assert!(f.is_file() && f.extension().unwrap() == "jpg", "{f:?}");
    let mut req = request(vec![src.clone()], "image_png", &out);
    req.settings.sequence.every = Some(0.5);
    let s = run(&req);
    let d = s.files[0].output.clone().unwrap();
    assert!(d.is_dir());
    let n = std::fs::read_dir(&d).unwrap().count();
    assert!((3..=5).contains(&n), "{n} images");
    let mut req = request(vec![src], "image_dpx", &out);
    req.settings.sequence.tc_numbering = true;
    req.settings.trims.insert(
        req.sources[0].to_string_lossy().to_string(),
        Trim {
            start: None,
            end: Some("0.2".into()),
        },
    );
    let s = run(&req);
    let d = s.files[0].output.clone().unwrap();
    let mut names: Vec<String> = std::fs::read_dir(&d)
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    names.sort();
    // 10:00:00:00 à 25 i/s = image 900 000.
    assert!(names[0].ends_with("_0900000.dpx"), "{names:?}");
}

#[test]
fn overlays_lut_and_subtitles_render() {
    if !ffmpeg_available() || !encoders::filters().contains("drawtext") {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = clip(dir.path());
    let lut = dir.path().join("identite.cube");
    let mut cube = String::from("LUT_3D_SIZE 2\n");
    for b in 0..2 {
        for g in 0..2 {
            for r in 0..2 {
                cube += &format!("{r} {g} {b}\n");
            }
        }
    }
    std::fs::write(&lut, cube).unwrap();
    let logo = dir.path().join("logo.png");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "color=c=white:s=200x100:d=0.04",
        "-frames:v",
        "1",
        logo.to_str().unwrap(),
    ]);
    let subs = dir.path().join("sous-titres.srt");
    std::fs::write(
        &subs,
        "1\n00:00:00,000 --> 00:00:01,500\nC'est « bon » : 100 %\n",
    )
    .unwrap();
    let mut req = request(vec![src], "h264", dir.path());
    req.settings.image.lut = Some(lut);
    req.settings.image.saturation = 1.2;
    req.settings.overlay.timecode = Some(TextStyle::default());
    req.settings.overlay.filename = Some(TextStyle::default());
    req.settings.overlay.text = Some(TextStyle {
        position: settings::Position::TopLeft,
        ..Default::default()
    });
    req.settings.overlay.text_value = "Plan 12/3 : « test » 100 %".into();
    req.settings.overlay.logo = Some(LogoStyle {
        path: logo,
        ..Default::default()
    });
    if encoders::filters().contains("subtitles") {
        req.settings.subtitles = SubtitleOptions {
            file: Some(subs.clone()),
            burn: true,
            size: 0,
        };
    }
    run(&req);
    // Sous-titres en piste, sans réencodage.
    let mut req = request(
        vec![clip_sized(dir.path(), "B.mov", "320x180", 1)],
        "subtitles",
        dir.path(),
    );
    req.settings.subtitles.file = Some(subs);
    let s = run(&req);
    let types = ffprobe(s.files[0].output.as_ref().unwrap(), "stream=codec_type");
    assert!(types.contains(&"subtitle".to_string()), "{types:?}");
}

#[test]
fn no_reencode_functions() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let a = clip_sized(dir.path(), "A.mov", "320x180", 2);
    let b = clip_sized(dir.path(), "B.mov", "320x180", 2);
    let out = dir.path().join("out");
    // Découpe.
    let mut req = request(vec![a.clone()], "cut", &out);
    req.settings.trims.insert(
        a.to_string_lossy().to_string(),
        Trim {
            start: Some("10:00:00:12".into()),
            end: Some("1.5".into()),
        },
    );
    let s = run(&req);
    let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
    assert!((info.duration - 1.02).abs() < 0.1, "{}", info.duration);
    assert_eq!(info.start_timecode.as_deref(), Some("10:00:00:12"));
    // Fusion.
    let s = run(&request(vec![a.clone(), b.clone()], "merge", &out));
    let f = s.files[0].output.clone().unwrap();
    assert!(f.file_name().unwrap().to_string_lossy().contains("_fusion"));
    assert!((crate::media::probe::probe(&f).unwrap().duration - 4.0).abs() < 0.1);
    // Insert : 1 s de B à 0,5 s dans A ; durée inchangée.
    let ins = clip_sized(dir.path(), "INS.mov", "320x180", 1);
    let mut req = request(vec![a.clone()], "insert", &out);
    req.settings.insert_file = Some(ins);
    req.settings.insert_at = Some("10:00:00:12".into());
    let s = run(&req);
    assert!(
        (crate::media::probe::probe(s.files[0].output.as_ref().unwrap())
            .unwrap()
            .duration
            - 2.0)
            .abs()
            < 0.1
    );
    // Conformation 25 vers 24 i/s.
    let mut req = request(vec![a.clone()], "conform", &out);
    req.settings.conform_rate = Some("24".into());
    let s = run(&req);
    let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!(info.video.unwrap().rate.nominal(), 24);
    assert!(
        (info.duration - 2.0 * 25.0 / 24.0).abs() < 0.1,
        "{}",
        info.duration
    );
    // Image seule.
    let s = run(&request(vec![a.clone()], "extract_video", &out));
    assert!(
        crate::media::probe::probe(s.files[0].output.as_ref().unwrap())
            .unwrap()
            .audio
            .is_empty()
    );
    // Une piste par fichier.
    let s = run(&request(vec![a.clone()], "extract_tracks", &out));
    assert_eq!(s.files[0].outputs.len(), 2);
    assert!(s.files[0].outputs[1]
        .to_string_lossy()
        .ends_with("A_A02.wav"));
    let w = read_info(&s.files[0].outputs[0]).unwrap();
    assert_eq!((w.channels, w.time_reference), (1, Some(1_728_000_000)));
    // Son remplacé, calé par timecode (son BWF à 10:00:01:00).
    let wav = dir.path().join("A.wav");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "sine=frequency=1000:sample_rate=48000:duration=3",
        "-c:a",
        "pcm_s24le",
        "-write_bext",
        "1",
        "-metadata",
        "time_reference=1728048000",
        wav.to_str().unwrap(),
    ]);
    let mut req = request(vec![a], "replace_audio", &out);
    req.settings.audio_files = vec![wav];
    req.settings.sync_tc = true;
    let s = run(&req);
    let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!(info.audio.len(), 1);
    assert!((info.duration - 2.0).abs() < 0.1);
}

#[test]
fn analyses_find_cuts_black_offline_silence() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("montage.mov");
    // 1 s de mire, 1 s de noir, 1 s de rouge (média hors ligne), 1 s de mire ; son : 1 s de silence à 2 s.
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=25:d=1",
        "-f",
        "lavfi",
        "-i",
        "color=black:s=320x180:r=25:d=1",
        "-f",
        "lavfi",
        "-i",
        "color=red:s=320x180:r=25:d=1",
        "-f",
        "lavfi",
        "-i",
        "testsrc2=s=320x180:r=25:d=1",
        "-f",
        "lavfi",
        "-i",
        "sine=f=440:r=48000:d=4,volume=enable='between(t,2,3)':volume=0",
        "-filter_complex",
        "[0:v][1:v][2:v][3:v]concat=n=4[v]",
        "-map",
        "[v]",
        "-map",
        "4:a",
        "-c:v",
        "mjpeg",
        "-c:a",
        "pcm_s16le",
        "-timecode",
        "01:00:00:00",
        src.to_str().unwrap(),
    ]);
    let out = dir.path().join("rapports");
    let s = run(&request(vec![src.clone()], "cut_detect", &out));
    let edl = std::fs::read_to_string(s.files[0].output.as_ref().unwrap()).unwrap();
    assert!(
        edl.matches("FROM CLIP NAME: montage.mov").count() >= 3,
        "{edl}"
    );
    let s = run(&request(vec![src.clone()], "black_detect", &out));
    let seg = &s.files[0].analysis.as_ref().unwrap().segments;
    assert_eq!(seg.len(), 1);
    assert_eq!(seg[0].start_tc.as_deref(), Some("01:00:01:00"));
    let s = run(&request(vec![src.clone()], "offline_detect", &out));
    let seg = &s.files[0].analysis.as_ref().unwrap().segments;
    assert_eq!(seg.len(), 1, "{seg:?}");
    assert!((seg[0].start - 2.0).abs() < 0.3, "{seg:?}");
    let s = run(&request(vec![src.clone()], "silence_detect", &out));
    let seg = &s.files[0].analysis.as_ref().unwrap().segments;
    assert_eq!(seg.len(), 1, "{seg:?}");
    let s = run(&request(vec![src.clone()], "framemd5", &out));
    let md5 = std::fs::read_to_string(s.files[0].output.as_ref().unwrap()).unwrap();
    assert!(md5.lines().filter(|l| !l.starts_with('#')).count() >= 100);
    if available("vmaf") {
        let s = run(&request(vec![src.clone()], "proxy_h264", &out));
        let proxy = s.files[0].output.clone().unwrap();
        let mut req = request(vec![proxy], "vmaf", &out);
        req.settings.reference = Some(dir.path().to_path_buf());
        let s = run(&req);
        let v = s.files[0].vmaf.unwrap();
        assert!(v > 20.0 && v <= 100.0, "{v}");
    }
}

#[test]
fn naming_checksum_and_batch_report() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let a = clip_sized(dir.path(), "JOUR1_A001.mov", "320x180", 1);
    let b = clip_sized(dir.path(), "JOUR1_A002.mov", "320x180", 1);
    let out = dir.path().join("out");
    let mut req = request(vec![a, b], "rewrap_mp4", &out);
    req.prefix = "FILM_".into();
    req.replace_from = "JOUR1_".into();
    req.replace_to = "J01_".into();
    req.numbering = true;
    req.number_start = 10;
    req.checksum = true;
    req.report = true;
    let s = run(&req);
    let names: Vec<String> = s
        .files
        .iter()
        .map(|f| {
            f.output
                .as_ref()
                .unwrap()
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert_eq!(names, ["FILM_J01_A001_010.mp4", "FILM_J01_A002_011.mp4"]);
    let sum = s.files[0].checksum.clone().unwrap();
    assert_eq!(
        sum,
        report::xxh128(s.files[0].output.as_ref().unwrap()).unwrap()
    );
    let csv = std::fs::read_to_string(s.report.unwrap()).unwrap();
    assert!(csv.contains(&sum) && csv.contains("FILM_J01_A002_011.mp4"));
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
    let mut req = request(vec![ixml_src.clone(), bext_src.clone()], "wav", &out);
    req.settings.sample_rate = Some(96_000);
    req.settings.bit_depth = Some(BitDepth::S16);
    let s = run(&req);
    let a = read_info(s.files[0].output.as_ref().unwrap()).unwrap();
    assert_eq!((a.sample_rate, a.bits, a.channels), (96_000, 16, 2));
    assert_eq!(a.ixml.scene.as_deref(), Some("12A"));
    assert_eq!(a.track_name(1), "HF");
    let b = read_info(s.files[1].output.as_ref().unwrap()).unwrap();
    assert_eq!(b.time_reference, Some(3_456_000_000));
    // Découpe : la référence avance du point d'entrée (0,5 s à 48 kHz).
    let mut req = request(vec![bext_src.clone()], "wav", &out);
    req.settings.trims.insert(
        bext_src.to_string_lossy().to_string(),
        Trim {
            start: Some("0.5".into()),
            end: None,
        },
    );
    let s = run(&req);
    assert_eq!(
        read_info(s.files[0].output.as_ref().unwrap())
            .unwrap()
            .time_reference,
        Some(1_728_024_000)
    );
    // Pistes séparées nommées d'après l'iXML.
    let s = run(&request(vec![ixml_src], "extract_tracks", &out));
    assert!(
        s.files[0].outputs[0]
            .to_string_lossy()
            .ends_with("12A_T3_A01_Perche.wav"),
        "{:?}",
        s.files[0].outputs
    );
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
    assert!(s.files[0].loudness.as_ref().unwrap().gain.unwrap() < 0.0);
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
    // Vidéo : même gain sur chaque piste.
    let v = clip(dir.path());
    let mut req = request(vec![v], "prores_proxy", dir.path());
    req.settings.loudness = Some(ebu);
    let s = run(&req);
    assert!(s.files[0].loudness.as_ref().unwrap().gain.is_some());
    assert_eq!(
        crate::media::probe::probe(s.files[0].output.as_ref().unwrap())
            .unwrap()
            .audio
            .len(),
        2
    );
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
    for preset in ["mp3", "aac", "opus", "aiff", "alac", "ac3", "vorbis"] {
        if !available(preset) {
            continue;
        }
        let s = run(&request(vec![src.clone()], preset, dir.path()));
        let info = crate::media::probe::probe(s.files[0].output.as_ref().unwrap()).unwrap();
        let a = &info.audio[0];
        let lossy = ["mp3", "aac", "opus", "ac3", "vorbis"].contains(&preset);
        assert_eq!(a.channels, if lossy { 2 } else { 4 }, "{preset}");
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
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
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
    for mode in [Existing::Rename, Existing::Overwrite] {
        let p = output_path(dir.path(), "clip", "mov", &src, mode, &taken).unwrap();
        assert_eq!(p, dir.path().join("clip_1.mov"));
    }
    assert_eq!(
        output_path(dir.path(), "clip", "mov", &src, Existing::Skip, &taken),
        None
    );
    let mut taken = HashSet::new();
    let out = dir.path().join("out");
    let a = output_path(&out, "clip", "mp4", &src, Existing::Overwrite, &taken).unwrap();
    taken.insert(a.clone());
    let b = output_path(&out, "clip", "mp4", &src, Existing::Overwrite, &taken).unwrap();
    assert_ne!(a, b);
    // Caractères interdits par Windows remplacés.
    let req = Request {
        prefix: "A:B ".into(),
        ..Default::default()
    };
    assert_eq!(req.output_name(&src, 0, ""), "A_B clip");
}
