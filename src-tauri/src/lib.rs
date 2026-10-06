//! Pont Tauri entre l'interface (`ui/`) et le cœur (`core/`).
//! Ce fichier ne contient aucune logique métier : il traduit les appels.

mod commands;
mod media;
mod offload;
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
        .manage(media::MediaState::default())
        .setup(|app| {
            use tauri::Manager;
            app.manage(offload::OffloadState::new(app.handle().clone()));
            Ok(())
        })
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
            offload::offload_volumes,
            offload::offload_preflight,
            offload::offload_start,
            offload::offload_cancel,
            offload::offload_eject,
            offload::reveal,
            media::media_fields,
            media::media_list,
            media::media_describe,
            media::media_thumbnail,
            media::media_filmstrip,
            media::media_waveform,
            media::media_set,
            media::media_export,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de VERIFLOW");
}
