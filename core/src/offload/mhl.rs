//! Écriture des fichiers ASC MHL v2 (American Society of Cinematographers,
//! Media Hash List) dans chaque destination (charte §7.1).
//!
//! Structure produite, conforme à l'implémentation de référence ascmhl :
//! `<racine>/ascmhl/NNNN_<racine>_<AAAA-MM-JJ_HHMMSSZ>.mhl` (une génération par
//! copie ou vérification) et `<racine>/ascmhl/ascmhl_chain.xml` (historique
//! chaîné par empreintes C4). Les empreintes de dossiers (contenu et structure)
//! suivent l'algorithme de référence.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use super::engine::{DestStatus, Hashes, OffloadSummary};
use super::hash::{digest_bytes, hash_data, hash_of_hash_list, HashAlgo};
use super::mhl_read::{parse_chain_file, ChainEntry};
use super::scan::{SourceInventory, IGNORED};
use crate::{Result, VERSION};

/// Dossier des rapports VERIFLOW dans chaque destination (exclu du MHL).
pub const REPORTS_DIR: &str = "_VERIFLOW";

/// Plusieurs offloads peuvent finir en même temps dans un même dossier :
/// lecture de la chaîne, numéro suivant et réécriture forment un bloc unique.
static MHL_LOCK: Mutex<()> = Mutex::new(());

/// Informations sur l'auteur de la copie (facultatives).
#[derive(Debug, Clone, Default)]
pub struct MhlAuthor {
    pub name: Option<String>,
    pub email: Option<String>,
    pub role: Option<String>,
    pub location: Option<String>,
    pub comment: Option<String>,
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Empreintes d'un dossier, par format.
#[derive(Default, Clone)]
struct DirHashes {
    content: Vec<String>,
    structure: Vec<String>,
}

fn basename(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map(|(p, _)| p).unwrap_or("")
}

/// Calcule les empreintes de contenu et de structure de chaque dossier
/// (clé "" = racine) pour un format, à partir des empreintes de fichiers.
fn directory_hashes(
    algo: HashAlgo,
    files: &[(&str, &str)],
    dirs: &[&str],
) -> BTreeMap<String, (String, String)> {
    let mut acc: BTreeMap<String, DirHashes> = BTreeMap::new();
    acc.insert(String::new(), DirHashes::default());
    for d in dirs {
        acc.insert(d.to_string(), DirHashes::default());
    }
    for (rel, digest) in files {
        let entry = acc.entry(parent(rel).to_owned()).or_default();
        entry.content.push(digest.to_string());
        let mut bytes = basename(rel).as_bytes().to_vec();
        bytes.extend(digest_bytes(algo, digest).unwrap_or_default());
        entry.structure.push(hash_data(algo, &bytes));
    }
    // Du plus profond au moins profond : chaque dossier est remonté dans son parent.
    let mut keys: Vec<String> = acc.keys().filter(|k| !k.is_empty()).cloned().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.matches('/').count()));
    let mut result = BTreeMap::new();
    for key in keys.into_iter().chain(std::iter::once(String::new())) {
        let h = acc.get(&key).cloned().unwrap_or_default();
        let content = hash_of_hash_list(algo, &h.content);
        let structure = hash_of_hash_list(algo, &h.structure);
        if !key.is_empty() {
            let p = acc.entry(parent(&key).to_owned()).or_default();
            p.content.push(content.clone());
            let mut bytes = basename(&key).as_bytes().to_vec();
            bytes.extend(digest_bytes(algo, &structure).unwrap_or_default());
            p.structure.push(hash_data(algo, &bytes));
        }
        result.insert(key, (content, structure));
    }
    result
}

fn mtime_rfc3339(path: &Path) -> Option<String> {
    let m = fs::metadata(path).ok()?.modified().ok()?;
    Some(
        chrono::DateTime::<chrono::Utc>::from(m)
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, false),
    )
}

/// Action d'une empreinte dans une génération (vocabulaire ASC MHL).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Fichier présenté pour la première fois (copie, ou nouveau fichier à la vérification).
    Original,
    /// Empreinte relue et conforme à l'historique.
    Verified,
    /// Empreinte relue et différente de l'historique.
    Failed,
}

impl Action {
    pub fn as_str(self) -> &'static str {
        match self {
            Action::Original => "original",
            Action::Verified => "verified",
            Action::Failed => "failed",
        }
    }
}

/// Un fichier à inscrire dans une génération.
#[derive(Debug, Clone)]
pub struct GenEntry {
    /// Chemin relatif, séparateur « / ».
    pub rel: String,
    pub size: u64,
    pub hashes: Hashes,
    pub action: Action,
}

/// Écrit une nouvelle génération MHL dans la destination `index` et met à jour
/// la chaîne. Renvoie le chemin du fichier .mhl créé.
pub fn write_generation(
    inv: &SourceInventory,
    summary: &OffloadSummary,
    dest_index: usize,
    dest_root: &Path,
    algorithms: &[HashAlgo],
    author: &MhlAuthor,
) -> Result<PathBuf> {
    let algos: Vec<HashAlgo> = {
        let mut a: Vec<HashAlgo> = algorithms.iter().copied().filter(|a| a.in_mhl()).collect();
        a.sort_by_key(|a| a.mhl_order());
        a.dedup();
        a
    };
    // Fichiers présents et vérifiés (ou en échec d'empreinte) dans cette destination.
    let entries: Vec<GenEntry> = summary
        .files
        .iter()
        .filter_map(|f| {
            let action = match &f.destinations[dest_index] {
                DestStatus::Verified | DestStatus::ResumedVerified => Action::Original,
                DestStatus::Failed(m) if m.starts_with("empreinte") => Action::Failed,
                _ => return None,
            };
            Some(GenEntry {
                rel: f.rel.clone(),
                size: f.size,
                hashes: f.hashes.clone(),
                action,
            })
        })
        .collect();
    let dirs: Vec<&str> = inv.dirs.iter().map(|d| d.rel.as_str()).collect();
    let comment = author.comment.clone().unwrap_or_else(|| {
        format!(
            "Copie de « {} » ({}) vérifiée par VERIFLOW",
            inv.name,
            inv.root.display()
        )
    });
    write_core(
        dest_root, &entries, &dirs, &algos, "transfer", &comment, author,
    )
}

/// Écrit une génération à partir d'entrées quelconques (vérification en place
/// d'un disque, par exemple). Les formats écrits sont ceux des entrées ; les
/// empreintes de dossiers ne comptent que les fichiers non `failed`.
pub fn write_entries(
    root: &Path,
    entries: &[GenEntry],
    dirs: &[&str],
    process: &str,
    comment: &str,
    author: &MhlAuthor,
) -> Result<PathBuf> {
    let mut algos: Vec<HashAlgo> = entries
        .iter()
        .flat_map(|e| e.hashes.iter().map(|(a, _)| *a))
        .filter(|a| a.in_mhl())
        .collect();
    algos.sort_by_key(|a| a.mhl_order());
    algos.dedup();
    write_core(root, entries, dirs, &algos, process, comment, author)
}

fn write_core(
    dest_root: &Path,
    entries: &[GenEntry],
    dirs: &[&str],
    algos: &[HashAlgo],
    process: &str,
    comment: &str,
    author: &MhlAuthor,
) -> Result<PathBuf> {
    let now = chrono::Utc::now();
    let date = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    let hashdate = esc(&date);

    // Empreintes de dossiers pour chaque format (seulement à partir des fichiers non en échec).
    let mut dir_hashes: BTreeMap<HashAlgo, BTreeMap<String, (String, String)>> = BTreeMap::new();
    for &algo in algos {
        let files: Vec<(&str, &str)> = entries
            .iter()
            .filter(|e| e.action != Action::Failed)
            .filter_map(|e| {
                e.hashes
                    .iter()
                    .find(|(a, _)| *a == algo)
                    .map(|(_, h)| (e.rel.as_str(), h.as_str()))
            })
            .collect();
        dir_hashes.insert(algo, directory_hashes(algo, &files, dirs));
    }
    let dir_block = |key: &str, indent: &str| -> String {
        let mut s = String::new();
        for tag in ["content", "structure"] {
            s += &format!("{indent}<{tag}>\n");
            for &algo in algos {
                let (c, st) = &dir_hashes[&algo][key];
                let v = if tag == "content" { c } else { st };
                s += &format!(
                    "{indent}  <{0} hashdate=\"{hashdate}\">{v}</{0}>\n",
                    algo.id()
                );
            }
            s += &format!("{indent}</{tag}>\n");
        }
        s
    };

    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml += "<hashlist version=\"2.0\" xmlns=\"urn:ASC:MHL:v2.0\">\n  <creatorinfo>\n";
    xml += &format!("    <creationdate>{}</creationdate>\n", esc(&date));
    xml += &format!(
        "    <hostname>{}</hostname>\n",
        esc(&gethostname::gethostname().to_string_lossy())
    );
    xml += &format!("    <tool version=\"{}\">VERIFLOW</tool>\n", esc(VERSION));
    if let Some(name) = author.name.as_deref().filter(|n| !n.trim().is_empty()) {
        let mut attrs = String::new();
        if let Some(e) = author.email.as_deref().filter(|e| e.contains('@')) {
            attrs += &format!(" email=\"{}\"", esc(e));
        }
        if let Some(r) = author.role.as_deref().filter(|r| !r.trim().is_empty()) {
            attrs += &format!(" role=\"{}\"", esc(r));
        }
        xml += &format!("    <author{attrs}>{}</author>\n", esc(name));
    }
    if let Some(l) = author.location.as_deref().filter(|l| !l.trim().is_empty()) {
        xml += &format!("    <location>{}</location>\n", esc(l));
    }
    xml += &format!("    <comment>{}</comment>\n", esc(comment));
    xml += &format!(
        "  </creatorinfo>\n  <processinfo>\n    <process>{}</process>\n    <roothash>\n",
        esc(process)
    );
    xml += &dir_block("", "      ");
    xml += "    </roothash>\n    <ignore>\n";
    for p in IGNORED.iter().chain(std::iter::once(&REPORTS_DIR)) {
        xml += &format!("      <pattern>{}</pattern>\n", esc(p));
    }
    xml += "      <pattern>ascmhl/</pattern>\n    </ignore>\n  </processinfo>\n  <hashes>\n";

    // Fichiers, puis chaque dossier après son contenu (ordre de l'outil de référence).
    let by_rel: HashMap<&str, &GenEntry> = entries.iter().map(|e| (e.rel.as_str(), e)).collect();
    let mut items: Vec<(String, bool)> = entries.iter().map(|e| (e.rel.clone(), false)).collect();
    items.extend(dirs.iter().map(|d| (d.to_string(), true)));
    items.sort_by(|a, b| {
        let ka = if a.1 {
            format!("{}/\u{10FFFF}", a.0)
        } else {
            a.0.clone()
        };
        let kb = if b.1 {
            format!("{}/\u{10FFFF}", b.0)
        } else {
            b.0.clone()
        };
        ka.cmp(&kb)
    });
    for (rel, is_dir) in items {
        if is_dir {
            let date_attr = mtime_rfc3339(&dest_root.join(&rel))
                .map(|d| format!(" lastmodificationdate=\"{}\"", esc(&d)))
                .unwrap_or_default();
            xml += &format!(
                "    <directoryhash>\n      <path{date_attr}>{}</path>\n",
                esc(&rel)
            );
            xml += &dir_block(&rel, "      ");
            xml += "    </directoryhash>\n";
        } else {
            let e = by_rel.get(rel.as_str()).expect("entrée connue");
            let action = e.action.as_str();
            let date_attr = mtime_rfc3339(&dest_root.join(&rel))
                .map(|d| format!(" lastmodificationdate=\"{}\"", esc(&d)))
                .unwrap_or_default();
            xml += &format!(
                "    <hash>\n      <path size=\"{}\"{date_attr}>{}</path>\n",
                e.size,
                esc(&rel)
            );
            let mut hs: Vec<&(HashAlgo, String)> =
                e.hashes.iter().filter(|(a, _)| a.in_mhl()).collect();
            hs.sort_by_key(|(a, _)| a.mhl_order());
            for (algo, value) in hs {
                xml += &format!(
                    "      <{0} action=\"{action}\" hashdate=\"{hashdate}\">{value}</{0}>\n",
                    algo.id()
                );
            }
            xml += "    </hash>\n";
        }
    }
    xml += "  </hashes>\n</hashlist>\n";

    // Numéro de génération suivant et chaîne (atomiques entre fils ; un verrou
    // empoisonné ne dit rien sur les fichiers, on le reprend).
    let _chain_guard = MHL_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mhl_dir = dest_root.join("ascmhl");
    fs::create_dir_all(&mhl_dir)?;
    let chain_path = mhl_dir.join("ascmhl_chain.xml");
    // Une chaîne illisible ou tronquée n'est jamais réécrite : l'historique
    // existant serait perdu. Seule une chaîne absente repart de zéro.
    let mut chain: Vec<ChainEntry> = match fs::read_to_string(&chain_path) {
        Ok(xml) => parse_chain_file(&xml)?,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(e) => return Err(e.into()),
    };
    let seq = chain.iter().map(|e| e.seq).max().unwrap_or(0) + 1;
    let root_name = dest_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".into());
    let file_name = format!(
        "{seq:04}_{root_name}_{}.mhl",
        now.format("%Y-%m-%d_%H%M%SZ")
    );
    let mhl_path = mhl_dir.join(&file_name);
    write_atomic(&mhl_path, xml.as_bytes())?;
    chain.push(ChainEntry {
        seq,
        file: file_name,
        c4: hash_data(HashAlgo::C4, xml.as_bytes()),
    });

    let mut cx = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ascmhldirectory xmlns=\"urn:ASC:MHL:DIRECTORY:v2.0\">\n",
    );
    for e in &chain {
        cx += &format!(
            "  <hashlist sequencenr=\"{}\">\n    <path>{}</path>\n",
            e.seq,
            esc(&e.file)
        );
        // Entrée d'un autre outil sans C4 : conservée telle quelle.
        if !e.c4.is_empty() {
            cx += &format!("    <c4>{}</c4>\n", e.c4);
        }
        cx += "  </hashlist>\n";
    }
    cx += "</ascmhldirectory>\n";
    write_atomic(&chain_path, cx.as_bytes())?;
    Ok(mhl_path)
}

/// Écriture atomique : fichier temporaire du même dossier, fsync, renommage,
/// puis fsync du dossier quand le système le permet. Une coupure laisse
/// l'ancien fichier ou le nouveau, jamais un fichier tronqué.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = path.with_file_name(format!(".{name}.vfpart"));
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    #[cfg(unix)]
    if let Some(dir) = path.parent().and_then(|d| fs::File::open(d).ok()) {
        let _ = dir.sync_all();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::engine::{run, OffloadSpec};
    use crate::offload::scan::scan;
    use std::sync::atomic::AtomicBool;

    /// Entrées (numéro, fichier, C4) de la chaîne écrite.
    fn parse_chain(xml: &str) -> Vec<(u32, String, String)> {
        parse_chain_file(xml)
            .unwrap()
            .into_iter()
            .map(|e| (e.seq, e.file, e.c4))
            .collect()
    }

    #[test]
    fn writes_generations_and_chain() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A002R2EC");
        fs::create_dir_all(src.join("Clips")).unwrap();
        fs::create_dir_all(src.join("Vide")).unwrap();
        fs::write(src.join("Clips/A002C006.mov"), b"abcde").unwrap();
        fs::write(src.join("Sidecar & notes.txt"), b"texte").unwrap();
        let dest = dir.path().join("SSD/A002R2EC");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128, HashAlgo::Sha256, HashAlgo::Md5],
            known: None,
            overlap: false,
        };
        let s = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        let p1 =
            write_generation(&inv, &s, 0, &dest, &spec.algorithms, &MhlAuthor::default()).unwrap();
        let xml = fs::read_to_string(&p1).unwrap();
        assert!(p1
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("0001_A002R2EC_"));
        assert!(xml.contains("<md5 action=\"original\""));
        assert!(xml.contains("<xxh128 action=\"original\""));
        assert!(!xml.contains("sha256"), "SHA-256 absent de la norme MHL");
        assert!(xml.contains("Sidecar &amp; notes.txt"));
        assert!(xml.contains("<path lastmodificationdate=\"") && xml.contains(">Vide</path>"));
        // md5 avant xxh128 (ordre du schéma XSD).
        assert!(xml.find("<md5 action").unwrap() < xml.find("<xxh128 action").unwrap());

        let p2 =
            write_generation(&inv, &s, 0, &dest, &spec.algorithms, &MhlAuthor::default()).unwrap();
        assert!(p2
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("0002_"));
        let chain = parse_chain(&fs::read_to_string(dest.join("ascmhl/ascmhl_chain.xml")).unwrap());
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].2, hash_data(HashAlgo::C4, xml.as_bytes()));
    }

    #[test]
    fn foreign_chain_entries_are_kept_without_c4() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        let mhl_dir = root.join("ascmhl");
        fs::create_dir_all(&mhl_dir).unwrap();
        // Autre outil : guillemets simples, aucun C4, un numéro absent.
        fs::write(
            mhl_dir.join("ascmhl_chain.xml"),
            "<ascmhldirectory><hashlist sequencenr='1'><path>0001_D_a.mhl</path></hashlist>\
             <hashlist><path>0002_D_b.mhl</path><c4>c4abc</c4></hashlist></ascmhldirectory>",
        )
        .unwrap();
        write_entries(&root, &[], &[], "in-place", "t", &MhlAuthor::default()).unwrap();
        let chain = parse_chain(&fs::read_to_string(mhl_dir.join("ascmhl_chain.xml")).unwrap());
        assert_eq!(chain.len(), 3);
        assert_eq!(
            (chain[0].0, chain[0].1.as_str(), chain[0].2.as_str()),
            (1, "0001_D_a.mhl", "")
        );
        assert_eq!((chain[1].0, chain[1].2.as_str()), (2, "c4abc"));
        assert_eq!(chain[2].0, 3);
        assert!(!chain[2].2.is_empty());
        let leftovers = fs::read_dir(&mhl_dir)
            .unwrap()
            .filter(|e| {
                e.as_ref()
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .ends_with(".vfpart")
            })
            .count();
        assert_eq!(leftovers, 0);
    }

    #[test]
    fn truncated_chain_is_not_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("D");
        let mhl_dir = root.join("ascmhl");
        fs::create_dir_all(&mhl_dir).unwrap();
        let broken = "<ascmhldirectory><hashlist sequencenr=\"1\"><path>0001_D_a.mhl</path>";
        fs::write(mhl_dir.join("ascmhl_chain.xml"), broken).unwrap();
        assert!(write_entries(&root, &[], &[], "in-place", "t", &MhlAuthor::default()).is_err());
        assert_eq!(
            fs::read_to_string(mhl_dir.join("ascmhl_chain.xml")).unwrap(),
            broken
        );
        assert_eq!(fs::read_dir(&mhl_dir).unwrap().count(), 1);
    }

    /// Deux écritures simultanées dans un même dossier : numéros 1 et 2, chaîne
    /// à deux entrées dont les C4 correspondent aux fichiers.
    #[test]
    fn concurrent_generations_same_root_keep_chain() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("A001");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("a.mov"), b"abc").unwrap();
        let dest = dir.path().join("D/A001");
        let inv = scan(&src).unwrap();
        let spec = OffloadSpec {
            destinations: vec![dest.clone()],
            algorithms: vec![HashAlgo::Xxh128],
            known: None,
            overlap: true,
        };
        let s = run(&inv, &spec, &AtomicBool::new(false), |_| {}).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let paths: Vec<PathBuf> = std::thread::scope(|sc| {
            let hs: Vec<_> = (0..2)
                .map(|_| {
                    sc.spawn(|| {
                        barrier.wait();
                        write_generation(
                            &inv,
                            &s,
                            0,
                            &dest,
                            &spec.algorithms,
                            &MhlAuthor::default(),
                        )
                        .unwrap()
                    })
                })
                .collect();
            hs.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let chain = parse_chain(&fs::read_to_string(dest.join("ascmhl/ascmhl_chain.xml")).unwrap());
        let mut seqs: Vec<u32> = chain.iter().map(|c| c.0).collect();
        seqs.sort();
        assert_eq!(seqs, [1, 2]);
        for (_, name, c4) in &chain {
            let bytes = fs::read(dest.join("ascmhl").join(name)).unwrap();
            assert_eq!(&hash_data(HashAlgo::C4, &bytes), c4);
        }
        assert_eq!(paths.len(), 2);
        assert_ne!(paths[0], paths[1]);
    }
}
