//! TRANSCODE : conversion via FFmpeg, préréglages et file d'attente (charte §7.6).
//!
//! Chaque fichier est écrit sous un nom provisoire (`.part`) puis renommé une
//! fois terminé : une conversion interrompue ne laisse jamais un fichier
//! incomplet sous son nom final, et l'original n'est jamais écrasé.

pub mod bwf;
pub mod encoders;
pub mod loudness;
pub mod preset;

use std::collections::HashSet;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub use loudness::Loudness;
pub use preset::{
    find, BitDepth, Category, Domain, EncoderChoice, LoudnessTarget, Preset, Settings, PRESETS,
};

use crate::media::probe::{probe, MediaInfo};
use crate::{tools, Error, Result};

/// Fichier déjà présent sous le nom de sortie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Existing {
    /// Nouveau nom (« _1 », « _2 »...).
    #[default]
    Rename,
    Overwrite,
    Skip,
}

/// Demande de conversion d'un lot de fichiers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Request {
    pub sources: Vec<PathBuf>,
    pub settings: Settings,
    /// Dossier de sortie ; à côté de chaque source si absent.
    #[serde(default)]
    pub dest: Option<PathBuf>,
    /// Ajouté au nom de chaque fichier produit.
    #[serde(default)]
    pub suffix: String,
    #[serde(default)]
    pub existing: Existing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Done,
    Skipped,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub source: PathBuf,
    pub output: Option<PathBuf>,
    pub status: Status,
    pub message: Option<String>,
    pub encoder: Option<EncoderChoice>,
    pub loudness: Option<Loudness>,
    pub seconds: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    FileStarted {
        index: usize,
        source: PathBuf,
    },
    /// `fraction` : avancement du fichier (0 à 1) ; `speed` : vitesse par
    /// rapport au temps réel (2.0 = deux fois plus vite).
    Progress {
        index: usize,
        fraction: f64,
        speed: f64,
    },
    FileDone {
        index: usize,
        result: FileResult,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub files: Vec<FileResult>,
    pub cancelled: bool,
    pub seconds: f64,
}

/// Chemin de sortie d'un fichier, selon la règle choisie pour les fichiers
/// déjà présents ; `None` : fichier à ignorer. `taken` : sorties déjà
/// attribuées dans ce lot (deux sources du même nom ne s'écrasent pas).
pub fn output_path(
    source: &Path,
    dest: Option<&Path>,
    suffix: &str,
    ext: &str,
    existing: Existing,
    taken: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let dir = dest
        .map(Path::to_path_buf)
        .or_else(|| source.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    let make = |n: u32| {
        let num = if n == 0 {
            String::new()
        } else {
            format!("_{n}")
        };
        dir.join(format!("{stem}{suffix}{num}.{ext}"))
    };
    let same_as_source = |p: &Path| p == source || (p.exists() && same_file(p, source));
    let first = make(0);
    let busy = |p: &Path| p.exists() || taken.contains(p);
    if !busy(&first) {
        return Some(first);
    }
    match existing {
        Existing::Skip if !taken.contains(&first) => None,
        Existing::Overwrite if !taken.contains(&first) && !same_as_source(&first) => Some(first),
        _ => (1..10_000).map(make).find(|p| !busy(p)),
    }
}

fn same_file(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// Nom provisoire pendant l'écriture.
fn part_path(output: &Path) -> PathBuf {
    let mut name = output.file_name().unwrap_or_default().to_os_string();
    name.push(".part");
    output.with_file_name(name)
}

/// Lance FFmpeg et suit son avancement jusqu'à la fin ou l'annulation.
fn run_ffmpeg(
    input: &Path,
    args: &[String],
    format: &str,
    out: &Path,
    duration: f64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(f64, f64),
) -> Result<()> {
    let mut cmd = tools::command("ffmpeg")?;
    cmd.args(["-hide_banner", "-nostdin", "-y", "-v", "error"])
        .args(["-progress", "pipe:1", "-nostats", "-stats_period", "0.5"])
        .arg("-i")
        .arg(input)
        .args(args)
        .args(["-f", format])
        .arg(out)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child: Child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Mutex::new(child);
    let finished = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);
    let (status, text) = thread::scope(|scope| -> Result<_> {
        let errors = scope.spawn(move || {
            let mut text = String::new();
            if let Some(mut e) = stderr {
                let _ = e.read_to_string(&mut text);
            }
            text
        });
        // Surveillance de l'annulation, indépendante du rythme des messages de FFmpeg.
        scope.spawn(|| {
            while !finished.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    cancelled.store(true, Ordering::Relaxed);
                    if let Ok(mut c) = child.lock() {
                        let _ = c.kill();
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        if let Some(out) = stdout {
            let (mut time, mut speed) = (0.0f64, 0.0f64);
            for line in BufReader::new(out).lines().map_while(|l| l.ok()) {
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                match k {
                    // Les deux clés sont en microsecondes (historique de FFmpeg).
                    "out_time_us" | "out_time_ms" => {
                        if let Ok(us) = v.trim().parse::<f64>() {
                            time = us / 1_000_000.0;
                        }
                    }
                    "speed" => speed = v.trim().trim_end_matches('x').parse().unwrap_or(speed),
                    "progress" => {
                        let f = if duration > 0.0 {
                            (time / duration).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        progress(if v.trim() == "end" { 1.0 } else { f }, speed);
                    }
                    _ => {}
                }
            }
        }
        let status = loop {
            let done = child.lock().unwrap_or_else(|e| e.into_inner()).try_wait();
            match done {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(e) => break Err(e),
            }
        };
        finished.store(true, Ordering::Relaxed);
        let text = errors.join().unwrap_or_default();
        Ok((status?, text))
    })?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    if !status.success() {
        let message = text
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("échec sans message")
            .trim()
            .to_owned();
        return Err(Error::Tool {
            tool: "ffmpeg".into(),
            message,
        });
    }
    Ok(())
}

/// Convertit (ou mesure) un fichier.
fn process(
    preset: &Preset,
    req: &Request,
    source: &Path,
    info: &MediaInfo,
    output: Option<&Path>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f64, f64),
) -> Result<(Option<EncoderChoice>, Option<Loudness>)> {
    let settings = &req.settings;
    let max = preset::max_channels(preset.kind, preset::total_channels(info));
    // Mesure : préréglage d'analyse, ou normalisation demandée.
    let mut loud = None;
    if preset.kind == preset::Kind::Analyze || (preset.is_audio() && settings.loudness.is_some()) {
        progress(0.0, 0.0);
        let mut l = loudness::measure(source, info, max)?;
        if let Some(target) = settings.loudness {
            if preset.kind != preset::Kind::Analyze && l.integrated.is_finite() {
                l.plan(target);
            }
        }
        loud = Some(l);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    let Some(output) = output else {
        progress(1.0, 0.0);
        return Ok((None, loud));
    };
    let gain = loud.as_ref().and_then(|l| l.gain);
    let plan = preset::build(preset, settings, info, gain)?;
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_path(output);
    let result = run_ffmpeg(
        source,
        &plan.args,
        preset.format,
        &part,
        info.duration,
        cancel,
        progress,
    );
    if let Err(e) = result {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    // Métadonnées de tournage d'un WAV source recopiées dans le WAV produit.
    if preset.format == "wav" && preset.is_audio() {
        let old = info.audio.first().map(|a| a.sample_rate).unwrap_or(0);
        let new = preset::output_rate(preset, settings, info);
        let bits = match preset::output_depth(preset, settings, info) {
            BitDepth::S16 => 16,
            BitDepth::S24 => 24,
            BitDepth::F32 => 32,
        };
        let _ = bwf::carry(source, &part, old, new, bits);
    }
    if output.exists() {
        std::fs::remove_file(output)?;
    }
    std::fs::rename(&part, output)?;
    Ok((plan.encoder, loud))
}

/// Traite un lot de fichiers l'un après l'autre.
pub fn execute(req: &Request, cancel: &AtomicBool, mut on: impl FnMut(Event)) -> Result<Summary> {
    let preset = find(&req.settings.preset).ok_or_else(|| {
        Error::Unsupported(format!("préréglage inconnu : {}", req.settings.preset))
    })?;
    let started = Instant::now();
    let mut files = Vec::new();
    let mut taken = HashSet::new();
    for (index, source) in req.sources.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        on(Event::FileStarted {
            index,
            source: source.clone(),
        });
        let t0 = Instant::now();
        let output = if preset.ext.is_empty() {
            Ok(None)
        } else {
            match output_path(
                source,
                req.dest.as_deref(),
                &req.suffix,
                preset.ext,
                req.existing,
                &taken,
            ) {
                Some(p) => Ok(Some(p)),
                None => Err(()),
            }
        };
        let mut result = FileResult {
            source: source.clone(),
            output: None,
            status: Status::Done,
            message: None,
            encoder: None,
            loudness: None,
            seconds: 0.0,
        };
        match output {
            Err(()) => {
                result.status = Status::Skipped;
                result.message = Some("fichier déjà présent".into());
            }
            Ok(output) => {
                if let Some(o) = &output {
                    taken.insert(o.clone());
                }
                let outcome = probe(source).and_then(|info| {
                    process(
                        preset,
                        req,
                        source,
                        &info,
                        output.as_deref(),
                        cancel,
                        &mut |fraction, speed| {
                            on(Event::Progress {
                                index,
                                fraction,
                                speed,
                            })
                        },
                    )
                });
                match outcome {
                    Ok((encoder, loud)) => {
                        result.output = output;
                        result.encoder = encoder;
                        result.loudness = loud;
                    }
                    Err(Error::Cancelled) => result.status = Status::Cancelled,
                    Err(e) => {
                        result.status = Status::Failed;
                        result.message = Some(e.to_string());
                    }
                }
            }
        }
        result.seconds = t0.elapsed().as_secs_f64();
        on(Event::FileDone {
            index,
            result: result.clone(),
        });
        files.push(result);
    }
    Ok(Summary {
        cancelled: cancel.load(Ordering::Relaxed),
        files,
        seconds: started.elapsed().as_secs_f64(),
    })
}

#[cfg(test)]
mod tests;
