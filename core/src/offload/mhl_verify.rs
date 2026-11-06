//! Revérification d'un disque à partir de son historique ASC MHL, sans la
//! source : chaque fichier est relu sur le support (cache contourné) et
//! comparé à son empreinte la plus récente, avec la meilleure méthode connue.
//!
//! Les rushes ne sont jamais modifiés. Seuls `ascmhl/` (une génération
//! `in-place`) et `_VERIFLOW/` (rapports) reçoivent des écritures, et rien
//! n'est écrit si la vérification est annulée.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime};

use serde::Serialize;
use walkdir::WalkDir;

use super::engine::{hash_from_disk_progress, DestStatus, Event, FileResult, Hashes};
use super::hash::HashAlgo;
use super::meter::{Meter, PhaseStat, Probe};
use super::mhl::{write_entries, Action, GenEntry, MhlAuthor, REPORTS_DIR};
use super::mhl_read::{best_algo, load_history, nested_error, nfc, to_path, Chain, MHL_DIR};
use super::scan::IGNORED;
use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum VerifyStatus {
    /// Empreinte relue identique à l'historique.
    Ok,
    /// Empreinte relue différente de l'historique.
    Different,
    /// Fichier de l'historique absent du disque.
    Missing,
    /// Fichier du disque absent de l'historique (n'est pas un échec).
    New,
    /// Lecture ou comparaison impossible : message du système, méthode
    /// d'empreinte non prise en charge ou chemin refusé de l'historique.
    Unreadable(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifyFile {
    /// Chemin relatif, séparateur « / ».
    pub rel: String,
    /// Taille sur le disque (taille de l'historique pour un fichier manquant, 0 si inconnue).
    pub size: u64,
    pub status: VerifyStatus,
    /// Méthode de comparaison.
    pub algo: Option<HashAlgo>,
    /// Empreinte de l'historique.
    pub expected: Option<String>,
    /// Empreinte relue.
    pub actual: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct VerifySummary {
    pub root: PathBuf,
    pub files: Vec<VerifyFile>,
    /// Octets relus.
    pub total_bytes: u64,
    /// Mesure de la relecture : octets, temps d'E/S, durée d'activité.
    pub read: PhaseStat,
    pub ok: usize,
    pub different: usize,
    pub missing: usize,
    pub new: usize,
    pub unreadable: usize,
    /// Générations de l'historique et anomalies de la chaîne.
    pub chain: Chain,
    pub started_at: String,
    pub finished_at: String,
    pub duration_s: f64,
    pub cancelled: bool,
    /// Aucun écart : ni différent, manquant, illisible, ni chaîne altérée.
    pub conform: bool,
    /// Génération `in-place` écrite (None si annulé ou écriture impossible).
    pub mhl: Option<PathBuf>,
    /// Raison de l'échec d'écriture de la génération (disque protégé en écriture...).
    pub write_error: Option<String>,
    /// L'historique ne référence aucune génération, ou aucun fichier alors que
    /// le disque en contient : rien à comparer, jamais conforme.
    pub empty_history: bool,
    /// Fichiers retirés de `files` par `for_ui` (conformes, nouveaux en excès).
    pub omitted: usize,
}

/// Nombre maximal de nouveaux fichiers listés dans la version pour l'interface.
const UI_NEW_LIMIT: usize = 200;

impl VerifySummary {
    /// Version envoyée à l'interface : compteurs, chaîne et écarts seulement.
    /// Les fichiers conformes (et les nouveaux au-delà de 200) sont retirés ;
    /// le détail complet reste dans le CSV. `omitted` en donne le nombre.
    pub fn for_ui(mut self) -> VerifySummary {
        let total = self.files.len();
        let mut new_seen = 0;
        self.files.retain(|f| match f.status {
            VerifyStatus::Ok => false,
            VerifyStatus::New => {
                new_seen += 1;
                new_seen <= UI_NEW_LIMIT
            }
            _ => true,
        });
        self.omitted = total - self.files.len();
        self
    }
}

fn rfc3339(t: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Filtre `*` et `?` (motifs `<ignore>` de la norme), à deux pointeurs avec
/// retour sur la dernière étoile : temps borné par le produit des longueurs.
fn glob(pat: &[char], s: &[char]) -> bool {
    let (mut p, mut i) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while i < s.len() {
        if p < pat.len() && pat[p] == '*' {
            star = Some((p, i));
            p += 1;
        } else if p < pat.len() && (pat[p] == '?' || pat[p] == s[i]) {
            p += 1;
            i += 1;
        } else if let Some((sp, si)) = star {
            p = sp + 1;
            i = si + 1;
            star = Some((sp, si + 1));
        } else {
            return false;
        }
    }
    pat[p..].iter().all(|c| *c == '*')
}

/// Un élément est exclu s'il correspond à un motif `<ignore>`. Un motif
/// sans « / » vise le nom, un motif avec « / » vise le chemin depuis la racine.
fn excluded(rel: &str, name: &str, is_dir: bool, patterns: &[String]) -> bool {
    patterns.iter().any(|p| {
        let dir_only = p.ends_with('/');
        if dir_only && !is_dir {
            return false;
        }
        let p = p.trim_matches('/');
        let target = if p.contains('/') || p.starts_with('/') {
            rel
        } else {
            name
        };
        let pc: Vec<char> = p.chars().collect();
        let tc: Vec<char> = target.chars().collect();
        glob(&pc, &tc)
    })
}

struct DiskFile {
    rel: String,
    path: PathBuf,
    size: u64,
    modified: SystemTime,
    /// Erreur de parcours : le fichier est signalé illisible.
    error: Option<String>,
}

/// Parcours du disque sans `ascmhl/`, `_VERIFLOW/` (racine seulement), éléments
/// système de la copie (`scan::IGNORED`) ni motifs `<ignore>`. Le dernier
/// élément du résultat est vrai si l'annulation a interrompu le parcours.
type Walked = (Vec<DiskFile>, Vec<String>, bool);

fn walk(root: &Path, patterns: &[String], cancel: &AtomicBool) -> Result<Walked> {
    let mut files = Vec::new();
    let mut dirs = Vec::new();
    let mut it = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter();
    while let Some(entry) = it.next() {
        if cancel.load(Ordering::Relaxed) {
            return Ok((files, dirs, true));
        }
        let entry = match entry {
            Ok(e) => e,
            Err(e) => {
                let rel = e.path().map(|p| rel_path(root, p)).unwrap_or_default();
                if !rel.is_empty() {
                    files.push(DiskFile {
                        rel,
                        path: e.path().map(Path::to_path_buf).unwrap_or_default(),
                        size: 0,
                        modified: SystemTime::UNIX_EPOCH,
                        error: Some(e.to_string()),
                    });
                }
                continue;
            }
        };
        if entry.depth() == 0 {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().is_dir();
        if is_dir && name == MHL_DIR {
            if entry.depth() > 1 {
                return Err(nested_error(root, entry.path()));
            }
            it.skip_current_dir();
            continue;
        }
        let rel = rel_path(root, entry.path());
        let reports = entry.depth() == 1 && name == REPORTS_DIR;
        if reports
            || IGNORED.contains(&name.as_str())
            || name.ends_with(".vfpart")
            || excluded(&rel, &name, is_dir, patterns)
        {
            if is_dir {
                it.skip_current_dir();
            }
            continue;
        }
        if is_dir {
            dirs.push(rel);
        } else if entry.file_type().is_file() {
            let (size, modified, error) = match entry.metadata() {
                Ok(m) => (
                    m.len(),
                    m.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    None,
                ),
                Err(e) => (0, SystemTime::UNIX_EPOCH, Some(e.to_string())),
            };
            files.push(DiskFile {
                rel,
                path: entry.path().to_path_buf(),
                size,
                modified,
                error,
            });
        }
    }
    Ok((files, dirs, false))
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// Vérifie le disque `root` contre son historique ASC MHL et écrit une
/// génération `in-place` (sauf annulation). Un échec d'écriture n'empêche pas
/// le résultat : il est rendu dans `write_error`.
pub fn verify_root(
    root: &Path,
    cancel: &AtomicBool,
    mut emit: impl FnMut(Event),
) -> Result<VerifySummary> {
    let started = Instant::now();
    let started_at = rfc3339(SystemTime::now());
    let history = load_history(root)?;
    // `_VERIFLOW` n'est exclu qu'à la racine (traité par le parcours).
    let patterns: Vec<String> = history
        .ignore
        .iter()
        .filter(|p| p.trim_matches('/') != REPORTS_DIR)
        .cloned()
        .collect();
    let (disk, dirs, walk_cancelled) = walk(root, &patterns, cancel)?;

    // Méthode des nouveaux fichiers : la plus fréquente parmi les meilleures de l'historique.
    let mut votes: HashMap<HashAlgo, usize> = HashMap::new();
    for (_, h) in history.files.values() {
        if let Some(a) = best_algo(h) {
            *votes.entry(a).or_default() += 1;
        }
    }
    let new_algo = votes
        .into_iter()
        .max_by_key(|(a, n)| (*n, std::cmp::Reverse(a.mhl_order())))
        .map(|(a, _)| a)
        .unwrap_or(HashAlgo::Xxh128);

    let total: u64 = disk
        .iter()
        .filter(|f| f.error.is_none())
        .map(|f| f.size)
        .sum();
    let mut done = 0u64;
    let io = Meter::new(started);
    let cpu = Meter::new(started);
    let mut files: Vec<VerifyFile> = Vec::new();
    let mut entries: Vec<GenEntry> = Vec::new();
    let mut cancelled = walk_cancelled;
    let progress = |done: u64, emit: &mut dyn FnMut(Event)| {
        emit(Event::Progress {
            done,
            total,
            rate: done as f64 / started.elapsed().as_secs_f64().max(1e-3),
        });
    };
    progress(0, &mut emit);
    let mut last_emit = Instant::now();

    for (index, f) in disk.iter().enumerate() {
        if cancelled || cancel.load(Ordering::Relaxed) {
            cancelled = true;
            break;
        }
        emit(Event::FileStarted {
            index,
            rel: f.rel.clone(),
            size: f.size,
        });
        let key = nfc(&f.rel);
        let known = history.files.get(&key);
        // Entrée connue sans aucune méthode reconnue : écart, jamais « nouveau ».
        if known.is_some_and(|(_, h)| best_algo(h).is_none()) {
            let methods = history
                .unsupported
                .get(&key)
                .map(|m| m.join(", "))
                .unwrap_or_default();
            let status = VerifyStatus::Unreadable(format!(
                "méthode d'empreinte non prise en charge ({methods})"
            ));
            emit(Event::FileDone {
                index,
                result: FileResult {
                    rel: f.rel.clone(),
                    size: f.size,
                    modified: rfc3339(f.modified),
                    hashes: Vec::new(),
                    destinations: vec![DestStatus::Failed("méthode non prise en charge".into())],
                },
            });
            done += f.size;
            progress(done, &mut emit);
            files.push(VerifyFile {
                rel: f.rel.clone(),
                size: f.size,
                status,
                algo: None,
                expected: None,
                actual: None,
            });
            continue;
        }
        let algo = known.and_then(|(_, h)| best_algo(h)).unwrap_or(new_algo);
        let expected =
            known.and_then(|(_, h)| h.iter().find(|(a, _)| *a == algo).map(|(_, v)| v.clone()));
        let read = match &f.error {
            Some(e) => Err(std::io::Error::other(e.clone())),
            None => hash_from_disk_progress(
                &f.path,
                &[algo],
                cancel,
                Some(Probe { io: &io, cpu: &cpu }),
                |n| {
                    // Au plus une dizaine d'événements par seconde.
                    if last_emit.elapsed().as_millis() >= 100 {
                        last_emit = Instant::now();
                        progress(done + n, &mut emit);
                    }
                },
            ),
        };
        let (status, actual): (VerifyStatus, Option<String>) = match read {
            Err(e) => {
                if cancel.load(Ordering::Relaxed) {
                    cancelled = true;
                    break;
                }
                (VerifyStatus::Unreadable(e.to_string()), None)
            }
            Ok(h) => {
                let actual = h.into_iter().find(|(a, _)| *a == algo).map(|(_, v)| v);
                let status = match (&expected, &actual) {
                    (None, _) => VerifyStatus::New,
                    (Some(e), Some(a)) if e == a => VerifyStatus::Ok,
                    _ => VerifyStatus::Different,
                };
                (status, actual)
            }
        };
        let action = match status {
            VerifyStatus::Ok => Some(Action::Verified),
            VerifyStatus::Different => Some(Action::Failed),
            VerifyStatus::New => Some(Action::Original),
            _ => None,
        };
        if let (Some(action), Some(a)) = (action, &actual) {
            entries.push(GenEntry {
                rel: f.rel.clone(),
                size: f.size,
                hashes: vec![(algo, a.clone())],
                action,
            });
        }
        let hashes: Hashes = actual.iter().map(|a| (algo, a.clone())).collect();
        let destination = match &status {
            VerifyStatus::Ok | VerifyStatus::New => DestStatus::Verified,
            VerifyStatus::Different => DestStatus::Failed("empreinte différente".into()),
            VerifyStatus::Unreadable(m) => DestStatus::Failed(format!("illisible : {m}")),
            VerifyStatus::Missing => unreachable!("un fichier lu existe"),
        };
        emit(Event::FileDone {
            index,
            result: FileResult {
                rel: f.rel.clone(),
                size: f.size,
                modified: rfc3339(f.modified),
                hashes,
                destinations: vec![destination],
            },
        });
        done += f.size;
        progress(done, &mut emit);
        files.push(VerifyFile {
            rel: f.rel.clone(),
            size: f.size,
            status,
            algo: Some(algo),
            expected,
            actual,
        });
    }

    // Fichiers de l'historique absents du disque (seulement si le parcours est complet).
    if !cancelled {
        let on_disk: std::collections::HashSet<String> = disk.iter().map(|f| nfc(&f.rel)).collect();
        let mut gone: Vec<(&String, &(Option<u64>, Hashes))> = history
            .files
            .iter()
            .filter(|(rel, _)| !on_disk.contains(rel.as_str()))
            // Élément système hashé par un autre outil (.DS_Store...) : ignoré comme à la copie.
            .filter(|(rel, _)| !rel.split('/').any(|c| IGNORED.contains(&c)))
            .collect();
        // Chemins refusés de l'historique : écarts, sans toucher au disque.
        for (rel, why) in &history.invalid {
            files.push(VerifyFile {
                rel: rel.clone(),
                size: 0,
                status: VerifyStatus::Unreadable(format!("chemin refusé : {why}")),
                algo: None,
                expected: None,
                actual: None,
            });
        }
        gone.sort_by(|a, b| a.0.cmp(b.0));
        for (rel, (size, h)) in gone {
            let algo = best_algo(h);
            files.push(VerifyFile {
                rel: rel.clone(),
                size: size.unwrap_or(0),
                status: VerifyStatus::Missing,
                algo,
                expected: algo
                    .and_then(|a| h.iter().find(|(x, _)| *x == a).map(|(_, v)| v.clone())),
                actual: None,
            });
        }
    }

    let count = |pred: fn(&VerifyStatus) -> bool| files.iter().filter(|f| pred(&f.status)).count();
    let ok = count(|s| *s == VerifyStatus::Ok);
    let different = count(|s| *s == VerifyStatus::Different);
    let missing = count(|s| *s == VerifyStatus::Missing);
    let new = count(|s| *s == VerifyStatus::New);
    let unreadable = count(|s| matches!(s, VerifyStatus::Unreadable(_)));

    // Génération `in-place` : verified / failed / original, jamais les manquants.
    let (mut mhl, mut write_error) = (None, None);
    if !cancelled {
        let dir_refs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        let name = super::scan::source_name(root);
        let comment = format!("Vérification de « {name} » par VERIFLOW");
        match write_entries(
            root,
            &entries,
            &dir_refs,
            "in-place",
            &comment,
            &MhlAuthor::default(),
        ) {
            Ok(p) => mhl = Some(p),
            Err(e) => write_error = Some(e.to_string()),
        }
    }

    let total_bytes = files
        .iter()
        .filter(|f| f.actual.is_some() || matches!(f.status, VerifyStatus::Unreadable(_)))
        .map(|f| f.size)
        .sum();
    let empty_history =
        history.chain.entries.is_empty() || (history.files.is_empty() && !disk.is_empty());
    let conform = !cancelled
        && !empty_history
        && different + missing + unreadable == 0
        && history.chain.issues.is_empty();
    Ok(VerifySummary {
        root: root.to_path_buf(),
        files,
        total_bytes,
        read: io.stat(),
        ok,
        different,
        missing,
        new,
        unreadable,
        chain: history.chain,
        started_at,
        finished_at: rfc3339(SystemTime::now()),
        duration_s: started.elapsed().as_secs_f64(),
        cancelled,
        conform,
        mhl,
        write_error,
        empty_history,
        omitted: 0,
    })
}

/// Chemin disque d'un fichier du résumé.
pub fn file_path(root: &Path, f: &VerifyFile) -> PathBuf {
    to_path(root, &f.rel)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::engine::{run, OffloadSpec};
    use crate::offload::hash::hash_data;
    use crate::offload::mhl::write_generation;
    use crate::offload::mhl_read::{find_histories, read_chain, ChainIssue};
    use crate::offload::scan::scan;
    use std::fs;

    /// Copie une petite carte (xxh128 + md5) et écrit la génération 1.
    fn offloaded(dir: &Path) -> PathBuf {
        let src = dir.join("A001");
        fs::create_dir_all(src.join("CLIP")).unwrap();
        fs::create_dir_all(src.join("VIDE")).unwrap();
        fs::write(src.join("CLIP/a.mov"), b"abcde").unwrap();
        fs::write(src.join("b.txt"), b"texte").unwrap();
        let dest = dir.join("SSD/A001");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128, HashAlgo::Md5],
            known: None,
            overlap: false,
            expected: None,
        };
        let s = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        write_generation(&inv, &s, 0, &dest, &spec.algorithms, &MhlAuthor::default()).unwrap();
        dest
    }

    fn run_verify(root: &Path) -> VerifySummary {
        verify_root(root, &AtomicBool::new(false), |_| {}).unwrap()
    }

    fn status(s: &VerifySummary, rel: &str) -> VerifyStatus {
        s.files
            .iter()
            .find(|f| f.rel == rel)
            .unwrap()
            .status
            .clone()
    }

    /// Première empreinte xxh128 de `<roothash>` d'un MHL.
    fn root_xxh128(mhl: &Path) -> String {
        let xml = fs::read_to_string(mhl).unwrap();
        let root = xml.split("<roothash>").nth(1).unwrap();
        let tag = root.split("<xxh128").nth(1).unwrap();
        tag.split('>')
            .nth(1)
            .unwrap()
            .split('<')
            .next()
            .unwrap()
            .to_owned()
    }

    fn first_mhl(root: &Path) -> PathBuf {
        let c = read_chain(root).unwrap();
        root.join("ascmhl").join(&c.entries[0].file)
    }

    #[test]
    fn verify_clean_disk_writes_verified_generation_same_root_hash() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let mut events = Vec::new();
        let s = verify_root(&dest, &AtomicBool::new(false), |e| events.push(e)).unwrap();
        assert!(s.conform && s.ok == 2 && s.different + s.missing + s.new + s.unreadable == 0);
        assert!(events.iter().any(|e| matches!(e, Event::Progress { .. })));
        let done = events
            .iter()
            .filter(|e| matches!(e, Event::FileDone { .. }))
            .count();
        assert_eq!(done, 2);
        let gen2 = s.mhl.clone().unwrap();
        assert!(gen2
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("0002_"));
        let xml = fs::read_to_string(&gen2).unwrap();
        assert!(xml.contains("<process>in-place</process>"));
        assert!(xml.contains("action=\"verified\"") && !xml.contains("action=\"original\""));
        assert_eq!(root_xxh128(&gen2), root_xxh128(&first_mhl(&dest)));
        // La génération écrite est cohérente avec la chaîne.
        assert!(read_chain(&dest).unwrap().issues.is_empty());
        // Une seconde vérification reste conforme et lit la génération 2.
        assert!(run_verify(&dest).conform);
    }

    #[test]
    fn verify_detects_modified_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::write(dest.join("CLIP/a.mov"), b"abcdX").unwrap();
        let s = run_verify(&dest);
        assert!(!s.conform && s.different == 1 && s.ok == 1);
        assert_eq!(status(&s, "CLIP/a.mov"), VerifyStatus::Different);
        let xml = fs::read_to_string(s.mhl.unwrap()).unwrap();
        assert!(xml.contains("action=\"failed\"") && xml.contains("action=\"verified\""));
        // L'empreinte en échec ne devient pas la référence.
        assert_eq!(run_verify(&dest).different, 1);
    }

    #[test]
    fn verify_detects_missing_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::remove_file(dest.join("b.txt")).unwrap();
        let s = run_verify(&dest);
        assert!(!s.conform && s.missing == 1);
        assert_eq!(status(&s, "b.txt"), VerifyStatus::Missing);
        let xml = fs::read_to_string(s.mhl.unwrap()).unwrap();
        assert!(
            !xml.contains("b.txt"),
            "un manquant ne figure pas dans la génération"
        );
    }

    #[test]
    fn verify_reports_new_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::write(dest.join("nouveau.wav"), b"neuf").unwrap();
        let s = run_verify(&dest);
        assert!(s.conform, "un nouveau fichier ne fait pas échouer");
        assert_eq!(s.new, 1);
        assert_eq!(status(&s, "nouveau.wav"), VerifyStatus::New);
        let xml = fs::read_to_string(s.mhl.unwrap()).unwrap();
        assert!(xml.contains("nouveau.wav") && xml.contains("action=\"original\""));
        // Il est désormais connu.
        let again = run_verify(&dest);
        assert_eq!((again.new, again.ok), (0, 3));
    }

    #[test]
    fn verify_detects_altered_mhl() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let mhl = first_mhl(&dest);
        let mut xml = fs::read_to_string(&mhl).unwrap();
        xml.push('\n');
        fs::write(&mhl, xml).unwrap();
        let s = run_verify(&dest);
        let name = mhl.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(s.chain.issues, [ChainIssue::Altered(name)]);
        assert!(!s.conform);
        assert_eq!(s.ok, 2);
    }

    #[test]
    fn verify_detects_missing_generation_file() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let mhl = first_mhl(&dest);
        let name = mhl.file_name().unwrap().to_string_lossy().into_owned();
        fs::remove_file(&mhl).unwrap();
        let s = run_verify(&dest);
        assert_eq!(s.chain.issues, [ChainIssue::Missing(name)]);
        assert!(!s.conform);
    }

    #[test]
    fn verify_cancel_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let before = read_chain(&dest).unwrap().entries.len();
        let s = verify_root(&dest, &AtomicBool::new(true), |_| {}).unwrap();
        assert!(s.cancelled && !s.conform && s.mhl.is_none());
        assert_eq!(read_chain(&dest).unwrap().entries.len(), before);
        assert_eq!(fs::read_dir(dest.join("ascmhl")).unwrap().count(), 2);
        assert!(!dest.join(REPORTS_DIR).exists());
    }

    #[test]
    fn verify_ignores_reports_dir() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::create_dir_all(dest.join("_VERIFLOW")).unwrap();
        fs::write(dest.join("_VERIFLOW/rapport.csv"), b"x").unwrap();
        fs::write(dest.join(".DS_Store"), b"x").unwrap();
        let s = run_verify(&dest);
        assert!(s.conform && s.new == 0);
        assert!(s.files.iter().all(|f| !f.rel.contains("_VERIFLOW")));
    }

    #[test]
    fn verify_without_ascmhl_lists_subfolders() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let parent = dest.parent().unwrap();
        let err = verify_root(parent, &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(err.to_string().contains("aucun historique"), "{err}");
        assert_eq!(find_histories(parent, 2), std::slice::from_ref(&dest));
        assert_eq!(find_histories(dir.path(), 2), [dest]);
        assert!(find_histories(dir.path(), 1).is_empty());
    }

    #[test]
    fn verify_uses_latest_hash_across_generations() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        // Le fichier est remplacé légitimement, une génération 2 l'enregistre.
        fs::write(dest.join("CLIP/a.mov"), b"abcdX").unwrap();
        let entry = GenEntry {
            rel: "CLIP/a.mov".into(),
            size: 5,
            hashes: vec![(HashAlgo::Xxh128, hash_data(HashAlgo::Xxh128, b"abcdX"))],
            action: Action::Original,
        };
        write_entries(
            &dest,
            &[entry],
            &[],
            "in-place",
            "test",
            &MhlAuthor::default(),
        )
        .unwrap();
        let s = run_verify(&dest);
        assert_eq!(status(&s, "CLIP/a.mov"), VerifyStatus::Ok);
        assert_eq!(
            status(&s, "b.txt"),
            VerifyStatus::Ok,
            "empreinte de la génération 1"
        );
        assert!(s.conform);
    }

    #[test]
    fn verify_matches_nfc_and_nfd_names() {
        // Historique en NFC, disque en NFD, puis l'inverse.
        for (on_disk, in_history) in [
            ("e\u{301}te\u{301}.mov", "\u{e9}t\u{e9}.mov"),
            ("\u{e9}t\u{e9}.mov", "e\u{301}te\u{301}.mov"),
        ] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().join("D");
            fs::create_dir_all(&root).unwrap();
            fs::write(root.join(on_disk), b"abc").unwrap();
            let entry = GenEntry {
                rel: in_history.into(),
                size: 3,
                hashes: vec![(HashAlgo::Xxh128, hash_data(HashAlgo::Xxh128, b"abc"))],
                action: Action::Original,
            };
            write_entries(&root, &[entry], &[], "in-place", "t", &MhlAuthor::default()).unwrap();
            let s = run_verify(&root);
            assert_eq!((s.ok, s.missing, s.new), (1, 0, 0), "{on_disk:?}");
            assert!(s.conform);
        }
    }

    #[test]
    fn verify_emits_progress_inside_a_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        fs::create_dir_all(&root).unwrap();
        let data = vec![7u8; 40 << 20];
        fs::write(root.join("gros.bin"), &data).unwrap();
        let entry = GenEntry {
            rel: "gros.bin".into(),
            size: data.len() as u64,
            hashes: vec![(HashAlgo::Xxh128, hash_data(HashAlgo::Xxh128, &data))],
            action: Action::Original,
        };
        write_entries(&root, &[entry], &[], "in-place", "t", &MhlAuthor::default()).unwrap();
        let mut dones = Vec::new();
        verify_root(&root, &AtomicBool::new(false), |e| {
            if let Event::Progress { done, .. } = e {
                dones.push(done);
            }
        })
        .unwrap();
        assert!(dones.windows(2).all(|w| w[0] <= w[1]));
        assert_eq!(*dones.last().unwrap(), data.len() as u64);
    }

    #[test]
    fn verify_names_with_quote_and_ampersand_keep_chain_valid() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("J1_L'USINE & A&B");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.mov"), b"abcde").unwrap();
        let dest = dir.path().join("SSD/J1_L'USINE & A&B");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128],
            known: None,
            overlap: false,
            expected: None,
        };
        let s = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        write_generation(&inv, &s, 0, &dest, &spec.algorithms, &MhlAuthor::default()).unwrap();
        for _ in 0..2 {
            let v = run_verify(&dest);
            assert!(v.conform && v.chain.issues.is_empty(), "{:?}", v.chain);
        }
        let chain = read_chain(&dest).unwrap();
        assert_eq!(chain.entries.len(), 3);
        for e in &chain.entries {
            assert!(dest.join("ascmhl").join(&e.file).is_file(), "{}", e.file);
            assert!(e.file.contains("J1_L'USINE & A&B"));
        }
    }

    #[test]
    fn verify_truncated_mhl_or_chain_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let mhl = first_mhl(&dest);
        let xml = fs::read_to_string(&mhl).unwrap();
        fs::write(&mhl, &xml[..xml.len() / 2]).unwrap();
        let err = verify_root(&dest, &AtomicBool::new(false), |_| {}).unwrap_err();
        let m = err.to_string();
        assert!(m.contains("tronqué") || m.contains("XML illisible"), "{m}");
        fs::write(&mhl, &xml).unwrap();
        let chain = dest.join("ascmhl/ascmhl_chain.xml");
        let cx = fs::read_to_string(&chain).unwrap();
        fs::write(&chain, &cx[..cx.len() - 20]).unwrap();
        let err = verify_root(&dest, &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(err.to_string().contains("XML illisible"), "{err}");
        // Rien n'a été écrit : la chaîne tronquée n'est pas réécrite.
        assert_eq!(fs::read_to_string(&chain).unwrap().len(), cx.len() - 20);
        assert_eq!(fs::read_dir(dest.join("ascmhl")).unwrap().count(), 2);
    }

    #[test]
    fn verify_empty_history_is_never_conform() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        fs::create_dir_all(root.join("ascmhl")).unwrap();
        fs::write(root.join("a.mov"), b"x").unwrap();
        fs::write(
            root.join("ascmhl/ascmhl_chain.xml"),
            "<?xml version=\"1.0\"?><ascmhldirectory></ascmhldirectory>",
        )
        .unwrap();
        let s = run_verify(&root);
        assert!(s.empty_history && !s.conform);
        // Génération sans aucun fichier alors que le disque en contient.
        let dest = offloaded(dir.path());
        let dir2 = tempfile::tempdir().unwrap();
        let root2 = dir2.path().join("E");
        fs::create_dir_all(&root2).unwrap();
        write_entries(&root2, &[], &[], "in-place", "t", &MhlAuthor::default()).unwrap();
        fs::write(root2.join("a.mov"), b"x").unwrap();
        let s = run_verify(&root2);
        assert!(s.empty_history && !s.conform && s.new == 1);
        assert!(run_verify(&dest).conform);
    }

    #[test]
    fn verify_flags_unsupported_method_and_unsafe_paths() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        let mhl_dir = root.join("ascmhl");
        fs::create_dir_all(&mhl_dir).unwrap();
        fs::write(root.join("a.mov"), b"abc").unwrap();
        fs::write(root.join("b.mov"), b"abc").unwrap();
        let mhl = r#"<?xml version="1.0"?><hashlist version="2.0">
<creatorinfo><creationdate>2024-01-01T00:00:00+00:00</creationdate></creatorinfo>
<processinfo><process>transfer</process></processinfo><hashes>
<hash><path size="3">a.mov</path><xxh3-128 action="original">00</xxh3-128></hash>
<hash><path size="3">b.mov</path><md5 action="original">900150983cd24fb0d6963f7d28e17f72</md5></hash>
<hash><path>../secret.mov</path><md5 action="original">00</md5></hash>
<hash><path>/etc/passwd</path><md5 action="original">00</md5></hash>
</hashes></hashlist>"#;
        let name = "0001_D_x.mhl";
        fs::write(mhl_dir.join(name), mhl).unwrap();
        fs::write(
            mhl_dir.join("ascmhl_chain.xml"),
            format!("<ascmhldirectory><hashlist sequencenr=\"1\"><path>{name}</path></hashlist></ascmhldirectory>"),
        )
        .unwrap();
        let s = run_verify(&root);
        match status(&s, "a.mov") {
            VerifyStatus::Unreadable(m) => assert!(m.contains("xxh3-128"), "{m}"),
            autre => panic!("{autre:?}"),
        }
        assert_eq!(status(&s, "b.mov"), VerifyStatus::Ok);
        assert_eq!(s.new, 0);
        assert!(matches!(
            status(&s, "../secret.mov"),
            VerifyStatus::Unreadable(_)
        ));
        assert!(matches!(
            status(&s, "/etc/passwd"),
            VerifyStatus::Unreadable(_)
        ));
        assert_eq!((s.unreadable, s.ok), (3, 1));
        assert!(!s.conform);
    }

    #[test]
    fn verify_latest_generation_replaces_hash_set() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        // Génération 2 : le fichier change et n'est décrit qu'en MD5. L'ancien
        // xxh128 ne doit plus servir de référence.
        fs::write(dest.join("CLIP/a.mov"), b"abcdX").unwrap();
        let entry = GenEntry {
            rel: "CLIP/a.mov".into(),
            size: 5,
            hashes: vec![(HashAlgo::Md5, hash_data(HashAlgo::Md5, b"abcdX"))],
            action: Action::Original,
        };
        write_entries(&dest, &[entry], &[], "in-place", "t", &MhlAuthor::default()).unwrap();
        let s = run_verify(&dest);
        assert_eq!(status(&s, "CLIP/a.mov"), VerifyStatus::Ok);
        assert!(s.conform);
    }

    #[test]
    fn verify_cancel_inside_a_file_writes_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let before = fs::read_dir(dest.join("ascmhl")).unwrap().count();
        let cancel = AtomicBool::new(false);
        let s = verify_root(&dest, &cancel, |e| {
            if let Event::FileStarted { .. } = e {
                cancel.store(true, Ordering::Relaxed); // demandée pendant la relecture
            }
        })
        .unwrap();
        assert!(s.cancelled && !s.conform && s.mhl.is_none() && s.write_error.is_none());
        assert_eq!(fs::read_dir(dest.join("ascmhl")).unwrap().count(), before);
        assert!(s.files.iter().all(|f| f.status != VerifyStatus::Missing));
    }

    #[test]
    fn verify_cancel_during_walk_reads_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let mut started = 0;
        let s = verify_root(&dest, &AtomicBool::new(true), |e| {
            if let Event::FileStarted { .. } = e {
                started += 1;
            }
        })
        .unwrap();
        assert!(s.cancelled && s.files.is_empty() && started == 0 && s.mhl.is_none());
    }

    #[test]
    fn verify_write_error_still_returns_result() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let chain = dest.join("ascmhl/ascmhl_chain.xml");
        let s = verify_root(&dest, &AtomicBool::new(false), |e| {
            // La chaîne devient illisible entre la lecture et l'écriture.
            if let Event::FileDone { .. } = e {
                if chain.is_file() {
                    fs::remove_file(&chain).unwrap();
                    fs::create_dir(&chain).unwrap();
                }
            }
        })
        .unwrap();
        assert!(s.mhl.is_none() && s.write_error.is_some());
        assert_eq!(s.ok, 2);
        assert!(s.conform, "le verdict ne dépend pas de l'écriture");
    }

    #[test]
    fn verify_skips_system_items_and_limits_reports_dir_to_root() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::create_dir_all(dest.join(".Spotlight-V100")).unwrap();
        fs::write(dest.join(".Spotlight-V100/db"), b"x").unwrap();
        fs::create_dir_all(dest.join("System Volume Information")).unwrap();
        fs::write(dest.join("System Volume Information/x"), b"x").unwrap();
        fs::create_dir_all(dest.join("CLIP/_VERIFLOW")).unwrap();
        fs::write(dest.join("CLIP/_VERIFLOW/vrai.mov"), b"x").unwrap();
        let s = run_verify(&dest);
        assert_eq!(s.new, 1);
        assert_eq!(status(&s, "CLIP/_VERIFLOW/vrai.mov"), VerifyStatus::New);
    }

    #[test]
    fn glob_is_not_exponential() {
        let text: Vec<char> = "a".repeat(200).chars().collect();
        let pat: Vec<char> = "a*a*a*a*a*a*a*a*a*b".chars().collect();
        assert!(!glob(&pat, &text));
        assert!(glob(
            &"*a?c*".chars().collect::<Vec<_>>(),
            &"xxabcxx".chars().collect::<Vec<_>>()
        ));
    }

    #[test]
    fn for_ui_keeps_counters_and_gaps_only() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::write(dest.join("CLIP/a.mov"), b"abcdX").unwrap();
        let s = run_verify(&dest);
        let ui = s.clone().for_ui();
        assert_eq!((ui.ok, ui.different), (1, 1));
        assert_eq!(ui.files.len(), 1);
        assert_eq!(ui.files[0].rel, "CLIP/a.mov");
        assert_eq!(ui.omitted, 1);
        assert_eq!(s.omitted, 0);
    }

    #[test]
    fn verify_ignores_system_files_hashed_by_another_tool() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        let entry = GenEntry {
            rel: "CLIP/.DS_Store".into(),
            size: 1,
            hashes: vec![(HashAlgo::Xxh128, hash_data(HashAlgo::Xxh128, b"x"))],
            action: Action::Original,
        };
        write_entries(&dest, &[entry], &[], "in-place", "t", &MhlAuthor::default()).unwrap();
        let s = run_verify(&dest);
        assert_eq!(s.missing, 0);
        assert!(s.conform);
    }

    #[test]
    fn nested_history_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let dest = offloaded(dir.path());
        fs::create_dir_all(dest.join("CLIP/ascmhl")).unwrap();
        let err = verify_root(&dest, &AtomicBool::new(false), |_| {}).unwrap_err();
        assert!(err.to_string().contains("imbriqué"), "{err}");
        assert_eq!(
            super::super::mhl_read::error_code(&err),
            Some("nested_history")
        );
    }

    #[test]
    fn ignore_patterns() {
        let p = vec!["*.tmp".to_string(), "cache/".to_string(), "a/b".to_string()];
        assert!(excluded("x/y.tmp", "y.tmp", false, &p));
        assert!(excluded("cache", "cache", true, &p));
        assert!(!excluded("cache", "cache", false, &p));
        assert!(excluded("a/b", "b", false, &p));
        assert!(!excluded("c/b", "b", false, &p));
    }
}
