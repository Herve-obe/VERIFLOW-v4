//! Commandes des logs du PLAYER : marqueurs enregistrés dans le projet et
//! exports EDL, ALE, CSV, FCPXML, OTIO.

use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use veriflow_core::player::logs::{LogClip, LogFormat, Marker};

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

const NO_PROJECT: &str =
    "Ouvre ou crée un projet pour enregistrer les marqueurs (ils ne sont jamais écrits dans les originaux).";

fn with_project<T>(
    app: &AppHandle,
    f: impl FnOnce(&veriflow_core::project::Project) -> veriflow_core::Result<T>,
) -> CmdResult<T> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    let project = guard.as_ref().ok_or(NO_PROJECT)?;
    f(project).map_err(text)
}

/// Marqueurs d'un média, ou de tout le projet si `path` est absent.
/// Sans projet ouvert : liste vide.
#[tauri::command]
pub fn markers_list(path: Option<String>, app: AppHandle) -> CmdResult<Vec<Marker>> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    match guard.as_ref() {
        Some(p) => p.markers(path.as_deref()).map_err(text),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn marker_save(marker: Marker, app: AppHandle) -> CmdResult<Marker> {
    with_project(&app, |p| p.save_marker(&marker))
}

#[tauri::command]
pub fn marker_delete(id: i64, app: AppHandle) -> CmdResult<()> {
    with_project(&app, |p| p.delete_marker(id))
}

/// Exporte les logs des médias `paths` (tous les médias marqués si vide)
/// dans `dest`. Renvoie le chemin écrit.
#[tauri::command]
pub async fn logs_export(
    format: LogFormat,
    paths: Vec<String>,
    dest: PathBuf,
    title: String,
    app: AppHandle,
) -> CmdResult<PathBuf> {
    let markers = with_project(&app, |p| p.markers(None))?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut wanted = paths;
        if wanted.is_empty() {
            for m in &markers {
                if !wanted.contains(&m.path) {
                    wanted.push(m.path.clone());
                }
            }
        }
        if wanted.is_empty() {
            return Err("aucun marqueur à exporter".to_owned());
        }
        let clips = wanted
            .iter()
            .map(|p| {
                let own = markers.iter().filter(|m| &m.path == p).cloned().collect();
                LogClip::from_media(Path::new(p), own).map_err(text)
            })
            .collect::<CmdResult<Vec<_>>>()?;
        let dest = if dest.extension().is_none() {
            dest.with_extension(format.extension())
        } else {
            dest
        };
        std::fs::write(&dest, format.render(&title, &clips)).map_err(text)?;
        Ok(dest)
    })
    .await
    .map_err(text)?
}
