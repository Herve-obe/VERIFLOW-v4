//! Sortie moniteur : l'image du lecteur, plein écran, sur un autre écran.
//! La fenêtre « monitor » relit l'image dans le cache du lecteur (aucun
//! second décodage) à chaque image affichée par la fenêtre principale.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub const LABEL: &str = "monitor";

#[derive(Serialize)]
pub struct Screen {
    index: usize,
    name: String,
    width: u32,
    height: u32,
    /// Écran de la fenêtre principale.
    main: bool,
}

/// Écrans branchés.
#[tauri::command]
pub fn monitor_screens(app: AppHandle) -> CmdResult<Vec<Screen>> {
    let main = app
        .get_webview_window("main")
        .and_then(|w| w.current_monitor().ok().flatten());
    Ok(app
        .available_monitors()
        .map_err(text)?
        .iter()
        .enumerate()
        .map(|(index, m)| Screen {
            index,
            name: m
                .name()
                .cloned()
                .unwrap_or_else(|| format!("{}", index + 1)),
            width: m.size().width,
            height: m.size().height,
            main: main
                .as_ref()
                .is_some_and(|c| c.position() == m.position() && c.size() == m.size()),
        })
        .collect())
}

/// Ouvre (ou déplace) la fenêtre moniteur, plein écran sur l'écran `index`.
#[tauri::command]
pub async fn monitor_open(
    index: usize,
    app: AppHandle,
    caller: tauri::WebviewWindow,
) -> CmdResult<()> {
    let screens = app.available_monitors().map_err(text)?;
    let screen = screens.get(index).ok_or("écran introuvable")?;
    let scale = screen.scale_factor();
    let pos = screen.position().to_logical::<f64>(scale);
    let size = screen.size().to_logical::<f64>(scale);
    let window = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => {
            WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("index.html?monitor=1".into()))
                .title("VERIFLOW : moniteur")
                .decorations(false)
                .background_color(tauri::window::Color(0, 0, 0, 255))
                .inner_size(size.width, size.height)
                .position(pos.x, pos.y)
                .build()
                .inspect(|w| {
                    // Fermée (Échap, ou par le système) : les lecteurs (fenêtre
                    // principale, lecteur rapide) sont prévenus.
                    let handle = app.clone();
                    w.on_window_event(move |e| {
                        if let WindowEvent::Destroyed = e {
                            let _ = handle.emit("monitor-closed", ());
                        }
                    });
                })
                .map_err(text)?
        }
    };
    window.set_fullscreen(false).map_err(text)?;
    window.set_position(pos).map_err(text)?;
    window.set_size(size).map_err(text)?;
    window.set_fullscreen(true).map_err(text)?;
    // Le clavier reste au lecteur qui a demandé le moniteur (Espace, J/K/L) :
    // la fenêtre plein écran le prenait (test d'Hervé du 26/10/2026). Un clic
    // sur le moniteur lui rend le clavier (Échap pour le fermer).
    let _ = caller.set_focus();
    Ok(())
}

#[tauri::command]
pub fn monitor_close(app: AppHandle) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.close().map_err(text)?;
    }
    Ok(())
}
