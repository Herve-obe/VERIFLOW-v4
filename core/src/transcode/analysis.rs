//! Analyses : détection de plans (EDL), de noir, de médias hors ligne, de
//! silences, et qualité VMAF.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::Serialize;

use super::build::{tc_after, Plan};
use super::catalog::Kind;
use super::run::run;
use crate::media::probe::{probe, MediaInfo};
use crate::player::timecode::{FrameRate, Timecode};
use crate::{Error, Result};

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

/// Passage détecté : début, fin (en secondes depuis le début du fichier) et
/// timecodes correspondants.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Segment {
    pub start: f64,
    pub end: Option<f64>,
    pub start_tc: Option<String>,
    pub end_tc: Option<String>,
}

/// Résultat d'une analyse.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Analysis {
    pub segments: Vec<Segment>,
    /// Score VMAF moyen (0 à 100).
    pub vmaf: Option<f64>,
}

fn tc_text(info: &MediaInfo, t: f64) -> Option<String> {
    tc_after(info, t).map(|tc| tc.to_string())
}

fn segment(info: &MediaInfo, start: f64, end: Option<f64>) -> Segment {
    Segment {
        start,
        end,
        start_tc: tc_text(info, start),
        end_tc: end.and_then(|e| tc_text(info, e)),
    }
}

/// Nombre après `key` dans une ligne (« black_start:1.24 », « lavfi.scd.time: 4 »).
fn number_after(line: &str, key: &str) -> Option<f64> {
    let rest = &line[line.find(key)? + key.len()..];
    let rest = rest.trim_start_matches([':', '=', ' ']);
    let end = rest
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-'))
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

/// Instants des changements de plan (filtre `scdet`).
pub fn parse_cuts(log: &str) -> Vec<f64> {
    log.lines()
        .filter(|l| l.contains("lavfi.scd.time"))
        .filter_map(|l| number_after(l, "lavfi.scd.time"))
        .collect()
}

/// Paires début / fin d'un filtre de détection (`blackdetect`, `silencedetect`).
pub fn parse_ranges(log: &str, start_key: &str, end_key: &str) -> Vec<(f64, Option<f64>)> {
    let mut out: Vec<(f64, Option<f64>)> = Vec::new();
    for line in log.lines() {
        if let Some(v) = number_after(line, start_key) {
            out.push((v, None));
        }
        if let Some(v) = number_after(line, end_key) {
            match out.last_mut() {
                Some(last) if last.1.is_none() => last.1 = Some(v),
                _ => out.push((0.0, Some(v))),
            }
        }
    }
    out
}

/// Images « hors ligne » : rouge uniforme des logiciels de montage (moyennes
/// de chrominance relevées par `signalstats`), regroupées en passages.
pub fn parse_offline(log: &str, step: f64) -> Vec<(f64, f64)> {
    let mut frames: Vec<f64> = Vec::new();
    let (mut time, mut u, mut v, mut sat) = (None, None, None, None);
    let flush = |time: Option<f64>,
                 u: Option<f64>,
                 v: Option<f64>,
                 sat: Option<f64>,
                 frames: &mut Vec<f64>| {
        if let (Some(t), Some(u), Some(v), Some(sat)) = (time, u, v, sat) {
            if v > 180.0 && u < 120.0 && sat > 60.0 {
                frames.push(t);
            }
        }
    };
    for line in log.lines() {
        if line.contains("pts_time") {
            flush(time, u, v, sat, &mut frames);
            time = number_after(line, "pts_time");
            u = None;
            v = None;
            sat = None;
        } else if line.contains("lavfi.signalstats.UAVG") {
            u = number_after(line, "lavfi.signalstats.UAVG");
        } else if line.contains("lavfi.signalstats.VAVG") {
            v = number_after(line, "lavfi.signalstats.VAVG");
        } else if line.contains("lavfi.signalstats.SATAVG") {
            sat = number_after(line, "lavfi.signalstats.SATAVG");
        }
    }
    flush(time, u, v, sat, &mut frames);
    let mut out: Vec<(f64, f64)> = Vec::new();
    for t in frames {
        match out.last_mut() {
            Some(last) if t - last.1 <= step * 1.5 => last.1 = t + step,
            _ => out.push((t, t + step)),
        }
    }
    out
}

/// Score VMAF dans les messages de FFmpeg.
pub fn parse_vmaf(log: &str) -> Option<f64> {
    log.lines()
        .filter(|l| l.contains("VMAF score"))
        .find_map(|l| number_after(l, "VMAF score"))
}

/// Liste de montage CMX 3600 : un événement par plan détecté.
pub fn to_edl(title: &str, clip: &str, info: &MediaInfo, cuts: &[f64]) -> String {
    let rate = info
        .video
        .as_ref()
        .map(|v| v.rate)
        .unwrap_or(FrameRate::new(25, 1));
    let start = info
        .start_tc()
        .unwrap_or(Timecode::from_frames(0, rate, false));
    let drop = start.drop_frame;
    let mut bounds = vec![0.0];
    bounds.extend(
        cuts.iter()
            .copied()
            .filter(|c| *c > 0.0 && *c < info.duration),
    );
    bounds.push(info.duration);
    let reel: String = title
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .take(8)
        .collect::<String>()
        .to_uppercase();
    let reel = if reel.is_empty() { s("AX") } else { reel };
    let frames = |t: f64| (t * rate.as_f64()).round() as i64;
    let mut record = Timecode::from_components(1, 0, 0, 0, rate, drop);
    let mut out = format!(
        "TITLE: {title}\nFCM: {}\n\n",
        if drop { "DROP FRAME" } else { "NON-DROP FRAME" }
    );
    for (n, w) in bounds.windows(2).enumerate() {
        let (a, b) = (frames(w[0]), frames(w[1]));
        if b <= a {
            continue;
        }
        let rec_out = record.offset(b - a);
        out += &format!(
            "{:03}  {reel:<8} V     C        {} {} {} {}\n* FROM CLIP NAME: {clip}\n",
            n + 1,
            start.offset(a),
            start.offset(b),
            record,
            rec_out
        );
        record = rec_out;
    }
    out
}

/// Tableau des passages (CSV, séparateur point-virgule, lisible dans Excel).
pub fn to_csv(segments: &[Segment]) -> String {
    let mut out = String::from("\u{feff}Début (s);Fin (s);Durée (s);TC début;TC fin\n");
    for sg in segments {
        let end = sg.end.map(|e| format!("{e:.3}")).unwrap_or_default();
        let dur = sg
            .end
            .map(|e| format!("{:.3}", e - sg.start))
            .unwrap_or_default();
        out += &format!(
            "{:.3};{end};{dur};{};{}\n",
            sg.start,
            sg.start_tc.clone().unwrap_or_default(),
            sg.end_tc.clone().unwrap_or_default()
        );
    }
    // Virgule décimale, comme Excel en français.
    out.replace('.', ",")
}

/// Original d'un fichier : le fichier donné, ou dans le dossier donné celui
/// dont le nom est le plus long début du nom du fichier (« A001C001 » pour
/// « A001C001_proxy »).
pub fn find_reference(source: &Path, reference: &Path) -> Option<PathBuf> {
    if reference.is_file() {
        return Some(reference.to_path_buf());
    }
    let stem = source.file_stem()?.to_string_lossy().to_lowercase();
    std::fs::read_dir(reference)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && p != source)
        .filter(|p| {
            crate::media::catalog::kind_of(p) == Some(crate::media::catalog::MediaKind::Video)
        })
        .filter_map(|p| {
            let r = p.file_stem()?.to_string_lossy().to_lowercase();
            stem.starts_with(&r).then_some((r.len(), p))
        })
        .max_by_key(|(n, _)| *n)
        .map(|(_, p)| p)
}

/// Qualité VMAF de `distorted` par rapport à `reference` (mise à la même taille).
pub fn vmaf(
    distorted: &Path,
    reference: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f64, f64),
) -> Result<f64> {
    let r = probe(reference)?;
    let d = probe(distorted)?;
    let rv = r
        .video
        .as_ref()
        .ok_or_else(|| Error::Unsupported("original sans image".into()))?;
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);
    let graph = format!(
        "[0:v]scale={}:{}:flags=bicubic,setpts=PTS-STARTPTS[d];[1:v]setpts=PTS-STARTPTS[r];[d][r]libvmaf=n_threads={threads}",
        rv.width, rv.height
    );
    let plan = Plan {
        extra_inputs: vec![super::build::Input {
            args: Vec::new(),
            path: reference.to_path_buf(),
        }],
        args: vec![s("-filter_complex"), graph, s("-an")],
        duration: d.duration,
        ..Default::default()
    };
    let log = run(&plan, distorted, None, true, cancel, progress)?;
    parse_vmaf(&log).ok_or_else(|| Error::Tool {
        tool: "ffmpeg".into(),
        message: "score VMAF absent".into(),
    })
}

/// Lance une détection et renvoie les passages trouvés. `offset` : début du
/// passage analysé dans le fichier (points d'entrée et de sortie).
#[allow(clippy::too_many_arguments)]
pub fn detect(
    kind: Kind,
    source: &Path,
    info: &MediaInfo,
    threshold: Option<f64>,
    input_args: &[String],
    offset: f64,
    duration: f64,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f64, f64),
) -> Result<Vec<Segment>> {
    const STEP: f64 = 0.25;
    let (filter, video) = match kind {
        Kind::CutDetect => (format!("scdet=t={}", threshold.unwrap_or(10.0)), true),
        Kind::BlackDetect => (
            format!("blackdetect=d=0.2:pix_th={}", threshold.unwrap_or(0.10)),
            true,
        ),
        Kind::OfflineDetect => (s("fps=4,scale=64:36,signalstats,metadata=print"), true),
        Kind::SilenceDetect => (
            format!("silencedetect=n={}dB:d=1", threshold.unwrap_or(-60.0)),
            false,
        ),
        _ => return Err(Error::Unsupported("analyse inconnue".into())),
    };
    if video && info.video.is_none() {
        return Err(Error::Unsupported(format!("{} : pas d'image", info.path)));
    }
    if !video && info.audio.is_empty() {
        return Err(Error::Unsupported(format!("{} : pas de son", info.path)));
    }
    let args = if video {
        vec![s("-map"), s("0:v:0"), s("-vf"), filter, s("-an")]
    } else {
        let graph = super::filters::audio_chain(0, info, None, &[filter], "aout");
        vec![
            s("-filter_complex"),
            graph,
            s("-map"),
            s("[aout]"),
            s("-vn"),
        ]
    };
    let plan = Plan {
        input_args: input_args.to_vec(),
        args,
        duration,
        ..Default::default()
    };
    let log = run(&plan, source, None, true, cancel, progress)?;
    let shift = |t: f64| t + offset;
    let segments = match kind {
        Kind::CutDetect => parse_cuts(&log)
            .into_iter()
            .map(|t| segment(info, shift(t), None))
            .collect(),
        Kind::BlackDetect => parse_ranges(&log, "black_start", "black_end")
            .into_iter()
            .map(|(a, b)| segment(info, shift(a), b.map(shift)))
            .collect(),
        Kind::SilenceDetect => parse_ranges(&log, "silence_start", "silence_end")
            .into_iter()
            .map(|(a, b)| segment(info, shift(a), Some(shift(b.unwrap_or(duration)))))
            .collect(),
        _ => parse_offline(&log, STEP)
            .into_iter()
            .map(|(a, b)| segment(info, shift(a), Some(shift(b.min(duration)))))
            .collect(),
    };
    Ok(segments)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_detection_logs() {
        let log = "[Parsed_scdet_0 @ 0x1] lavfi.scd.score: 21.560, lavfi.scd.time: 1\n[Parsed_scdet_0 @ 0x1] lavfi.scd.score: 30.1, lavfi.scd.time: 4.04\n";
        assert_eq!(parse_cuts(log), vec![1.0, 4.04]);
        let log = "[blackdetect @ 0x1] black_start:1 black_end:2.5 black_duration:1.5\n";
        assert_eq!(
            parse_ranges(log, "black_start", "black_end"),
            vec![(1.0, Some(2.5))]
        );
        let log = "[silencedetect @ 0x1] silence_start: 1.2\n[silencedetect @ 0x1] silence_end: 4 | silence_duration: 2.8\n[silencedetect @ 0x1] silence_start: 9\n";
        assert_eq!(
            parse_ranges(log, "silence_start", "silence_end"),
            vec![(1.2, Some(4.0)), (9.0, None)]
        );
        assert_eq!(
            parse_vmaf("[Parsed_libvmaf_4 @ 0x1] VMAF score: 95.123456\n"),
            Some(95.123456)
        );
    }

    #[test]
    fn red_frames_become_offline_ranges() {
        let frame = |t: f64, u: f64, v: f64, sat: f64| {
            format!("frame:0 pts:0 pts_time:{t}\nlavfi.signalstats.SATAVG={sat}\nlavfi.signalstats.UAVG={u}\nlavfi.signalstats.VAVG={v}\n")
        };
        let log = [
            frame(0.0, 128.0, 128.0, 2.0),
            frame(0.25, 90.0, 240.0, 110.0),
            frame(0.5, 90.0, 240.0, 110.0),
            frame(0.75, 128.0, 128.0, 2.0),
        ]
        .concat();
        assert_eq!(parse_offline(&log, 0.25), vec![(0.25, 0.75)]);
    }

    #[test]
    fn edl_lists_one_event_per_shot() {
        let info = MediaInfo {
            path: "a.mov".into(),
            format: "mov".into(),
            duration: 4.0,
            size: 0,
            video: Some(crate::media::probe::VideoStream {
                index: 0,
                codec: "h264".into(),
                width: 1920,
                height: 1080,
                rate: FrameRate::new(25, 1),
                pix_fmt: "yuv420p".into(),
                frame_count: 100,
                start_time: 0.0,
            }),
            audio: vec![],
            start_timecode: Some("10:00:00:00".into()),
            tags: Default::default(),
        };
        let edl = to_edl("A001C001", "A001C001.mov", &info, &[1.0, 2.5]);
        assert!(
            edl.contains(
                "001  A001C001 V     C        10:00:00:00 10:00:01:00 01:00:00:00 01:00:01:00"
            ),
            "{edl}"
        );
        assert!(
            edl.contains(
                "003  A001C001 V     C        10:00:02:13 10:00:04:00 01:00:02:13 01:00:04:00"
            ),
            "{edl}"
        );
        assert_eq!(edl.matches("FROM CLIP NAME").count(), 3);
    }
}
