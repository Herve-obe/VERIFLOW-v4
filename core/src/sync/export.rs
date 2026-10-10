//! Exports SYNC : re-wrap sans réencodage (image de la caméra + son de
//! l'enregistreur calé), FCPXML et OpenTimelineIO.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::SyncFile;
use crate::media::probe::probe;
use crate::player::logs::{fcp_time, file_url, xml_esc};
use crate::player::timecode::FrameRate;
use crate::transcode::build::{Input, Plan};
use crate::transcode::filters::{audio_chain, mono_tracks};
use crate::transcode::{output_path, write_output, Existing};
use crate::{Error, Result};

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

/// Vidéo, et son calé avec son décalage (début du son - début de la vidéo).
pub struct Item<'a> {
    pub video: &'a SyncFile,
    pub audio: Option<(&'a SyncFile, f64)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct RewrapOptions {
    /// Dossier de sortie ; à côté de chaque vidéo sinon.
    pub dest: Option<PathBuf>,
    /// "mov" ou "mxf" (une piste mono par canal).
    pub format: String,
    /// Garder le son témoin de la caméra après les pistes de l'enregistreur.
    pub keep_camera_audio: bool,
    pub suffix: String,
    pub existing: Existing,
}

impl Default for RewrapOptions {
    fn default() -> Self {
        Self {
            dest: None,
            format: s("mov"),
            keep_camera_audio: true,
            suffix: s("_sync"),
            existing: Existing::Rename,
        }
    }
}

/// Canaux du son à garder : tous sauf la piste LTC.
fn kept_channels(audio: &SyncFile) -> Vec<usize> {
    (0..audio.channels as usize)
        .filter(|c| Some(*c) != audio.ltc_channel || audio.channels == 1)
        .collect()
}

/// Nouveau fichier : image de la vidéo recopiée, son de l'enregistreur calé
/// (pistes nommées d'après l'iXML), timecode de la vidéo.
pub fn rewrap(
    video: &SyncFile,
    audio: &SyncFile,
    offset: f64,
    opts: &RewrapOptions,
    taken: &mut HashSet<PathBuf>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f64, f64),
) -> Result<PathBuf> {
    let format = if opts.format == "mxf" { "mxf" } else { "mov" };
    let vinfo = probe(&video.path)?;
    let ainfo = probe(&audio.path)?;
    if ainfo.audio.is_empty() {
        return Err(Error::Unsupported(format!("{} : pas de son", audio.name)));
    }
    let mut in_args = Vec::new();
    if offset > 0.0 {
        in_args.extend([s("-itsoffset"), format!("{offset:.6}")]);
    } else if offset < 0.0 {
        in_args.extend([s("-ss"), format!("{:.6}", -offset)]);
    }
    let channels = kept_channels(audio);
    let pan = if channels.len() as u32 == audio.channels {
        None
    } else {
        let layout: Vec<String> = channels
            .iter()
            .enumerate()
            .map(|(o, c)| format!("c{o}=c{c}"))
            .collect();
        Some(format!("pan={}c|{}", channels.len(), layout.join("|")))
    };
    let mut a = vec![
        s("-map_metadata"),
        s("0"),
        s("-map"),
        s("0:v:0"),
        s("-c:v"),
        s("copy"),
    ];
    let mut names: Vec<String> = Vec::new();
    if format == "mxf" {
        let (graph, outs) = mono_tracks(
            1,
            &ainfo,
            pan,
            &[s("aresample=48000")],
            channels.len() as u32,
        );
        a.extend([s("-filter_complex"), graph]);
        for o in outs {
            a.extend([s("-map"), o]);
        }
        names.extend(
            channels
                .iter()
                .map(|c| audio.tracks.get(*c).cloned().unwrap_or_default()),
        );
    } else if let Some(p) = pan {
        a.extend([
            s("-filter_complex"),
            audio_chain(1, &ainfo, Some(p), &[], "aout"),
            s("-map"),
            s("[aout]"),
        ]);
        names.push(audio.name.clone());
    } else {
        a.extend([s("-map"), s("1:a")]);
        names.push(audio.name.clone());
    }
    if opts.keep_camera_audio && !vinfo.audio.is_empty() {
        a.extend([s("-map"), s("0:a")]);
    }
    a.extend([s("-c:a"), s("pcm_s24le")]);
    for (i, n) in names.iter().enumerate().filter(|(_, n)| !n.is_empty()) {
        a.extend([format!("-metadata:s:a:{i}"), format!("title={n}")]);
    }
    a.extend([s("-t"), format!("{:.3}", video.duration)]);
    if let Some(tc) = &video.timecode {
        a.extend([s("-timecode"), tc.clone()]);
    }
    let plan = Plan {
        extra_inputs: vec![Input {
            args: in_args,
            path: audio.path.clone(),
        }],
        args: a,
        format: s(format),
        duration: video.duration,
        ..Default::default()
    };
    let dir = opts
        .dest
        .clone()
        .or_else(|| video.path.parent().map(PathBuf::from))
        .unwrap_or_default();
    let stem = video
        .path
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let name = format!("{stem}{}", opts.suffix);
    let out = output_path(&dir, &name, format, &video.path, opts.existing, taken)
        .ok_or_else(|| Error::AlreadyExists(dir.join(&name).display().to_string()))?;
    taken.insert(out.clone());
    write_output(&plan, &video.path, &out, format, None, cancel, progress)?;
    Ok(out)
}

/// Temps FCPXML d'un nombre d'échantillons.
fn sample_time(samples: i64, rate: u32) -> String {
    fcp_time(samples, FrameRate::new(rate.max(1), 1))
}

fn rate_of(v: &SyncFile) -> FrameRate {
    v.rate.unwrap_or(FrameRate::new(25, 1))
}

fn frames(seconds: f64, rate: FrameRate) -> i64 {
    (seconds * rate.as_f64()).round() as i64
}

fn samples(seconds: f64, rate: u32) -> i64 {
    (seconds * rate as f64).round() as i64
}

/// FCPXML 1.10 : un plan par vidéo, son de l'enregistreur en clip attaché,
/// calé à l'échantillon (Final Cut Pro, DaVinci Resolve).
pub fn to_fcpxml(title: &str, items: &[Item]) -> String {
    let mut formats: Vec<(u32, u32, FrameRate)> = Vec::new();
    for it in items {
        let f = (
            it.video.width.max(1),
            it.video.height.max(1),
            rate_of(it.video),
        );
        if !formats.contains(&f) {
            formats.push(f);
        }
    }
    let fmt = |v: &SyncFile| {
        let f = (v.width.max(1), v.height.max(1), rate_of(v));
        format!("r{}", formats.iter().position(|x| *x == f).unwrap_or(0) + 1)
    };
    let mut res = String::new();
    for (i, (w, h, r)) in formats.iter().enumerate() {
        res += &format!(
            "    <format id=\"r{}\" frameDuration=\"{}\" width=\"{w}\" height=\"{h}\"/>\n",
            i + 1,
            fcp_time(1, *r)
        );
    }
    let mut spine = String::new();
    let seq_rate = items
        .first()
        .map(|i| rate_of(i.video))
        .unwrap_or(FrameRate::new(25, 1));
    let mut rec: i64 = 0;
    // Un même son peut servir à plusieurs plans : une seule ressource par fichier.
    let mut sounds: Vec<&std::path::Path> = Vec::new();
    for (i, it) in items.iter().enumerate() {
        let v = it.video;
        let r = rate_of(v);
        let v_start = frames(v.start.unwrap_or(0.0), r);
        let v_dur = frames(v.duration, r).max(1);
        res += &format!(
            "    <asset id=\"v{i}\" name=\"{}\" start=\"{}\" duration=\"{}\" hasVideo=\"1\" hasAudio=\"{}\" format=\"{}\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{}\">\n      <media-rep kind=\"original-media\" src=\"{}\"/>\n    </asset>\n",
            xml_esc(&v.name),
            fcp_time(v_start, r),
            fcp_time(v_dur, r),
            u8::from(v.channels > 0),
            fmt(v),
            v.channels.max(1),
            v.sample_rate.max(48_000),
            xml_esc(&file_url(&v.path.to_string_lossy())),
        );
        spine += &format!(
            "            <asset-clip ref=\"v{i}\" offset=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\" tcFormat=\"NDF\">\n",
            fcp_time(frames(rec as f64 / seq_rate.as_f64(), seq_rate), seq_rate),
            xml_esc(&v.name),
            fcp_time(v_start, r),
            fcp_time(v_dur, r),
        );
        if let Some((a, offset)) = it.audio {
            let sr = a.sample_rate.max(1);
            let a_start = samples(a.start.unwrap_or(0.0), sr);
            let a_dur = samples(a.duration, sr);
            let k = match sounds.iter().position(|p| *p == a.path.as_path()) {
                Some(k) => k,
                None => {
                    sounds.push(&a.path);
                    let k = sounds.len() - 1;
                    res += &format!(
                "    <asset id=\"a{k}\" name=\"{}\" start=\"{}\" duration=\"{}\" hasAudio=\"1\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{sr}\">\n      <media-rep kind=\"original-media\" src=\"{}\"/>\n    </asset>\n",
                xml_esc(&a.name),
                sample_time(a_start, sr),
                sample_time(a_dur.max(1), sr),
                a.channels.max(1),
                xml_esc(&file_url(&a.path.to_string_lossy())),
                    );
                    k
                }
            };
            // Position dans le plan vidéo et point d'entrée dans le son.
            let lead = offset.max(0.0);
            let skip = (-offset).max(0.0);
            let dur = (v.duration - lead).min(a.duration - skip);
            if dur > 0.0 {
                let parent = v.start.unwrap_or(0.0) + lead;
                spine += &format!(
                    "              <asset-clip ref=\"a{k}\" lane=\"-1\" offset=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\"/>\n",
                    sample_time(samples(parent, sr), sr),
                    xml_esc(&a.name),
                    sample_time(a_start + samples(skip, sr), sr),
                    sample_time(samples(dur, sr), sr),
                );
            }
        }
        spine += "            </asset-clip>\n";
        rec += (v.duration * seq_rate.as_f64()).round() as i64;
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.10\">\n  <resources>\n{res}  </resources>\n  <library>\n    <event name=\"VERIFLOW SYNC\">\n      <project name=\"{}\">\n        <sequence format=\"r1\" duration=\"{}\" tcStart=\"0s\" tcFormat=\"NDF\">\n          <spine>\n{spine}          </spine>\n        </sequence>\n      </project>\n    </event>\n  </library>\n</fcpxml>\n",
        xml_esc(title),
        fcp_time(rec, seq_rate),
    )
}

fn rt(value: f64, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate, "value": value })
}

fn range(start: f64, dur: f64, rate: f64) -> Value {
    json!({ "OTIO_SCHEMA": "TimeRange.1", "start_time": rt(start, rate), "duration": rt(dur, rate) })
}

fn clip(f: &SyncFile, src_start: f64, dur: f64, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "Clip.2",
        "name": f.name,
        "source_range": range(src_start, dur, rate),
        "media_references": {
            "DEFAULT_MEDIA": {
                "OTIO_SCHEMA": "ExternalReference.1",
                "name": f.name,
                "target_url": file_url(&f.path.to_string_lossy()),
                "available_range": range((f.start.unwrap_or(0.0) * rate).round(), (f.duration * rate).round(), rate),
                "available_image_bounds": null,
                "metadata": {},
            }
        },
        "active_media_reference_key": "DEFAULT_MEDIA",
        "markers": [],
        "effects": [],
        "enabled": true,
        "metadata": {},
    })
}

fn gap(dur: f64, rate: f64) -> Value {
    json!({
        "OTIO_SCHEMA": "Gap.1",
        "name": "",
        "source_range": range(0.0, dur, rate),
        "effects": [],
        "markers": [],
        "enabled": true,
        "metadata": {},
    })
}

/// OpenTimelineIO : piste vidéo, et piste son de l'enregistreur calée à
/// l'échantillon (blancs pour combler).
pub fn to_otio(title: &str, items: &[Item]) -> String {
    let mut video = Vec::new();
    let mut sound = Vec::new();
    for it in items {
        let v = it.video;
        let r = rate_of(v).as_f64();
        let v_frames = (v.duration * r).round();
        video.push(clip(v, (v.start.unwrap_or(0.0) * r).round(), v_frames, r));
        // Le son occupe la même durée que la vidéo, blancs compris.
        let sr = it
            .audio
            .map(|(a, _)| a.sample_rate.max(1) as f64)
            .unwrap_or(48_000.0);
        let slot = (v.duration * sr).round();
        match it.audio {
            Some((a, offset)) => {
                let lead = (offset.max(0.0) * sr).round();
                let skip = (-offset).max(0.0);
                let dur = ((v.duration - offset.max(0.0)).min(a.duration - skip) * sr)
                    .round()
                    .max(0.0);
                if lead > 0.0 {
                    sound.push(gap(lead, sr));
                }
                sound.push(clip(
                    a,
                    ((a.start.unwrap_or(0.0) + skip) * sr).round(),
                    dur,
                    sr,
                ));
                if slot - lead - dur > 0.0 {
                    sound.push(gap(slot - lead - dur, sr));
                }
            }
            None => sound.push(gap(slot, sr)),
        }
    }
    let track = |name: &str, kind: &str, children: Vec<Value>| {
        json!({
            "OTIO_SCHEMA": "Track.1",
            "name": name,
            "kind": kind,
            "children": children,
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
        })
    };
    let doc = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": title,
        "global_start_time": null,
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "children": [track("V1", "Video", video), track("A1", "Audio", sound)],
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
        },
        "metadata": { "veriflow": { "sync": true } },
    });
    serde_json::to_string_pretty(&doc).unwrap_or_default()
}
