//! Préparation et exécution complète d'un offload : modèle d'arborescence,
//! contrôles préalables, copie, MHL et rapports (charte §7.1).

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};

use super::cascade::{self, PendingCascade, Stage1Summary};
use super::engine::{check_space, run, DestStatus, Event, OffloadSpec, OffloadSummary};
use super::existing::{self, Elsewhere, ExistingMode, Known, Present};
use super::hash::HashAlgo;
use super::mhl::{write_generation, MhlAuthor};
use super::report::{write_reports, CascadeInfo, ReportData, ReportFiles, ReportInfo};
use super::scan::{scan, source_name, SourceInventory};
use crate::{Error, Result};

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
    /// Conduite envers les rushes déjà présents en destination.
    #[serde(default)]
    pub existing: ExistingMode,
    /// Date du modèle `{date}` (AAAA-MM-JJ). Absente : date du jour. Une
    /// reprise garde la date de la copie interrompue (même dossier).
    #[serde(default)]
    pub date: Option<String>,
    /// Dossiers finals imposés (un par destination), à la place du modèle :
    /// reprise d'une copie trouvée dans un autre dossier.
    #[serde(default)]
    pub roots: Option<Vec<PathBuf>>,
    /// Dossiers des copies précédentes du projet (rempli par l'application,
    /// pas par l'interface) : une carte réutilisée sans formatage (copiée
    /// lundi, recopiée mardi avec de nouveaux plans) y est retrouvée même si
    /// le dossier du jour porte un autre nom.
    #[serde(default)]
    pub history: Vec<PathBuf>,
    /// Destinations en cascade (étape 2), copiées plus tard depuis la copie
    /// vérifiée de l'étape 1 sans relire la carte.
    #[serde(default)]
    pub cascade: Vec<PathBuf>,
    /// Les destinations lentes de `destinations` (disque dur, réseau) passent
    /// en étape 2, sauf si toutes sont lentes ou toutes rapides.
    #[serde(default)]
    pub cascade_auto: bool,
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
    /// Octets manquants par destination de l'étape 1 (null = assez de place, et
    /// toujours null pour une destination de l'étape 2).
    pub missing_space: Vec<Option<u64>>,
    /// Octets manquants par destination de l'étape 2 : simple avertissement,
    /// elle ne bloque pas la copie de la carte (l'étape 2 recontrôle l'espace
    /// à son démarrage). Toujours null pour une destination de l'étape 1.
    pub cascade_space_warning: Vec<Option<u64>>,
    /// Algorithmes ajoutés d'office (identifiants, ex. `xxh128`) : avec une
    /// étape 2, l'historique MHL de l'étape 1 doit contenir au moins une
    /// empreinte (SHA-256 seul n'y figure pas). Vide sinon.
    pub added_algorithms: Vec<String>,
    /// Destinations contenant déjà une copie vérifiée (historique ascmhl).
    pub already_in_destination: Vec<bool>,
    /// Rushes de la source déjà présents dans chaque destination.
    pub present: Vec<Present>,
    /// Date utilisée pour le modèle `{date}`.
    pub date: String,
    /// Par destination, si le dossier final ne contient encore rien : copies
    /// de cette carte trouvées dans d'autres dossiers (autre date...).
    pub elsewhere: Vec<Vec<Elsewhere>>,
    /// Étape (1 ou 2) de chaque destination, dans l'ordre de `roots` (celui de
    /// `destinations` puis de `cascade`). Seule l'étape 1 est copiée tout de suite.
    pub stages: Vec<u8>,
    #[serde(skip)]
    pub inventory: Option<SourceInventory>,
}

pub fn local_date() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Toutes les destinations de la demande : immédiates puis en cascade.
fn all_destinations(req: &OffloadRequest) -> Vec<PathBuf> {
    req.destinations
        .iter()
        .chain(&req.cascade)
        .cloned()
        .collect()
}

/// Étape (1 ou 2) de chaque destination de `destinations` puis de `cascade`.
/// Avec `cascade_auto`, les destinations lentes de `destinations` passent en
/// étape 2, sauf si toutes sont lentes ou toutes rapides. Les destinations de
/// `cascade` sont toujours en étape 2. Sans destination immédiate, la demande
/// est refusée (`cascade_no_immediate`) : la carte ne serait jamais copiée.
pub fn split_cascade(req: &OffloadRequest, is_slow: impl Fn(&Path) -> bool) -> Result<Vec<u8>> {
    let mut stages = vec![1u8; req.destinations.len()];
    if req.cascade_auto {
        let slow: Vec<bool> = req.destinations.iter().map(|d| is_slow(d)).collect();
        let n_slow = slow.iter().filter(|s| **s).count();
        if n_slow > 0 && n_slow < slow.len() {
            for (stage, slow) in stages.iter_mut().zip(&slow) {
                if *slow {
                    *stage = 2;
                }
            }
        }
    }
    stages.extend(req.cascade.iter().map(|_| 2));
    if !stages.is_empty() && !stages.contains(&1) {
        return Err(Error::Unsupported(
            "cascade_no_immediate : aucune destination immédiate (étape 1)".into(),
        ));
    }
    Ok(stages)
}

/// Demande équivalente avec les étapes résolues : `destinations` ne garde que
/// l'étape 1, `cascade` toute l'étape 2, sans choix automatique. Les étapes
/// sont ainsi calculées une fois (lecture des volumes) puis transmises à
/// l'exécution, qui ne les recalcule pas.
pub fn resolve_stages(req: &OffloadRequest, stages: &[u8]) -> OffloadRequest {
    let n = req.destinations.len();
    let stage_of = |i: usize| stages.get(i).copied().unwrap_or(1);
    let all = all_destinations(req);
    let mut out = req.clone();
    out.destinations = (0..all.len())
        .filter(|&i| stage_of(i) == 1)
        .map(|i| all[i].clone())
        .collect();
    out.cascade = (0..all.len())
        .filter(|&i| stage_of(i) != 1)
        .map(|i| all[i].clone())
        .collect();
    out.cascade_auto = false;
    // Dossiers imposés (reprise) : ceux des destinations restées immédiates.
    out.roots = req.roots.as_ref().filter(|r| r.len() == n).map(|r| {
        (0..n)
            .filter(|&i| stage_of(i) == 1)
            .map(|i| r[i].clone())
            .collect()
    });
    out
}

/// Algorithmes de la copie. Avec une étape 2, XXH128 est ajouté d'office si
/// aucun algorithme n'entre dans le MHL : sans empreinte dans l'historique de
/// l'étape 1, l'étape 2 n'aurait rien à comparer. Renvoie aussi les ajouts.
fn stage_algorithms(requested: &[HashAlgo], has_cascade: bool) -> (Vec<HashAlgo>, Vec<HashAlgo>) {
    let mut algos = if requested.is_empty() {
        vec![HashAlgo::Xxh128]
    } else {
        requested.to_vec()
    };
    let mut added = Vec::new();
    if has_cascade && !algos.iter().any(|a| a.in_mhl()) {
        algos.push(HashAlgo::Xxh128);
        added.push(HashAlgo::Xxh128);
    }
    (algos, added)
}

/// Dossiers finals d'après le nom de source et la date (source de vérité
/// commune à `preflight` et `planned_roots`), dans l'ordre de
/// `all_destinations`. Les dossiers imposés (`roots`) ne concernent que les
/// destinations immédiates.
fn planned_roots_for(req: &OffloadRequest, card: &str, date: &str) -> Vec<PathBuf> {
    let rel = render_template(&req.template, card, &req.vars, date);
    let imposed = req
        .roots
        .as_ref()
        .filter(|r| r.len() == req.destinations.len());
    all_destinations(req)
        .iter()
        .enumerate()
        .map(|(i, d)| match imposed.and_then(|r| r.get(i)) {
            Some(r) => crate::absolute_path(r),
            None => crate::absolute_path(&d.join(&rel)),
        })
        .collect()
}

/// Dossiers finals de chaque destination (immédiates puis en cascade), sans
/// parcourir la source (le nom de la carte suffit).
pub fn planned_roots(req: &OffloadRequest) -> Vec<PathBuf> {
    let date = req.date.clone().unwrap_or_else(local_date);
    let name = source_name(&crate::absolute_path(&req.source));
    planned_roots_for(req, &name, &date)
}

/// Dossiers finals de l'étape 1 et de l'étape 2, sans parcourir la source :
/// sert à l'ordonnancement des copies simultanées.
pub fn planned_split(req: &OffloadRequest) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    planned_split_with(req, super::storage::is_slow)
}

fn planned_split_with(
    req: &OffloadRequest,
    is_slow: impl Fn(&Path) -> bool,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>)> {
    let stages = split_cascade(req, is_slow)?;
    Ok(partition(planned_roots(req), &stages))
}

/// Sépare des éléments (un par destination) selon leur étape.
fn partition<T>(items: Vec<T>, stages: &[u8]) -> (Vec<T>, Vec<T>) {
    let (mut one, mut two) = (Vec::new(), Vec::new());
    for (item, stage) in items.into_iter().zip(stages) {
        if *stage == 1 {
            one.push(item);
        } else {
            two.push(item);
        }
    }
    (one, two)
}

/// Inventorie la source et contrôle les destinations, sans rien écrire.
pub fn preflight(req: &OffloadRequest) -> Result<Preflight> {
    preflight_with(req, super::storage::is_slow)
}

fn preflight_with(req: &OffloadRequest, is_slow: impl Fn(&Path) -> bool) -> Result<Preflight> {
    let stages = split_cascade(req, is_slow)?;
    let inv = scan(&crate::absolute_path(&req.source))?;
    let date = req.date.clone().unwrap_or_else(local_date);
    let roots = planned_roots_for(req, &inv.name, &date);
    let present: Vec<Present> = roots.iter().map(|r| existing::present(&inv, r)).collect();
    let elsewhere = roots
        .iter()
        .zip(all_destinations(req))
        .zip(&present)
        .map(|((root, base), p)| {
            if p.files > 0 || p.partial > 0 {
                Vec::new()
            } else {
                existing::find_elsewhere_with_history(
                    &inv,
                    &crate::absolute_path(&base),
                    root,
                    &req.history,
                )
            }
        })
        .collect();
    let added_algorithms = stage_algorithms(&req.algorithms, stages.contains(&2))
        .1
        .iter()
        .map(|a| a.id().to_owned())
        .collect();
    let space = check_space(&inv, &roots);
    let stage_space = |wanted: u8| -> Vec<Option<u64>> {
        space
            .iter()
            .zip(&stages)
            .map(|(m, st)| if *st == wanted { *m } else { None })
            .collect()
    };
    Ok(Preflight {
        source_name: inv.name.clone(),
        files: inv.files.len(),
        total_bytes: inv.total_bytes,
        fingerprint: inv.fingerprint.clone(),
        missing_space: stage_space(1),
        cascade_space_warning: stage_space(2),
        added_algorithms,
        already_in_destination: roots
            .iter()
            .map(|r| r.join("ascmhl/ascmhl_chain.xml").exists())
            .collect(),
        present,
        elsewhere,
        date,
        roots,
        stages,
        inventory: Some(inv),
    })
}

/// Échec d'écriture de l'historique MHL d'une destination. `code` est le code
/// stable d'une erreur de cascade (`cascade_history`, `cascade_diverged`).
#[derive(Debug, Clone, Serialize)]
pub struct MhlFailure {
    pub code: Option<String>,
    pub message: String,
}

impl MhlFailure {
    pub(super) fn of(e: &Error) -> MhlFailure {
        MhlFailure {
            code: cascade::error_code(e).map(str::to_owned),
            message: cascade::message(e),
        }
    }
}

/// Résultat complet d'un offload (étape 1 s'il y a une cascade).
#[derive(Debug, Clone, Serialize)]
pub struct JobResult {
    pub summary: OffloadSummary,
    /// Dossiers de l'étape copiée (étape 1 pour un offload, étape 2 pour une cascade).
    pub roots: Vec<PathBuf>,
    pub mhl: Vec<Option<PathBuf>>,
    /// Raison de l'absence de MHL, par destination (même ordre que `mhl`).
    pub mhl_errors: Vec<Option<MhlFailure>>,
    pub reports: Vec<ReportFiles>,
    pub source_name: String,
    pub fingerprint: String,
    /// La carte peut être éjectée : copie terminée, aucun fichier en échec et
    /// toutes les destinations de l'étape 1 vérifiées. Toujours faux pour une étape 2.
    pub card_free: bool,
    /// Étape 2 à faire plus tard (None sans cascade, ou si l'étape 1 n'a laissé
    /// aucune copie vérifiée avec son historique).
    pub cascade: Option<PendingCascade>,
}

/// Le recouvrement copie/vérification est exclu dès qu'une destination est un
/// disque dur mécanique (deux accès simultanés s'y gênent).
pub(super) fn overlap_allowed(roots: &[PathBuf], is_hdd: impl Fn(&Path) -> bool) -> bool {
    !roots.iter().any(|r| is_hdd(r))
}

/// Exécute l'offload : copie et vérification de l'étape 1, puis MHL et
/// rapports. Les destinations en cascade ne sont pas touchées : `cascade` du
/// résultat décrit l'étape 2 à lancer. `known` est indexé comme `pre.roots`
/// (ou seulement sur les destinations de l'étape 1).
pub fn execute(
    req: &OffloadRequest,
    pre: Preflight,
    cancel: &AtomicBool,
    emit: impl FnMut(Event),
    project: Option<&str>,
    known: Option<Known>,
) -> Result<JobResult> {
    // Pas de mise en veille (ni d'App Nap sous macOS) pendant le travail.
    let _awake = crate::power::keep_awake();
    let inv = match pre.inventory {
        Some(inv) => inv,
        None => scan(&crate::absolute_path(&req.source))?,
    };
    let all = pre.roots.len();
    let stages = if pre.stages.len() == all {
        pre.stages.clone()
    } else {
        vec![1; all]
    };
    let immediate: Vec<usize> = (0..all).filter(|&i| stages[i] == 1).collect();
    let (roots1, roots2) = partition(pre.roots.clone(), &stages);
    let known = known.map(|mut k| {
        if k.identical.len() == all && immediate.len() != all {
            k.identical = immediate.iter().map(|&i| k.identical[i].clone()).collect();
        }
        k
    });
    // Pas de recouvrement si une destination de l'étape 1 est un disque dur
    // mécanique (celles de l'étape 2 ne sont pas copiées ici).
    let overlap = overlap_allowed(&roots1, super::storage::is_hdd);
    let spec = OffloadSpec {
        destinations: roots1,
        algorithms: stage_algorithms(&req.algorithms, !roots2.is_empty()).0,
        known: match req.existing {
            ExistingMode::Complete => known,
            _ => None,
        },
        overlap,
        expected: None,
    };
    if req.existing == ExistingMode::Replace {
        existing::remove(&inv, &spec.destinations)?;
    }
    let summary = run(&inv, &spec, cancel, emit)?;
    let author = MhlAuthor {
        name: req.operator.clone(),
        comment: None,
        ..Default::default()
    };
    let written: Vec<Result<PathBuf>> = if summary.cancelled {
        Vec::new()
    } else {
        spec.destinations
            .iter()
            .enumerate()
            .map(|(i, d)| write_generation(&inv, &summary, i, d, &spec.algorithms, &author))
            .collect()
    };
    let mhl_errors: Vec<Option<MhlFailure>> = if summary.cancelled {
        vec![None; spec.destinations.len()]
    } else {
        written
            .iter()
            .map(|r| r.as_ref().err().map(MhlFailure::of))
            .collect()
    };
    let mhl: Vec<Option<PathBuf>> = if summary.cancelled {
        vec![None; spec.destinations.len()]
    } else {
        written.into_iter().map(|r| r.ok()).collect()
    };
    // Étape 2 : seules comptent les destinations dont tous les fichiers sont
    // vérifiés et dont l'historique est écrit.
    let sound: Vec<PathBuf> = spec
        .destinations
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            mhl[*i].is_some()
                && summary
                    .files
                    .iter()
                    .all(|f| !matches!(f.destinations[*i], DestStatus::Failed(_)))
        })
        .map(|(_, d)| d.clone())
        .collect();
    let cascade_possible = !summary.cancelled && !roots2.is_empty() && !sound.is_empty();
    // Carte libérable : tout est vérifié, l'historique de chaque destination de
    // l'étape 1 est écrit (sans lui l'étape 2 est impossible) et, s'il y a une
    // étape 2, elle peut être lancée.
    let card_free = !summary.cancelled
        && summary.failed_files == 0
        && !spec.destinations.is_empty()
        && mhl.iter().all(Option::is_some)
        && (roots2.is_empty() || cascade_possible);
    let info = ReportInfo {
        project: project
            .map(str::to_owned)
            .or_else(|| req.vars.projet.clone()),
        operator: req.operator.clone(),
        notes: req.notes.clone(),
    };
    let planned = CascadeInfo::Stage1 {
        planned: &roots2,
        card_free,
    };
    let reports = write_reports(&ReportData {
        inv: &inv,
        summary: &summary,
        spec: &spec,
        info: &info,
        mhl: &mhl,
        cascade: (!roots2.is_empty()).then_some(&planned),
    })?;
    let pending = cascade_possible.then(|| {
        PendingCascade::new(
            &inv,
            sound,
            roots2.clone(),
            spec.algorithms.clone(),
            &info,
            pre.date.clone(),
            Stage1Summary::of(&summary, &spec.destinations, &reports),
        )
    });
    Ok(JobResult {
        source_name: inv.name.clone(),
        fingerprint: inv.fingerprint.clone(),
        roots: spec.destinations,
        summary,
        mhl,
        mhl_errors,
        reports,
        card_free,
        cascade: pending,
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
    fn overlap_is_refused_when_a_destination_is_a_hard_disk() {
        let roots = [PathBuf::from("/ssd/A001"), PathBuf::from("/hdd/A001")];
        assert!(overlap_allowed(&roots, |_| false));
        assert!(!overlap_allowed(&roots, |p| p.starts_with("/hdd")));
        assert!(overlap_allowed(&[], |_| true));
    }

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
            existing: ExistingMode::Verify,
            date: None,
            roots: None,
            history: Vec::new(),
            cascade: Vec::new(),
            cascade_auto: false,
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
            None,
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

    /// Carte copiée lundi, réutilisée mardi sans formatage : les plans du
    /// lundi sont retrouvés dans le dossier de la veille, même si le dossier
    /// du jour porte un autre nom (retour d'Hervé du 31/10/2026).
    #[test]
    fn reused_card_found_in_previous_copy() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        std::fs::create_dir_all(src.join("CLIP")).unwrap();
        std::fs::write(src.join("CLIP/C0001.MP4"), b"lundi").unwrap();
        let request = |jour: &str, history: Vec<PathBuf>| OffloadRequest {
            source: src.clone(),
            destinations: vec![dir.path().join("SSD1")],
            template: "{jour}_{carte}".into(),
            vars: TemplateVars {
                jour: Some(jour.into()),
                ..Default::default()
            },
            algorithms: vec![HashAlgo::Xxh128],
            operator: None,
            notes: None,
            existing: ExistingMode::Verify,
            date: None,
            roots: None,
            history,
            cascade: Vec::new(),
            cascade_auto: false,
        };
        let monday = request("J01", Vec::new());
        let pre = preflight(&monday).unwrap();
        let monday_root = pre.roots[0].clone();
        execute(&monday, pre, &AtomicBool::new(false), |_| {}, None, None).unwrap();

        // Mardi : nouveau plan sur la même carte, dossier du jour « J02_A001 ».
        std::fs::write(src.join("CLIP/C0002.MP4"), b"mardi").unwrap();
        let without = preflight(&request("J02", Vec::new())).unwrap();
        assert!(
            without.elsewhere[0].is_empty(),
            "nom différent : rien sans l'historique"
        );
        let tuesday = preflight(&request("J02", vec![monday_root.clone()])).unwrap();
        assert_eq!(tuesday.present[0].files, 0);
        let found = &tuesday.elsewhere[0];
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].root, monday_root);
        assert_eq!(found[0].present.files, 1, "le plan du lundi est retrouvé");
    }

    fn request(src: &Path, dest: &Path) -> OffloadRequest {
        OffloadRequest {
            source: src.to_path_buf(),
            destinations: vec![dest.to_path_buf()],
            template: "{jour}/{carte}".into(),
            vars: TemplateVars {
                jour: Some("J01".into()),
                ..Default::default()
            },
            algorithms: vec![HashAlgo::Xxh128],
            operator: None,
            notes: None,
            existing: ExistingMode::Verify,
            date: None,
            roots: None,
            history: Vec::new(),
            cascade: Vec::new(),
            cascade_auto: false,
        }
    }

    #[test]
    fn planned_roots_matches_preflight() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::write(src.join("a.mp4"), b"x").unwrap();
        let mut req = request(&src, &dir.path().join("SSD1"));
        req.destinations.push(dir.path().join("SSD2"));
        assert_eq!(planned_roots(&req), preflight(&req).unwrap().roots);
        // Dossiers imposés (reprise).
        req.roots = Some(vec![dir.path().join("X/a"), dir.path().join("Y/b")]);
        assert_eq!(planned_roots(&req), preflight(&req).unwrap().roots);
        assert_eq!(planned_roots(&req)[0], dir.path().join("X/a"));
    }

    /// Deux cartes copiées en même temps vers la même destination de base.
    #[test]
    fn two_executes_in_parallel_to_same_disk() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("SSD");
        let reqs: Vec<OffloadRequest> = ["A001", "B001"]
            .iter()
            .map(|n| {
                let src = dir.path().join(n);
                std::fs::create_dir_all(src.join("CLIP")).unwrap();
                std::fs::write(src.join("CLIP/C1.MP4"), n.as_bytes().repeat(1000)).unwrap();
                request(&src, &dest)
            })
            .collect();
        let results: Vec<JobResult> = std::thread::scope(|sc| {
            let hs: Vec<_> = reqs
                .iter()
                .map(|req| {
                    sc.spawn(move || {
                        let pre = preflight(req).unwrap();
                        execute(req, pre, &AtomicBool::new(false), |_| {}, None, None).unwrap()
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        for (res, name) in results.iter().zip(["A001", "B001"]) {
            assert_eq!(res.summary.failed_files, 0);
            assert_eq!(res.roots[0], dest.join("J01").join(name));
            let mhl = res.mhl[0].as_ref().expect("MHL écrit");
            assert!(mhl
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("0001_"));
            let chain =
                std::fs::read_to_string(res.roots[0].join("ascmhl/ascmhl_chain.xml")).unwrap();
            assert_eq!(chain.matches("<hashlist").count(), 1);
        }
    }
}
