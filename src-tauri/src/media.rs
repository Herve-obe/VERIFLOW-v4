//! Commandes MEDIA : catalogue, descriptions, aperçus, métadonnées, exports.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::ipc::Response;
use tauri::{AppHandle, Manager, State};
use tokio::sync::Semaphore;
use veriflow_core::media::catalog::{describe, list, MediaDetails, MediaEntry};
use veriflow_core::media::export::{
    merged, to_ale, to_csv, working_copies, write_xmp_sidecars, MediaRecord,
};
use veriflow_core::media::fields::{FieldDef, FIELDS};
use veriflow_core::media::preview::{filmstrip, thumbnail, waveform, Waveform};

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Limite le nombre de processus FFmpeg lancés en parallèle pour les aperçus.
pub struct MediaState {
    previews: Arc<Semaphore>,
}

impl Default for MediaState {
    fn default() -> Self {
        let n = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .clamp(2, 6);
        Self {
            previews: Arc::new(Semaphore::new(n)),
        }
    }
}

fn cache_dir(app: &AppHandle) -> PathBuf {
    app.path()
        .app_cache_dir()
        .unwrap_or_else(|_| std::env::temp_dir().join("veriflow"))
        .join("previews")
}

fn edits_of(app: &AppHandle, path: &Path) -> BTreeMap<String, String> {
    let state = app.state::<AppState>();
    let Ok(guard) = state.project.lock() else {
        return BTreeMap::new();
    };
    guard
        .as_ref()
        .and_then(|p| p.media_meta(&path.display().to_string()).ok())
        .unwrap_or_default()
}

#[tauri::command]
pub fn media_fields() -> &'static [FieldDef] {
    FIELDS
}

#[tauri::command]
pub async fn media_list(dir: PathBuf, recursive: bool) -> CmdResult<Vec<MediaEntry>> {
    tauri::async_runtime::spawn_blocking(move || list(&dir, recursive))
        .await
        .map_err(text)?
        .map_err(text)
}

#[derive(Serialize)]
pub struct Described {
    path: PathBuf,
    details: MediaDetails,
    /// Métadonnées effectives (fichier + éditions du projet).
    values: BTreeMap<String, String>,
    /// Champs modifiés dans le projet.
    edited: Vec<String>,
}

/// Décrit plusieurs médias en parallèle (analyse FFprobe, BWF, iXML).
#[tauri::command]
pub async fn media_describe(paths: Vec<PathBuf>, app: AppHandle) -> CmdResult<Vec<Described>> {
    let details: Vec<(PathBuf, MediaDetails)> = tauri::async_runtime::spawn_blocking(move || {
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
            .min(8);
        let chunk = paths.len().div_ceil(workers).max(1);
        std::thread::scope(|s| {
            let handles: Vec<_> = paths
                .chunks(chunk)
                .map(|c| {
                    s.spawn(move || {
                        c.iter()
                            .map(|p| (p.clone(), describe(p)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles
                .into_iter()
                .flat_map(|h| h.join().unwrap_or_default())
                .collect()
        })
    })
    .await
    .map_err(text)?;
    Ok(details
        .into_iter()
        .map(|(path, details)| {
            let edits = edits_of(&app, &path);
            Described {
                values: merged(&details.embedded, &edits),
                edited: edits.keys().cloned().collect(),
                path,
                details,
            }
        })
        .collect())
}

async fn with_permit<T: Send + 'static>(
    state: &MediaState,
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    let _permit = state.previews.clone().acquire_owned().await.map_err(text)?;
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(text)?
}

#[tauri::command]
pub async fn media_thumbnail(
    path: PathBuf,
    app: AppHandle,
    state: State<'_, MediaState>,
) -> CmdResult<Response> {
    let cache = cache_dir(&app);
    with_permit(&state, move || {
        let f = thumbnail(&path, &cache).map_err(text)?;
        std::fs::read(f).map(Response::new).map_err(text)
    })
    .await
}

/// Image `index` de la bande d'aperçu (les 8 images sont calculées au premier appel).
#[tauri::command]
pub async fn media_filmstrip(
    path: PathBuf,
    index: usize,
    app: AppHandle,
    state: State<'_, MediaState>,
) -> CmdResult<Response> {
    let cache = cache_dir(&app);
    with_permit(&state, move || {
        let frames = filmstrip(&path, &cache).map_err(text)?;
        let f = frames.get(index).ok_or("image hors limites")?;
        std::fs::read(f).map(Response::new).map_err(text)
    })
    .await
}

#[tauri::command]
pub async fn media_waveform(
    path: PathBuf,
    app: AppHandle,
    state: State<'_, MediaState>,
) -> CmdResult<Waveform> {
    let cache = cache_dir(&app);
    with_permit(&state, move || waveform(&path, &cache).map_err(text)).await
}

/// Enregistre des champs sur un ou plusieurs médias (projet ouvert requis).
#[tauri::command]
pub fn media_set(
    paths: Vec<PathBuf>,
    values: BTreeMap<String, String>,
    app: AppHandle,
) -> CmdResult<usize> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    let project = guard
        .as_ref()
        .ok_or("Ouvre ou crée un projet pour enregistrer les métadonnées : elles ne sont jamais écrites dans les originaux.")?;
    let paths: Vec<String> = paths.iter().map(|p| p.display().to_string()).collect();
    project.set_media_meta(&paths, &values).map_err(text)
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Csv,
    Ale,
    Xmp,
    WorkingCopy,
}

#[derive(Serialize)]
pub struct ExportResult {
    written: Vec<PathBuf>,
    errors: Vec<String>,
}

/// Exporte les métadonnées des médias choisis. `out` : fichier (CSV, ALE)
/// ou dossier (XMP, copies de travail) ; `base` : dossier de référence.
#[tauri::command]
pub async fn media_export(
    kind: ExportKind,
    paths: Vec<PathBuf>,
    base: PathBuf,
    out: PathBuf,
    app: AppHandle,
) -> CmdResult<ExportResult> {
    let edits: Vec<BTreeMap<String, String>> = paths.iter().map(|p| edits_of(&app, p)).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let records: Vec<MediaRecord> = paths
            .iter()
            .zip(edits)
            .map(|(p, e)| {
                let details = describe(p);
                MediaRecord {
                    path: p.clone(),
                    values: merged(&details.embedded, &e),
                    details,
                }
            })
            .collect();
        let mut result = ExportResult {
            written: Vec::new(),
            errors: Vec::new(),
        };
        match kind {
            ExportKind::Csv => {
                std::fs::write(&out, to_csv(&records)).map_err(text)?;
                result.written.push(out);
            }
            ExportKind::Ale => {
                std::fs::write(&out, to_ale(&records)).map_err(text)?;
                result.written.push(out);
            }
            ExportKind::Xmp => {
                result.written = write_xmp_sidecars(&records, &base, &out).map_err(text)?
            }
            ExportKind::WorkingCopy => {
                for (src, r) in working_copies(&records, &base, &out) {
                    match r {
                        Ok(p) => result.written.push(p),
                        Err(e) => result.errors.push(format!("{} : {e}", src.display())),
                    }
                }
            }
        }
        Ok(result)
    })
    .await
    .map_err(text)?
}
