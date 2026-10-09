//! TRANSCODE : conversion via FFmpeg, préréglages et file d'attente (charte §7.6).
//!
//! Chaque fichier est écrit sous un nom provisoire (`.part`) puis renommé une
//! fois terminé : une conversion interrompue ne laisse jamais un fichier
//! incomplet sous son nom final, et l'original n'est jamais écrasé.

pub mod analysis;
pub mod build;
pub mod bwf;
pub mod catalog;
pub mod encoders;
pub mod filters;
pub mod loudness;
pub mod report;
pub mod run;
pub mod settings;
pub mod special;
pub mod video;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::{Deserialize, Serialize};

pub use analysis::{Analysis, Segment};
pub use catalog::{find, Category, Domain, Kind, Preset, PRESETS};
pub use loudness::Loudness;
pub use settings::{AudioMode, BitDepth, LoudnessTarget, Settings};
pub use video::EncoderChoice;

use crate::media::catalog::{kind_of, MediaKind};
use crate::media::probe::{probe, MediaInfo};
use crate::{Error, Result};
use build::{build, source_ext, Job, Plan};

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
#[serde(default)]
pub struct Request {
    pub sources: Vec<PathBuf>,
    pub settings: Settings,
    /// Dossier de sortie ; à côté de chaque source si absent.
    pub dest: Option<PathBuf>,
    /// Nommage : préfixe, suffixe, remplacement de texte, numérotation.
    pub prefix: String,
    pub suffix: String,
    pub replace_from: String,
    pub replace_to: String,
    pub numbering: bool,
    pub number_start: u32,
    pub number_digits: u32,
    pub existing: Existing,
    /// Empreinte XXH128 de chaque fichier produit.
    pub checksum: bool,
    /// Rapport CSV du lot dans le dossier de sortie.
    pub report: bool,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            settings: Settings::default(),
            dest: None,
            prefix: String::new(),
            suffix: String::new(),
            replace_from: String::new(),
            replace_to: String::new(),
            numbering: false,
            number_start: 1,
            number_digits: 3,
            existing: Existing::Rename,
            checksum: false,
            report: false,
        }
    }
}

impl Request {
    /// Nom (sans extension) du fichier produit à partir de la source n° `index`.
    pub fn output_name(&self, source: &Path, index: usize, extra: &str) -> String {
        let mut stem = source
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !self.replace_from.is_empty() {
            stem = stem.replace(&self.replace_from, &self.replace_to);
        }
        let mut name = format!("{}{stem}{}{extra}", self.prefix, self.suffix);
        if self.numbering {
            let n = self.number_start as usize + index;
            name += &format!(
                "_{n:0width$}",
                width = self.number_digits.clamp(1, 8) as usize
            );
        }
        // Caractères refusés par Windows dans un nom de fichier.
        name.chars()
            .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
            .collect()
    }

    fn dir_for(&self, source: &Path) -> PathBuf {
        self.dest
            .clone()
            .or_else(|| source.parent().map(Path::to_path_buf))
            .unwrap_or_default()
    }
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
    /// Fichier (ou dossier d'images) produit, ou rapport d'analyse.
    pub output: Option<PathBuf>,
    /// Tous les fichiers produits (une piste par fichier : plusieurs).
    pub outputs: Vec<PathBuf>,
    pub status: Status,
    pub message: Option<String>,
    pub encoder: Option<EncoderChoice>,
    pub loudness: Option<Loudness>,
    pub analysis: Option<Analysis>,
    pub checksum: Option<String>,
    pub size: Option<u64>,
    /// Qualité VMAF du fichier produit (option de vérification).
    pub vmaf: Option<f64>,
    pub seconds: f64,
}

impl FileResult {
    fn new(source: &Path) -> Self {
        Self {
            source: source.to_path_buf(),
            output: None,
            outputs: Vec::new(),
            status: Status::Done,
            message: None,
            encoder: None,
            loudness: None,
            analysis: None,
            checksum: None,
            size: None,
            vmaf: None,
            seconds: 0.0,
        }
    }
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
        result: Box<FileResult>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct Summary {
    pub files: Vec<FileResult>,
    pub cancelled: bool,
    pub seconds: f64,
    /// Rapport CSV du lot.
    pub report: Option<PathBuf>,
}

/// Chemin de sortie selon la règle choisie pour les fichiers déjà présents ;
/// `None` : fichier à ignorer. `taken` : sorties déjà attribuées dans ce lot
/// (deux sources du même nom ne s'écrasent pas). `ext` vide : dossier.
pub fn output_path(
    dir: &Path,
    name: &str,
    ext: &str,
    source: &Path,
    existing: Existing,
    taken: &HashSet<PathBuf>,
) -> Option<PathBuf> {
    let make = |n: u32| {
        let num = if n == 0 {
            String::new()
        } else {
            format!("_{n}")
        };
        if ext.is_empty() {
            dir.join(format!("{name}{num}"))
        } else {
            dir.join(format!("{name}{num}.{ext}"))
        }
    };
    let same_as_source = |p: &Path| p == source || (p.exists() && same_file(p, source));
    let first = make(0);
    let busy = |p: &Path| p.exists() || taken.contains(p);
    if !busy(&first) {
        return Some(first);
    }
    match existing {
        Existing::Skip if !taken.contains(&first) => None,
        Existing::Overwrite
            if !taken.contains(&first) && !same_as_source(&first) && !first.is_dir() =>
        {
            Some(first)
        }
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

fn is_wav(path: &Path) -> bool {
    kind_of(path) == Some(MediaKind::Audio) && matches!(source_ext(path).as_str(), "wav" | "bwf")
}

/// Exécute un plan vers `output` (fichier, ou dossier pour une séquence d'images).
fn write_output(
    plan: &Plan,
    source: &Path,
    output: &Path,
    ext: &str,
    retime: Option<bwf::Retime>,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f64, f64),
) -> Result<()> {
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = part_path(output);
    let input = plan.input.clone().unwrap_or_else(|| source.to_path_buf());
    let result = if plan.sequence {
        let _ = std::fs::remove_dir_all(&part);
        std::fs::create_dir_all(&part)?;
        let name = output
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let pattern = part.join(format!("{name}_%07d.{ext}"));
        run::run(plan, &input, Some(&pattern), false, cancel, progress).map(|_| ())
    } else {
        run::run(plan, &input, Some(&part), false, cancel, progress).map(|_| ())
    };
    if let Err(e) = result {
        let _ = if plan.sequence {
            std::fs::remove_dir_all(&part)
        } else {
            std::fs::remove_file(&part)
        };
        return Err(e);
    }
    // Métadonnées de tournage d'un WAV source recopiées dans le WAV produit.
    if let Some(t) = retime.filter(|_| plan.format == "wav" && is_wav(source)) {
        let bits = wav_bits(&part).unwrap_or(24);
        let _ = bwf::carry(source, &part, &t, bits);
    }
    if output.exists() {
        if output.is_dir() {
            std::fs::remove_dir_all(output)?;
        } else {
            std::fs::remove_file(output)?;
        }
    }
    std::fs::rename(&part, output)?;
    Ok(())
}

/// Résolution du WAV produit (lue dans son en-tête).
fn wav_bits(path: &Path) -> Option<u16> {
    crate::media::wav::read_info(path).ok().map(|w| w.bits)
}

/// Mesure de loudness avant normalisation : sur le son tel qu'il sera produit.
fn measure_for(
    preset: &Preset,
    settings: &Settings,
    info: &MediaInfo,
    source: &Path,
) -> Result<Loudness> {
    let channels = filters::total_channels(info);
    let max = if preset.is_audio() {
        build::max_channels(preset.kind, channels)
    } else {
        match settings.audio_mode.unwrap_or(preset.audio_mode) {
            AudioMode::FirstTwo => Some(2),
            _ => None,
        }
    };
    loudness::measure(source, info, max)
}

struct Ctx<'a> {
    preset: &'static Preset,
    req: &'a Request,
    cancel: &'a AtomicBool,
}

/// Traite un fichier (conversion, extraction ou analyse).
fn process(
    ctx: &Ctx,
    index: usize,
    source: &Path,
    taken: &mut HashSet<PathBuf>,
    progress: &mut dyn FnMut(f64, f64),
    result: &mut FileResult,
) -> Result<()> {
    let (preset, req, cancel) = (ctx.preset, ctx.req, ctx.cancel);
    let settings = &req.settings;
    let info = probe(source)?;
    let ext = if preset.ext == "*" {
        source_ext(source)
    } else {
        preset.ext.to_owned()
    };
    let dir = req.dir_for(source);
    let mut job = Job {
        preset,
        settings,
        info: &info,
        source,
        gain: None,
    };
    // Sortie principale (sauf analyses sans fichier et pistes séparées).
    let reserve = |extra: &str, ext: &str, taken: &mut HashSet<PathBuf>| -> Result<PathBuf> {
        let name = req.output_name(source, index, extra);
        let p = output_path(&dir, &name, ext, source, req.existing, taken)
            .ok_or_else(|| Error::AlreadyExists(dir.join(&name).display().to_string()))?;
        taken.insert(p.clone());
        Ok(p)
    };

    match preset.kind {
        Kind::Loudness => {
            progress(0.0, 0.0);
            let mut l = measure_for(preset, settings, &info, source)?;
            l.gain = None;
            result.loudness = Some(l);
            progress(1.0, 0.0);
            return Ok(());
        }
        Kind::Vmaf => {
            let reference = settings
                .reference
                .as_deref()
                .and_then(|r| analysis::find_reference(source, r))
                .ok_or_else(|| {
                    Error::Unsupported("original introuvable pour la comparaison VMAF".into())
                })?;
            let score = analysis::vmaf(source, &reference, cancel, progress)?;
            result.analysis = Some(Analysis {
                segments: Vec::new(),
                vmaf: Some(score),
            });
            result.vmaf = Some(score);
            result.message = Some(format!("original : {}", reference.display()));
            return Ok(());
        }
        Kind::CutDetect | Kind::BlackDetect | Kind::OfflineDetect | Kind::SilenceDetect => {
            let (input_args, start, duration) = build::trim(&job)?;
            let segments = analysis::detect(
                preset.kind,
                source,
                &info,
                settings.threshold,
                &input_args,
                start,
                duration,
                cancel,
                progress,
            )?;
            let out = reserve("", &ext, taken)?;
            if let Some(d) = out.parent() {
                std::fs::create_dir_all(d)?;
            }
            let text = if preset.kind == Kind::CutDetect {
                let cuts: Vec<f64> = segments.iter().map(|s| s.start).collect();
                let name = source
                    .file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let clip = source
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                analysis::to_edl(&name, &clip, &info, &cuts)
            } else {
                analysis::to_csv(&segments)
            };
            std::fs::write(&out, text)?;
            result.output = Some(out.clone());
            result.outputs.push(out);
            result.analysis = Some(Analysis {
                segments,
                vmaf: None,
            });
            return Ok(());
        }
        _ => {}
    }

    // Normalisation : mesure, puis gain.
    let normalize = settings.loudness.filter(|_| {
        (preset.is_audio() || preset.has_video_audio())
            && !info.audio.is_empty()
            && settings.audio_mode != Some(AudioMode::None)
    });
    if let Some(target) = normalize {
        progress(0.0, 0.0);
        let mut l = measure_for(preset, settings, &info, source)?;
        if l.integrated.is_finite() {
            job.gain = Some(l.plan(target));
        }
        result.loudness = Some(l);
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }

    // Plans et fichiers produits.
    let mut plans: Vec<(Plan, PathBuf)> = Vec::new();
    match preset.kind {
        Kind::ExtractTracks => {
            let (input_args, start, duration) = build::trim(&job)?;
            for (extra, plan) in special::tracks(source, &info, &input_args, start, duration)? {
                let out = reserve(&extra, "wav", taken)?;
                plans.push((plan, out));
            }
        }
        Kind::Insert => {
            let file = settings
                .insert_file
                .clone()
                .ok_or_else(|| Error::Unsupported("plan à insérer non choisi".into()))?;
            let at = settings
                .insert_at
                .clone()
                .ok_or_else(|| Error::Unsupported("point d'insertion non indiqué".into()))?;
            let insert_info = probe(&file)?;
            let plan = special::insert(source, &info, &file, &insert_info, &at, &job.format())?;
            plans.push((plan, reserve("", &ext, taken)?));
        }
        _ => {
            let plan = build(&job)?;
            let out = if plan.sequence {
                reserve("", "", taken)?
            } else {
                reserve("", &ext, taken)?
            };
            plans.push((plan, out));
        }
    }
    let total = plans.len();
    for (i, (plan, out)) in plans.iter().enumerate() {
        let src_rate = info.audio.first().map(|a| a.sample_rate).unwrap_or(0);
        let new_rate = if preset.is_audio() {
            build::output_rate(preset, settings, &info)
        } else {
            src_rate
        };
        let retime = bwf::Retime {
            old_rate: src_rate,
            new_rate,
            shift: plan.start,
        };
        let mut sub = |f: f64, sp: f64| progress((i as f64 + f) / total as f64, sp);
        write_output(plan, source, out, &ext, Some(retime), cancel, &mut sub)?;
        result.outputs.push(out.clone());
        if result.encoder.is_none() {
            result.encoder = plan.encoder.clone();
        }
    }
    result.output = result.outputs.first().cloned();
    // Vérifications : empreinte, qualité VMAF.
    if let Some(out) = result.output.clone().filter(|o| o.is_file()) {
        result.size = std::fs::metadata(&out).ok().map(|m| m.len());
        if req.checksum {
            result.checksum = Some(report::xxh128(&out)?);
        }
        if settings.vmaf_after && preset.is_video() && settings.trim_for(source).is_none() {
            result.vmaf = Some(analysis::vmaf(&out, source, cancel, &mut |_, _| {})?);
        }
    }
    Ok(())
}

/// Fusion : tous les fichiers du lot en un seul.
fn merge(ctx: &Ctx, on: &mut dyn FnMut(Event)) -> FileResult {
    let req = ctx.req;
    let first = req.sources[0].clone();
    on(Event::FileStarted {
        index: 0,
        source: first.clone(),
    });
    let t0 = Instant::now();
    let mut result = FileResult::new(&first);
    let outcome = (|| -> Result<()> {
        let mut list = Vec::new();
        for s in &req.sources {
            list.push((s.clone(), probe(s)?));
        }
        let ext = source_ext(&first);
        let plan = special::merge(&list, catalog::format_for_ext(&ext))?;
        let extra = if req.suffix.is_empty() { "_fusion" } else { "" };
        let name = req.output_name(&first, 0, extra);
        let out = output_path(
            &req.dir_for(&first),
            &name,
            &ext,
            &first,
            req.existing,
            &HashSet::new(),
        )
        .ok_or_else(|| Error::AlreadyExists(name.clone()))?;
        let mut progress = |fraction: f64, speed: f64| {
            on(Event::Progress {
                index: 0,
                fraction,
                speed,
            })
        };
        write_output(&plan, &first, &out, &ext, None, ctx.cancel, &mut progress)?;
        result.size = std::fs::metadata(&out).ok().map(|m| m.len());
        if req.checksum {
            result.checksum = Some(report::xxh128(&out)?);
        }
        result.output = Some(out.clone());
        result.outputs.push(out);
        Ok(())
    })();
    finish(&mut result, outcome, t0);
    on(Event::FileDone {
        index: 0,
        result: Box::new(result.clone()),
    });
    result
}

fn finish(result: &mut FileResult, outcome: Result<()>, t0: Instant) {
    match outcome {
        Ok(()) => {}
        Err(Error::Cancelled) => result.status = Status::Cancelled,
        Err(Error::AlreadyExists(p)) => {
            result.status = Status::Skipped;
            result.message = Some(format!("fichier déjà présent : {p}"));
        }
        Err(e) => {
            result.status = Status::Failed;
            result.message = Some(e.to_string());
        }
    }
    result.seconds = t0.elapsed().as_secs_f64();
}

/// Traite un lot de fichiers l'un après l'autre.
pub fn execute(req: &Request, cancel: &AtomicBool, mut on: impl FnMut(Event)) -> Result<Summary> {
    let preset = find(&req.settings.preset).ok_or_else(|| {
        Error::Unsupported(format!("préréglage inconnu : {}", req.settings.preset))
    })?;
    if let Some(why) = preset.unavailable() {
        return Err(Error::Unsupported(format!("{} : {why}", preset.label_fr)));
    }
    let started = Instant::now();
    let ctx = Ctx {
        preset,
        req,
        cancel,
    };
    let mut files = Vec::new();
    if preset.kind == Kind::Merge {
        if req.sources.len() < 2 {
            return Err(Error::Unsupported("fusion : au moins deux fichiers".into()));
        }
        files.push(merge(&ctx, &mut on));
    } else {
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
            let mut result = FileResult::new(source);
            let mut progress = |fraction: f64, speed: f64| {
                on(Event::Progress {
                    index,
                    fraction,
                    speed,
                })
            };
            let outcome = process(&ctx, index, source, &mut taken, &mut progress, &mut result);
            finish(&mut result, outcome, t0);
            on(Event::FileDone {
                index,
                result: Box::new(result.clone()),
            });
            files.push(result);
        }
    }
    let report = if req.report && !files.is_empty() {
        let dir = req
            .dest
            .clone()
            .or_else(|| {
                files
                    .iter()
                    .find_map(|f| f.output.as_ref()?.parent().map(Path::to_path_buf))
            })
            .or_else(|| req.sources[0].parent().map(Path::to_path_buf));
        dir.and_then(|d| report::write(&d, preset.label_fr, &files).ok())
    } else {
        None
    };
    Ok(Summary {
        cancelled: cancel.load(Ordering::Relaxed),
        files,
        seconds: started.elapsed().as_secs_f64(),
        report,
    })
}

#[cfg(test)]
mod tests;
