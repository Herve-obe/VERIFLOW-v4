//! Rushes déjà présents en destination (copie interrompue, volontairement ou
//! non) : repérage rapide, puis vérification de leurs empreintes avant de
//! relancer la copie. L'utilisateur choisit ensuite de compléter la copie
//! (seuls les rushes absents ou différents sont copiés) ou de tout recopier.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use super::engine::{hash_from_disk, temp_path, Hashes};
use super::hash::HashAlgo;
use super::scan::SourceInventory;
use crate::{Error, Result};

/// Conduite à tenir envers les rushes déjà présents en destination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExistingMode {
    /// Les fichiers complets présents sont revérifiés ; un fichier de taille
    /// différente n'est jamais écrasé (signalé en échec).
    #[default]
    Verify,
    /// Compléter : les rushes identiques (vérifiés avant la copie) sont
    /// conservés, les autres sont copiés (les fichiers différents remplacés).
    Complete,
    /// Tout recopier : les rushes de la source présents en destination sont
    /// effacés avant la copie.
    Replace,
}

/// Repérage rapide (sans lecture des fichiers) d'une destination.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Present {
    /// Rushes de la source déjà présents (même chemin).
    pub files: usize,
    pub bytes: u64,
    /// Copies inachevées (fichiers temporaires laissés par une coupure).
    pub partial: usize,
}

/// Rushes de la source déjà présents dans `root`.
pub fn present(inv: &SourceInventory, root: &Path) -> Present {
    let mut p = Present::default();
    for f in &inv.files {
        let path = root.join(&f.rel);
        if let Ok(m) = fs::metadata(&path) {
            if m.is_file() {
                p.files += 1;
                p.bytes += m.len();
            }
        }
        if temp_path(&path).exists() {
            p.partial += 1;
        }
    }
    p
}

/// Copie (partielle ou complète) de la carte trouvée dans un autre dossier
/// d'une destination.
#[derive(Debug, Clone, Serialize)]
pub struct Elsewhere {
    pub root: PathBuf,
    #[serde(flatten)]
    pub present: Present,
}

/// Nombre maximal de dossiers examinés par niveau lors de la recherche.
const SEARCH_LIMIT: usize = 5000;

/// Cherche sous `base` d'autres dossiers contenant déjà des rushes de la
/// source : dossiers de même nom que le dossier final `root`, à la même
/// profondeur (la même carte rangée sous une autre date, par exemple). Le
/// plus fourni d'abord.
pub fn find_elsewhere(inv: &SourceInventory, base: &Path, root: &Path) -> Vec<Elsewhere> {
    let (Ok(rel), Some(name)) = (root.strip_prefix(base), root.file_name()) else {
        return Vec::new();
    };
    let depth = rel.components().count();
    if depth < 2 {
        return Vec::new(); // dossier final directement sous la destination
    }
    // Dossiers parents possibles, niveau par niveau (dossiers cachés et
    // historiques ascmhl ignorés).
    let mut level = vec![base.to_path_buf()];
    for _ in 0..depth - 1 {
        let mut next = Vec::new();
        for dir in &level {
            let Ok(entries) = fs::read_dir(dir) else {
                continue;
            };
            for e in entries.flatten() {
                let n = e.file_name();
                let n = n.to_string_lossy();
                if n.starts_with('.') || n == "ascmhl" {
                    continue;
                }
                if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    next.push(e.path());
                }
                if next.len() >= SEARCH_LIMIT {
                    break;
                }
            }
        }
        level = next;
    }
    let mut found: Vec<Elsewhere> = level
        .into_iter()
        .map(|parent| parent.join(name))
        .filter(|c| c.as_path() != root && c.is_dir())
        .map(|c| Elsewhere {
            present: present(inv, &c),
            root: c,
        })
        .filter(|e| e.present.files > 0 || e.present.partial > 0)
        .collect();
    found.sort_by_key(|e| std::cmp::Reverse(e.present.files));
    found.truncate(5);
    found
}

/// Résultat de la vérification d'une destination.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DestCheck {
    /// Rushes présents et identiques à la source (empreintes égales).
    pub identical: usize,
    pub identical_bytes: u64,
    /// Rushes présents mais différents de la source (taille ou empreinte).
    pub different: Vec<String>,
    /// Rushes absents.
    pub missing: usize,
}

/// Empreintes établies par la vérification, reprises par la copie pour ne
/// pas relire une seconde fois les rushes identiques.
#[derive(Debug, Clone, Default)]
pub struct Known {
    /// Empreintes de la source, par chemin relatif.
    pub source: HashMap<String, Hashes>,
    /// Par destination : rushes identiques, avec leur taille et leur date au
    /// moment du contrôle (un fichier modifié depuis est recopié).
    pub identical: Vec<HashMap<String, (u64, Option<SystemTime>)>>,
}

impl Known {
    /// Vrai si le rush `rel` de la destination `dest` a été vérifié identique
    /// et n'a pas changé depuis.
    pub fn is_identical(&self, dest: usize, rel: &str, path: &Path) -> bool {
        let Some(&(len, modified)) = self.identical.get(dest).and_then(|m| m.get(rel)) else {
            return false;
        };
        fs::metadata(path)
            .map(|m| m.len() == len && m.modified().ok() == modified)
            .unwrap_or(false)
    }
}

/// Avancement de la vérification (octets lus sur le total à lire).
#[derive(Debug, Clone, Copy, Serialize)]
pub struct CheckProgress {
    pub done: u64,
    pub total: u64,
}

/// Vérifie les rushes déjà présents dans chaque destination : chaque rush
/// présent avec la bonne taille est relu, ainsi que la source, et leurs
/// empreintes comparées.
pub fn verify(
    inv: &SourceInventory,
    roots: &[PathBuf],
    algos: &[HashAlgo],
    cancel: &AtomicBool,
    mut progress: impl FnMut(CheckProgress),
) -> Result<(Vec<DestCheck>, Known)> {
    let algos: Vec<HashAlgo> = if algos.is_empty() {
        vec![HashAlgo::Xxh128]
    } else {
        algos.to_vec()
    };
    let n = roots.len();
    let mut checks = vec![DestCheck::default(); n];
    let mut known = Known {
        source: HashMap::new(),
        identical: vec![HashMap::new(); n],
    };

    // Inventaire des candidats (présents avec la bonne taille).
    let mut plan: Vec<(usize, Vec<usize>)> = Vec::new();
    let mut total = 0u64;
    for (fi, f) in inv.files.iter().enumerate() {
        let mut candidates = Vec::new();
        for (d, root) in roots.iter().enumerate() {
            match fs::metadata(root.join(&f.rel)) {
                Ok(m) if m.is_file() && m.len() == f.size => candidates.push(d),
                Ok(_) => checks[d].different.push(f.rel.clone()),
                Err(_) => checks[d].missing += 1,
            }
        }
        if !candidates.is_empty() {
            total += f.size * (1 + candidates.len() as u64);
            plan.push((fi, candidates));
        }
    }

    let mut done = 0u64;
    progress(CheckProgress { done, total });
    for (fi, candidates) in plan {
        let f = &inv.files[fi];
        let dests: Vec<(usize, PathBuf)> = candidates
            .iter()
            .map(|&d| (d, roots[d].join(&f.rel)))
            .collect();
        // Source et destinations relues en parallèle (disques différents).
        let (source, results) = thread::scope(|s| {
            let algos = &algos;
            let src = s.spawn(move || hash_from_disk(&f.path, algos, cancel));
            let hs: Vec<_> = dests
                .iter()
                .map(|(d, p)| {
                    let meta = fs::metadata(p).ok();
                    (*d, meta, s.spawn(move || hash_from_disk(p, algos, cancel)))
                })
                .collect();
            let source = src
                .join()
                .unwrap_or_else(|_| Err(std::io::Error::other("lecture interrompue")));
            let results: Vec<_> = hs
                .into_iter()
                .map(|(d, meta, h)| {
                    let r = h
                        .join()
                        .unwrap_or_else(|_| Err(std::io::Error::other("lecture interrompue")));
                    (d, meta, r)
                })
                .collect();
            (source, results)
        });
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        done += f.size * (1 + candidates.len() as u64);
        progress(CheckProgress { done, total });
        let Ok(source) = source else {
            // Source illisible : rien n'est conservé, la copie le signalera.
            for &d in &candidates {
                checks[d].different.push(f.rel.clone());
            }
            continue;
        };
        for (d, meta, r) in results {
            match r {
                Ok(h) if h == source => {
                    checks[d].identical += 1;
                    checks[d].identical_bytes += f.size;
                    known.identical[d].insert(
                        f.rel.clone(),
                        (f.size, meta.and_then(|m| m.modified().ok())),
                    );
                }
                _ => checks[d].different.push(f.rel.clone()),
            }
        }
        known.source.insert(f.rel.clone(), source);
    }
    Ok((checks, known))
}

/// Efface de chaque destination les rushes de la source déjà présents, et les
/// copies inachevées. Les autres fichiers de la destination (rapports, MHL,
/// fichiers étrangers à la source) ne sont pas touchés.
pub fn remove(inv: &SourceInventory, roots: &[PathBuf]) -> Result<usize> {
    let mut removed = 0;
    for root in roots {
        for f in &inv.files {
            let path = root.join(&f.rel);
            if path.is_file() {
                fs::remove_file(&path)?;
                removed += 1;
            }
            let _ = fs::remove_file(temp_path(&path));
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::engine::{run, DestStatus, OffloadSpec};
    use crate::offload::scan::scan;

    fn card(dir: &Path) -> PathBuf {
        let root = dir.join("A001");
        fs::create_dir_all(root.join("CLIP")).unwrap();
        for i in 1..=4 {
            let data: Vec<u8> = (0..50_000 + i * 1000).map(|b| (b % 251) as u8).collect();
            fs::write(root.join(format!("CLIP/C000{i}.MP4")), data).unwrap();
        }
        root
    }

    #[test]
    fn completes_an_interrupted_copy() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD/A001");
        let inv = scan(&src).unwrap();
        // Copie interrompue : 1 identique, 1 corrompu (même taille), 1 tronqué,
        // 1 absent, plus un fichier temporaire laissé par la coupure.
        fs::create_dir_all(dest.join("CLIP")).unwrap();
        fs::copy(src.join("CLIP/C0001.MP4"), dest.join("CLIP/C0001.MP4")).unwrap();
        let mut bad = fs::read(src.join("CLIP/C0002.MP4")).unwrap();
        bad[100] ^= 0xFF;
        fs::write(dest.join("CLIP/C0002.MP4"), &bad).unwrap();
        fs::write(dest.join("CLIP/C0003.MP4"), b"tronque").unwrap();
        fs::write(dest.join("CLIP/.C0004.MP4.vfpart"), b"partiel").unwrap();

        let roots = vec![dest.clone()];
        let p = present(&inv, &dest);
        assert_eq!((p.files, p.partial), (3, 1));

        let cancel = AtomicBool::new(false);
        let algos = [HashAlgo::Xxh128];
        let (checks, known) = verify(&inv, &roots, &algos, &cancel, |_| {}).unwrap();
        assert_eq!(checks[0].identical, 1);
        assert_eq!(checks[0].missing, 1);
        let mut diff = checks[0].different.clone();
        diff.sort();
        assert_eq!(diff, vec!["CLIP/C0002.MP4", "CLIP/C0003.MP4"]);

        // Compléter : l'identique est conservé sans relecture, les autres copiés.
        let spec = OffloadSpec {
            destinations: roots.clone(),
            algorithms: algos.to_vec(),
            known: Some(known),
        };
        let s = run(&inv, &spec, &cancel, |_| {}).unwrap();
        assert_eq!(s.failed_files, 0);
        let st: HashMap<&str, &DestStatus> = s
            .files
            .iter()
            .map(|f| (f.rel.as_str(), &f.destinations[0]))
            .collect();
        assert_eq!(st["CLIP/C0001.MP4"], &DestStatus::ResumedVerified);
        assert_eq!(st["CLIP/C0002.MP4"], &DestStatus::Verified);
        assert_eq!(st["CLIP/C0003.MP4"], &DestStatus::Verified);
        assert_eq!(st["CLIP/C0004.MP4"], &DestStatus::Verified);
        for f in &inv.files {
            assert_eq!(
                fs::read(&f.path).unwrap(),
                fs::read(dest.join(&f.rel)).unwrap()
            );
        }
        assert!(!dest.join("CLIP/.C0004.MP4.vfpart").exists());
        // Le rush conservé a bien les empreintes de la source dans le résultat.
        assert!(s.files.iter().all(|f| !f.hashes.is_empty()));
    }

    #[test]
    fn replace_removes_only_source_files() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD/A001");
        let inv = scan(&src).unwrap();
        fs::create_dir_all(dest.join("CLIP")).unwrap();
        fs::copy(src.join("CLIP/C0001.MP4"), dest.join("CLIP/C0001.MP4")).unwrap();
        fs::write(dest.join("CLIP/.C0002.MP4.vfpart"), b"partiel").unwrap();
        fs::write(dest.join("notes.txt"), b"a garder").unwrap();
        assert_eq!(remove(&inv, std::slice::from_ref(&dest)).unwrap(), 1);
        assert!(!dest.join("CLIP/C0001.MP4").exists());
        assert!(!dest.join("CLIP/.C0002.MP4.vfpart").exists());
        assert!(dest.join("notes.txt").exists());
    }

    #[test]
    fn finds_a_partial_copy_under_another_date() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let inv = scan(&src).unwrap();
        let base = dir.path().join("SSD");
        // Copie partielle de la veille, et une autre carte sans rapport.
        let old = base.join("2026-10-07/A001/CLIP");
        fs::create_dir_all(&old).unwrap();
        fs::copy(src.join("CLIP/C0001.MP4"), old.join("C0001.MP4")).unwrap();
        fs::create_dir_all(base.join("2026-10-07/B002/CLIP")).unwrap();
        fs::create_dir_all(base.join("2026-10-06/A001")).unwrap();
        let today = base.join("2026-10-08/A001");
        let found = find_elsewhere(&inv, &base, &today);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].root, base.join("2026-10-07/A001"));
        assert_eq!(found[0].present.files, 1);
        // Modèle sans sous-dossier : rien à chercher.
        assert!(find_elsewhere(&inv, &base, &base.join("A001")).is_empty());
    }

    #[test]
    fn verification_can_be_cancelled() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD/A001");
        let inv = scan(&src).unwrap();
        fs::create_dir_all(dest.join("CLIP")).unwrap();
        fs::copy(src.join("CLIP/C0001.MP4"), dest.join("CLIP/C0001.MP4")).unwrap();
        let cancel = AtomicBool::new(true);
        let r = verify(&inv, &[dest], &[HashAlgo::Xxh128], &cancel, |_| {});
        assert!(matches!(r, Err(Error::Cancelled)));
    }
}
