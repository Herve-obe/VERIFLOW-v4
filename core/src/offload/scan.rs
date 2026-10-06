//! Inventaire d'une source : liste des fichiers à copier, en conservant
//! intégralement l'arborescence de la carte (charte §7.1).

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use walkdir::WalkDir;

use super::hash::{hash_data, HashAlgo};
use crate::{Error, Result};

/// Éléments système ignorés (jamais présents sur une carte caméra d'origine).
/// Repris dans la section `<ignore>` du MHL.
pub const IGNORED: &[&str] = &[
    ".DS_Store",
    ".Spotlight-V100",
    ".fseventsd",
    ".Trashes",
    ".TemporaryItems",
    "System Volume Information",
    "$RECYCLE.BIN",
    "ascmhl",
];

#[derive(Debug, Clone, Serialize)]
pub struct SourceFile {
    /// Chemin relatif à la racine de la source, séparateur « / ».
    pub rel: String,
    pub path: PathBuf,
    pub size: u64,
    #[serde(skip)]
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceInventory {
    pub root: PathBuf,
    /// Nom de la source (nom du dossier ou de la carte).
    pub name: String,
    pub files: Vec<SourceFile>,
    pub total_bytes: u64,
    /// Empreinte de la liste (chemins, tailles, dates) : identifie une carte
    /// déjà copiée sans relire son contenu.
    pub fingerprint: String,
}

fn is_ignored(name: &str) -> bool {
    IGNORED.contains(&name) || name.ends_with(".vfpart")
}

/// Nom lisible d'une source : nom du dossier, ou du volume (« E: » sous Windows).
pub fn source_name(root: &Path) -> String {
    root.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .or_else(|| {
            root.components().next().map(|c| {
                c.as_os_str()
                    .to_string_lossy()
                    .trim_end_matches(['\\', ':'])
                    .to_owned()
            })
        })
        .filter(|n| !n.is_empty())
        .unwrap_or_else(|| "SOURCE".to_owned())
}

/// Parcourt la source (ordre stable, trié par chemin).
pub fn scan(root: &Path) -> Result<SourceInventory> {
    if !root.is_dir() {
        return Err(Error::NotFound(root.display().to_string()));
    }
    let mut files = Vec::new();
    let walker = WalkDir::new(root)
        .follow_links(false)
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !is_ignored(&e.file_name().to_string_lossy()));
    for entry in walker {
        let entry = entry.map_err(|e| Error::Io(e.into()))?;
        if !entry.file_type().is_file() {
            continue;
        }
        let meta = entry.metadata().map_err(|e| Error::Io(e.into()))?;
        let rel = entry
            .path()
            .strip_prefix(root)
            .expect("chemin sous la racine")
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        files.push(SourceFile {
            rel,
            path: entry.path().to_path_buf(),
            size: meta.len(),
            modified: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        });
    }
    let total_bytes = files.iter().map(|f| f.size).sum();
    let listing: String = files
        .iter()
        .map(|f| {
            let secs = f
                .modified
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            format!("{}\t{}\t{}\n", f.rel, f.size, secs)
        })
        .collect();
    Ok(SourceInventory {
        name: source_name(root),
        root: root.to_path_buf(),
        fingerprint: hash_data(HashAlgo::Xxh128, listing.as_bytes()),
        total_bytes,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scans_tree_sorted_and_skips_system_files() {
        let dir = tempfile::tempdir().unwrap();
        let card = dir.path().join("A001");
        std::fs::create_dir_all(card.join("PRIVATE/M4ROOT/CLIP")).unwrap();
        std::fs::write(card.join("PRIVATE/M4ROOT/CLIP/C0002.MP4"), b"22").unwrap();
        std::fs::write(card.join("PRIVATE/M4ROOT/CLIP/C0001.MP4"), b"1").unwrap();
        std::fs::write(card.join(".DS_Store"), b"x").unwrap();
        std::fs::create_dir_all(card.join(".Spotlight-V100")).unwrap();
        std::fs::write(card.join(".Spotlight-V100/db"), b"x").unwrap();
        let inv = scan(&card).unwrap();
        let rels: Vec<_> = inv.files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(
            rels,
            [
                "PRIVATE/M4ROOT/CLIP/C0001.MP4",
                "PRIVATE/M4ROOT/CLIP/C0002.MP4"
            ]
        );
        assert_eq!(inv.total_bytes, 3);
        assert_eq!(inv.name, "A001");
        // Même contenu : même empreinte d'inventaire.
        assert_eq!(scan(&card).unwrap().fingerprint, inv.fingerprint);
    }
}
