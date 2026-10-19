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
///
/// Position et taille en pixels physiques de l'écran visé : en coordonnées
/// « logiques », deux écrans à des échelles différentes (portable à 150 %,
/// écran externe à 100 %) décalent la fenêtre, qui restait alors sur l'écran
/// principal. Sous Windows et Linux, la fenêtre sans bord couvre exactement
/// l'écran (le système la traite comme plein écran) : le plein écran du
/// système s'appliquerait à l'écran où se trouve la fenêtre à cet instant,
/// pas forcément à celui demandé. macOS garde son plein écran, une fois la
/// fenêtre sur le bon écran.
#[tauri::command]
pub async fn monitor_open(index: usize, app: AppHandle) -> CmdResult<()> {
    let screens = app.available_monitors().map_err(text)?;
    let screen = screens.get(index).ok_or("écran introuvable")?;
    let pos = *screen.position();
    let size = *screen.size();
    let window = match app.get_webview_window(LABEL) {
        Some(w) => w,
        None => {
            WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("index.html?monitor=1".into()))
                .title("VERIFLOW : moniteur")
                .decorations(false)
                .background_color(tauri::window::Color(0, 0, 0, 255))
                // Cachée jusqu'à sa mise en place : pas d'éclair sur l'écran principal.
                .visible(false)
                .build()
                .inspect(|w| {
                    // Fermée (Échap, ou par le système) : la fenêtre principale est prévenue.
                    let handle = app.clone();
                    w.on_window_event(move |e| {
                        if let WindowEvent::Destroyed = e {
                            let _ = handle.emit_to("main", "monitor-closed", ());
                        }
                    });
                })
                .map_err(text)?
        }
    };
    window.set_fullscreen(false).map_err(text)?;
    // Deux passes : en changeant d'écran, Windows peut rééchelonner la fenêtre
    // une fois arrivée ; la seconde passe la remet aux dimensions exactes.
    for _ in 0..2 {
        window.set_position(pos).map_err(text)?;
        window.set_size(size).map_err(text)?;
        std::thread::sleep(std::time::Duration::from_millis(80));
    }
    window.show().map_err(text)?;
    if cfg!(target_os = "macos") {
        window.set_fullscreen(true).map_err(text)?;
    } else {
        window.set_always_on_top(true).map_err(text)?;
    }
    Ok(())
}

#[tauri::command]
pub fn monitor_close(app: AppHandle) -> CmdResult<()> {
    if let Some(w) = app.get_webview_window(LABEL) {
        w.close().map_err(text)?;
    }
    Ok(())
}
