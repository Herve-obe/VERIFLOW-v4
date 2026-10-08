//! Écriture des fichiers ASC MHL v2 (American Society of Cinematographers,
//! Media Hash List) dans chaque destination (charte §7.1).
//!
//! Structure produite, conforme à l'implémentation de référence ascmhl :
//! `<racine>/ascmhl/NNNN_<racine>_<AAAA-MM-JJ_HHMMSSZ>.mhl` (une génération par
//! copie ou vérification) et `<racine>/ascmhl/ascmhl_chain.xml` (historique
//! chaîné par empreintes C4). Les empreintes de dossiers (contenu et structure)
//! suivent l'algorithme de référence.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::engine::{DestStatus, OffloadSummary};
use super::hash::{digest_bytes, hash_data, hash_of_hash_list, HashAlgo};
use super::scan::{SourceInventory, IGNORED};
use crate::{Result, VERSION};

/// Dossier des rapports VERIFLOW dans chaque destination (exclu du MHL).
pub const REPORTS_DIR: &str = "_VERIFLOW";

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
    let now = chrono::Utc::now();
    let date = now.to_rfc3339_opts(chrono::SecondsFormat::Secs, false);
    let hashdate = esc(&date);

    // Fichiers présents et vérifiés (ou en échec d'empreinte) dans cette destination.
    let entries: Vec<(&super::engine::FileResult, &'static str)> = summary
        .files
        .iter()
        .filter_map(|f| match &f.destinations[dest_index] {
            DestStatus::Verified | DestStatus::ResumedVerified => Some((f, "original")),
            DestStatus::Failed(m) if m.starts_with("empreinte") => Some((f, "failed")),
            _ => None,
        })
        .collect();
    let dirs: Vec<&str> = inv.dirs.iter().map(|d| d.rel.as_str()).collect();

    // Empreintes de dossiers pour chaque format (seulement à partir des fichiers vérifiés).
    let mut dir_hashes: BTreeMap<HashAlgo, BTreeMap<String, (String, String)>> = BTreeMap::new();
    for &algo in &algos {
        let files: Vec<(&str, &str)> = entries
            .iter()
            .filter(|(_, action)| *action == "original")
            .filter_map(|(f, _)| {
                f.hashes
                    .iter()
                    .find(|(a, _)| *a == algo)
                    .map(|(_, h)| (f.rel.as_str(), h.as_str()))
            })
            .collect();
        dir_hashes.insert(algo, directory_hashes(algo, &files, &dirs));
    }
    let dir_block = |key: &str, indent: &str| -> String {
        let mut s = String::new();
        for tag in ["content", "structure"] {
            s += &format!("{indent}<{tag}>\n");
            for &algo in &algos {
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
    let comment = author.comment.clone().unwrap_or_else(|| {
        format!(
            "Copie de « {} » ({}) vérifiée par VERIFLOW",
            inv.name,
            inv.root.display()
        )
    });
    xml += &format!("    <comment>{}</comment>\n", esc(&comment));
    xml += "  </creatorinfo>\n  <processinfo>\n    <process>transfer</process>\n    <roothash>\n";
    xml += &dir_block("", "      ");
    xml += "    </roothash>\n    <ignore>\n";
    for p in IGNORED.iter().chain(std::iter::once(&REPORTS_DIR)) {
        xml += &format!("      <pattern>{}</pattern>\n", esc(p));
    }
    xml += "      <pattern>ascmhl/</pattern>\n    </ignore>\n  </processinfo>\n  <hashes>\n";

    // Fichiers, puis chaque dossier après son contenu (ordre de l'outil de référence).
    let mut items: Vec<(String, bool)> = entries
        .iter()
        .map(|(f, _)| (f.rel.clone(), false))
        .collect();
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
            let (f, action) = entries
                .iter()
                .find(|(f, _)| f.rel == rel)
                .expect("entrée connue");
            let date_attr = mtime_rfc3339(&dest_root.join(&rel))
                .map(|d| format!(" lastmodificationdate=\"{}\"", esc(&d)))
                .unwrap_or_default();
            xml += &format!(
                "    <hash>\n      <path size=\"{}\"{date_attr}>{}</path>\n",
                f.size,
                esc(&rel)
            );
            let mut hs: Vec<&(HashAlgo, String)> =
                f.hashes.iter().filter(|(a, _)| a.in_mhl()).collect();
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

    // Numéro de génération suivant et chaîne.
    let mhl_dir = dest_root.join("ascmhl");
    fs::create_dir_all(&mhl_dir)?;
    let chain_path = mhl_dir.join("ascmhl_chain.xml");
    let mut chain: Vec<(u32, String, String)> = fs::read_to_string(&chain_path)
        .map(|s| parse_chain(&s))
        .unwrap_or_default();
    let seq = chain.iter().map(|(n, _, _)| *n).max().unwrap_or(0) + 1;
    let root_name = dest_root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "root".into());
    let file_name = format!(
        "{seq:04}_{root_name}_{}.mhl",
        now.format("%Y-%m-%d_%H%M%SZ")
    );
    let mhl_path = mhl_dir.join(&file_name);
    fs::write(&mhl_path, xml.as_bytes())?;
    chain.push((seq, file_name, hash_data(HashAlgo::C4, xml.as_bytes())));

    let mut cx = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<ascmhldirectory xmlns=\"urn:ASC:MHL:DIRECTORY:v2.0\">\n",
    );
    for (n, path, c4) in &chain {
        cx += &format!(
            "  <hashlist sequencenr=\"{n}\">\n    <path>{}</path>\n    <c4>{c4}</c4>\n  </hashlist>\n",
            esc(path)
        );
    }
    cx += "</ascmhldirectory>\n";
    fs::write(&chain_path, cx)?;
    Ok(mhl_path)
}

/// Lit les entrées d'un fichier ascmhl_chain.xml existant.
fn parse_chain(xml: &str) -> Vec<(u32, String, String)> {
    let mut out = Vec::new();
    for block in xml.split("<hashlist").skip(1) {
        let seq = block
            .split("sequencenr=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .and_then(|s| s.parse().ok());
        let tag = |t: &str| {
            block
                .split(&format!("<{t}>"))
                .nth(1)
                .and_then(|s| s.split(&format!("</{t}>")).next())
                .map(|s| s.trim().to_owned())
        };
        if let (Some(seq), Some(path), Some(c4)) = (seq, tag("path"), tag("c4")) {
            out.push((seq, path, c4));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::engine::{run, OffloadSpec};
    use crate::offload::scan::scan;
    use std::sync::atomic::AtomicBool;

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
}
