//! Lecteur rapide de MEDIA dans une vraie fenêtre du système : déplaçable sur
//! un autre écran, redimensionnable, réductible. Une seule fenêtre, réutilisée
//! d'un média à l'autre ; elle libère le lecteur (vidéo et son) en se fermant.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, State, WebviewUrl,
    WebviewWindowBuilder, WindowEvent,
};

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

pub const LABEL: &str = "quick";
/// Emplacement du lecteur utilisé par la fenêtre (vidéo et son).
const SLOT: &str = "preview";

/// Média à lire.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuickMedia {
    pub path: String,
    /// « video » ou « audio ».
    pub kind: String,
    pub name: String,
}

/// Position et taille mémorisées (points logiques).
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct Geometry {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

/// Média demandé, relu par la fenêtre à son chargement.
#[derive(Default)]
pub struct QuickState(Mutex<Option<QuickMedia>>);

/// La géométrie tombe-t-elle (au moins en partie) sur un écran branché ?
fn on_screen(app: &AppHandle, g: &Geometry) -> bool {
    app.available_monitors().is_ok_and(|screens| {
        screens.iter().any(|s| {
            let scale = s.scale_factor();
            let p = s.position().to_logical::<f64>(scale);
            let z = s.size().to_logical::<f64>(scale);
            // Barre de titre visible : le haut de la fenêtre est sur l'écran.
            g.x + g.width > p.x + 40.0
                && g.x < p.x + z.width - 40.0
                && g.y >= p.y - 10.0
                && g.y < p.y + z.height - 40.0
        })
    })
}

/// Ouvre le lecteur rapide sur `media`, ou recharge la fenêtre déjà ouverte.
/// Commande asynchrone : sous Windows, créer une fenêtre depuis une commande
/// synchrone bloque l'application (fenêtre blanche, test d'Hervé du 23/10/2026).
#[tauri::command]
pub async fn quick_open(
    app: AppHandle,
    media: QuickMedia,
    geometry: Option<Geometry>,
    state: State<'_, QuickState>,
) -> CmdResult<()> {
    let title = format!("{} : VERIFLOW", media.name);
    *state.0.lock().map_err(text)? = Some(media.clone());
    if let Some(w) = app.get_webview_window(LABEL) {
        w.emit_to(LABEL, "quick-load", &media).map_err(text)?;
        let _ = w.set_title(&title);
        let _ = w.unminimize();
        let _ = w.set_focus();
        return Ok(());
    }
    // Dernière place connue si elle est encore sur un écran branché ; sinon
    // au centre.
    let geometry = geometry.filter(|g| on_screen(&app, g));
    let builder =
        WebviewWindowBuilder::new(&app, LABEL, WebviewUrl::App("index.html?quick=1".into()))
            .title(&title)
            .min_inner_size(480.0, 320.0);
    let builder = match geometry {
        Some(g) => builder
            .inner_size(g.width.max(480.0), g.height.max(320.0))
            .position(g.x, g.y),
        None => builder.inner_size(1200.0, 800.0).center(),
    };
    let window = builder.build().map_err(text)?;
    if let Some(g) = geometry {
        // Deux passes : la taille et la position suivent l'échelle de l'écran
        // visé, connue seulement une fois la fenêtre placée.
        let _ = window.set_position(LogicalPosition::new(g.x, g.y));
        let _ = window.set_size(LogicalSize::new(g.width, g.height));
    }
    let handle = app.clone();
    window.on_window_event(move |e| {
        if let WindowEvent::Destroyed = e {
            handle.state::<crate::player::PlayerState>().release(SLOT);
            if let Ok(mut m) = handle.state::<QuickState>().0.lock() {
                *m = None;
            }
        }
    });
    Ok(())
}

/// Média à afficher, demandé par la fenêtre à son chargement.
#[tauri::command]
pub fn quick_pending(state: State<'_, QuickState>) -> CmdResult<Option<QuickMedia>> {
    Ok(state.0.lock().map_err(text)?.clone())
}

/// Ferme le lecteur rapide (fermeture de la fenêtre principale).
pub fn close(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(LABEL) {
        let _ = w.close();
    }
}
