//! Commandes OFFLOAD : contrôles, file d'attente de copies, progression en
//! direct (événements), annulation, éjection.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use veriflow_core::offload::engine::Event;
use veriflow_core::offload::job::{execute, preflight, JobResult, OffloadRequest, Preflight};
use veriflow_core::offload::storage::{self, Volume};
use veriflow_core::project::PreviousOffload;

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct Job {
    id: u64,
    request: OffloadRequest,
    eject: bool,
    cancel: Arc<AtomicBool>,
}

pub struct OffloadState {
    queue: Mutex<Sender<Job>>,
    cancels: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>,
    next_id: AtomicU64,
}

/// Événement envoyé à l'interface (canal « offload »).
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Notice {
    Queued {
        job: u64,
        source: String,
    },
    Running {
        job: u64,
        files: usize,
        total_bytes: u64,
        roots: Vec<PathBuf>,
    },
    Engine {
        job: u64,
        event: Event,
    },
    Done {
        job: u64,
        result: JobResult,
        ejected: Option<Result<(), String>>,
    },
    Failed {
        job: u64,
        error: String,
    },
}

impl OffloadState {
    /// Démarre le fil de traitement : les cartes sont copiées l'une après l'autre.
    pub fn new(app: AppHandle) -> Self {
        let (tx, rx) = channel::<Job>();
        let cancels: Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>> = Arc::default();
        let worker_cancels = cancels.clone();
        thread::Builder::new()
            .name("veriflow-offload".into())
            .spawn(move || {
                for job in rx {
                    run_job(&app, &job);
                    if let Ok(mut c) = worker_cancels.lock() {
                        c.remove(&job.id);
                    }
                }
            })
            .expect("fil d'offload");
        Self {
            queue: Mutex::new(tx),
            cancels,
            next_id: AtomicU64::new(1),
        }
    }
}

fn project_name(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().ok()?;
    guard.as_ref().and_then(|p| p.info().ok()).map(|i| i.name)
}

fn run_job(app: &AppHandle, job: &Job) {
    let send = |n: Notice| {
        let _ = app.emit("offload", n);
    };
    let pre = match preflight(&job.request) {
        Ok(p) => p,
        Err(e) => {
            return send(Notice::Failed {
                job: job.id,
                error: e.to_string(),
            })
        }
    };
    send(Notice::Running {
        job: job.id,
        files: pre.files,
        total_bytes: pre.total_bytes,
        roots: pre.roots.clone(),
    });
    let project = project_name(app);
    let result = execute(
        &job.request,
        pre,
        &job.cancel,
        |event| send(Notice::Engine { job: job.id, event }),
        project.as_deref(),
    );
    match result {
        Ok(result) => {
            // Historique dans le projet ouvert (détection des cartes déjà copiées).
            if let Ok(guard) = app.state::<AppState>().project.lock() {
                if let Some(p) = guard.as_ref() {
                    let _ = p.record_offload(&result, &job.request.source);
                }
            }
            let success = !result.summary.cancelled && result.summary.failed_files == 0;
            let ejected = (job.eject && success).then(|| {
                storage::volume_of(&job.request.source)
                    .filter(|v| v.removable)
                    .ok_or_else(|| "source non amovible : pas d'éjection".to_owned())
                    .and_then(|v| storage::eject(&v.mount_point).map_err(text))
            });
            send(Notice::Done {
                job: job.id,
                result,
                ejected,
            });
        }
        Err(e) => send(Notice::Failed {
            job: job.id,
            error: e.to_string(),
        }),
    }
}

#[tauri::command]
pub fn offload_volumes() -> Vec<Volume> {
    storage::volumes()
}

#[derive(Serialize)]
pub struct PreflightView {
    #[serde(flatten)]
    pre: Preflight,
    /// Destinations sur disque dur mécanique (copie plus lente).
    hdd: Vec<bool>,
    source_hdd: bool,
    /// Copies déjà enregistrées dans le projet pour cette carte.
    previous: Vec<PreviousOffload>,
}

#[tauri::command]
pub async fn offload_preflight(
    request: OffloadRequest,
    app: AppHandle,
) -> CmdResult<PreflightView> {
    let pre =
        tauri::async_runtime::spawn_blocking(move || preflight(&request).map(|p| (p, request)))
            .await
            .map_err(text)?
            .map_err(text)?;
    let (pre, request) = pre;
    let previous = {
        let state = app.state::<AppState>();
        let guard = state.project.lock().map_err(text)?;
        guard
            .as_ref()
            .and_then(|p| p.previous_offloads(&pre.fingerprint).ok())
            .unwrap_or_default()
    };
    Ok(PreflightView {
        hdd: request
            .destinations
            .iter()
            .map(|d| storage::is_hdd(d))
            .collect(),
        source_hdd: storage::is_hdd(&request.source),
        previous,
        pre,
    })
}

/// Ajoute une copie à la file d'attente ; renvoie son numéro.
#[tauri::command]
pub fn offload_start(
    request: OffloadRequest,
    eject: bool,
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<u64> {
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .cancels
        .lock()
        .map_err(text)?
        .insert(id, cancel.clone());
    let source = request.source.display().to_string();
    state
        .queue
        .lock()
        .map_err(text)?
        .send(Job {
            id,
            request,
            eject,
            cancel,
        })
        .map_err(text)?;
    let _ = app.emit("offload", Notice::Queued { job: id, source });
    Ok(id)
}

#[tauri::command]
pub fn offload_cancel(job: u64, state: State<'_, OffloadState>) -> CmdResult<()> {
    if let Some(c) = state.cancels.lock().map_err(text)?.get(&job) {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn offload_eject(mount_point: PathBuf) -> CmdResult<()> {
    storage::eject(&mount_point).map_err(text)
}

/// Ouvre un fichier (rapport) ou un dossier avec l'application du système.
#[tauri::command]
pub fn reveal(path: PathBuf) -> CmdResult<()> {
    // Chemin absolu aux séparateurs natifs : l'Explorateur Windows ouvre
    // « Documents » sans message d'erreur s'il ne comprend pas le chemin.
    let path = veriflow_core::absolute_path(&path);
    if !path.exists() {
        return Err(format!("{} : introuvable", path.display()));
    }
    open_with_system(&path).map_err(text)
}

fn open_with_system(path: &Path) -> std::io::Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c.spawn()?;
        return Ok(());
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(path).spawn().map(|_| ())
}
