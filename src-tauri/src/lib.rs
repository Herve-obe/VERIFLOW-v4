//! Pont Tauri entre l'interface (`ui/`) et le cœur (`core/`).
//! Ce fichier ne contient aucune logique métier : il traduit les appels.

mod commands;

use std::sync::Mutex;

use veriflow_core::project::Project;

/// État partagé de l'application : le projet ouvert, s'il y en a un.
#[derive(Default)]
pub struct AppState {
    pub project: Mutex<Option<Project>>,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::project_create,
            commands::project_open,
            commands::project_close,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de VERIFLOW");
}
