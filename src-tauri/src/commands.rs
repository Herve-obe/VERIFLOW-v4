//! Commandes appelables depuis l'interface (voir `ui/src/lib/api.ts`).

use std::path::PathBuf;

use serde::Serialize;
use tauri::State;
use veriflow_core::project::{Project, ProjectInfo};

use crate::AppState;

#[derive(Serialize)]
pub struct AppInfo {
    name: &'static str,
    version: &'static str,
    os: &'static str,
    arch: &'static str,
}

/// Les erreurs sont transmises à l'interface sous forme de texte lisible.
type CmdResult<T> = Result<T, String>;

fn to_string<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "VERIFLOW",
        version: veriflow_core::VERSION,
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
    }
}

#[tauri::command]
pub fn project_create(path: PathBuf, state: State<'_, AppState>) -> CmdResult<ProjectInfo> {
    let project = Project::create(&path, None).map_err(to_string)?;
    let info = project.info().map_err(to_string)?;
    *state.project.lock().map_err(to_string)? = Some(project);
    Ok(info)
}

#[tauri::command]
pub fn project_open(path: PathBuf, state: State<'_, AppState>) -> CmdResult<ProjectInfo> {
    let project = Project::open(&path).map_err(to_string)?;
    let info = project.info().map_err(to_string)?;
    *state.project.lock().map_err(to_string)? = Some(project);
    Ok(info)
}

#[tauri::command]
pub fn project_save(state: State<'_, AppState>) -> CmdResult<()> {
    let guard = state.project.lock().map_err(to_string)?;
    guard
        .as_ref()
        .ok_or_else(|| "aucun projet ouvert".to_owned())?
        .save()
        .map_err(to_string)
}

/// Copie le projet ouvert sous un autre nom ; la copie devient le projet ouvert.
#[tauri::command]
pub fn project_save_as(path: PathBuf, state: State<'_, AppState>) -> CmdResult<ProjectInfo> {
    let mut guard = state.project.lock().map_err(to_string)?;
    let current = guard
        .as_ref()
        .ok_or_else(|| "aucun projet ouvert".to_owned())?;
    let copy = current.save_as(&path).map_err(to_string)?;
    let info = copy.info().map_err(to_string)?;
    *guard = Some(copy);
    Ok(info)
}

#[tauri::command]
pub fn project_close(state: State<'_, AppState>) -> CmdResult<()> {
    *state.project.lock().map_err(to_string)? = None;
    Ok(())
}
