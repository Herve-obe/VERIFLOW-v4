//! Catalogue des médias d'un dossier et description complète d'un fichier
//! (charte §7.2) : informations techniques et métadonnées intégrées.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde::Serialize;
use walkdir::WalkDir;

use super::probe::{probe, MediaInfo};
use super::wav::{read_info, WavInfo};
use crate::offload::scan::IGNORED;
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MediaKind {
    Video,
    Audio,
    Image,
}

const VIDEO: &[&str] = &[
    "mov", "mp4", "mxf", "mts", "m2ts", "mkv", "avi", "m4v", "mpg", "mpeg", "webm", "braw", "r3d",
    "crm", "3gp",
];
const AUDIO: &[&str] = &[
    "wav", "bwf", "rf64", "w64", "aif", "aiff", "flac", "mp3", "m4a", "aac", "ogg", "opus",
];
const IMAGE: &[&str] = &["jpg", "jpeg", "png", "tif", "tiff", "dng", "dpx", "exr"];

/// Type de média d'après l'extension.
pub fn kind_of(path: &Path) -> Option<MediaKind> {
    let ext = path.extension()?.to_string_lossy().to_lowercase();
    if VIDEO.contains(&ext.as_str()) {
        Some(MediaKind::Video)
    } else if AUDIO.contains(&ext.as_str()) {
        Some(MediaKind::Audio)
    } else if IMAGE.contains(&ext.as_str()) {
        Some(MediaKind::Image)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaEntry {
    pub path: PathBuf,
    pub name: String,
    /// Chemin relatif au dossier parcouru.
    pub rel: String,
    pub kind: MediaKind,
    pub size: u64,
    /// Date de modification (RFC 3339).
    pub modified: String,
}

fn skip(name: &str) -> bool {
    IGNORED.contains(&name)
        || name.starts_with("._")
        || name == "_VERIFLOW"
        || name.ends_with(".vfpart")
}

/// Nombre maximal de médias listés (protège d'un parcours involontaire d'un disque entier).
pub const MAX_ENTRIES: usize = 5000;

/// Liste les médias d'un dossier (récursif ou non), triés par chemin,
/// dans la limite de `MAX_ENTRIES`.
pub fn list(dir: &Path, recursive: bool) -> Result<Vec<MediaEntry>> {
    let mut out = Vec::new();
    let walker = WalkDir::new(dir)
        .max_depth(if recursive { usize::MAX } else { 1 })
        .sort_by_file_name()
        .into_iter()
        .filter_entry(|e| e.depth() == 0 || !skip(&e.file_name().to_string_lossy()));
    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let Some(kind) = kind_of(entry.path()) else {
            continue;
        };
        let Ok(meta) = entry.metadata() else { continue };
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        out.push(MediaEntry {
            path: entry.path().to_path_buf(),
            name: entry.file_name().to_string_lossy().into_owned(),
            rel: entry
                .path()
                .strip_prefix(dir)
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default(),
            kind,
            size: meta.len(),
            modified: chrono::DateTime::<chrono::Utc>::from(modified)
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        });
    }
    Ok(out)
}

/// Description complète d'un média.
#[derive(Debug, Clone, Serialize)]
pub struct MediaDetails {
    pub kind: Option<MediaKind>,
    /// Analyse FFprobe (tous formats), si disponible.
    pub probe: Option<MediaInfo>,
    /// En-tête BWF / iXML pour les WAV.
    pub wav: Option<WavInfo>,
    /// Métadonnées lues dans le fichier, avec les identifiants de `fields`.
    pub embedded: BTreeMap<String, String>,
    /// Erreur d'analyse éventuelle (fichier illisible, format inconnu).
    pub error: Option<String>,
}

/// Lit les informations techniques et les métadonnées intégrées d'un fichier.
pub fn describe(path: &Path) -> MediaDetails {
    let kind = kind_of(path);
    let mut embedded = BTreeMap::new();
    let mut error = None;
    let wav = if kind == Some(MediaKind::Audio) {
        match read_info(path) {
            Ok(w) => Some(w),
            Err(e) => {
                // Pas un WAV : FFprobe prendra le relais.
                if path
                    .extension()
                    .is_some_and(|e| e.eq_ignore_ascii_case("wav"))
                {
                    error = Some(e.to_string());
                }
                None
            }
        }
    } else {
        None
    };
    if let Some(w) = &wav {
        let x = &w.ixml;
        let mut put = |k: &str, v: &Option<String>| {
            if let Some(v) = v.as_ref().filter(|v| !v.is_empty()) {
                embedded.insert(k.to_owned(), v.clone());
            }
        };
        put("project", &x.project);
        put("scene", &x.scene);
        put("take", &x.take);
        put("reel", &x.tape);
        put("comment", &x.note);
        if let Some(c) = x.circled {
            embedded.insert("circled".into(), c.to_string());
        }
        for (i, n) in x.track_names.iter().enumerate() {
            if let Some(n) = n.as_ref().filter(|n| !n.is_empty()) {
                embedded.insert(format!("track.{}", i + 1), n.clone());
            }
        }
    }
    let probe = match probe(path) {
        Ok(p) => Some(p),
        Err(e) => {
            if wav.is_none() && error.is_none() {
                error = Some(e.to_string());
            }
            None
        }
    };
    if let Some(p) = &probe {
        // Étiquettes QuickTime / MXF courantes.
        for (tag, field) in [
            ("reel_name", "reel"),
            ("com.apple.quicktime.reelname", "reel"),
            ("title", "clip_name"),
            ("comment", "comment"),
            ("com.apple.quicktime.camera.identifier", "camera"),
            ("com.apple.quicktime.keywords", "keywords"),
            ("com.apple.quicktime.director", "director"),
        ] {
            if let Some(v) = p.tags.get(tag).filter(|v| !v.trim().is_empty()) {
                embedded
                    .entry(field.to_owned())
                    .or_insert_with(|| v.trim().to_owned());
            }
        }
    }
    MediaDetails {
        kind,
        probe,
        wav,
        embedded,
        error,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::wav::write_test_wav;

    #[test]
    fn lists_media_and_reads_ixml_fields() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("CLIP")).unwrap();
        std::fs::write(dir.path().join("CLIP/A001.MOV"), b"x").unwrap();
        std::fs::write(dir.path().join("CLIP/._A001.MOV"), b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let ixml = "<BWFXML><SCENE>12A</SCENE><TAKE>3</TAKE><CIRCLED>TRUE</CIRCLED><TAPE>SD01</TAPE>\
            <TRACK_LIST><TRACK><CHANNEL_INDEX>1</CHANNEL_INDEX><NAME>Perche</NAME></TRACK></TRACK_LIST></BWFXML>";
        write_test_wav(
            &dir.path().join("12A_T3.WAV"),
            2,
            48_000,
            24,
            false,
            480,
            Some(ixml),
            |_, _| 0.0,
        )
        .unwrap();

        let all = list(dir.path(), true).unwrap();
        let names: Vec<_> = all.iter().map(|e| e.rel.as_str()).collect();
        assert_eq!(names, ["12A_T3.WAV", "CLIP/A001.MOV"]);
        assert_eq!(list(dir.path(), false).unwrap().len(), 1);

        let d = describe(&dir.path().join("12A_T3.WAV"));
        assert_eq!(d.kind, Some(MediaKind::Audio));
        assert_eq!(d.embedded.get("scene").map(String::as_str), Some("12A"));
        assert_eq!(d.embedded.get("take").map(String::as_str), Some("3"));
        assert_eq!(d.embedded.get("reel").map(String::as_str), Some("SD01"));
        assert_eq!(d.embedded.get("circled").map(String::as_str), Some("true"));
        assert_eq!(
            d.embedded.get("track.1").map(String::as_str),
            Some("Perche")
        );
        assert_eq!(d.wav.unwrap().channels, 2);
    }
}
