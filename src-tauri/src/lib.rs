//! Pont Tauri entre l'interface (`ui/`) et le cœur (`core/`).
//! Ce fichier ne contient aucune logique métier : il traduit les appels.

mod commands;
mod player;

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
        .manage(player::PlayerState::default())
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::project_create,
            commands::project_open,
            commands::project_close,
            player::video_open,
            player::video_frame,
            player::video_close,
            player::audio_open,
            player::audio_close,
            player::audio_transport,
            player::audio_seek,
            player::audio_track,
            player::audio_status,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de VERIFLOW");
}
