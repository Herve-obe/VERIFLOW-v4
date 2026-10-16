//! Exports SYNC : re-wrap sans réencodage (image de la caméra + son de
//! l'enregistreur calé), FCPXML et OpenTimelineIO.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::SyncFile;
use crate::aaf::mobs as aaf_mobs;
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
    /// En MOV aussi, une piste mono par canal (toujours le cas en MXF).
    pub mono_tracks: bool,
    /// Corriger la dérive mesurée en rééchantillonnant le son de l'enregistreur.
    pub correct_drift: bool,
    pub suffix: String,
    pub existing: Existing,
}

impl Default for RewrapOptions {
    fn default() -> Self {
        Self {
            dest: None,
            format: s("mov"),
            keep_camera_audio: true,
            mono_tracks: false,
            correct_drift: true,
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
/// Correction de dérive : (décalage à appliquer au début du son, vitesse).
///
/// `offset` est mesuré au milieu du passage commun et `drift` est l'écart
/// accumulé sur tout ce passage (positif : son en retard à la fin). Le son est
/// lu `speed` fois plus vite (rééchantillonnage, comme un pull-up/pull-down),
/// et décalé pour rester calé au milieu.
pub fn drift_correction(video: &SyncFile, audio: &SyncFile, offset: f64, drift: f64) -> (f64, f64) {
    let from = offset.max(0.0);
    let to = (offset + audio.duration).min(video.duration);
    let common = to - from;
    if common <= 0.0 {
        return (offset, 1.0);
    }
    let speed = 1.0 + drift / common;
    let middle = (from + to) / 2.0;
    // Instant du son entendu au milieu : il doit y rester après accélération.
    (middle - (middle - offset) / speed, speed)
}

/// Calage d'une paire : décalage et dérive mesurée (s).
#[derive(Debug, Clone, Copy, Default)]
pub struct Timing {
    pub offset: f64,
    pub drift: Option<f64>,
}

pub fn rewrap(
    video: &SyncFile,
    audio: &SyncFile,
    timing: Timing,
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
    let (offset, speed) = match timing.drift {
        Some(d) if opts.correct_drift && d != 0.0 => {
            drift_correction(video, audio, timing.offset, d)
        }
        _ => (timing.offset, 1.0),
    };
    let rate = ainfo.audio[0].sample_rate.max(1);
    // Rééchantillonnage : le son est lu `speed` fois plus vite.
    let speed_filters = if speed == 1.0 {
        Vec::new()
    } else {
        vec![
            format!("asetrate={:.6}", rate as f64 * speed),
            format!("aresample={rate}"),
        ]
    };
    let mut in_args = Vec::new();
    if offset > 0.0 {
        in_args.extend([s("-itsoffset"), format!("{offset:.6}")]);
    } else if offset < 0.0 {
        // Le saut se fait dans le temps du fichier, avant l'accélération.
        in_args.extend([s("-ss"), format!("{:.6}", -offset * speed)]);
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
    if format == "mxf" || opts.mono_tracks {
        let (graph, outs) = mono_tracks(
            1,
            &ainfo,
            pan,
            &[speed_filters.clone(), vec![s("aresample=48000")]].concat(),
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
    } else if pan.is_some() || speed != 1.0 {
        a.extend([
            s("-filter_complex"),
            audio_chain(1, &ainfo, pan, &speed_filters, "aout"),
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

/// Fichier lié dans l'AAF : sa cadence image s'il en a une.
fn aaf_media<'a>(f: &'a SyncFile, seq_rate: FrameRate) -> aaf_mobs::Media<'a> {
    let info = probe(&f.path).ok();
    let rate = |r: FrameRate| (r.num as i32, r.den.max(1) as i32);
    let picture = f.video.then(|| {
        let v = info.as_ref().and_then(|i| i.video.as_ref());
        let r = f.rate.or(v.map(|v| v.rate)).unwrap_or(seq_rate);
        aaf_mobs::Picture {
            rate: rate(r),
            frames: v
                .map(|v| v.frame_count)
                .filter(|n| *n > 0)
                .unwrap_or_else(|| frames(f.duration, r).max(1)),
            width: f.width.max(v.map_or(0, |v| v.width)),
            height: f.height.max(v.map_or(0, |v| v.height)),
            codec: v.map(|v| v.codec.clone()).unwrap_or_default(),
            pix_fmt: v.map(|v| v.pix_fmt.clone()).unwrap_or_default(),
        }
    });
    let mut sounds: Vec<aaf_mobs::Sound> = info
        .as_ref()
        .map(|i| {
            i.audio
                .iter()
                .filter(|a| a.channels > 0 && a.sample_rate > 0)
                .map(|a| aaf_mobs::Sound {
                    sample_rate: a.sample_rate,
                    channels: a.channels,
                    bits: a.bits.unwrap_or(24),
                    length: samples(f.duration, a.sample_rate),
                })
                .collect()
        })
        .unwrap_or_default();
    if sounds.is_empty() && f.channels > 0 && f.sample_rate > 0 {
        sounds.push(aaf_mobs::Sound {
            sample_rate: f.sample_rate,
            channels: f.channels,
            bits: 24,
            length: samples(f.duration, f.sample_rate),
        });
    }
    let tc_rate = f.rate.unwrap_or(seq_rate);
    aaf_mobs::Media {
        path: &f.path,
        url: file_url(&crate::absolute_path(&f.path).to_string_lossy()),
        name: f
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| f.name.clone()),
        picture,
        sounds,
        wave: if f.video {
            None
        } else {
            aaf_mobs::wave_summary(&f.path)
        },
        timecode: f.start.map(|s| {
            (
                frames(s, tc_rate),
                rate(tc_rate),
                f.timecode.as_deref().is_some_and(|t| t.contains(';')),
            )
        }),
    }
}

/// AAF (Media Composer, Pro Tools, DaVinci Resolve) : séquence avec piste de
/// timecode, V1 (les plans bout à bout), une piste par canal de
/// l'enregistreur calée à l'échantillon, puis le son témoin des caméras.
/// Les médias sont liés (aucune essence copiée dans le fichier).
pub fn write_aaf(path: &std::path::Path, title: &str, items: &[Item]) -> Result<()> {
    use aaf_mobs::{filler, sequence, slot, source_clip, Kind};
    let seq_rate = items
        .first()
        .map(|i| rate_of(i.video))
        .unwrap_or(FrameRate::new(25, 1));
    let sr = items
        .iter()
        .find_map(|i| i.audio.map(|(a, _)| a.sample_rate))
        .filter(|r| *r > 0)
        .unwrap_or(48_000);
    let edit = (seq_rate.num as i32, seq_rate.den.max(1) as i32);

    // Un lien par fichier, même s'il sert à plusieurs plans.
    let mut mobs = Vec::new();
    let mut linked: Vec<(PathBuf, aaf_mobs::Linked)> = Vec::new();
    let mut link = |f: &SyncFile, mobs: &mut Vec<aaf_mobs::Mob>| -> usize {
        if let Some(i) = linked.iter().position(|(p, _)| *p == f.path) {
            return i;
        }
        let mut l = aaf_mobs::link(&aaf_media(f, seq_rate));
        mobs.append(&mut l.mobs);
        linked.push((f.path.clone(), l));
        linked.len() - 1
    };
    let refs: Vec<(usize, Option<usize>)> = items
        .iter()
        .map(|it| {
            (
                link(it.video, &mut mobs),
                it.audio.map(|(a, _)| link(a, &mut mobs)),
            )
        })
        .collect();

    let rec_tracks = items
        .iter()
        .filter_map(|i| i.audio.map(|(a, _)| kept_channels(a).len()))
        .max()
        .unwrap_or(0);
    let cam_tracks = refs
        .iter()
        .map(|(v, _)| linked[*v].1.sounds.len())
        .max()
        .unwrap_or(0);
    let mut picture = Vec::new();
    let mut rec: Vec<Vec<aaf_mobs::Component>> = (0..rec_tracks).map(|_| Vec::new()).collect();
    let mut cam: Vec<Vec<aaf_mobs::Component>> = (0..cam_tracks).map(|_| Vec::new()).collect();
    let mut pos: i64 = 0;
    let to_samples = |f: i64| (f as f64 * sr as f64 / seq_rate.as_f64()).round() as i64;
    for (it, (v, a)) in items.iter().zip(&refs) {
        let len = frames(it.video.duration, seq_rate).max(1);
        let span = to_samples(pos + len) - to_samples(pos);
        let vl = &linked[*v].1;
        match vl.picture {
            Some(s) => picture.push(source_clip(Kind::Picture, len, 0, &vl.master, s)),
            None => picture.push(filler(Kind::Picture, len)),
        }
        // Son de l'enregistreur : blanc, clip calé, blanc.
        let placed = it.audio.zip(*a).map(|((af, offset), ai)| {
            let lead = samples(offset.max(0.0), sr).min(span);
            let skip = (-offset).max(0.0);
            let dur = samples(
                (it.video.duration - offset.max(0.0)).min(af.duration - skip),
                sr,
            )
            .clamp(0, span - lead);
            (
                kept_channels(af),
                &linked[ai].1,
                lead,
                samples(skip, sr),
                dur,
            )
        });
        for (k, track) in rec.iter_mut().enumerate() {
            match &placed {
                Some((kept, al, lead, start, dur)) if *dur > 0 && k < kept.len() => {
                    let s = al.sounds.get(kept[k]).map_or(kept[k] as u32 + 1, |s| s.0);
                    if *lead > 0 {
                        track.push(filler(Kind::Sound, *lead));
                    }
                    track.push(source_clip(Kind::Sound, *dur, *start, &al.master, s));
                    if span - lead - dur > 0 {
                        track.push(filler(Kind::Sound, span - lead - dur));
                    }
                }
                _ => track.push(filler(Kind::Sound, span)),
            }
        }
        // Son témoin de la caméra, sur toute la durée du plan.
        for (k, track) in cam.iter_mut().enumerate() {
            match vl.sounds.get(k) {
                Some((s, _)) => track.push(source_clip(Kind::Sound, span, 0, &vl.master, *s)),
                None => track.push(filler(Kind::Sound, span)),
            }
        }
        pos += len;
    }

    let mut slots = vec![
        slot(
            1,
            "TC1",
            edit,
            aaf_mobs::timecode(pos, 0, seq_rate.nominal() as u16, false),
            Some(1),
        ),
        slot(2, "V1", edit, sequence(Kind::Picture, picture), Some(1)),
    ];
    for (k, parts) in rec.into_iter().chain(cam).enumerate() {
        let n = k as u32 + 1;
        slots.push(slot(
            n + 2,
            &format!("A{n}"),
            (sr as i32, 1),
            sequence(Kind::Sound, parts),
            Some(n),
        ));
    }
    let mut comp = aaf_mobs::mob(aaf_mobs::COMPOSITION, aaf_mobs::mob_id(title), title, slots);
    comp.obj = comp.obj.data(0x4408, aaf_mobs::usage_top_level());
    mobs.push(comp);
    crate::aaf::write(path, &aaf_mobs::header(mobs))
}
