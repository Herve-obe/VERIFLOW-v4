//! Localisation et lancement des outils externes FFmpeg / FFprobe.
//!
//! FFmpeg tourne dans un processus séparé : un rush corrompu ne peut pas
//! faire planter VERIFLOW, et le binaire peut être mis à jour indépendamment.

use std::path::PathBuf;
use std::process::Command;

use crate::{Error, Result};

/// Variable d'environnement permettant d'imposer le dossier contenant FFmpeg.
pub const FFMPEG_DIR_ENV: &str = "VERIFLOW_FFMPEG_DIR";

fn exe_name(tool: &str) -> String {
    if cfg!(windows) {
        format!("{tool}.exe")
    } else {
        tool.to_owned()
    }
}

/// Cherche un outil : variable d'environnement, puis à côté de l'exécutable
/// VERIFLOW (version embarquée), puis dans le PATH du système.
pub fn locate(tool: &str) -> Option<PathBuf> {
    let name = exe_name(tool);
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os(FFMPEG_DIR_ENV) {
        candidates.push(PathBuf::from(dir).join(&name));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
    {
        candidates.push(dir.join(&name));
    }
    if let Some(path) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&path).map(|d| d.join(&name)));
    }
    // Emplacements d'installation courants : sous macOS, les applications
    // lancées depuis le Finder n'héritent pas du PATH du terminal (Homebrew).
    let common: &[&str] = if cfg!(target_os = "macos") {
        &["/opt/homebrew/bin", "/usr/local/bin", "/opt/local/bin"]
    } else if cfg!(windows) {
        &["C:\\ffmpeg\\bin", "C:\\Program Files\\ffmpeg\\bin"]
    } else {
        &["/usr/bin", "/usr/local/bin", "/snap/bin"]
    };
    candidates.extend(common.iter().map(|d| PathBuf::from(d).join(&name)));
    candidates.into_iter().find(|p| p.is_file())
}

/// Prépare une commande pour l'outil demandé, sans fenêtre console sous Windows.
pub fn command(tool: &str) -> Result<Command> {
    let path = locate(tool).ok_or_else(|| Error::ToolMissing(tool.to_owned()))?;
    #[allow(unused_mut)]
    let mut cmd = Command::new(path);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    Ok(cmd)
}
