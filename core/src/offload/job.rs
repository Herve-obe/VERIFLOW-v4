//! Préparation et exécution complète d'un offload : modèle d'arborescence,
//! contrôles préalables, copie, MHL et rapports (charte §7.1).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use super::engine::{check_space, run, Event, OffloadSpec, OffloadSummary};
use super::hash::HashAlgo;
use super::mhl::{write_generation, MhlAuthor};
use super::report::{write_reports, ReportData, ReportFiles, ReportInfo};
use super::scan::{scan, SourceInventory};
use crate::Result;

/// Modèle d'arborescence par défaut : un dossier par jour, puis la carte.
pub const DEFAULT_TEMPLATE: &str = "{date}/{carte}";

/// Variables disponibles dans le modèle d'arborescence.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateVars {
    pub projet: Option<String>,
    pub jour: Option<String>,
    pub camera: Option<String>,
}

/// Demande d'offload envoyée par l'interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OffloadRequest {
    pub source: PathBuf,
    /// Dossiers de base des destinations (disques).
    pub destinations: Vec<PathBuf>,
    pub template: String,
    pub vars: TemplateVars,
    pub algorithms: Vec<HashAlgo>,
    #[serde(default)]
    pub operator: Option<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

/// Remplace les caractères interdits dans un nom de dossier (Windows inclus).
fn sanitize(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    cleaned.trim().trim_end_matches('.').to_owned()
}

/// Calcule le chemin relatif issu du modèle. Les segments vides sont omis.
pub fn render_template(template: &str, card: &str, vars: &TemplateVars, date: &str) -> PathBuf {
    let value = |name: &str| -> String {
        match name {
            "carte" | "card" => card.to_owned(),
            "date" => date.to_owned(),
            "projet" | "project" => vars.projet.clone().unwrap_or_default(),
            "jour" | "day" => vars.jour.clone().unwrap_or_default(),
            "camera" | "caméra" => vars.camera.clone().unwrap_or_default(),
            _ => String::new(),
        }
    };
    let mut out = PathBuf::new();
    for segment in template.split(['/', '\\']) {
        let mut rendered = String::new();
        let mut rest = segment;
        while let Some(start) = rest.find('{') {
            rendered += &rest[..start];
            match rest[start..].find('}') {
                Some(end) => {
                    rendered += &value(&rest[start + 1..start + end].to_lowercase());
                    rest = &rest[start + end + 1..];
                }
                None => {
                    rendered += &rest[start..];
                    rest = "";
                }
            }
        }
        rendered += rest;
        let s = sanitize(&rendered);
        if !s.is_empty() {
            out.push(s);
        }
    }
    if out.as_os_str().is_empty() {
        out.push(sanitize(card));
    }
    out
}

/// Résultat des contrôles avant copie, affiché à l'utilisateur.
#[derive(Debug, Clone, Serialize)]
pub struct Preflight {
    pub source_name: String,
    pub files: usize,
    pub total_bytes: u64,
    pub fingerprint: String,
    /// Dossier final de chaque destination.
    pub roots: Vec<PathBuf>,
    /// Octets manquants par destination (null = assez de place).
    pub missing_space: Vec<Option<u64>>,
    /// Destinations contenant déjà une copie vérifiée (historique ascmhl).
    pub already_in_destination: Vec<bool>,
    #[serde(skip)]
    pub inventory: Option<SourceInventory>,
}

pub fn local_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Inventorie la source et contrôle les destinations, sans rien écrire.
pub fn preflight(req: &OffloadRequest) -> Result<Preflight> {
    let inv = scan(&crate::absolute_path(&req.source))?;
    let rel = render_template(&req.template, &inv.name, &req.vars, &local_date());
    let roots: Vec<PathBuf> = req
        .destinations
        .iter()
        .map(|d| crate::absolute_path(&d.join(&rel)))
        .collect();
    Ok(Preflight {
        source_name: inv.name.clone(),
        files: inv.files.len(),
        total_bytes: inv.total_bytes,
        fingerprint: inv.fingerprint.clone(),
        missing_space: check_space(&inv, &roots),
        already_in_destination: roots
            .iter()
            .map(|r| r.join("ascmhl/ascmhl_chain.xml").exists())
            .collect(),
        roots,
        inventory: Some(inv),
    })
}

/// Résultat complet d'un offload.
#[derive(Debug, Clone, Serialize)]
pub struct JobResult {
    pub summary: OffloadSummary,
    pub roots: Vec<PathBuf>,
    pub mhl: Vec<Option<PathBuf>>,
    pub reports: Vec<ReportFiles>,
    pub source_name: String,
    pub fingerprint: String,
}

/// Exécute l'offload : copie et vérification, puis MHL et rapports.
pub fn execute(
    req: &OffloadRequest,
    pre: Preflight,
    cancel: &AtomicBool,
    emit: impl FnMut(Event),
    project: Option<&str>,
) -> Result<JobResult> {
    let inv = match pre.inventory {
        Some(inv) => inv,
        None => scan(&crate::absolute_path(&req.source))?,
    };
    let spec = OffloadSpec {
        destinations: pre.roots.clone(),
        algorithms: if req.algorithms.is_empty() {
            vec![HashAlgo::Xxh128]
        } else {
            req.algorithms.clone()
        },
    };
    let summary = run(&inv, &spec, cancel, emit)?;
    let author = MhlAuthor {
        name: req.operator.clone(),
        comment: None,
        ..Default::default()
    };
    let mhl: Vec<Option<PathBuf>> = if summary.cancelled {
        vec![None; spec.destinations.len()]
    } else {
        spec.destinations
            .iter()
            .enumerate()
            .map(|(i, d)| write_generation(&inv, &summary, i, d, &spec.algorithms, &author).ok())
            .collect()
    };
    let info = ReportInfo {
        project: project
            .map(str::to_owned)
            .or_else(|| req.vars.projet.clone()),
        operator: req.operator.clone(),
        notes: req.notes.clone(),
    };
    let reports = write_reports(&ReportData {
        inv: &inv,
        summary: &summary,
        spec: &spec,
        info: &info,
        mhl: &mhl,
    })?;
    Ok(JobResult {
        source_name: inv.name.clone(),
        fingerprint: inv.fingerprint.clone(),
        roots: spec.destinations,
        summary,
        mhl,
        reports,
    })
}

/// Racine d'un volume ou dossier pour l'affichage.
pub fn display(p: &Path) -> String {
    p.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_templates() {
        let v = TemplateVars {
            projet: Some("Court métrage".into()),
            jour: Some("J03".into()),
            camera: Some("A".into()),
        };
        let r = |t: &str| render_template(t, "A001", &v, "2026-10-06");
        assert_eq!(
            r("{date}/{carte}"),
            PathBuf::from("2026-10-06").join("A001")
        );
        assert_eq!(
            r("{projet}/{jour}/CAM_{camera}/{carte}"),
            PathBuf::from("Court métrage")
                .join("J03")
                .join("CAM_A")
                .join("A001")
        );
        // Segments vides omis, caractères interdits remplacés.
        let empty = TemplateVars::default();
        assert_eq!(
            render_template("{projet}/{carte}", "A:001", &empty, "d"),
            PathBuf::from("A_001")
        );
        assert_eq!(
            render_template("", "A001", &empty, "d"),
            PathBuf::from("A001")
        );
    }

    #[test]
    fn full_job_with_preflight() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        std::fs::create_dir_all(src.join("CLIP")).unwrap();
        std::fs::write(src.join("CLIP/C0001.MP4"), b"video").unwrap();
        let req = OffloadRequest {
            source: src,
            destinations: vec![dir.path().join("SSD1"), dir.path().join("SSD2")],
            template: "{jour}/{carte}".into(),
            vars: TemplateVars {
                jour: Some("J01".into()),
                ..Default::default()
            },
            algorithms: vec![HashAlgo::Xxh128],
            operator: Some("DIT".into()),
            notes: None,
        };
        let pre = preflight(&req).unwrap();
        assert_eq!(pre.files, 1);
        assert_eq!(pre.roots[0], dir.path().join("SSD1/J01/A001"));
        assert_eq!(pre.already_in_destination, vec![false, false]);
        let res = execute(
            &req,
            pre,
            &AtomicBool::new(false),
            |_| {},
            Some("Projet test"),
        )
        .unwrap();
        assert_eq!(res.summary.failed_files, 0);
        assert!(res
            .mhl
            .iter()
            .all(|m| m.as_ref().is_some_and(|p| p.exists())));
        assert_eq!(res.reports.len(), 2);
        // Deuxième passage : la copie est détectée dans les destinations.
        assert_eq!(
            preflight(&req).unwrap().already_in_destination,
            vec![true, true]
        );
    }
}
