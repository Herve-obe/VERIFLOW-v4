//! Barre de menus native de macOS. Son contenu (libellés traduits, raccourcis)
//! vient de l'interface, qui décrit aussi la barre dessinée de Windows et de
//! Linux : une seule liste de commandes pour les deux. Un clic sur une entrée
//! renvoie son identifiant à l'interface (événement « menu »), qui exécute la
//! commande.

use serde::Deserialize;
use tauri::AppHandle;

type CmdResult<T> = Result<T, String>;

// Lus seulement par le menu natif de macOS.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Item {
    /// Commande de l'interface.
    Command {
        id: String,
        label: String,
        /// Raccourci au format de Tauri (« CmdOrCtrl+Shift+S »).
        accelerator: Option<String>,
        enabled: bool,
    },
    /// Entrée prise en charge par le système (copier, coller, quitter...) :
    /// indispensable sous macOS pour que les champs de texte y répondent.
    Native {
        which: String,
    },
    Separator,
    Submenu {
        label: String,
        items: Vec<Item>,
    },
}

#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Deserialize)]
pub struct Menu {
    pub label: String,
    pub items: Vec<Item>,
}

/// Remplace la barre de menus de macOS. Sans effet ailleurs : la barre y est
/// dessinée par l'interface.
#[tauri::command]
pub fn menu_set(app: AppHandle, menus: Vec<Menu>) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    {
        let menu = native::build(&app, &menus).map_err(|e| e.to_string())?;
        app.set_menu(menu).map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (app, menus);
    Ok(())
}

/// Relaie les clics du menu natif vers l'interface.
pub fn listen(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    app.on_menu_event(|app, event| {
        use tauri::Emitter;
        let _ = app.emit_to("main", "menu", event.id().0.clone());
    });
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

#[cfg(target_os = "macos")]
mod native {
    use super::{Item, Menu};
    use tauri::menu::{IsMenuItem, Menu as NativeMenu, MenuItem, PredefinedMenuItem, Submenu};
    use tauri::{AppHandle, Runtime};

    fn item<R: Runtime>(app: &AppHandle<R>, item: &Item) -> tauri::Result<Box<dyn IsMenuItem<R>>> {
        Ok(match item {
            Item::Command {
                id,
                label,
                accelerator,
                enabled,
            } => Box::new(MenuItem::with_id(
                app,
                id.as_str(),
                label,
                *enabled,
                accelerator.as_deref(),
            )?),
            Item::Separator => Box::new(PredefinedMenuItem::separator(app)?),
            Item::Native { which } => Box::new(match which.as_str() {
                "undo" => PredefinedMenuItem::undo(app, None)?,
                "redo" => PredefinedMenuItem::redo(app, None)?,
                "cut" => PredefinedMenuItem::cut(app, None)?,
                "copy" => PredefinedMenuItem::copy(app, None)?,
                "paste" => PredefinedMenuItem::paste(app, None)?,
                "select_all" => PredefinedMenuItem::select_all(app, None)?,
                "hide" => PredefinedMenuItem::hide(app, None)?,
                "hide_others" => PredefinedMenuItem::hide_others(app, None)?,
                "show_all" => PredefinedMenuItem::show_all(app, None)?,
                "minimize" => PredefinedMenuItem::minimize(app, None)?,
                "fullscreen" => PredefinedMenuItem::fullscreen(app, None)?,
                "quit" => PredefinedMenuItem::quit(app, None)?,
                _ => PredefinedMenuItem::separator(app)?,
            }),
            Item::Submenu { label, items } => Box::new(submenu(app, label, items)?),
        })
    }

    fn submenu<R: Runtime>(
        app: &AppHandle<R>,
        label: &str,
        items: &[Item],
    ) -> tauri::Result<Submenu<R>> {
        let built = items
            .iter()
            .map(|i| item(app, i))
            .collect::<tauri::Result<Vec<_>>>()?;
        let refs: Vec<&dyn IsMenuItem<R>> = built.iter().map(|b| b.as_ref()).collect();
        Submenu::with_items(app, label, true, &refs)
    }

    pub fn build<R: Runtime>(app: &AppHandle<R>, menus: &[Menu]) -> tauri::Result<NativeMenu<R>> {
        let subs = menus
            .iter()
            .map(|m| submenu(app, &m.label, &m.items))
            .collect::<tauri::Result<Vec<_>>>()?;
        let refs: Vec<&dyn IsMenuItem<R>> = subs.iter().map(|s| s as &dyn IsMenuItem<R>).collect();
        NativeMenu::with_items(app, &refs)
    }
}
