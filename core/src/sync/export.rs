//! Export SYNC : re-wrap sans réencodage (image de la caméra + son de
//! l'enregistreur calé). Les timelines FCPXML et OTIO ont été retirées
//! (décision d'Hervé du 19/10/2026) ; leur code est conservé sur la branche
//! `archive/sync-timelines`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use super::SyncFile;
use crate::media::probe::probe;
use crate::transcode::build::{Input, Plan};
use crate::transcode::filters::{audio_chain, mono_tracks};
use crate::transcode::{output_path, write_output, Existing};
use crate::{Error, Result};

fn s<T: ToString>(v: T) -> String {
    v.to_string()
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
            mono_tracks: true,
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
