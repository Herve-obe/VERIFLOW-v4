//! Commande des exports vers le montage (FCPXML, XMEML, EDL, ALE, OTIO, CSV
//! Resolve) depuis MEDIA. Marche sans projet ouvert, alors sans marqueurs.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};
use veriflow_core::editorial::{export, EditClip, EditFormat, EditOptions, EditResult};
use veriflow_core::media::catalog::describe;
use veriflow_core::media::export::{merged, MediaRecord};
use veriflow_core::player::logs::Marker;

use crate::media::edits_of;
use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Clé de comparaison d'un chemin : sous Windows, la casse et le sens des
/// séparateurs ne comptent pas.
fn path_key(path: &str, windows: bool) -> String {
    if windows {
        path.replace('\\', "/").to_lowercase()
    } else {
        path.to_owned()
    }
}

/// Marqueurs du projet par chemin normalisé (vide sans projet ouvert).
fn project_markers(app: &AppHandle) -> CmdResult<HashMap<String, Vec<Marker>>> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    let mut by_path: HashMap<String, Vec<Marker>> = HashMap::new();
    if let Some(p) = guard.as_ref() {
        for m in p.markers(None).map_err(text)? {
            by_path
                .entry(path_key(&m.path, cfg!(windows)))
                .or_default()
                .push(m);
        }
    }
    Ok(by_path)
}

/// Exporte les médias `paths` dans chacun des `formats`, dans le dossier `out`.
/// Les marqueurs viennent du projet ouvert, s'il y en a un.
#[tauri::command]
pub async fn editorial_export(
    formats: Vec<EditFormat>,
    paths: Vec<PathBuf>,
    options: EditOptions,
    out: PathBuf,
    app: AppHandle,
) -> CmdResult<EditResult> {
    if formats.is_empty() {
        return Err("aucun format choisi".into());
    }
    if paths.is_empty() {
        return Err("aucun média à exporter".into());
    }
    let mut markers = if options.markers {
        project_markers(&app)?
    } else {
        HashMap::new()
    };
    let edits: Vec<_> = paths.iter().map(|p| edits_of(&app, p)).collect();
    tauri::async_runtime::spawn_blocking(move || {
        let clips: Vec<EditClip> = paths
            .iter()
            .zip(edits)
            .map(|(p, e)| {
                let details = describe(p);
                let own = markers
                    .remove(&path_key(&p.display().to_string(), cfg!(windows)))
                    .unwrap_or_default();
                let record = MediaRecord {
                    path: p.clone(),
                    values: merged(&details.embedded, &e),
                    details,
                };
                EditClip::from_record(&record, own)
            })
            .collect();
        run(&formats, &clips, &options, &out)
    })
    .await
    .map_err(text)?
}

/// Un export par format ; fichiers et avertissements sont cumulés.
fn run(
    formats: &[EditFormat],
    clips: &[EditClip],
    options: &EditOptions,
    out: &Path,
) -> CmdResult<EditResult> {
    let mut total = EditResult::default();
    for &f in formats {
        let r = export(f, clips, options, out).map_err(text)?;
        total.written.extend(r.written);
        for w in r.warnings {
            if !total.warnings.contains(&w) {
                total.warnings.push(w);
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_key_windows_ignores_case_and_separators() {
        assert_eq!(
            path_key(r"D:\Rushes\A001.MOV", true),
            path_key("d:/rushes/a001.mov", true)
        );
        assert_ne!(path_key("/a/B.mov", false), path_key("/a/b.mov", false));
    }

    #[test]
    fn run_cumulates_formats_and_warnings() {
        let dir = std::env::temp_dir().join(format!("veriflow-editorial-{}", std::process::id()));
        let none: Vec<EditClip> = Vec::new();
        let r = run(
            &[EditFormat::Edl, EditFormat::Ale],
            &none,
            &EditOptions::default(),
            &dir,
        )
        .unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(r.written.len(), 2);
    }
}
