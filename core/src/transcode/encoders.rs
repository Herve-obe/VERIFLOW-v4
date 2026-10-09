//! Encodeurs et filtres disponibles dans le FFmpeg du poste (charte §6.2).
//!
//! Un encodeur listé par FFmpeg n'est pas forcément utilisable (pas de carte
//! NVIDIA, pilote absent, version du système trop ancienne) : chacun est
//! essayé une fois sur une image de test, et le résultat est gardé en mémoire.

use std::collections::{HashMap, HashSet};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};

use crate::tools;

/// Noms des encodeurs compilés dans le FFmpeg utilisé.
pub fn compiled() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = tools::command("ffmpeg").ok().and_then(|mut c| {
            c.args(["-hide_banner", "-encoders"])
                .stdin(Stdio::null())
                .output()
                .ok()
        });
        out.map(|o| parse_encoders(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    })
}

/// Lit la liste de `ffmpeg -encoders` : « V....D libx264  description ».
pub fn parse_encoders(text: &str) -> HashSet<String> {
    let mut started = false;
    let mut names = HashSet::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("------") {
            started = true;
            continue;
        }
        if !started {
            continue;
        }
        let mut it = line.split_whitespace();
        if let (Some(flags), Some(name)) = (it.next(), it.next()) {
            if flags.len() == 6 {
                names.insert(name.to_owned());
            }
        }
    }
    names
}

/// Vrai si l'encodeur, avec ces réglages, produit réellement un fichier sur ce
/// poste. `audio` : essai sur un signal audio plutôt qu'une image.
pub fn works(args: &[String], audio: bool) -> bool {
    static CACHE: OnceLock<Mutex<HashMap<String, bool>>> = OnceLock::new();
    let Some(name) = args
        .iter()
        .position(|a| a == "-c:v" || a == "-c:a")
        .and_then(|i| args.get(i + 1))
    else {
        return false;
    };
    if !compiled().contains(name) {
        return false;
    }
    let key = args.join(" ");
    let cache = CACHE.get_or_init(Mutex::default);
    if let Some(&ok) = cache
        .lock()
        .ok()
        .and_then(|c| c.get(&key).copied())
        .as_ref()
    {
        return ok;
    }
    let input = if audio {
        "sine=frequency=1000:sample_rate=48000:duration=0.5"
    } else {
        "testsrc2=size=1280x720:rate=25:duration=0.2"
    };
    let ok = tools::command("ffmpeg")
        .ok()
        .and_then(|mut c| {
            c.args(["-v", "error", "-hide_banner", "-f", "lavfi", "-i", input])
                .args(args)
                .args(["-f", "null", "-"])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .ok()
        })
        .is_some_and(|s| s.success());
    if let Ok(mut c) = cache.lock() {
        c.insert(key, ok);
    }
    ok
}

/// Noms des filtres compilés dans le FFmpeg utilisé.
pub fn filters() -> &'static HashSet<String> {
    static NAMES: OnceLock<HashSet<String>> = OnceLock::new();
    NAMES.get_or_init(|| {
        let out = tools::command("ffmpeg").ok().and_then(|mut c| {
            c.args(["-hide_banner", "-filters"])
                .stdin(Stdio::null())
                .output()
                .ok()
        });
        out.map(|o| parse_filters(&String::from_utf8_lossy(&o.stdout)))
            .unwrap_or_default()
    })
}

/// Lit la liste de `ffmpeg -filters` : « TSC lut3d  V->V  description »
/// (deux colonnes de drapeaux seulement depuis FFmpeg 8).
pub fn parse_filters(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let (flags, name, io) = (it.next()?, it.next()?, it.next()?);
            let flags_ok =
                (2..=3).contains(&flags.len()) && flags.chars().all(|c| "TSC.".contains(c));
            (flags_ok && io.contains("->")).then(|| name.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_encoder_list() {
        let text = "Encoders:\n V..... = Video\n ------\n V....D libx264              libx264 H.264\n A....D aac                  AAC\n";
        let names = parse_encoders(text);
        assert!(names.contains("libx264"));
        assert!(names.contains("aac"));
        assert!(!names.contains("="));
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn parses_filter_list() {
        let text = "Filters:\n  T.. = Timeline support\n TSC lut3d             V->V       Adjust colors using a 3D LUT.\n .. ebur128           A->N       EBU R128 scanner.\n";
        let names = parse_filters(text);
        assert!(names.contains("lut3d") && names.contains("ebur128"));
        assert_eq!(names.len(), 2);
    }
}
