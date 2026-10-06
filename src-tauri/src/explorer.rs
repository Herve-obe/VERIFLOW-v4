//! Explorateur de dossiers en direct : surveillance des dossiers affichés
//! (événements du système), des volumes branchés ou retirés, et relecture
//! périodique des disques réseau où la surveillance du système est peu fiable.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use veriflow_core::explorer::{list_dirs, DirEntry};
use veriflow_core::offload::storage::{self, Volume};

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Systèmes de fichiers réseau : relecture toutes les 5 s au lieu des événements.
const NETWORK_FS: &[&str] = &[
    "nfs",
    "nfs4",
    "smbfs",
    "cifs",
    "smb2",
    "smb3",
    "afpfs",
    "webdav",
    "fuse.sshfs",
    "9p",
];

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Notice {
    /// Contenu modifié dans ces dossiers (ou ces dossiers eux-mêmes).
    Changed {
        paths: Vec<PathBuf>,
    },
    Volumes {
        volumes: Vec<Volume>,
    },
}

struct Inner {
    watcher: Option<RecommendedWatcher>,
    /// Dossiers demandés par chaque vue (« offload », « media-tree », « media-list »...).
    requests: HashMap<String, (Vec<PathBuf>, bool)>,
    /// Dossiers actuellement surveillés et leur mode.
    active: HashMap<PathBuf, bool>,
}

pub struct ExplorerState {
    inner: Arc<Mutex<Inner>>,
}

fn is_network(path: &Path) -> bool {
    storage::volume_of(path)
        .is_some_and(|v| NETWORK_FS.contains(&v.file_system.to_lowercase().as_str()))
}

/// Signature d'un dossier pour la relecture périodique (sous-dossiers et date).
fn signature(path: &Path) -> String {
    let mtime = std::fs::metadata(path).and_then(|m| m.modified()).ok();
    let names: Vec<String> = std::fs::read_dir(path)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .collect();
    format!("{mtime:?}|{}", names.join("/"))
}

impl ExplorerState {
    pub fn new(app: AppHandle) -> Self {
        let pending: Arc<Mutex<BTreeSet<PathBuf>>> = Arc::default();
        let sink = pending.clone();
        let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res {
                let mut p = sink.lock().expect("verrou");
                for path in event.paths {
                    if let Some(parent) = path.parent() {
                        p.insert(parent.to_path_buf());
                    }
                    p.insert(path);
                }
            }
        })
        .ok();
        let inner = Arc::new(Mutex::new(Inner {
            watcher,
            requests: HashMap::new(),
            active: HashMap::new(),
        }));

        // Regroupement des événements (300 ms) pour ne pas inonder l'interface.
        let emit_app = app.clone();
        thread::Builder::new()
            .name("veriflow-explorer-events".into())
            .spawn(move || loop {
                thread::sleep(Duration::from_millis(300));
                let paths: Vec<PathBuf> = std::mem::take(&mut *pending.lock().expect("verrou"))
                    .into_iter()
                    .collect();
                if !paths.is_empty() {
                    let _ = emit_app.emit("explorer", Notice::Changed { paths });
                }
            })
            .expect("fil d'événements");

        // Volumes branchés ou retirés (2 s) et disques réseau (5 s).
        let poll_inner = inner.clone();
        thread::Builder::new()
            .name("veriflow-explorer-poll".into())
            .spawn(move || {
                let mut last_volumes: Vec<(PathBuf, String)> = Vec::new();
                let mut signatures: HashMap<PathBuf, String> = HashMap::new();
                let mut tick = 0u32;
                loop {
                    let volumes = storage::volumes();
                    let key: Vec<(PathBuf, String)> = volumes
                        .iter()
                        .map(|v| (v.mount_point.clone(), v.name.clone()))
                        .collect();
                    if key != last_volumes {
                        last_volumes = key;
                        let _ = app.emit("explorer", Notice::Volumes { volumes });
                    }
                    if tick.is_multiple_of(5) {
                        let watched: Vec<PathBuf> = poll_inner
                            .lock()
                            .map(|i| i.active.keys().cloned().collect())
                            .unwrap_or_default();
                        let mut changed = Vec::new();
                        for path in watched.into_iter().filter(|p| is_network(p)) {
                            let sig = signature(&path);
                            if signatures.get(&path).is_some_and(|old| *old != sig) {
                                changed.push(path.clone());
                            }
                            signatures.insert(path, sig);
                        }
                        if !changed.is_empty() {
                            let _ = app.emit("explorer", Notice::Changed { paths: changed });
                        }
                    }
                    tick = tick.wrapping_add(1);
                    thread::sleep(Duration::from_secs(1));
                }
            })
            .expect("fil de surveillance des volumes");

        Self { inner }
    }
}

/// Met à jour l'ensemble des dossiers surveillés (union des demandes de toutes les vues).
fn apply(inner: &mut Inner) {
    let mut wanted: HashMap<PathBuf, bool> = HashMap::new();
    for (paths, recursive) in inner.requests.values() {
        for p in paths {
            let r = wanted.entry(p.clone()).or_insert(false);
            *r |= *recursive;
        }
    }
    let Some(watcher) = inner.watcher.as_mut() else {
        return;
    };
    let stale: Vec<PathBuf> = inner
        .active
        .iter()
        .filter(|(p, r)| wanted.get(*p) != Some(r))
        .map(|(p, _)| p.clone())
        .collect();
    for p in stale {
        let _ = watcher.unwatch(&p);
        inner.active.remove(&p);
    }
    for (p, recursive) in wanted {
        if inner.active.contains_key(&p) || !p.is_dir() {
            continue;
        }
        let mode = if recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };
        if watcher.watch(&p, mode).is_ok() {
            inner.active.insert(p, recursive);
        }
    }
}

#[tauri::command]
pub async fn explorer_list(path: PathBuf) -> CmdResult<Vec<DirEntry>> {
    tauri::async_runtime::spawn_blocking(move || list_dirs(&path))
        .await
        .map_err(text)?
        .map_err(text)
}

#[tauri::command]
pub fn explorer_volumes() -> Vec<Volume> {
    storage::volumes()
}

/// Dossiers de destination des offloads du projet ouvert (existants seulement).
#[tauri::command]
pub fn explorer_project_roots(app: AppHandle) -> Vec<PathBuf> {
    let state = app.state::<AppState>();
    let Ok(guard) = state.project.lock() else {
        return Vec::new();
    };
    let roots = guard
        .as_ref()
        .and_then(|p| p.offload_roots().ok())
        .unwrap_or_default();
    let mut seen = HashSet::new();
    roots
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_dir() && seen.insert(p.clone()))
        .collect()
}

/// Déclare les dossiers qu'une vue souhaite suivre (remplace sa demande précédente).
#[tauri::command]
pub fn explorer_watch(
    owner: String,
    paths: Vec<PathBuf>,
    recursive: bool,
    state: tauri::State<'_, ExplorerState>,
) -> CmdResult<()> {
    let mut inner = state.inner.lock().map_err(text)?;
    if paths.is_empty() {
        inner.requests.remove(&owner);
    } else {
        inner.requests.insert(owner, (paths, recursive));
    }
    apply(&mut inner);
    Ok(())
}
