//! Commandes de l'onglet REPORT : rapports image et son enregistrés dans le
//! projet, pré-remplissage depuis les médias, exports PDF, CSV, XLSX, HTML.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use veriflow_core::player::timecode::FrameRate;
use veriflow_core::project::{Project, ReportSummary};
use veriflow_core::report::pdf::Branding;
use veriflow_core::report::{
    columns, export, header_fields, pdf, rows_from_media, Column, HeaderField, Lang, Report,
    ReportKind, Template,
};

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

const NO_PROJECT: &str =
    "Ouvre ou crée un projet : les rapports sont enregistrés dans le fichier projet.";

fn with_project<T>(
    app: &AppHandle,
    f: impl FnOnce(&Project) -> veriflow_core::Result<T>,
) -> CmdResult<T> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    let project = guard.as_ref().ok_or(NO_PROJECT)?;
    f(project).map_err(text)
}

/// Rapports du projet (liste vide sans projet ouvert).
#[tauri::command]
pub fn report_list(app: AppHandle) -> CmdResult<Vec<ReportSummary>> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    match guard.as_ref() {
        Some(p) => p.reports().map_err(text),
        None => Ok(Vec::new()),
    }
}

#[tauri::command]
pub fn report_get(id: i64, app: AppHandle) -> CmdResult<Report> {
    with_project(&app, |p| p.report(id))
}

/// Crée un rapport vide, numéroté à la suite des rapports du même type.
#[tauri::command]
pub fn report_create(kind: ReportKind, template: Template, app: AppHandle) -> CmdResult<Report> {
    let mut r = Report::new(kind, template);
    r.header
        .insert("date".into(), veriflow_core::offload::job::local_date());
    with_project(&app, |p| p.save_report(&r))
}

#[tauri::command]
pub fn report_save(report: Report, app: AppHandle) -> CmdResult<Report> {
    with_project(&app, |p| p.save_report(&report))
}

#[tauri::command]
pub fn report_delete(id: i64, app: AppHandle) -> CmdResult<()> {
    with_project(&app, |p| p.delete_report(id))
}

/// Champs d'en-tête et colonnes d'un rapport, pour le formulaire.
#[derive(Serialize)]
pub struct ReportSchema {
    header: &'static [HeaderField],
    columns: Vec<Column>,
}

#[tauri::command]
pub fn report_schema(kind: ReportKind, template: Template, tracks: u8, lang: Lang) -> ReportSchema {
    let tracks: Vec<usize> = match kind {
        ReportKind::Image => Vec::new(),
        ReportKind::Sound => (1..=tracks.max(1) as usize).collect(),
    };
    ReportSchema {
        header: header_fields(kind),
        columns: columns(kind, template, &tracks, lang),
    }
}

/// Cadence d'un champ de rapport (« 25 », « 23.976 ») ; 25 par défaut.
fn rate_of(label: &str) -> Option<FrameRate> {
    Some(match label.trim() {
        "23.976" | "23.98" => FrameRate::new(24000, 1001),
        "29.97" => FrameRate::new(30000, 1001),
        "59.94" => FrameRate::new(60000, 1001),
        "" => return None,
        other => FrameRate::new(other.parse::<f64>().ok()?.round() as u32, 1),
    })
}

/// Ajoute au rapport une ligne par média (sans l'enregistrer : l'interface
/// enregistre le rapport renvoyé).
#[tauri::command]
pub async fn report_add_media(
    mut report: Report,
    paths: Vec<PathBuf>,
    app: AppHandle,
) -> CmdResult<Report> {
    // Champs saisis dans MEDIA, lus avant de partir en tâche de fond.
    let metas: Vec<_> = {
        let state = app.state::<AppState>();
        let guard = state.project.lock().map_err(text)?;
        paths
            .iter()
            .map(|p| {
                guard
                    .as_ref()
                    .and_then(|pr| pr.media_meta(&p.display().to_string()).ok())
                    .unwrap_or_default()
            })
            .collect()
    };
    tauri::async_runtime::spawn_blocking(move || {
        let fps = rate_of(report.header("fps"));
        let (rows, header) = rows_from_media(report.kind, &paths, fps, |path| {
            paths
                .iter()
                .position(|p| p.display().to_string() == path)
                .and_then(|i| metas.get(i).cloned())
                .unwrap_or_default()
        });
        report.add(rows, header);
        Ok(report)
    })
    .await
    .map_err(text)?
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExportFormat {
    Pdf,
    Csv,
    Xlsx,
    Html,
}

/// Présentation des rapports (logo, nom de l'école ou de la production),
/// enregistrée dans le projet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReportBranding {
    pub logo: Option<String>,
    pub organization: String,
}

#[tauri::command]
pub fn report_branding_get(app: AppHandle) -> CmdResult<ReportBranding> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().map_err(text)?;
    let Some(p) = guard.as_ref() else {
        return Ok(ReportBranding::default());
    };
    Ok(ReportBranding {
        logo: p
            .meta("report.logo")
            .map_err(text)?
            .filter(|s| !s.is_empty()),
        organization: p
            .meta("report.organization")
            .map_err(text)?
            .unwrap_or_default(),
    })
}

#[tauri::command]
pub fn report_branding_set(branding: ReportBranding, app: AppHandle) -> CmdResult<()> {
    with_project(&app, |p| {
        p.set_meta("report.logo", branding.logo.as_deref().unwrap_or(""))?;
        p.set_meta("report.organization", &branding.organization)
    })
}

/// Exporte un rapport dans `dest`. Renvoie le chemin écrit.
#[tauri::command]
pub async fn report_export(
    report: Report,
    format: ExportFormat,
    dest: PathBuf,
    lang: Lang,
    app: AppHandle,
) -> CmdResult<PathBuf> {
    let branding = report_branding_get(app.clone())?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = match format {
            ExportFormat::Pdf => {
                let logo_path = branding.logo.as_deref().map(Path::new);
                let logo = match logo_path {
                    Some(p) => Some(
                        std::fs::read(p)
                            .map_err(|e| format!("logo illisible ({}) : {e}", p.display()))?,
                    ),
                    None => None,
                };
                let ext = logo_path
                    .and_then(|p| p.extension())
                    .map(|e| e.to_string_lossy().into_owned())
                    .unwrap_or_default();
                pdf::render(
                    &report,
                    lang,
                    &Branding {
                        logo,
                        logo_ext: ext,
                        organization: branding.organization.clone(),
                    },
                )
                .map_err(text)?
            }
            ExportFormat::Csv => export::to_csv(&report, lang).into_bytes(),
            ExportFormat::Html => export::to_html(&report, lang).into_bytes(),
            ExportFormat::Xlsx => export::to_xlsx(&report, lang).map_err(text)?,
        };
        std::fs::write(&dest, bytes).map_err(|e| format!("{} : {e}", dest.display()))?;
        Ok(dest)
    })
    .await
    .map_err(text)?
}
