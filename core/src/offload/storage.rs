//! Supports de stockage : liste des volumes, détection des disques durs
//! mécaniques (avertissement de lenteur) et éjection sécurisée (charte §7.1).

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;
use sysinfo::{DiskKind, Disks};

use crate::{Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct Volume {
    pub mount_point: PathBuf,
    pub name: String,
    pub file_system: String,
    pub total: u64,
    pub available: u64,
    pub removable: bool,
    /// "ssd", "hdd" ou "unknown".
    pub kind: &'static str,
}

fn kind_str(k: DiskKind) -> &'static str {
    match k {
        DiskKind::SSD => "ssd",
        DiskKind::HDD => "hdd",
        _ => "unknown",
    }
}

/// Volumes montés, hors volumes système virtuels.
pub fn volumes() -> Vec<Volume> {
    let disks = Disks::new_with_refreshed_list();
    let mut out: Vec<Volume> = disks
        .list()
        .iter()
        .filter(|d| d.total_space() > 0)
        .filter(|d| {
            let fs = d.file_system().to_string_lossy().to_lowercase();
            !["tmpfs", "devtmpfs", "overlay", "squashfs", "proc", "sysfs"].contains(&fs.as_str())
        })
        .map(|d| Volume {
            mount_point: d.mount_point().to_path_buf(),
            name: {
                let n = d.name().to_string_lossy().into_owned();
                let mp = d.mount_point();
                // Sous macOS et Linux, le nom du dossier de montage est le nom du volume.
                mp.file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .filter(|f| !f.is_empty())
                    .unwrap_or(if n.is_empty() {
                        mp.display().to_string()
                    } else {
                        n
                    })
            },
            file_system: d.file_system().to_string_lossy().into_owned(),
            total: d.total_space(),
            available: d.available_space(),
            removable: d.is_removable(),
            kind: kind_str(d.kind()),
        })
        .collect();
    out.sort_by(|a, b| {
        b.removable
            .cmp(&a.removable)
            .then(a.mount_point.cmp(&b.mount_point))
    });
    out
}

/// Volume contenant un chemin (point de montage le plus long qui le précède).
pub fn volume_of(path: &Path) -> Option<Volume> {
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    volumes()
        .into_iter()
        .filter(|v| path.starts_with(&v.mount_point))
        .max_by_key(|v| v.mount_point.as_os_str().len())
}

/// Vrai si le chemin est sur un disque dur mécanique (copie plus lente).
pub fn is_hdd(path: &Path) -> bool {
    volume_of(path).is_some_and(|v| v.kind == "hdd")
}

/// Éjecte proprement un volume amovible (après la copie).
pub fn eject(mount_point: &Path) -> Result<()> {
    let mp = mount_point.display().to_string();
    let output = if cfg!(target_os = "macos") {
        Command::new("diskutil").args(["eject", &mp]).output()
    } else if cfg!(windows) {
        let letter = mp.trim_end_matches(['\\', '/']).to_owned();
        let script = format!(
            "(New-Object -ComObject Shell.Application).Namespace(17).ParseName('{letter}').InvokeVerb('Eject')"
        );
        Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .output()
    } else {
        // Linux : démontage utilisateur (GNOME, KDE), sinon umount.
        Command::new("gio")
            .args(["mount", "--unmount"])
            .arg(&mp)
            .output()
            .and_then(|o| {
                if o.status.success() {
                    Ok(o)
                } else {
                    Command::new("umount").arg(&mp).output()
                }
            })
    };
    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(Error::Tool {
            tool: "éjection".into(),
            message: String::from_utf8_lossy(&o.stderr).trim().to_owned(),
        }),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_volumes_and_finds_the_one_of_a_path() {
        let dir = tempfile::tempdir().unwrap();
        // Dans un conteneur, la liste peut être vide : on vérifie seulement la cohérence.
        if let Some(v) = volume_of(dir.path()) {
            assert!(dir
                .path()
                .canonicalize()
                .unwrap()
                .starts_with(&v.mount_point));
            assert!(v.total >= v.available);
        }
        let _ = is_hdd(dir.path());
    }
}
