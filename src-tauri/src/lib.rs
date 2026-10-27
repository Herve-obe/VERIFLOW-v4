//! Pont Tauri entre l'interface (`ui/`) et le cœur (`core/`).
//! Ce fichier ne contient aucune logique métier : il traduit les appels.

mod commands;
mod explorer;
mod legal;
mod logs;
#[cfg(target_os = "macos")]
mod macos;
mod media;
mod menu;
mod monitor;
mod offload;
mod player;
mod quick;
mod report;
mod subtitles;
mod sync;
mod transcode;

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
        .plugin(tauri_plugin_clipboard_manager::init())
        // Taille, place et état agrandi de la fenêtre principale retenus d'une
        // ouverture à l'autre (les fenêtres moniteur et lecteur rapide gèrent
        // leur place elles-mêmes).
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_denylist(&[monitor::LABEL, "quick"])
                .build(),
        )
        .manage(AppState::default())
        .manage(player::PlayerState::default())
        .manage(media::MediaState::default())
        .manage(sync::SyncState::default())
        .manage(quick::QuickState::default())
        .setup(|app| {
            use tauri::Manager;
            app.manage(offload::OffloadState::new(app.handle().clone()));
            app.manage(explorer::ExplorerState::new(app.handle().clone()));
            app.manage(transcode::TranscodeState::new(app.handle().clone()));
            menu::listen(app.handle());
            // Première ouverture (aucune place retenue) : fenêtre agrandie à
            // l'écran (demande d'Hervé du 26/10/2026).
            let remembered = app
                .path()
                .app_config_dir()
                .map(|d| d.join(tauri_plugin_window_state::DEFAULT_FILENAME).exists())
                .unwrap_or(false);
            if !remembered {
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.maximize();
                }
            }
            // Windows : barre de titre dessinée par l'interface (menus,
            // projet, boutons de la fenêtre), à la place de celle du système.
            #[cfg(windows)]
            if let Some(main) = app.get_webview_window("main") {
                let _ = main.set_decorations(false);
            }
            // Fermer la fenêtre principale ferme aussi la sortie moniteur et
            // le lecteur rapide.
            if let Some(main) = app.get_webview_window("main") {
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { .. } = event {
                        let _ = monitor::monitor_close(handle.clone());
                        quick::close(&handle);
                    }
                });
            }
            #[cfg(target_os = "macos")]
            if let Some(main) = app.get_webview_window("main") {
                let handle = main.clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::Resized(_) = event {
                        macos::refresh_webview_frame(&handle);
                    }
                });
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::project_create,
            commands::project_open,
            commands::project_close,
            commands::project_save,
            commands::project_save_as,
            menu::menu_set,
            player::video_open,
            player::video_frame,
            player::video_close,
            player::video_lut,
            player::audio_outputs,
            player::audio_ltc_scan,
            player::audio_open,
            player::audio_close,
            player::audio_transport,
            player::audio_seek,
            player::audio_track,
            player::audio_status,
            offload::offload_volumes,
            offload::offload_preflight,
            offload::offload_start,
            offload::offload_check_existing,
            offload::offload_template_preview,
            offload::offload_card_info,
            offload::offload_check_cancel,
            offload::offload_cancel,
            offload::offload_eject,
            offload::reveal,
            explorer::explorer_list,
            explorer::explorer_volumes,
            explorer::explorer_project_roots,
            explorer::explorer_watch,
            media::media_fields,
            media::media_list,
            media::media_describe,
            media::media_thumbnail,
            media::media_filmstrip,
            media::media_waveform,
            media::media_set,
            media::media_export,
            logs::markers_list,
            logs::marker_save,
            logs::marker_delete,
            logs::logs_export,
            report::report_list,
            report::report_get,
            report::report_create,
            report::report_save,
            report::report_delete,
            report::report_schema,
            report::report_add_media,
            report::report_branding_get,
            report::report_branding_set,
            report::report_export,
            monitor::monitor_screens,
            monitor::monitor_open,
            monitor::monitor_close,
            quick::quick_open,
            quick::quick_pending,
            legal::legal_text,
            legal::ffmpeg_info,
            subtitles::subtitles_load,
            subtitles::subtitles_save,
            subtitles::subtitles_check,
            sync::sync_expand,
            sync::sync_analyze,
            sync::sync_cancel,
            sync::sync_refine,
            sync::sync_waveforms,
            sync::sync_rewrap,
            transcode::transcode_presets,
            transcode::transcode_encoder,
            transcode::transcode_preset_write,
            transcode::transcode_preset_read,
            transcode::transcode_expand,
            transcode::transcode_start,
            transcode::transcode_cancel,
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de VERIFLOW");
}
