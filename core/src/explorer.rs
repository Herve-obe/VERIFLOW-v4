//! Explorateur de dossiers (onglets OFFLOAD et MEDIA) : sous-dossiers d'un
//! dossier, sans les fichiers ni les dossiers système.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::offload::scan::IGNORED;
use crate::Result;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct DirEntry {
    pub path: PathBuf,
    pub name: String,
    /// Vrai si le dossier contient au moins un sous-dossier (flèche de dépliage).
    pub has_children: bool,
}

/// Dossiers masqués : système, cachés (« . »), rapports et MHL de VERIFLOW.
pub fn hidden(name: &str) -> bool {
    name.starts_with('.') || name.starts_with('$') || IGNORED.contains(&name) || name == "_VERIFLOW"
}

fn subdirs(dir: &Path) -> impl Iterator<Item = (PathBuf, String)> {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            (!hidden(&name)).then(|| (e.path(), name))
        })
}

/// Sous-dossiers d'un dossier, triés sans tenir compte de la casse.
pub fn list_dirs(dir: &Path) -> Result<Vec<DirEntry>> {
    // « D: » seul désigne le dossier courant du lecteur, pas sa racine.
    let dir = &crate::absolute_path(dir);
    std::fs::read_dir(dir)?; // erreur explicite si le dossier est illisible
    let mut out: Vec<DirEntry> = subdirs(dir)
        .map(|(path, name)| DirEntry {
            has_children: subdirs(&path).next().is_some(),
            path,
            name,
        })
        .collect();
    out.sort_by_key(|d| d.name.to_lowercase());
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_only_visible_directories() {
        let dir = tempfile::tempdir().unwrap();
        for d in [
            "b_jour2/CARTE",
            "A_jour1",
            ".cache",
            "_VERIFLOW",
            "ascmhl",
            "System Volume Information",
        ] {
            std::fs::create_dir_all(dir.path().join(d)).unwrap();
        }
        std::fs::write(dir.path().join("fichier.mov"), b"x").unwrap();
        let l = list_dirs(dir.path()).unwrap();
        let names: Vec<_> = l.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["A_jour1", "b_jour2"]);
        assert!(!l[0].has_children && l[1].has_children);
        assert!(list_dirs(&dir.path().join("absent")).is_err());
    }
}
