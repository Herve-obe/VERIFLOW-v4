//! SYNC (charte §7.5) : appariement des vidéos et des sons de l'enregistreur,
//! par timecode des métadonnées, par LTC enregistré sur une piste, ou par
//! corrélation des formes d'onde ; détection de la dérive d'horloge ;
//! exports re-wrap, FCPXML et OTIO.
//!
//! Les heures sont exprimées en secondes depuis minuit, et le décalage d'une
//! paire est `début du son - début de la vidéo` : positif, le son commence
//! après la vidéo.

pub mod correlate;
pub mod export;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::{Deserialize, Serialize};

use crate::media::catalog::{kind_of, MediaKind};
use crate::media::probe::probe;
use crate::player::timecode::{FrameRate, Timecode};
use crate::{Error, Result};
use correlate::{decode_mono, gcc_phat};

const DAY: f64 = 86_400.0;
/// Fréquence de travail de la corrélation fine.
const FINE_RATE: u32 = 16_000;
/// Fréquence de la recherche sans timecode (sur de longues durées).
const COARSE_RATE: u32 = 2_000;
/// Netteté minimale d'un pic de corrélation pour être retenu.
pub const MIN_CONFIDENCE: f64 = 0.25;

/// Origine de l'heure de début d'un fichier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeSource {
    /// Timecode des métadonnées (piste tmcd, MXF, référence BWF).
    Metadata,
    /// LTC lu sur une piste son.
    Ltc,
    None,
}

/// Fichier vidéo ou son, tel que vu par SYNC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncFile {
    pub path: PathBuf,
    pub name: String,
    pub video: bool,
    /// Heure de début (secondes depuis minuit).
    pub start: Option<f64>,
    pub source: TimeSource,
    /// Timecode de début affiché.
    pub timecode: Option<String>,
    pub duration: f64,
    /// Cadence et taille d'image (vidéos).
    pub rate: Option<FrameRate>,
    pub width: u32,
    pub height: u32,
    pub sample_rate: u32,
    pub channels: u32,
    /// Canal portant du LTC (exclu de la corrélation et du son exporté).
    pub ltc_channel: Option<usize>,
    /// Noms des pistes (iXML des WAV d'enregistreur).
    pub tracks: Vec<String>,
    pub error: Option<String>,
}

/// Méthode qui a fixé le décalage d'une paire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Method {
    Timecode,
    Ltc,
    Waveform,
    Manual,
}

/// Vidéo et son associés.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pair {
    /// Indice de la vidéo et du son dans les listes de l'analyse.
    pub video: usize,
    pub audio: Option<usize>,
    /// Début du son moins début de la vidéo (secondes).
    pub offset: f64,
    pub method: Method,
    /// Décalage confirmé ou corrigé par la forme d'onde.
    pub refined: bool,
    /// Netteté du pic de corrélation (0 à 1).
    pub confidence: Option<f64>,
    /// Dérive mesurée entre le début et la fin du plan (secondes, positive :
    /// le son est en retard à la fin du plan quand il est calé au début).
    pub drift: Option<f64>,
    /// Dérive exprimée en images de la vidéo.
    pub drift_frames: Option<f64>,
    pub validated: bool,
    pub note: Option<String>,
}

/// Méthode d'appariement choisie pour le lot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// Timecode ou LTC d'abord, forme d'onde selon les options.
    #[default]
    Auto,
    /// Timecode ou LTC seulement, sans corrélation.
    Timecode,
    /// Forme d'onde seulement : le timecode est ignoré, chaque plan est
    /// cherché dans tous les sons du lot.
    Waveform,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(default)]
pub struct Options {
    pub mode: Mode,
    /// Affiner le décalage par la forme d'onde quand les deux ont du son.
    pub refine: bool,
    /// Chercher par la forme d'onde les vidéos sans timecode commun.
    pub waveform_search: bool,
    /// Mesurer la dérive d'horloge.
    pub drift: bool,
    /// Fenêtre de recherche autour du décalage donné par le timecode (s).
    pub window: f64,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            mode: Mode::Auto,
            refine: true,
            waveform_search: true,
            drift: true,
            window: 2.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Analysis {
    pub videos: Vec<SyncFile>,
    pub audios: Vec<SyncFile>,
    pub pairs: Vec<Pair>,
}

/// Étape de l'analyse, pour l'avancement.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    Reading {
        index: usize,
        total: usize,
        name: String,
    },
    Matching {
        index: usize,
        total: usize,
        name: String,
    },
}

fn tc_seconds(tc: Timecode) -> f64 {
    tc.frames as f64 * tc.rate.den as f64 / tc.rate.num as f64
}

/// Timecode d'une heure (secondes depuis minuit) à la cadence donnée.
pub fn timecode_at(seconds: f64, rate: FrameRate, drop: bool) -> String {
    let frames = (seconds.rem_euclid(DAY) * rate.as_f64()).round() as i64;
    Timecode::from_frames(frames, rate, drop).to_string()
}

/// Cherche du LTC sur chaque canal du début du fichier : (canal, heure de début du fichier).
fn find_ltc(path: &Path) -> Option<(usize, f64, String)> {
    let found = crate::player::audio::producer::scan_ltc(&[path.to_path_buf()], 4.0).ok()?;
    let sr = probe(path).ok()?.audio.first()?.sample_rate.max(1) as f64;
    found.into_iter().enumerate().find_map(|(c, d)| {
        let d = d?;
        let f = d.first;
        let fps = d.fps.max(1.0);
        let nominal = fps.round();
        let tc = (f.hours as f64 * 3600.0 + f.minutes as f64 * 60.0 + f.seconds as f64)
            + f.frames as f64 / nominal;
        // Cadences NTSC : le timecode compte plus lentement que l'horloge.
        let tc = if (fps - nominal).abs() > 0.01 {
            tc * 1.001
        } else {
            tc
        };
        // La trame lue commence une image avant la fin de son mot de synchronisation.
        let at = f.sample as f64 / sr - 1.0 / fps;
        Some((c, tc - at, d.timecode))
    })
}

/// Lit ce que SYNC doit savoir d'un fichier.
pub fn describe(path: &Path) -> SyncFile {
    let name = path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let mut f = SyncFile {
        path: path.to_path_buf(),
        name,
        video: kind_of(path) == Some(MediaKind::Video),
        start: None,
        source: TimeSource::None,
        timecode: None,
        duration: 0.0,
        rate: None,
        width: 0,
        height: 0,
        sample_rate: 0,
        channels: 0,
        ltc_channel: None,
        tracks: Vec::new(),
        error: None,
    };
    let info = match probe(path) {
        Ok(i) => i,
        Err(e) => {
            f.error = Some(e.to_string());
            return f;
        }
    };
    f.video = info.video.is_some();
    f.duration = info.duration;
    f.rate = info.video.as_ref().map(|v| v.rate);
    f.width = info.video.as_ref().map(|v| v.width).unwrap_or(0);
    f.height = info.video.as_ref().map(|v| v.height).unwrap_or(0);
    f.sample_rate = info.audio.first().map(|a| a.sample_rate).unwrap_or(0);
    f.channels = info.audio.iter().map(|a| a.channels.max(1)).sum();
    if let Some(tc) = info.start_tc() {
        f.start = Some(tc_seconds(tc));
        f.source = TimeSource::Metadata;
        f.timecode = Some(tc.to_string());
    } else if let Ok(w) = crate::media::wav::read_info(path) {
        if let Some(r) = w.time_reference {
            f.start = Some(r as f64 / w.sample_rate.max(1) as f64);
            f.source = TimeSource::Metadata;
        }
        f.tracks = (0..w.channels as usize).map(|c| w.track_name(c)).collect();
    }
    // LTC : heure de début si les métadonnées n'en donnent pas, et canal à
    // exclure dans tous les cas.
    if f.channels > 0 {
        if let Some((c, start, tc)) = find_ltc(path) {
            f.ltc_channel = Some(c);
            if f.start.is_none() {
                f.start = Some(start);
                f.source = TimeSource::Ltc;
                f.timecode = Some(tc);
            }
        }
    }
    if f.timecode.is_none() {
        if let Some(s) = f.start {
            f.timecode = Some(timecode_at(s, FrameRate::new(25, 1), false));
        }
    }
    f
}

/// Recouvrement (secondes) de deux plages horaires, minuit compris.
fn overlap(a_start: f64, a_dur: f64, b_start: f64, b_dur: f64) -> (f64, f64) {
    // Décalage ramené à moins d'une demi-journée (plans à cheval sur minuit).
    let mut off = b_start - a_start;
    if off > DAY / 2.0 {
        off -= DAY;
    } else if off < -DAY / 2.0 {
        off += DAY;
    }
    let from = off.max(0.0);
    let to = (off + b_dur).min(a_dur);
    ((to - from).max(0.0), off)
}

/// Appariement par l'heure : pour chaque vidéo, le son qui la recouvre le plus.
pub fn match_by_time(videos: &[SyncFile], audios: &[SyncFile]) -> Vec<Pair> {
    videos
        .iter()
        .enumerate()
        .map(|(vi, v)| {
            let best = v.start.and_then(|vs| {
                audios
                    .iter()
                    .enumerate()
                    .filter_map(|(ai, a)| {
                        let (ov, off) = overlap(vs, v.duration, a.start?, a.duration);
                        (ov > 0.0).then_some((ai, ov, off))
                    })
                    .max_by(|x, y| x.1.total_cmp(&y.1))
            });
            let method = match (v.source, best.map(|b| audios[b.0].source)) {
                (TimeSource::Ltc, _) | (_, Some(TimeSource::Ltc)) => Method::Ltc,
                _ => Method::Timecode,
            };
            Pair {
                video: vi,
                audio: best.map(|b| b.0),
                offset: best.map(|b| b.2).unwrap_or(0.0),
                method,
                refined: false,
                confidence: None,
                drift: None,
                drift_frames: None,
                validated: false,
                note: None,
            }
        })
        .collect()
}

/// Décalage fin par la forme d'onde, autour d'un décalage attendu. `at` :
/// position dans la vidéo de l'extrait comparé (secondes).
pub fn refine_at(
    video: &SyncFile,
    audio: &SyncFile,
    expected: f64,
    window: f64,
    at: f64,
    length: f64,
) -> Result<(f64, f64)> {
    let vi = probe(&video.path)?;
    let ai = probe(&audio.path)?;
    let a = decode_mono(&video.path, &vi, at, length, FINE_RATE, video.ltc_channel)?;
    // Extrait du son couvrant la même plage, élargi de la fenêtre de recherche.
    let b_start = (at - expected - window).max(0.0);
    let b = decode_mono(
        &audio.path,
        &ai,
        b_start,
        length + 2.0 * window,
        FINE_RATE,
        audio.ltc_channel,
    )?;
    let rate = FINE_RATE as f64;
    // Décalage attendu en échantillons dans b : at - expected - b_start.
    let center = ((at - expected - b_start) * rate).round() as i64;
    let w = (window * rate) as i64;
    let lag = gcc_phat(&a, &b, center - w, center + w).ok_or_else(|| {
        Error::Unsupported("corrélation impossible (son trop court ou silencieux)".into())
    })?;
    // offset = position vidéo - position son du même instant.
    let offset = at - (b_start + lag.samples as f64 / rate);
    Ok((offset, lag.confidence))
}

/// Recherche sans timecode : le son dont la forme d'onde ressemble le plus au
/// son témoin de la vidéo, et le décalage.
pub fn search(
    video: &SyncFile,
    audios: &[SyncFile],
    cancel: &AtomicBool,
) -> Result<Option<(usize, f64, f64)>> {
    let vi = probe(&video.path)?;
    if vi.audio.is_empty() {
        return Ok(None);
    }
    let length = video.duration.clamp(1.0, 60.0);
    let a = decode_mono(
        &video.path,
        &vi,
        0.0,
        length,
        COARSE_RATE,
        video.ltc_channel,
    )?;
    let mut best: Option<(usize, f64, f64)> = None;
    for (ai, audio) in audios.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let Ok(info) = probe(&audio.path) else {
            continue;
        };
        if info.audio.is_empty() {
            continue;
        }
        let dur = audio.duration.min(1800.0);
        let Ok(b) = decode_mono(&audio.path, &info, 0.0, dur, COARSE_RATE, audio.ltc_channel)
        else {
            continue;
        };
        let rate = COARSE_RATE as f64;
        // Le son peut commencer avant ou après la vidéo.
        let Some(lag) = gcc_phat(&a, &b, -(a.len() as i64) + 64, b.len() as i64 - 64) else {
            continue;
        };
        if best.is_none_or(|(_, _, c)| lag.confidence > c) {
            best = Some((ai, -(lag.samples as f64) / rate, lag.confidence));
        }
    }
    Ok(best.filter(|(_, _, c)| *c >= MIN_CONFIDENCE))
}

/// Analyse un lot : lecture du temps, appariement, affinage, dérive.
pub fn analyze(
    videos: &[PathBuf],
    audios: &[PathBuf],
    opts: Options,
    cancel: &AtomicBool,
    mut on: impl FnMut(Event),
) -> Result<Analysis> {
    let total = videos.len() + audios.len();
    let read =
        |paths: &[PathBuf], base: usize, on: &mut dyn FnMut(Event)| -> Result<Vec<SyncFile>> {
            let mut out = Vec::new();
            for (i, p) in paths.iter().enumerate() {
                if cancel.load(Ordering::Relaxed) {
                    return Err(Error::Cancelled);
                }
                on(Event::Reading {
                    index: base + i,
                    total,
                    name: p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string(),
                });
                out.push(describe(p));
            }
            Ok(out)
        };
    let v = read(videos, 0, &mut on)?;
    let a = read(audios, videos.len(), &mut on)?;
    let mut pairs = match_by_time(&v, &a);
    if opts.mode == Mode::Waveform {
        // L'heure des fichiers est ignorée : chaque plan est cherché partout.
        for p in &mut pairs {
            p.audio = None;
            p.offset = 0.0;
        }
    }
    let search_all =
        opts.mode == Mode::Waveform || (opts.mode == Mode::Auto && opts.waveform_search);
    for (i, pair) in pairs.iter_mut().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let video = &v[pair.video];
        on(Event::Matching {
            index: i,
            total: v.len(),
            name: video.name.clone(),
        });
        let has_sound = video.channels > 0;
        if pair.audio.is_none() && search_all && has_sound {
            match search(video, &a, cancel) {
                Ok(Some((ai, offset, conf))) => {
                    pair.audio = Some(ai);
                    pair.offset = offset;
                    pair.method = Method::Waveform;
                    pair.refined = true;
                    pair.confidence = Some(conf);
                }
                Ok(None) => pair.note = Some("aucun son correspondant".into()),
                Err(Error::Cancelled) => return Err(Error::Cancelled),
                Err(e) => pair.note = Some(e.to_string()),
            }
        }
        let Some(ai) = pair.audio else {
            if pair.note.is_none() {
                pair.note = Some(if opts.mode == Mode::Waveform {
                    "pas de son témoin".into()
                } else if video.start.is_none() {
                    "pas de timecode ni de son témoin".into()
                } else {
                    "aucun son à la même heure".into()
                });
            }
            continue;
        };
        let audio = &a[ai];
        if audio.channels == 0 || !has_sound {
            continue;
        }
        // Zone commune aux deux fichiers, dans le temps de la vidéo.
        let from = pair.offset.max(0.0);
        let to = (pair.offset + audio.duration).min(video.duration);
        let common = to - from;
        if common < 1.0 {
            continue;
        }
        // Une paire trouvée par la forme d'onde (recherche grossière) est
        // toujours affinée ; une paire calée par timecode, sur demande.
        let refine = if pair.method == Method::Waveform {
            common > 2.0
        } else {
            opts.mode == Mode::Auto && opts.refine
        };
        if refine {
            let length = common.min(10.0);
            let at = from + (common - length) / 2.0;
            let window = if pair.method == Method::Waveform {
                0.05
            } else {
                opts.window
            };
            match refine_at(video, audio, pair.offset, window, at, length) {
                Ok((offset, conf)) if conf >= MIN_CONFIDENCE => {
                    let moved = offset - pair.offset;
                    pair.offset = offset;
                    pair.refined = true;
                    pair.confidence = Some(conf);
                    if moved.abs() > 1.0 / video.rate.map(|r| r.as_f64()).unwrap_or(25.0)
                        && pair.method != Method::Waveform
                    {
                        pair.note = Some(format!(
                            "timecode corrigé de {} s par la forme d'onde",
                            format!("{moved:+.3}").replace('.', ",")
                        ));
                    }
                }
                Ok((_, conf)) => {
                    pair.confidence = Some(conf);
                    pair.note = Some(
                        "forme d'onde peu ressemblante : décalage du timecode conservé".into(),
                    );
                }
                Err(e) => pair.note = Some(e.to_string()),
            }
        }
        // Dérive : décalage mesuré au début et à la fin de la zone commune.
        // En « timecode seul », le décalage n'est pas affiné : la mesure, qui
        // compare le début et la fin, cherche alors dans toute la tolérance.
        let measured = pair.refined || opts.mode == Mode::Timecode;
        if opts.drift && measured && common >= 60.0 {
            // Extraits courts : une dérive étale le pic de corrélation.
            let length = 4.0;
            let w = if pair.refined { 0.1 } else { opts.window };
            let start = refine_at(video, audio, pair.offset, w, from + 2.0, length);
            let end = refine_at(video, audio, pair.offset, w, to - length - 2.0, length);
            if let (Ok((o1, c1)), Ok((o2, c2))) = (start, end) {
                if c1 >= MIN_CONFIDENCE && c2 >= MIN_CONFIDENCE {
                    let span = to - from - length - 4.0;
                    // Ramenée à la durée complète de la zone commune.
                    let drift = (o1 - o2) * common / span.max(1.0);
                    pair.drift = Some(drift);
                    let fps = video.rate.map(|r| r.as_f64()).unwrap_or(25.0);
                    pair.drift_frames = Some(drift * fps);
                }
            }
        }
    }
    Ok(Analysis {
        videos: v,
        audios: a,
        pairs,
    })
}

#[cfg(test)]
mod tests;
