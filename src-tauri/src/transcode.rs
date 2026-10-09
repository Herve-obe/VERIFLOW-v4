//! Commandes TRANSCODE : préréglages, encodeurs du poste, file d'attente de
//! conversions avec progression en direct (événements) et annulation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use veriflow_core::media::catalog::{self, MediaKind};
use veriflow_core::transcode::{
    self, execute, AudioMode, BitDepth, Category, Domain, EncoderChoice, Event, Request, Summary,
    PRESETS,
};

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct Job {
    id: u64,
    request: Request,
    cancel: Arc<AtomicBool>,
}

pub struct TranscodeState {
    queue: Mutex<Sender<Job>>,
    cancels: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    next_id: AtomicU64,
}

/// Événement envoyé à l'interface (canal « transcode »).
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Notice {
    Queued { job: u64, files: usize },
    Running { job: u64 },
    Engine { job: u64, event: Event },
    Done { job: u64, summary: Summary },
    Failed { job: u64, error: String },
}

impl TranscodeState {
    /// Démarre le fil de traitement : les lots sont convertis l'un après l'autre.
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = channel::<Job>();
        let cancels: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>> = Arc::default();
        let worker_cancels = cancels.clone();
        thread::Builder::new()
            .name("veriflow-transcode".into())
            .spawn(move || {
                for job in rx {
                    let id = job.id;
                    let send = |n: Notice| {
                        let _ = app.emit("transcode", n);
                    };
                    if job.cancel.load(Ordering::Relaxed) {
                        send(Notice::Failed {
                            job: id,
                            error: "annulé avant le début".into(),
                        });
                    } else {
                        send(Notice::Running { job: id });
                        match execute(&job.request, &job.cancel, |event| {
                            send(Notice::Engine { job: id, event })
                        }) {
                            Ok(summary) => send(Notice::Done { job: id, summary }),
                            Err(e) => send(Notice::Failed {
                                job: id,
                                error: e.to_string(),
                            }),
                        }
                    }
                    if let Ok(mut c) = worker_cancels.lock() {
                        c.remove(&id);
                    }
                }
            })
            .expect("fil de conversion");
        Self {
            queue: Mutex::new(tx),
            cancels,
            next_id: AtomicU64::new(1),
        }
    }
}

/// Préréglage tel que présenté dans l'interface.
#[derive(Serialize)]
pub struct PresetView {
    id: &'static str,
    category: Category,
    domain: Domain,
    label: &'static str,
    /// Famille (« video », « audio », « image », « conform »...) : réglages à afficher.
    kind: &'static str,
    ext: &'static str,
    scale: &'static str,
    suffix: &'static str,
    /// Image réencodée : réglages d'image, incrustations, son des vidéos.
    video: bool,
    /// Taille d'image libre (les formats broadcast imposent la leur).
    scale_free: bool,
    /// Fichier son produit (fréquence, résolution, normalisation).
    audio: bool,
    video_bitrate: bool,
    encoder_choice: bool,
    bit_depths: Vec<BitDepth>,
    audio_bitrates: Vec<u32>,
    default_audio_bitrate: u32,
    audio_mode: AudioMode,
    analysis: bool,
    /// Raison pour laquelle le préréglage est indisponible sur ce poste.
    unavailable: Option<String>,
}

#[tauri::command]
pub async fn transcode_presets(lang: String) -> CmdResult<Vec<PresetView>> {
    let fr = lang != "en";
    // Disponibilité : interroge FFmpeg (liste des encodeurs et filtres).
    tauri::async_runtime::spawn_blocking(move || {
        PRESETS
            .iter()
            .map(|p| {
                let (rates, default_rate) = p.audio_bitrates();
                PresetView {
                    id: p.id,
                    category: p.category,
                    domain: p.domain,
                    label: if fr { p.label_fr } else { p.label_en },
                    kind: p.kind.id(),
                    ext: p.ext,
                    scale: p.scale,
                    suffix: p.suffix,
                    video: p.is_video() || p.is_image(),
                    scale_free: p.has_scale(),
                    audio: p.is_audio(),
                    video_bitrate: p.has_video_bitrate(),
                    encoder_choice: p.has_encoder_choice(),
                    bit_depths: p.bit_depths().to_vec(),
                    audio_bitrates: rates.to_vec(),
                    default_audio_bitrate: default_rate,
                    audio_mode: p.audio_mode,
                    analysis: p.category == Category::Analysis,
                    unavailable: p.unavailable(),
                }
            })
            .collect()
    })
    .await
    .map_err(text)
}

/// Encodeur qui sera utilisé pour ce préréglage sur ce poste (essai compris,
/// d'où l'appel asynchrone).
#[tauri::command]
pub async fn transcode_encoder(preset: String, software: bool) -> CmdResult<Option<EncoderChoice>> {
    tauri::async_runtime::spawn_blocking(move || {
        transcode::find(&preset).and_then(|p| transcode::video::video_encoder(p, software))
    })
    .await
    .map_err(text)
}

/// Préréglage personnel : enregistré dans un fichier choisi (partage).
#[tauri::command]
pub fn transcode_preset_write(path: PathBuf, content: String) -> CmdResult<()> {
    std::fs::write(path, content).map_err(text)
}

#[tauri::command]
pub fn transcode_preset_read(path: PathBuf) -> CmdResult<String> {
    std::fs::read_to_string(path).map_err(text)
}

/// Fichiers vidéo et son contenus dans les chemins donnés (dossiers parcourus).
#[tauri::command]
pub async fn transcode_expand(paths: Vec<PathBuf>) -> CmdResult<Vec<PathBuf>> {
    tauri::async_runtime::spawn_blocking(move || {
        let wanted = |k: Option<MediaKind>| matches!(k, Some(MediaKind::Video | MediaKind::Audio));
        let mut out = Vec::new();
        for p in paths {
            if p.is_dir() {
                let list = catalog::list(&p, true).map_err(text)?;
                out.extend(
                    list.into_iter()
                        .filter(|e| wanted(Some(e.kind)))
                        .map(|e| e.path),
                );
            } else if wanted(catalog::kind_of(&p)) {
                out.push(p);
            }
        }
        Ok(out)
    })
    .await
    .map_err(text)?
}

/// Ajoute un lot à la file d'attente ; renvoie son numéro.
#[tauri::command]
pub fn transcode_start(
    request: Request,
    app: AppHandle,
    state: State<'_, TranscodeState>,
) -> CmdResult<u64> {
    if request.sources.is_empty() {
        return Err("aucun fichier à convertir".into());
    }
    if transcode::find(&request.settings.preset).is_none() {
        return Err(format!("préréglage inconnu : {}", request.settings.preset));
    }
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancels
        .lock()
        .map_err(text)?
        .insert(id, cancel.clone());
    let files = request.sources.len();
    // Annoncé avant l'envoi : le fil peut commencer aussitôt.
    let _ = app.emit("transcode", Notice::Queued { job: id, files });
    state
        .queue
        .lock()
        .map_err(text)?
        .send(Job {
            id,
            request,
            cancel,
        })
        .map_err(text)?;
    Ok(id)
}

#[tauri::command]
pub fn transcode_cancel(job: u64, state: State<'_, TranscodeState>) -> CmdResult<()> {
    if let Some(c) = state.cancels.lock().map_err(text)?.get(&job) {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}
