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
use veriflow_core::transcode::preset::{default_kbps, BitDepth};
use veriflow_core::transcode::{
    self, execute, Category, Domain, EncoderChoice, Event, Request, Summary, PRESETS,
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
    ext: &'static str,
    scale: &'static str,
    suffix: &'static str,
    /// Le préréglage produit une image (taille réglable).
    video: bool,
    /// Le préréglage produit un fichier son (fréquence, normalisation).
    audio: bool,
    video_bitrate: bool,
    encoder_choice: bool,
    bit_depths: Vec<BitDepth>,
    audio_bitrates: Vec<u32>,
    default_audio_bitrate: u32,
    analysis: bool,
}

#[tauri::command]
pub fn transcode_presets(lang: String) -> Vec<PresetView> {
    let fr = lang != "en";
    PRESETS
        .iter()
        .map(|p| {
            let (rates, default_rate) = p.audio_bitrates();
            PresetView {
                id: p.id,
                category: p.category,
                domain: p.domain,
                label: if fr { p.label_fr } else { p.label_en },
                ext: p.ext,
                scale: p.scale,
                suffix: p.suffix,
                video: p.is_video(),
                audio: p.is_audio(),
                video_bitrate: p.has_video_bitrate(),
                encoder_choice: p.has_encoder_choice(),
                bit_depths: p.bit_depths().to_vec(),
                audio_bitrates: rates.to_vec(),
                default_audio_bitrate: default_rate,
                analysis: p.category == Category::Analysis,
            }
        })
        .collect()
}

/// Encodeur qui sera utilisé pour ce préréglage sur ce poste (essai compris,
/// d'où l'appel asynchrone).
#[tauri::command]
pub async fn transcode_encoder(preset: String, software: bool) -> CmdResult<Option<EncoderChoice>> {
    tauri::async_runtime::spawn_blocking(move || {
        transcode::find(&preset).and_then(|p| transcode::preset::video_encoder(p, software))
    })
    .await
    .map_err(text)
}

/// Débit vidéo automatique (Mbit/s) pour une hauteur d'image.
#[tauri::command]
pub fn transcode_default_mbps(height: u32, hevc: bool) -> f64 {
    default_kbps(height, hevc) as f64 / 1000.0
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
