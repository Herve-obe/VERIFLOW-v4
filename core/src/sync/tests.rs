//! Essais réels (FFmpeg requis ; ignorés sinon) : caméra et enregistreur qui
//! partagent le même son (bruit rose), comme deux micros dans la même pièce.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use super::*;
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

/// Son « de la pièce » : bruit rose reproductible de `seconds` secondes.
fn room(dir: &Path, seconds: u32) -> PathBuf {
    let p = dir.join("room.wav");
    let src = format!("anoisesrc=d={seconds}:c=pink:r=48000:a=0.4:seed=42");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        &src,
        "-c:a",
        "pcm_s24le",
        p.to_str().unwrap(),
    ]);
    p
}

/// Caméra : `len` s d'image, son témoin = la pièce à partir de `from` s, plus
/// un autre bruit (micro différent). Timecode facultatif.
fn camera(dir: &Path, room: &Path, name: &str, from: f64, len: f64, tc: Option<&str>) -> PathBuf {
    let p = dir.join(name);
    let video = format!("testsrc2=size=320x180:rate=25:duration={len}");
    let graph = format!(
        "[1:a]atrim={from}:{},asetpts=PTS-STARTPTS,volume=0.7[s];[2:a]volume=0.25[n];[s][n]amix=inputs=2:normalize=0[a]",
        from + len
    );
    let other = format!("anoisesrc=d={len}:c=white:r=48000:a=0.4:seed=7");
    let mut args = vec![
        "-f",
        "lavfi",
        "-i",
        &video,
        "-i",
        room.to_str().unwrap(),
        "-f",
        "lavfi",
        "-i",
        &other,
        "-filter_complex",
        &graph,
        "-map",
        "0:v",
        "-map",
        "[a]",
        "-c:v",
        "mpeg2video",
        "-q:v",
        "6",
        "-c:a",
        "pcm_s24le",
    ];
    if let Some(tc) = tc {
        args.extend(["-timecode", tc]);
    }
    args.push(p.to_str().unwrap());
    ffmpeg(&args);
    p
}

/// Enregistreur : la pièce entière, BWF à l'heure `start` (secondes depuis minuit).
fn recorder(dir: &Path, room: &Path, name: &str, start: f64, filter: &str) -> PathBuf {
    let p = dir.join(name);
    let tr = format!("time_reference={}", (start * 48_000.0).round() as u64);
    ffmpeg(&[
        "-i",
        room.to_str().unwrap(),
        "-af",
        filter,
        "-c:a",
        "pcm_s24le",
        "-write_bext",
        "1",
        "-metadata",
        &tr,
        p.to_str().unwrap(),
    ]);
    p
}

fn analyze_one(videos: Vec<PathBuf>, audios: Vec<PathBuf>) -> Analysis {
    analyze(
        &videos,
        &audios,
        Options::default(),
        &AtomicBool::new(false),
        |_| {},
    )
    .unwrap()
}

#[test]
fn timecode_pairs_and_waveform_corrects_a_wrong_timecode() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let room = room(dir.path(), 40);
    // Vidéo à 10:00:00:00, son de la pièce à partir de 10 s : le son a commencé 10 s avant.
    let v = camera(
        dir.path(),
        &room,
        "A001C001.mov",
        10.0,
        20.0,
        Some("10:00:00:00"),
    );
    // BWF faux de 0,3 s (horloge de l'enregistreur mal réglée).
    let a = recorder(
        dir.path(),
        &room,
        "SON_001.WAV",
        36_000.0 - 10.0 + 0.3,
        "anull",
    );
    let other = recorder(dir.path(), &room, "SON_002.WAV", 50_000.0, "anull");
    let r = analyze_one(vec![v], vec![other, a]);
    let p = &r.pairs[0];
    assert_eq!(p.audio, Some(1), "le son à la bonne heure");
    assert_eq!(p.method, Method::Timecode);
    assert!(p.refined, "{:?}", p.note);
    assert!(
        (p.offset + 10.0).abs() < 0.001,
        "décalage {} (attendu -10 s)",
        p.offset
    );
    assert!(p.confidence.unwrap() >= MIN_CONFIDENCE);
    assert!(
        p.note.as_deref().unwrap_or("").contains("corrigé"),
        "{:?}",
        p.note
    );
    assert_eq!(r.videos[0].timecode.as_deref(), Some("10:00:00:00"));
}

#[test]
fn waveform_alone_finds_the_sound_without_timecode() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let room = room(dir.path(), 40);
    let v = camera(dir.path(), &room, "SANS_TC.mov", 12.5, 15.0, None);
    let unrelated = dir.path().join("AUTRE.wav");
    ffmpeg(&[
        "-f",
        "lavfi",
        "-i",
        "anoisesrc=d=40:c=pink:r=48000:a=0.4:seed=3",
        unrelated.to_str().unwrap(),
    ]);
    let a = dir.path().join("PIECE.wav");
    std::fs::copy(&room, &a).unwrap();
    let r = analyze_one(vec![v], vec![unrelated, a]);
    let p = &r.pairs[0];
    assert_eq!(p.audio, Some(1));
    assert_eq!(p.method, Method::Waveform);
    assert!((p.offset + 12.5).abs() < 0.001, "décalage {}", p.offset);
}

#[test]
fn ltc_on_the_camera_gives_the_time_and_is_excluded() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let room = room(dir.path(), 30);
    // Son caméra stéréo : gauche = micro, droite = LTC 14:30:00:00 à 25 i/s.
    let ltc = crate::media::ltc::tests::encode((14, 30, 0, 0), 25, 25 * 12, 48_000);
    let ltc_wav = dir.path().join("ltc.wav");
    crate::media::wav::write_test_wav(
        &ltc_wav,
        1,
        48_000,
        24,
        false,
        ltc.len() as u64,
        None,
        |n, _| ltc[n as usize] as f64 * 0.5,
    )
    .unwrap();
    let mic = camera(dir.path(), &room, "mic.mov", 5.0, 12.0, None);
    let v = dir.path().join("TENTACLE.mov");
    ffmpeg(&[
        "-i",
        mic.to_str().unwrap(),
        "-i",
        ltc_wav.to_str().unwrap(),
        "-filter_complex",
        "[0:a][1:a]amerge=inputs=2[a]",
        "-map",
        "0:v",
        "-map",
        "[a]",
        "-c:v",
        "copy",
        "-c:a",
        "pcm_s24le",
        "-t",
        "12",
        v.to_str().unwrap(),
    ]);
    let f = describe(&v);
    assert_eq!(f.source, TimeSource::Ltc);
    assert_eq!(f.ltc_channel, Some(1));
    assert!((f.start.unwrap() - 52_200.0).abs() < 0.05, "{:?}", f.start);
    // Son de la pièce à 14:29:55:00 : décalage attendu 0 après les 5 s de différence de début.
    let a = recorder(dir.path(), &room, "SON.WAV", 52_200.0 - 5.0, "anull");
    let r = analyze_one(vec![v], vec![a]);
    let p = &r.pairs[0];
    assert_eq!(p.method, Method::Ltc);
    assert!(
        (p.offset + 5.0).abs() < 0.002,
        "décalage {} ({:?})",
        p.offset,
        p.note
    );
}

#[test]
fn clock_drift_is_measured() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let room = room(dir.path(), 110);
    let v = camera(
        dir.path(),
        &room,
        "LONG.mov",
        10.0,
        90.0,
        Some("10:00:00:00"),
    );
    // Enregistreur qui tourne 600 ppm trop vite : 54 ms (1,35 image) d'écart sur 90 s.
    let a = recorder(
        dir.path(),
        &room,
        "DERIVE.WAV",
        36_000.0 - 10.0,
        "asetrate=48029,aresample=48000",
    );
    let r = analyze_one(vec![v], vec![a]);
    let p = &r.pairs[0];
    let frames = p
        .drift_frames
        .unwrap_or_else(|| panic!("dérive non mesurée : {p:?}"));
    // Enregistreur trop rapide : le son est en avance à la fin (dérive négative).
    assert!((frames + 1.35).abs() < 0.3, "dérive {frames} images");
}

#[test]
fn rewrap_and_timelines() {
    if !ffmpeg_available() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let room = room(dir.path(), 40);
    let v = camera(
        dir.path(),
        &room,
        "A001C002.mov",
        10.0,
        8.0,
        Some("10:00:00:00"),
    );
    let a = recorder(dir.path(), &room, "SON.WAV", 35_990.0, "anull");
    let r = analyze_one(vec![v], vec![a]);
    let p = &r.pairs[0];
    let (video, audio) = (&r.videos[0], &r.audios[0]);
    let mut taken = std::collections::HashSet::new();
    for fmt in ["mov", "mxf"] {
        let opts = export::RewrapOptions {
            dest: Some(dir.path().join("sync")),
            format: fmt.into(),
            ..Default::default()
        };
        let out = export::rewrap(
            video,
            audio,
            p.offset,
            &opts,
            &mut taken,
            &AtomicBool::new(false),
            &mut |_, _| {},
        )
        .unwrap();
        assert!(out
            .to_string_lossy()
            .ends_with(&format!("A001C002_sync.{fmt}")));
        let info = probe(&out).unwrap();
        assert_eq!(
            info.video.as_ref().unwrap().codec,
            "mpeg2video",
            "image recopiée"
        );
        assert_eq!(
            info.audio.len(),
            2,
            "{fmt} : son de l'enregistreur + son caméra"
        );
        assert!((info.duration - 8.0).abs() < 0.1, "{}", info.duration);
        assert_eq!(info.start_timecode.as_deref(), Some("10:00:00:00"));
        // Le son calé ressemble au son témoin sans décalage.
        let synced = describe(&out);
        let (offset, conf) = refine_at(video, &synced, 0.0, 0.5, 1.0, 6.0).unwrap();
        assert!(
            offset.abs() < 0.002 && conf > MIN_CONFIDENCE,
            "{offset} {conf}"
        );
    }
    let items = [export::Item {
        video,
        audio: Some((audio, p.offset)),
    }];
    let xml = export::to_fcpxml("Essai", &items);
    assert!(
        xml.contains("lane=\"-1\"") && xml.contains("SON.WAV"),
        "{xml}"
    );
    // Son de l'enregistreur commencé 10 s avant : entrée à 35 990 s + 10 s = 36 000 s.
    assert!(xml.contains("start=\"36000s\""), "{xml}");
    let otio: serde_json::Value = serde_json::from_str(&export::to_otio("Essai", &items)).unwrap();
    let a1 = &otio["tracks"]["children"][1]["children"];
    assert_eq!(a1[0]["OTIO_SCHEMA"], "Clip.2");
    assert_eq!(
        a1[0]["source_range"]["start_time"]["value"],
        36_000.0 * 48_000.0
    );
}
