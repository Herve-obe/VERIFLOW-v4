//! Copie en cascade : étape 2 d'un offload (charte §7.1).
//!
//! L'étape 1 copie et vérifie la carte vers les destinations immédiates (SSD),
//! puis la déclare libérable. L'étape 2, plus tard, recopie la copie vérifiée
//! de l'étape 1 vers les destinations lentes (disque dur, NAS). Elle ne relit
//! jamais la carte : la source est un dossier de l'étape 1, contrôlé contre les
//! empreintes de son historique ASC MHL, dont elle prolonge la chaîne.

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use super::engine::{
    check_space, run, DestStatus, Event, FileResult, Hashes, OffloadSpec, OffloadSummary,
};
use super::hash::{hash_data, HashAlgo};
use super::job::{overlap_allowed, JobResult, MhlFailure};
use super::mhl::{write_cascade_generation, Action, GenEntry, MhlAuthor, REPORTS_DIR};
use super::mhl_read::{load_history, nfc, read_chain, to_path, History, MHL_DIR};
use super::report::{write_reports, CascadeInfo, ReportData, ReportFiles, ReportInfo};
use super::scan::{scan, SourceInventory};
use crate::{Error, Result};

/// Message d'un fichier de l'historique introuvable dans la copie de l'étape 1.
const MISSING_IN_STAGE1: &str = "absent de la copie vérifiée (étape 1)";

/// Ce que l'étape 1 a produit, repris dans le rapport de l'étape 2.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage1Summary {
    pub started_at: String,
    pub finished_at: String,
    pub duration_s: f64,
    pub total_bytes: u64,
    pub files: usize,
    /// Débit global de l'étape 1, en octets par seconde.
    pub effective_rate: Option<f64>,
    /// Facteur limitant de l'étape 1 (`source`, `dest:<i>`, `cpu` ou vide).
    pub bottleneck: String,
    /// Dossiers de l'étape 1 copiés.
    pub destinations: Vec<PathBuf>,
    /// Noms des fichiers de rapport de l'étape 1 (recopiés en étape 2).
    #[serde(default)]
    pub reports: Vec<String>,
}

impl Stage1Summary {
    pub(super) fn of(
        summary: &OffloadSummary,
        destinations: &[PathBuf],
        reports: &[ReportFiles],
    ) -> Stage1Summary {
        let mut names: Vec<String> = Vec::new();
        for r in reports {
            let pdf = file_name(&r.pdf);
            let all = [
                Some(file_name(&r.csv)),
                r.rates_csv.as_deref().map(file_name),
                Some(file_name(&r.html)),
                Some(format!("{pdf}.sha256")),
                Some(pdf),
            ];
            for n in all.into_iter().flatten() {
                if !names.contains(&n) {
                    names.push(n);
                }
            }
        }
        Stage1Summary {
            started_at: summary.started_at.clone(),
            finished_at: summary.finished_at.clone(),
            duration_s: summary.duration_s,
            total_bytes: summary.total_bytes,
            files: summary.files.len(),
            effective_rate: summary.throughput.effective_rate,
            bottleneck: summary.throughput.bottleneck.clone(),
            destinations: destinations.to_vec(),
            reports: names,
        }
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Étape 2 en attente : tout ce qu'il faut pour la lancer plus tard, y compris
/// après un redémarrage (conservée dans un fichier JSON par l'application).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingCascade {
    /// Identifiant stable : empreinte de la carte et destinations de l'étape 2.
    pub key: String,
    /// Nom de la carte d'origine.
    pub card: String,
    /// Chemin de la carte d'origine.
    pub source: PathBuf,
    /// Empreinte de l'inventaire de la carte (identifie la carte copiée).
    pub fingerprint: String,
    /// Dossiers de l'étape 1 sains (copie vérifiée et historique écrit).
    pub stage1_roots: Vec<PathBuf>,
    /// Dossiers de l'étape 2.
    pub stage2_roots: Vec<PathBuf>,
    pub algorithms: Vec<HashAlgo>,
    pub operator: Option<String>,
    pub notes: Option<String>,
    pub project: Option<String>,
    /// Date de l'étape 1 (AAAA-MM-JJ).
    pub date: String,
    pub stage1: Stage1Summary,
}

impl PendingCascade {
    pub(super) fn new(
        inv: &SourceInventory,
        stage1_roots: Vec<PathBuf>,
        stage2_roots: Vec<PathBuf>,
        algorithms: Vec<HashAlgo>,
        info: &ReportInfo,
        date: String,
        stage1: Stage1Summary,
    ) -> PendingCascade {
        let seed = std::iter::once(inv.fingerprint.clone())
            .chain(stage2_roots.iter().map(|r| r.display().to_string()))
            .collect::<Vec<_>>()
            .join("\n");
        PendingCascade {
            key: hash_data(HashAlgo::Xxh128, seed.as_bytes()),
            card: inv.name.clone(),
            source: inv.root.clone(),
            fingerprint: inv.fingerprint.clone(),
            stage1_roots,
            stage2_roots,
            algorithms,
            operator: info.operator.clone(),
            notes: info.notes.clone(),
            project: info.project.clone(),
            date,
            stage1,
        }
    }
}

/// Erreur de cascade : le code stable ouvre le message, le pont s'en sert
/// pour la traduction.
fn cascade_err(code: &str, detail: impl std::fmt::Display) -> Error {
    Error::Report(format!("{code} : {detail}"))
}

/// Code stable d'une erreur de cascade (`cascade_history`, `cascade_diverged`,
/// `cascade_no_source`, `cascade_no_immediate`).
pub fn error_code(e: &Error) -> Option<&'static str> {
    match e {
        Error::Report(m) if m.starts_with("cascade_no_source : ") => Some("cascade_no_source"),
        Error::Report(m) if m.starts_with("no_space : ") => Some("no_space"),
        Error::Unsupported(m) if m.starts_with("cascade_no_immediate : ") => {
            Some("cascade_no_immediate")
        }
        _ => super::mhl::cascade_error_code(e),
    }
}

/// Refus si une destination de l'étape 2 manque d'espace (`no_space`).
fn space_error(missing: &[Option<u64>]) -> Result<()> {
    if missing.iter().any(Option::is_some) {
        return Err(cascade_err(
            "no_space",
            "espace insuffisant sur une destination de l'étape 2",
        ));
    }
    Ok(())
}

/// Message d'une erreur pour l'interface : pour une erreur à code stable
/// (`cascade_xxx`, `no_space`), le texte nu « code : détail », sans le préfixe
/// de la variante (« média non pris en charge : », « rapport : »).
pub fn message(e: &Error) -> String {
    match (error_code(e), e) {
        (Some(_), Error::Report(m) | Error::Unsupported(m)) => m.clone(),
        _ => e.to_string(),
    }
}

/// Historique intact : chaîne lisible, non vide, sans altération.
fn intact(root: &Path) -> bool {
    read_chain(root).is_ok_and(|c| !c.entries.is_empty() && c.issues.is_empty())
}

/// Première racine de l'étape 1 dont l'historique est intact.
pub fn pick_source(roots: &[PathBuf]) -> Option<&Path> {
    roots.iter().map(PathBuf::as_path).find(|r| intact(r))
}

/// Fichier de l'historique introuvable dans la copie de l'étape 1.
#[derive(Debug, Clone)]
pub struct MissingFile {
    pub rel: String,
    pub size: u64,
    pub hashes: Hashes,
}

/// Inventaire de l'étape 2 : la copie de l'étape 1, limitée à son historique.
pub struct StageInventory {
    pub inv: SourceInventory,
    pub missing: Vec<MissingFile>,
}

fn in_reports_dir(rel: &str) -> bool {
    rel == REPORTS_DIR || rel.starts_with(&format!("{REPORTS_DIR}/"))
}

/// Parcourt la copie de l'étape 1 en ne gardant que les fichiers de son
/// historique (sans `_VERIFLOW`, sans fichier ajouté depuis). Un fichier de
/// l'historique absent du disque est rendu dans `missing`.
pub fn inventory(stage1_root: &Path, history: &History, card: &str) -> Result<StageInventory> {
    let scanned = scan(stage1_root)?;
    let files: Vec<_> = scanned
        .files
        .into_iter()
        .filter(|f| !in_reports_dir(&f.rel) && history.files.contains_key(&nfc(&f.rel)))
        .collect();
    let present: HashSet<String> = files.iter().map(|f| nfc(&f.rel)).collect();
    let mut missing: Vec<MissingFile> = history
        .files
        .iter()
        .filter(|(rel, _)| !present.contains(*rel) && !in_reports_dir(rel))
        .map(|(rel, (size, hashes))| MissingFile {
            rel: rel.clone(),
            size: size.unwrap_or(0),
            hashes: hashes.clone(),
        })
        .collect();
    missing.sort_by(|a, b| a.rel.cmp(&b.rel));
    let dirs = scanned
        .dirs
        .into_iter()
        .filter(|d| !in_reports_dir(&d.rel))
        .collect();
    Ok(StageInventory {
        inv: SourceInventory {
            root: scanned.root,
            name: card.to_owned(),
            total_bytes: files.iter().map(|f| f.size).sum(),
            fingerprint: scanned.fingerprint,
            files,
            dirs,
        },
        missing,
    })
}

/// Copie `src` vers `dst` puis relit les deux : la copie est retirée si elle
/// diffère. Un fichier déjà présent n'est jamais écrasé : il compte comme
/// conforme seulement s'il est identique. Vrai si `dst` est identique à `src`.
fn copy_identical(src: &Path, dst: &Path) -> bool {
    let attempt = || -> std::io::Result<bool> {
        let bytes = fs::read(src)?;
        if dst.exists() {
            return Ok(fs::read(dst)? == bytes);
        }
        let tmp = dst.with_extension("vfpart");
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, dst)?;
        let same = fs::read(dst)? == bytes;
        if !same {
            let _ = fs::remove_file(dst);
        }
        Ok(same)
    };
    attempt().unwrap_or(false)
}

/// Rapports de cette étape 1 (`Stage1Summary::reports`), copiés à l'identique
/// dans `_VERIFLOW/` de chaque destination de l'étape 2 puis comparés.
/// Renvoie, par destination, les fichiers conformes sur le total.
fn copy_stage1_reports(source: &Path, dests: &[PathBuf], names: &[String]) -> Vec<(usize, usize)> {
    let names: Vec<&String> = names
        .iter()
        .filter(|n| source.join(REPORTS_DIR).join(n).is_file())
        .collect();
    dests
        .iter()
        .map(|dest| {
            let dir = dest.join(REPORTS_DIR);
            if names.is_empty() || fs::create_dir_all(&dir).is_err() {
                return (0, names.len());
            }
            let ok = names
                .iter()
                .filter(|n| copy_identical(&source.join(REPORTS_DIR).join(n), &dir.join(n)))
                .count();
            (ok, names.len())
        })
        .collect()
}

/// Exécute l'étape 2 : copie de la copie vérifiée de l'étape 1 vers chaque
/// destination en cascade, contrôlée contre l'historique de l'étape 1, puis
/// prolongement de cet historique et rapports. Une étape 2 interrompue se
/// reprend en relançant cette fonction : les fichiers déjà copiés sont
/// revérifiés sans être recopiés.
pub fn execute(
    pending: &PendingCascade,
    cancel: &AtomicBool,
    emit: impl FnMut(Event),
) -> Result<JobResult> {
    // Pas de mise en veille (ni d'App Nap sous macOS) pendant le travail.
    let _awake = crate::power::keep_awake();
    let source = pick_source(&pending.stage1_roots)
        .ok_or_else(|| {
            cascade_err(
                "cascade_no_source",
                "aucune copie de l'étape 1 avec un historique intact",
            )
        })?
        .to_path_buf();
    let history = load_history(&source).map_err(|e| cascade_err("cascade_history", e))?;
    let StageInventory { mut inv, missing } = inventory(&source, &history, &pending.card)?;
    inv.fingerprint = pending.fingerprint.clone();
    let expected: HashMap<String, Hashes> = history
        .files
        .iter()
        .map(|(rel, (_, hashes))| (rel.clone(), hashes.clone()))
        .collect();
    let spec = OffloadSpec {
        destinations: pending.stage2_roots.clone(),
        algorithms: if pending.algorithms.is_empty() {
            vec![HashAlgo::Xxh128]
        } else {
            pending.algorithms.clone()
        },
        known: None,
        overlap: overlap_allowed(&pending.stage2_roots, super::storage::is_hdd),
        expected: Some(expected),
    };
    // L'espace a pu changer depuis l'étape 1 : recontrôlé avant d'écrire.
    space_error(&check_space(&inv, &spec.destinations))?;
    let mut summary = run(&inv, &spec, cancel, emit)?;
    let n = spec.destinations.len();
    if !summary.cancelled {
        for m in &missing {
            summary.files.push(FileResult {
                rel: m.rel.clone(),
                size: m.size,
                modified: String::new(),
                hashes: m.hashes.clone(),
                destinations: vec![DestStatus::Failed(MISSING_IN_STAGE1.into()); n],
            });
        }
        summary.failed_files += missing.len();
    }

    let author = MhlAuthor {
        name: pending.operator.clone(),
        ..Default::default()
    };
    let mut mhl: Vec<Option<PathBuf>> = vec![None; n];
    let mut mhl_errors: Vec<Option<MhlFailure>> = vec![None; n];
    if !summary.cancelled {
        let dirs: Vec<&str> = inv.dirs.iter().map(|d| d.rel.as_str()).collect();
        for (i, dest) in spec.destinations.iter().enumerate() {
            let entries: Vec<GenEntry> = summary
                .files
                .iter()
                .filter(|f| {
                    matches!(
                        f.destinations[i],
                        DestStatus::Verified | DestStatus::ResumedVerified
                    )
                })
                .map(|f| GenEntry {
                    rel: f.rel.clone(),
                    size: f.size,
                    hashes: f.hashes.clone(),
                    action: Action::Verified,
                })
                .collect();
            match write_cascade_generation(&source, dest, &entries, &dirs, &author) {
                Ok(p) => mhl[i] = Some(p),
                Err(e) => mhl_errors[i] = Some(MhlFailure::of(&e)),
            }
        }
    }

    let reports_copied = copy_stage1_reports(&source, &spec.destinations, &pending.stage1.reports);
    let reference = read_chain(&source)
        .ok()
        .and_then(|c| c.entries.last().cloned())
        .and_then(|e| {
            let path = to_path(&source.join(MHL_DIR), &e.file);
            let sha = hash_data(HashAlgo::Sha256, &fs::read(&path).ok()?);
            Some((path, sha))
        });
    let info = ReportInfo {
        project: pending.project.clone(),
        operator: pending.operator.clone(),
        notes: pending.notes.clone(),
    };
    let context = CascadeInfo::Stage2 {
        stage1: &pending.stage1,
        source: &source,
        reference,
        reports_copied: &reports_copied,
    };
    let reports = write_reports(&ReportData {
        inv: &inv,
        summary: &summary,
        spec: &spec,
        info: &info,
        mhl: &mhl,
        cascade: Some(&context),
    })?;
    Ok(JobResult {
        source_name: pending.card.clone(),
        fingerprint: pending.fingerprint.clone(),
        roots: spec.destinations,
        summary,
        mhl,
        mhl_errors,
        reports,
        card_free: false,
        cascade: None,
    })
}

// ---------------------------------------------------------------------------
// Étapes 2 en attente : fichier JSON tenu par l'application.
// ---------------------------------------------------------------------------

/// Les lectures-modifications-écritures du fichier ne se croisent pas.
static STORE_LOCK: Mutex<()> = Mutex::new(());

fn read_store(path: &Path) -> Result<Vec<PendingCascade>> {
    match fs::read(path) {
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map_err(|e| Error::Report(format!("étapes 2 en attente illisibles : {e}"))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e.into()),
    }
}

/// Écriture atomique : fichier temporaire, fsync, renommage.
fn write_store(path: &Path, list: &[PendingCascade]) -> Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let json = serde_json::to_vec_pretty(list)
        .map_err(|e| Error::Report(format!("étapes 2 en attente : {e}")))?;
    let tmp = path.with_extension("json.vfpart");
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(&json)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Étapes 2 en attente (liste vide si le fichier n'existe pas encore).
pub fn load(path: &Path) -> Result<Vec<PendingCascade>> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    read_store(path)
}

/// Inscrit une étape 2 en attente (remplace celle de même clé). Un fichier
/// dont le JSON est illisible est mis de côté sous un nom horodaté
/// (`.corrompu-<date>`, jamais écrasé) ; toute erreur d'E/S est propagée.
pub fn save(path: &Path, pending: &PendingCascade) -> Result<()> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list: Vec<PendingCascade> = match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(&bytes) {
            Ok(list) => list,
            Err(_) => {
                set_aside(path)?;
                Vec::new()
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    list.retain(|p| p.key != pending.key);
    list.push(pending.clone());
    write_store(path, &list)
}

/// Renomme un fichier illisible en `<nom>.corrompu-<date>` (numéroté si ce nom existe).
fn set_aside(path: &Path) -> Result<()> {
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    for n in 0.. {
        let suffix = if n == 0 {
            String::new()
        } else {
            format!("-{n}")
        };
        let target = path.with_file_name(format!("{name}.corrompu-{stamp}{suffix}"));
        if !target.exists() {
            fs::rename(path, target)?;
            break;
        }
    }
    Ok(())
}

/// Retire une étape 2 en attente. Faux si la clé n'y était pas.
pub fn remove(path: &Path, key: &str) -> Result<bool> {
    let _guard = STORE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut list = read_store(path)?;
    let before = list.len();
    list.retain(|p| p.key != key);
    if list.len() == before {
        return Ok(false);
    }
    write_store(path, &list)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::job::{execute as execute_stage1, preflight, OffloadRequest, TemplateVars};
    use crate::offload::mhl_verify::verify_root;
    use std::sync::atomic::Ordering;

    fn card(dir: &Path) -> PathBuf {
        let root = dir.join("A001");
        fs::create_dir_all(root.join("CLIP")).unwrap();
        for i in 0..6u8 {
            let data: Vec<u8> = (0..(40_000 + i as usize * 7))
                .map(|k| (k as u8) ^ i)
                .collect();
            fs::write(root.join(format!("CLIP/C000{i}.MP4")), data).unwrap();
        }
        fs::write(root.join("notes.txt"), b"plan de tournage").unwrap();
        root
    }

    fn request(src: &Path, ssd: &[PathBuf], hdd: &[PathBuf]) -> OffloadRequest {
        OffloadRequest {
            source: src.to_path_buf(),
            destinations: ssd.to_vec(),
            template: "{carte}".into(),
            vars: TemplateVars::default(),
            algorithms: vec![HashAlgo::Xxh128, HashAlgo::Md5],
            operator: Some("DIT".into()),
            notes: None,
            existing: Default::default(),
            date: Some("2026-11-05".into()),
            roots: None,
            history: Vec::new(),
            cascade: hdd.to_vec(),
            cascade_auto: false,
        }
    }

    /// Étape 1 vers un SSD, étape 2 prévue vers un disque dur.
    fn stage1(dir: &Path) -> (PathBuf, PathBuf, PathBuf, JobResult) {
        let src = card(dir);
        let (ssd, hdd) = (dir.join("SSD"), dir.join("HDD"));
        let req = request(&src, std::slice::from_ref(&ssd), std::slice::from_ref(&hdd));
        let pre = preflight(&req).unwrap();
        let res = execute_stage1(
            &req,
            pre,
            &AtomicBool::new(false),
            |_| {},
            Some("Projet"),
            None,
        )
        .unwrap();
        (src, ssd.join("A001"), hdd.join("A001"), res)
    }

    /// Contenu complet d'un dossier : chemin relatif, octets.
    fn snapshot(root: &Path) -> Vec<(String, Vec<u8>)> {
        let mut out: Vec<(String, Vec<u8>)> = walkdir::WalkDir::new(root)
            .into_iter()
            .flatten()
            .filter(|e| e.file_type().is_file())
            .map(|e| {
                let rel = e.path().strip_prefix(root).unwrap().display().to_string();
                (rel, fs::read(e.path()).unwrap())
            })
            .collect();
        out.sort();
        out
    }

    fn no_cancel() -> AtomicBool {
        AtomicBool::new(false)
    }

    #[test]
    fn cascade_full_chain_validates_with_verify_root() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        assert!(res.card_free);
        assert_eq!(res.roots, vec![ssd.clone()]);
        assert!(!hdd.exists(), "l'étape 1 ne touche pas l'étape 2");
        let pending = res.cascade.expect("étape 2 à faire");
        assert_eq!(pending.stage2_roots, vec![hdd.clone()]);
        assert_eq!(pending.stage1_roots, vec![ssd.clone()]);

        let out = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert_eq!(out.summary.failed_files, 0);
        assert!(!out.summary.cancelled);
        assert!(out.mhl[0].is_some());
        assert!(!out.card_free && out.cascade.is_none());
        let chain = read_chain(&hdd).unwrap();
        assert_eq!(
            chain.entries.len(),
            2,
            "génération de l'étape 1 puis transfer"
        );
        assert!(chain.issues.is_empty());

        for root in [&ssd, &hdd] {
            let v = verify_root(root, &no_cancel(), |_| {}).unwrap();
            assert!(v.conform, "{root:?} non conforme : {:?}", v.files);
            assert_eq!(v.ok, 7);
        }
    }

    #[test]
    fn cascade_never_reads_card() {
        let dir = tempfile::tempdir().unwrap();
        let (src, _, hdd, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        fs::remove_dir_all(&src).unwrap();
        let out = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert_eq!(out.summary.failed_files, 0);
        assert_eq!(out.summary.files.len(), 7);
        assert!(hdd.join("CLIP/C0003.MP4").is_file());
    }

    #[test]
    fn cascade_resume_after_cancel_reverifies_without_recopy() {
        let dir = tempfile::tempdir().unwrap();
        let (_, _, hdd, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        let cancel = AtomicBool::new(false);
        let first = execute(&pending, &cancel, |e| {
            if let Event::FileDone { .. } = e {
                cancel.store(true, Ordering::SeqCst);
            }
        })
        .unwrap();
        assert!(first.summary.cancelled);
        assert!(first.mhl.iter().all(Option::is_none));
        assert!(
            !hdd.join("ascmhl").exists(),
            "pas d'historique avant la fin"
        );

        let second = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert_eq!(second.summary.failed_files, 0);
        let resumed = second
            .summary
            .files
            .iter()
            .filter(|f| f.destinations[0] == DestStatus::ResumedVerified)
            .count();
        assert!(resumed >= 1, "les fichiers déjà copiés sont revérifiés");
        let t = &second.summary.throughput;
        assert!(
            t.write[0].bytes < second.summary.total_bytes,
            "pas de recopie"
        );
        assert!(second.mhl[0].is_some());
        let v = verify_root(&hdd, &no_cancel(), |_| {}).unwrap();
        assert!(v.conform);
    }

    #[test]
    fn cascade_stage2_failure_leaves_stage1_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let ssd = dir.path().join("SSD");
        // Le « disque » de l'étape 2 est un fichier : rien n'y est écrivable.
        let blocked = dir.path().join("HDD");
        fs::write(&blocked, b"pas un dossier").unwrap();
        let req = request(
            &src,
            std::slice::from_ref(&ssd),
            std::slice::from_ref(&blocked),
        );
        let pre = preflight(&req).unwrap();
        let res = execute_stage1(&req, pre, &no_cancel(), |_| {}, None, None).unwrap();
        let pending = res.cascade.unwrap();
        let before = snapshot(&ssd);
        let out = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert!(out.summary.failed_files > 0);
        assert!(out.mhl[0].is_none());
        assert!(out.mhl_errors[0].is_some());
        assert_eq!(snapshot(&ssd), before);
    }

    #[test]
    fn cascade_modified_stage1_copy_is_failed() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        // Un octet change, la taille reste la même.
        let target = ssd.join("CLIP/C0002.MP4");
        let mut bytes = fs::read(&target).unwrap();
        bytes[100] ^= 0xFF;
        fs::write(&target, bytes).unwrap();

        let out = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert_eq!(out.summary.failed_files, 1);
        let bad = out
            .summary
            .files
            .iter()
            .find(|f| f.rel == "CLIP/C0002.MP4")
            .unwrap();
        assert_eq!(
            bad.destinations[0],
            DestStatus::Failed(
                "source de l'étape 2 différente de la copie vérifiée de l'étape 1".into()
            )
        );
        assert!(!hdd.join("CLIP/C0002.MP4").exists());
        assert!(!hdd.join("CLIP/.C0002.MP4.vfpart").exists());
        // La génération de l'étape 2 n'inscrit pas le fichier en échec.
        let v = verify_root(&hdd, &no_cancel(), |_| {}).unwrap();
        assert_eq!(v.missing, 1);
        assert_eq!(v.different, 0);
    }

    #[test]
    fn cascade_missing_file_in_stage1_copy_failed() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        fs::remove_file(ssd.join("CLIP/C0004.MP4")).unwrap();
        let out = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert_eq!(out.summary.failed_files, 1);
        let gone = out
            .summary
            .files
            .iter()
            .find(|f| f.rel == "CLIP/C0004.MP4")
            .unwrap();
        assert_eq!(
            gone.destinations[0],
            DestStatus::Failed(MISSING_IN_STAGE1.into())
        );
        assert!(gone.size > 0 && !gone.hashes.is_empty());
        assert!(!hdd.join("CLIP/C0004.MP4").exists());
    }

    #[test]
    fn cascade_without_intact_history_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        // Chaîne illisible : jamais de copie sans historique de référence.
        fs::remove_file(ssd.join("ascmhl/ascmhl_chain.xml")).unwrap();
        assert!(pick_source(&pending.stage1_roots).is_none());
        let err = execute(&pending, &no_cancel(), |_| {}).unwrap_err();
        assert_eq!(error_code(&err), Some("cascade_no_source"));
        assert!(!hdd.exists());
    }

    #[test]
    fn pick_source_skips_roots_with_broken_history() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, _, _) = stage1(dir.path());
        let none = dir.path().join("vide");
        fs::create_dir_all(&none).unwrap();
        let roots = vec![none, ssd.clone()];
        assert_eq!(pick_source(&roots), Some(ssd.as_path()));
        assert!(pick_source(&roots[..1]).is_none());
    }

    #[test]
    fn inventory_is_limited_to_history() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, _, _) = stage1(dir.path());
        assert!(ssd.join("_VERIFLOW").is_dir());
        fs::write(ssd.join("ajoute_apres.txt"), b"hors historique").unwrap();
        let history = load_history(&ssd).unwrap();
        let StageInventory { inv, missing } = inventory(&ssd, &history, "A001").unwrap();
        let rels: Vec<&str> = inv.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels.len(), 7);
        assert!(!rels.contains(&"ajoute_apres.txt"));
        assert!(rels.iter().all(|r| !r.starts_with("_VERIFLOW")));
        assert!(inv.dirs.iter().all(|d| !d.rel.starts_with("_VERIFLOW")));
        assert_eq!(inv.name, "A001");
        assert!(missing.is_empty());
    }

    #[test]
    fn card_free_only_when_all_stage1_verified() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let good = dir.path().join("SSD1");
        let blocked = dir.path().join("SSD2");
        fs::write(&blocked, b"pas un dossier").unwrap();
        let hdd = dir.path().join("HDD");

        // Tout vérifié : carte libérable.
        let req = request(
            &src,
            std::slice::from_ref(&good),
            std::slice::from_ref(&hdd),
        );
        let pre = preflight(&req).unwrap();
        let ok = execute_stage1(&req, pre, &no_cancel(), |_| {}, None, None).unwrap();
        assert!(ok.card_free);

        // Une destination de l'étape 1 en échec : non libérable, mais l'étape 2
        // reste possible depuis la copie saine.
        let req = request(&src, &[good.clone(), blocked], std::slice::from_ref(&hdd));
        let pre = preflight(&req).unwrap();
        let partial = execute_stage1(&req, pre, &no_cancel(), |_| {}, None, None).unwrap();
        assert!(!partial.card_free);
        assert!(partial.summary.failed_files > 0);
        let pending = partial.cascade.expect("copie saine disponible");
        assert_eq!(pending.stage1_roots, vec![good.join("A001")]);

        // Annulée : non libérable, pas d'étape 2.
        let req = request(&src, &[dir.path().join("SSD3")], &[hdd]);
        let pre = preflight(&req).unwrap();
        let cancelled =
            execute_stage1(&req, pre, &AtomicBool::new(true), |_| {}, None, None).unwrap();
        assert!(cancelled.summary.cancelled);
        assert!(!cancelled.card_free);
        assert!(cancelled.cascade.is_none());

        // Sans cascade : carte libérable, aucune étape 2.
        let req = request(&src, &[dir.path().join("SSD4")], &[]);
        let pre = preflight(&req).unwrap();
        let plain = execute_stage1(&req, pre, &no_cancel(), |_| {}, None, None).unwrap();
        assert!(plain.card_free && plain.cascade.is_none());
    }

    #[test]
    fn auto_split_uses_slow_detection() {
        use crate::offload::job::split_cascade;
        let dirs: Vec<PathBuf> = ["/ssd", "/hdd", "/nas"].map(PathBuf::from).to_vec();
        let slow = |p: &Path| p.starts_with("/hdd") || p.starts_with("/nas");
        let mut req = request(Path::new("/carte"), &dirs, &[]);
        // Sans automatisme : tout en étape 1.
        assert_eq!(split_cascade(&req, slow).unwrap(), vec![1, 1, 1]);
        req.cascade_auto = true;
        assert_eq!(split_cascade(&req, slow).unwrap(), vec![1, 2, 2]);
        // Tous lents ou tous rapides : tout reste immédiat.
        assert_eq!(split_cascade(&req, |_| true).unwrap(), vec![1, 1, 1]);
        assert_eq!(split_cascade(&req, |_| false).unwrap(), vec![1, 1, 1]);
        // Destinations en cascade explicites, toujours en étape 2.
        req.cascade = vec![PathBuf::from("/archive")];
        assert_eq!(split_cascade(&req, slow).unwrap(), vec![1, 2, 2, 2]);
        // Aucune destination immédiate : refusé.
        req.destinations.clear();
        let err = split_cascade(&req, slow).unwrap_err();
        assert_eq!(error_code(&err), Some("cascade_no_immediate"));
        req.cascade.clear();
        assert!(split_cascade(&req, slow).unwrap().is_empty());
    }

    #[test]
    fn preflight_checks_space_on_cascade_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        // Fichier creux énorme : rien n'est écrit, mais l'espace requis dépasse le libre.
        let big = fs::File::create(src.join("CLIP/enorme.MP4")).unwrap();
        if big.set_len(1 << 52).is_err() {
            return;
        }
        let req = request(&src, &[dir.path().join("SSD")], &[dir.path().join("HDD")]);
        let pre = crate::offload::job::preflight(&req).unwrap();
        assert_eq!(pre.stages, vec![1, 2]);
        assert_eq!(pre.roots.len(), 2);
        assert_eq!(pre.missing_space.len(), 2);
        assert_eq!(pre.present.len(), 2);
        assert_eq!(pre.already_in_destination.len(), 2);
        assert_eq!(pre.elsewhere.len(), 2);
        assert!(pre.roots[1].ends_with("HDD/A001"));
        // L'étape 2 ne bloque pas la copie de la carte : simple avertissement.
        assert!(
            pre.cascade_space_warning[1].is_some(),
            "espace de l'étape 2 contrôlé"
        );
        assert_eq!(pre.cascade_space_warning[0], None);
        assert!(
            pre.missing_space[0].is_some(),
            "espace de l'étape 1 contrôlé"
        );
        assert_eq!(pre.missing_space[1], None);
    }

    #[test]
    fn pending_store_roundtrip_and_remove() {
        let dir = tempfile::tempdir().unwrap();
        let (_, _, _, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        let store = dir.path().join("data/offload_cascades.json");
        assert!(load(&store).unwrap().is_empty());
        save(&store, &pending).unwrap();
        save(&store, &pending).unwrap(); // même clé : remplacée
        let mut other = pending.clone();
        other.key = "autre".into();
        save(&store, &other).unwrap();
        let list = load(&store).unwrap();
        assert_eq!(list.len(), 2);
        let back = list.iter().find(|p| p.key == pending.key).unwrap();
        assert_eq!(back.card, "A001");
        assert_eq!(back.stage2_roots, pending.stage2_roots);
        assert_eq!(back.algorithms, pending.algorithms);
        assert_eq!(back.stage1.files, 7);
        assert!(remove(&store, &pending.key).unwrap());
        assert!(!remove(&store, &pending.key).unwrap());
        assert_eq!(load(&store).unwrap().len(), 1);
        // Fichier illisible : signalé à la lecture, mis de côté à l'inscription,
        // sous un nom qui n'en écrase jamais un autre.
        let aside = |dir: &Path| {
            fs::read_dir(dir)
                .unwrap()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().contains(".corrompu-"))
                .count()
        };
        for round in 1..=2 {
            fs::write(&store, b"{ pas du json").unwrap();
            assert!(load(&store).is_err());
            save(&store, &pending).unwrap();
            assert_eq!(load(&store).unwrap().len(), 1);
            assert_eq!(aside(store.parent().unwrap()), round);
        }
        // Erreur d'E/S (ici : le chemin est un dossier) : propagée, rien de déplacé.
        let blocked = dir.path().join("data/dossier.json");
        fs::create_dir_all(&blocked).unwrap();
        assert!(save(&blocked, &pending).is_err());
        assert!(blocked.is_dir());
    }

    #[test]
    fn cascade_report_lists_both_stages() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        let html1 = fs::read_to_string(&res.reports[0].html).unwrap();
        assert!(html1.contains("Cascade"));
        assert!(html1.contains("étape 2 prévue vers"));
        assert!(html1.contains("Carte libérable"));
        assert!(html1.contains(&hdd.display().to_string()));

        let out = execute(&res.cascade.unwrap(), &no_cancel(), |_| {}).unwrap();
        let html2 = fs::read_to_string(&out.reports[0].html).unwrap();
        assert!(html2.contains("OFFLOAD EN CASCADE (étape 2)"));
        assert!(html2.contains("copie vérifiée de l'étape 1"));
        assert!(html2.contains(&ssd.display().to_string()));
        assert!(html2.contains("Étape 1 : début"));
        assert!(html2.contains("MHL de l'étape 1"));
        assert!(html2.contains("SHA-256"));
        assert!(!html2.contains("Empreintes hors MHL"));
        let csv2 = out.reports[0].csv.file_name().unwrap().to_string_lossy();
        assert!(csv2.starts_with("A001_cascade_"));
    }

    #[test]
    fn cascade_report_mentions_sha256_computed_in_stage1() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let mut req = request(&src, &[dir.path().join("SSD")], &[dir.path().join("HDD")]);
        req.algorithms = vec![HashAlgo::Xxh128, HashAlgo::Sha256];
        let pre = preflight(&req).unwrap();
        let res = execute_stage1(&req, pre, &no_cancel(), |_| {}, None, None).unwrap();
        let out = execute(&res.cascade.unwrap(), &no_cancel(), |_| {}).unwrap();
        assert_eq!(out.summary.failed_files, 0);
        let html = fs::read_to_string(&out.reports[0].html).unwrap();
        assert!(html.contains("Empreintes hors MHL"));
        assert!(html.contains("absente du MHL"));
    }

    #[test]
    fn stage1_reports_copied_identically() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        let out = execute(&res.cascade.unwrap(), &no_cancel(), |_| {}).unwrap();
        let reports: Vec<_> = fs::read_dir(ssd.join(REPORTS_DIR))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(reports.len(), 5, "csv, débit, html, pdf, empreinte du pdf");
        for r in reports {
            let name = r.file_name();
            assert!(name.to_string_lossy().starts_with("A001_offload_"));
            let copy = hdd.join(REPORTS_DIR).join(&name);
            assert_eq!(fs::read(r.path()).unwrap(), fs::read(copy).unwrap());
        }
        // Les rapports de l'étape 2 s'y ajoutent sans les remplacer.
        let own = out.reports[0].html.parent().unwrap();
        assert_eq!(own, hdd.join(REPORTS_DIR));
        assert_eq!(fs::read_dir(own).unwrap().count(), 10);
    }

    #[test]
    fn card_not_free_when_stage1_history_cannot_be_written() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let ssd = dir.path().join("SSD");
        // Un fichier à la place du dossier ascmhl : copie possible, historique non.
        fs::create_dir_all(ssd.join("A001")).unwrap();
        fs::write(ssd.join("A001/ascmhl"), b"occupe").unwrap();
        let req = request(&src, std::slice::from_ref(&ssd), &[dir.path().join("HDD")]);
        let pre = preflight(&req).unwrap();
        let res = execute_stage1(&req, pre, &no_cancel(), |_| {}, None, None).unwrap();
        assert_eq!(res.summary.failed_files, 0);
        assert!(res.mhl[0].is_none());
        assert!(res.mhl_errors[0].is_some());
        assert!(!res.card_free, "sans historique, la carte reste à garder");
        assert!(res.cascade.is_none());
        let html = fs::read_to_string(&res.reports[0].html).unwrap();
        assert!(html.contains("Carte libérable</td><td class=\"m\">non<"));
    }

    #[test]
    fn stage1_report_never_overwrites_a_different_report() {
        let dir = tempfile::tempdir().unwrap();
        let (_, ssd, hdd, res) = stage1(dir.path());
        let pending = res.cascade.unwrap();
        assert_eq!(pending.stage1.reports.len(), 5);
        // Un autre rapport de la même carte, sans lien avec cette étape 1.
        let foreign = ssd.join(REPORTS_DIR).join("A001_offload_ancien.csv");
        fs::write(&foreign, b"ancien").unwrap();
        // Un rapport différent déjà présent à la destination : conservé.
        let name = pending.stage1.reports[0].clone();
        fs::create_dir_all(hdd.join(REPORTS_DIR)).unwrap();
        fs::write(hdd.join(REPORTS_DIR).join(&name), b"autre contenu").unwrap();
        let out = execute(&pending, &no_cancel(), |_| {}).unwrap();
        assert_eq!(
            fs::read(hdd.join(REPORTS_DIR).join(&name)).unwrap(),
            b"autre contenu"
        );
        assert!(!hdd
            .join(REPORTS_DIR)
            .join("A001_offload_ancien.csv")
            .exists());
        let html = fs::read_to_string(&out.reports[0].html).unwrap();
        assert!(html.contains("4/5 recopiés"));
    }

    #[test]
    fn stage2_space_is_rechecked_and_messages_are_bare() {
        assert!(space_error(&[None, None]).is_ok());
        let err = space_error(&[None, Some(10)]).unwrap_err();
        assert_eq!(error_code(&err), Some("no_space"));
        assert!(message(&err).starts_with("no_space : "));
        let e = Error::Unsupported("cascade_no_immediate : x".into());
        assert_eq!(message(&e), "cascade_no_immediate : x");
        let e = Error::Report("cascade_no_source : y".into());
        assert_eq!(message(&e), "cascade_no_source : y");
        let e = Error::Report("autre chose".into());
        assert_eq!(message(&e), "rapport : autre chose");
    }
}
