//! Textes juridiques livrés avec VERIFLOW (Aide > Licences, À propos) :
//! licence GPLv3, CLUF, CGU, composants tiers, FFmpeg. Ils sont copiés dans
//! les ressources de l'installeur (dossier `licenses/`) ; en développement,
//! lus directement dans le dépôt.

use std::path::PathBuf;

use serde::Serialize;
use tauri::{AppHandle, Manager};

type CmdResult<T> = Result<T, String>;

/// Documents disponibles : nom demandé par l'interface, fichier des
/// ressources, fichier du dépôt (développement).
const DOCS: [(&str, &str, &str); 6] = [
    ("gpl", "LICENSE-GPL-3.0.txt", "../LICENSE"),
    ("cluf", "CLUF.md", "../legal/CLUF.md"),
    ("cgu", "CGU.md", "../legal/CGU.md"),
    (
        "tiers",
        "COMPOSANTS_TIERS.md",
        "../legal/COMPOSANTS_TIERS.md",
    ),
    ("ffmpeg", "FFMPEG.txt", "licenses/FFMPEG.txt"),
    ("third_party", "THIRD_PARTY.txt", "licenses/THIRD_PARTY.txt"),
];

/// Texte d'un document juridique (liste fermée : aucun autre fichier lisible).
#[tauri::command]
pub fn legal_text(name: String, app: AppHandle) -> CmdResult<String> {
    let (_, file, dev) = DOCS
        .iter()
        .find(|(n, _, _)| *n == name)
        .ok_or_else(|| format!("document inconnu : {name}"))?;
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(dir) = app.path().resource_dir() {
        candidates.push(dir.join("licenses").join(file));
    }
    candidates.push(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(dev));
    candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .ok_or_else(|| format!("{file} introuvable dans cette installation"))
}

#[derive(Serialize)]
pub struct FfmpegInfo {
    /// Première ligne de `ffmpeg -version`.
    version: String,
    /// Options de compilation (`ffmpeg -buildconf`).
    buildconf: String,
}

/// Version et configuration du FFmpeg réellement utilisé par VERIFLOW.
#[tauri::command]
pub async fn ffmpeg_info() -> CmdResult<FfmpegInfo> {
    tauri::async_runtime::spawn_blocking(|| {
        let run = |arg: &str| -> CmdResult<String> {
            let out = veriflow_core::tools::command("ffmpeg")
                .map_err(|e| e.to_string())?
                .args(["-hide_banner", arg])
                .output()
                .map_err(|e| e.to_string())?;
            Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
        };
        Ok(FfmpegInfo {
            version: run("-version")?
                .lines()
                .next()
                .unwrap_or_default()
                .to_owned(),
            buildconf: run("-buildconf")?,
        })
    })
    .await
    .map_err(|e| e.to_string())?
}
