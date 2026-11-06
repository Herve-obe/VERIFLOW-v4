//! Exports vers le montage (FCPXML, XMEML, EDL, ALE, OTIO, CSV Resolve) depuis
//! MEDIA : des clips de chutier, sans séquence (sauf l'EDL). Ce module porte
//! le modèle commun, les chemins et le répartiteur `export` ; chaque format a
//! son fichier.

pub mod fcpxml;
pub mod model;
pub mod otio;
pub mod paths;
pub mod resolve_csv;
pub mod xmeml;

#[cfg(test)]
pub(crate) mod testutil;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use model::{EditClip, EditFormat, EditOptions, FcpxmlVersion, PathMode, SyncedSound};

use crate::media::export::ale_files;
use crate::player::logs::{fps_text, to_edl_whole, LogClip};
use crate::Result;

/// Avertissement structuré : `code` permet à l'interface de le traduire
/// (`ale_mixed_rates`, `edl_mixed_rates`, `xmeml_rounding`,
/// `sound_no_overlap`), `detail` garde le texte français complet. Un code
/// n'est émis qu'une fois par export ; `clips` liste les clips concernés.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub detail: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clips: Vec<String>,
}

impl Warning {
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
            clips: Vec::new(),
        }
    }

    /// Avertissement qui porte sur ces clips.
    pub fn for_clips(code: &str, detail: impl Into<String>, clips: Vec<String>) -> Self {
        Self {
            clips,
            ..Self::new(code, detail)
        }
    }
}

/// Avertissement des sons synchronisés qui ne recouvrent pas l'image : ils ne
/// sont pas écrits (FCPXML, XMEML, OTIO).
fn no_overlap_warning(clips: &[EditClip]) -> Option<Warning> {
    let names: Vec<String> = clips
        .iter()
        .filter(|c| c.sound.as_ref().is_some_and(|s| !s.overlaps(&c.log)))
        .map(|c| c.log.name.clone())
        .collect();
    (!names.is_empty()).then(|| {
        Warning::for_clips(
            "sound_no_overlap",
            format!(
                "{} clip(s) : le son synchronisé ne recouvre pas l'image, il n'est pas écrit ({})",
                names.len(),
                names.join(", ")
            ),
            names,
        )
    })
}

/// Fichier créé sans jamais en écraser un : si `stem.ext` existe, essaie
/// `stem_2.ext`, `stem_3.ext`... Rend le chemin réellement écrit.
fn write_new(out: &Path, stem: &str, ext: &str, text: &str) -> Result<PathBuf> {
    use std::io::Write;
    for n in 1u32.. {
        let name = if n == 1 {
            format!("{stem}.{ext}")
        } else {
            format!("{stem}_{n}.{ext}")
        };
        let path = out.join(name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut f) => {
                // Écriture ratée : pas de fichier partiel laissé au montage.
                if let Err(e) = f.write_all(text.as_bytes()) {
                    drop(f);
                    let _ = std::fs::remove_file(&path);
                    return Err(e.into());
                }
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    unreachable!("boucle sans fin")
}

/// Ajoute à l'erreur la liste des fichiers déjà écrits, pour que le message
/// dise ce qui existe sur le disque.
fn with_written(e: crate::Error, written: &[PathBuf]) -> crate::Error {
    if written.is_empty() {
        return e;
    }
    let names: Vec<String> = written
        .iter()
        .map(|p| {
            p.file_name().map_or_else(
                || p.display().to_string(),
                |n| n.to_string_lossy().into_owned(),
            )
        })
        .collect();
    crate::Error::Io(std::io::Error::other(format!(
        "{e} (déjà écrits : {})",
        names.join(", ")
    )))
}

/// Longueur maximale du nom de fichier tiré du titre, en octets.
const STEM_MAX_BYTES: usize = 200;

/// Résultat d'un export : fichiers écrits et avertissements à montrer.
#[derive(Debug, Clone, Default, Serialize)]
pub struct EditResult {
    pub written: Vec<PathBuf>,
    pub warnings: Vec<Warning>,
}

/// Nom de fichier sûr (Windows compris) tiré du titre.
fn file_stem(title: &str) -> String {
    let s: String = title
        .trim()
        .chars()
        .map(|c| {
            if c.is_control() || "/\\:*?\"<>|".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    // Tronqué sans couper un caractère UTF-8.
    let mut end = s.len().min(STEM_MAX_BYTES);
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    let s = s[..end].trim_matches(['.', ' ']).to_owned();
    if s.is_empty() {
        "VERIFLOW".into()
    } else if is_reserved(&s) {
        format!("_{s}")
    } else {
        s
    }
}

/// Nom réservé par Windows (CON, PRN, AUX, NUL, COM1-9, LPT1-9), avec ou sans
/// extension (« CON.fcpxml » l'est aussi).
fn is_reserved(name: &str) -> bool {
    let base = name.split('.').next().unwrap_or("").trim_end();
    let up = base.to_ascii_uppercase();
    matches!(up.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|p| {
            up.strip_prefix(p)
                .is_some_and(|n| matches!(n.as_bytes(), [b'1'..=b'9']))
        })
}

fn mixed_rates(clips: &[EditClip]) -> bool {
    let mut rates = clips.iter().filter(|c| c.log.has_video).map(|c| c.log.rate);
    rates.next().is_some_and(|first| rates.any(|r| r != first))
}

/// Écrit `clips` au `format` voulu dans le dossier `out` (créé au besoin),
/// sous le nom du titre. Les chemins relatifs le sont par rapport à
/// `opts.base`, à défaut au dossier `out`. Un ALE n'ayant qu'une cadence, des
/// cadences mêlées donnent un fichier par cadence. Un fichier existant n'est
/// jamais écrasé : le nom prend un suffixe `_2`, `_3`... (`written` rend le
/// vrai chemin). L'EDL garde chaque clip entier, tous ses marqueurs en `* LOC`.
pub fn export(
    format: EditFormat,
    clips: &[EditClip],
    opts: &EditOptions,
    out: &Path,
) -> Result<EditResult> {
    std::fs::create_dir_all(out)?;
    let mut opts = opts.clone();
    // Base absente ou vide : le dossier du fichier écrit, seul à donner des
    // chemins relatifs justes.
    if opts.base.as_ref().is_none_or(|b| b.as_os_str().is_empty()) {
        opts.base = Some(out.to_path_buf());
    }
    let mut clips = clips.to_vec();
    if !opts.markers {
        for c in &mut clips {
            c.log.markers.clear();
        }
    }
    let stem = file_stem(&opts.title);
    let ext = format.extension();
    let mut res = EditResult::default();
    let mut files: Vec<(String, String)> = Vec::new();
    match format {
        EditFormat::Fcpxml => {
            res.warnings.extend(no_overlap_warning(&clips));
            files.push((stem, fcpxml::to_fcpxml(&clips, &opts)));
        }
        EditFormat::Xmeml => {
            res.warnings.extend(no_overlap_warning(&clips));
            res.warnings.extend(xmeml::rounding_warnings(&clips));
            files.push((stem, xmeml::to_xmeml(&clips, &opts)));
        }
        EditFormat::Edl => {
            if mixed_rates(&clips) {
                res.warnings.push(Warning::new(
                    "edl_mixed_rates",
                    "EDL : les cadences diffèrent, tout est ramené à celle du premier clip",
                ));
            }
            let logs: Vec<LogClip> = clips.iter().map(|c| c.log.clone()).collect();
            files.push((stem, to_edl_whole(&opts.title, &logs)));
        }
        EditFormat::Ale => {
            let ales = ale_files(&clips, opts.ale_ascii);
            if ales.len() > 1 {
                res.warnings.push(Warning::new(
                    "ale_mixed_rates",
                    format!(
                        "ALE : {} cadences différentes, un fichier par cadence",
                        ales.len()
                    ),
                ));
            }
            let split = ales.len() > 1;
            for (rate, text) in ales {
                let name = if split {
                    format!("{stem}_{}", fps_text(rate))
                } else {
                    stem.clone()
                };
                files.push((name, text));
            }
        }
        EditFormat::Otio => {
            res.warnings.extend(no_overlap_warning(&clips));
            files.push((stem, otio::to_otio_bin(&clips, &opts)));
        }
        EditFormat::ResolveCsv => files.push((stem, resolve_csv::to_resolve_csv(&clips))),
    }
    for (name, text) in files {
        match write_new(out, &name, ext, &text) {
            Ok(path) => res.written.push(path),
            Err(e) => return Err(with_written(e, &res.written)),
        }
    }
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::timecode::FrameRate;
    use testutil::clip;

    const ALL: [EditFormat; 6] = [
        EditFormat::Fcpxml,
        EditFormat::Xmeml,
        EditFormat::Edl,
        EditFormat::Ale,
        EditFormat::Otio,
        EditFormat::ResolveCsv,
    ];

    #[test]
    fn export_writes_extension() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("sous/dossier");
        let clips = [clip("A", FrameRate::new(25, 1), 0, false)];
        let opts = EditOptions {
            title: "Jour 1: A/B".into(),
            ..Default::default()
        };
        for f in ALL {
            let r = export(f, &clips, &opts, &out).unwrap();
            assert_eq!(r.written.len(), 1);
            let p = &r.written[0];
            assert_eq!(p.extension().unwrap(), f.extension());
            assert_eq!(p.file_stem().unwrap(), "Jour 1_ A_B");
            let text = std::fs::read_to_string(p).unwrap();
            assert!(!text.is_empty() && !text.starts_with('\u{feff}'));
        }
        // Cadences mêlées : un ALE par cadence, avec avertissement.
        let mixed = [
            clip("A", FrameRate::new(25, 1), 0, false),
            clip("B", FrameRate::new(24000, 1001), 0, false),
        ];
        let r = export(EditFormat::Ale, &mixed, &opts, &out).unwrap();
        assert_eq!(r.written.len(), 2);
        assert_eq!(r.warnings.len(), 1);
        assert_eq!(r.warnings[0].code, "ale_mixed_rates");
        let j = serde_json::to_value(&r.warnings[0]).unwrap();
        assert!(j["detail"].as_str().unwrap().starts_with("ALE"));
        assert!(r.written[1].ends_with("Jour 1_ A_B_23.976.ale"));
    }

    #[test]
    fn edl_whole_clips_2997df() {
        let mut a = clip("A", FrameRate::new(30000, 1001), 107_892, true);
        a.log.markers = vec![testutil::marker(
            &a,
            10,
            Some((10, 19)),
            crate::player::logs::Color::Red,
        )];
        let dir = tempfile::tempdir().unwrap();
        let opts = EditOptions {
            markers: false,
            ..Default::default()
        };
        export(EditFormat::Edl, &[a], &opts, dir.path()).unwrap();
        let edl = std::fs::read_to_string(dir.path().join("VERIFLOW.edl")).unwrap();
        assert!(edl.contains("FCM: DROP FRAME"));
        // Clip entier (100 images) malgré la plage des marqueurs.
        assert_eq!(edl.matches("A001").count(), 1);
        assert!(edl.contains("01:00:00;00 01:00:03;10 01:00:00;00 01:00:03;10"));
    }

    #[test]
    fn edl_markers_keep_whole_clip() {
        let mut a = clip("A", FrameRate::new(25, 1), 90_000, false);
        a.log.markers = vec![
            testutil::marker(&a, 10, Some((10, 19)), crate::player::logs::Color::Red),
            testutil::marker(&a, 50, None, crate::player::logs::Color::Green),
            testutil::marker(&a, 80, Some((80, 89)), crate::player::logs::Color::Blue),
        ];
        let dir = tempfile::tempdir().unwrap();
        // markers = true par défaut.
        export(EditFormat::Edl, &[a], &EditOptions::default(), dir.path()).unwrap();
        let edl = std::fs::read_to_string(dir.path().join("VERIFLOW.edl")).unwrap();
        // Un seul événement, clip entier (100 images), sans perte de marqueur.
        assert_eq!(edl.matches("* FROM CLIP NAME").count(), 1);
        assert!(edl.contains("01:00:00:00 01:00:04:00 01:00:00:00 01:00:04:00"));
        assert_eq!(edl.matches("* LOC:").count(), 3);
        assert!(edl.contains("* LOC: 01:00:00:10 RED"));
        assert!(edl.contains("* LOC: 01:00:02:00 GREEN"));
        assert!(edl.contains("* LOC: 01:00:03:05 BLUE"));
    }

    #[test]
    fn otio_relative_url_uses_output_dir() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = clip("A", FrameRate::new(25, 1), 0, false);
        a.log.path = dir.path().join("rushes/J1/A.mov").display().to_string();
        let out = dir.path().join("montage");
        for base in [None, Some(PathBuf::new())] {
            let opts = EditOptions {
                paths: PathMode::Relative,
                base,
                ..Default::default()
            };
            let r = export(EditFormat::Otio, std::slice::from_ref(&a), &opts, &out).unwrap();
            let v: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&r.written[0]).unwrap()).unwrap();
            assert_eq!(
                v["children"][0]["media_references"]["DEFAULT_MEDIA"]["target_url"],
                "../rushes/J1/A.mov"
            );
        }
    }

    #[test]
    fn export_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        let a = [clip("A", FrameRate::new(25, 1), 0, false)];
        let opts = EditOptions::default();
        let first = export(EditFormat::Edl, &a, &opts, dir.path()).unwrap();
        let first_text = std::fs::read_to_string(&first.written[0]).unwrap();
        std::fs::write(&first.written[0], "ORIGINAL").unwrap();
        let second = export(EditFormat::Edl, &a, &opts, dir.path()).unwrap();
        let third = export(EditFormat::Edl, &a, &opts, dir.path()).unwrap();
        assert_eq!(second.written[0], dir.path().join("VERIFLOW_2.edl"));
        assert_eq!(third.written[0], dir.path().join("VERIFLOW_3.edl"));
        assert_eq!(
            std::fs::read_to_string(&first.written[0]).unwrap(),
            "ORIGINAL"
        );
        assert_eq!(
            std::fs::read_to_string(&second.written[0]).unwrap(),
            first_text
        );
    }

    #[test]
    fn file_stem_reserved_windows_names() {
        for (title, stem) in [
            ("CON", "_CON"),
            ("nul", "_nul"),
            ("Lpt3.v2", "_Lpt3.v2"),
            ("com1", "_com1"),
            ("Console", "Console"),
            ("COM10", "COM10"),
            ("COM0", "COM0"),
            ("", "VERIFLOW"),
        ] {
            assert_eq!(file_stem(title), stem, "{title}");
        }
    }

    #[test]
    fn file_stem_truncated_on_char_boundary() {
        // « é » fait 2 octets : 150 caractères = 300 octets.
        let long = "é".repeat(150);
        let stem = file_stem(&long);
        assert!(stem.len() <= STEM_MAX_BYTES && stem.len() >= STEM_MAX_BYTES - 1);
        assert!(stem.chars().all(|c| c == 'é'));
        assert_eq!(file_stem(&"a".repeat(500)).len(), STEM_MAX_BYTES);
    }

    #[test]
    fn failed_format_lists_written_files() {
        let dir = tempfile::tempdir().unwrap();
        // Cadences mêlées : deux ALE ; le second nom est pris par un dossier.
        let mixed = [
            clip("A", FrameRate::new(25, 1), 0, false),
            clip("B", FrameRate::new(24000, 1001), 0, false),
        ];
        std::fs::create_dir(dir.path().join("VERIFLOW_23.976.ale")).unwrap();
        let err = export(EditFormat::Ale, &mixed, &EditOptions::default(), dir.path());
        // Le dossier occupe le nom : le fichier suivant prend le suffixe _2, pas d'échec.
        assert!(err.is_ok());
        // Sortie qui n'est pas un dossier : échec sans fichier écrit, erreur brute.
        let file = dir.path().join("plein");
        std::fs::write(&file, "x").unwrap();
        assert!(export(EditFormat::Edl, &mixed, &EditOptions::default(), &file).is_err());
        // Message avec la liste des fichiers déjà écrits.
        let e = with_written(
            crate::Error::Io(std::io::Error::other("disque plein")),
            &[dir.path().join("a.fcpxml"), dir.path().join("b.edl")],
        );
        assert!(e.to_string().ends_with("(déjà écrits : a.fcpxml, b.edl)"));
    }

    #[test]
    fn no_overlap_warning_once_with_names() {
        let dir = tempfile::tempdir().unwrap();
        let mut a = clip("A", FrameRate::new(25, 1), 0, false);
        a.sound = Some(testutil::sound(30.0));
        let mut b = clip("B", FrameRate::new(25, 1), 0, false);
        b.sound = Some(testutil::sound(40.0));
        let ok = clip("C", FrameRate::new(25, 1), 0, false);
        let r = export(
            EditFormat::Fcpxml,
            &[a, b, ok],
            &EditOptions::default(),
            dir.path(),
        )
        .unwrap();
        assert_eq!(r.warnings.len(), 1);
        assert_eq!(r.warnings[0].code, "sound_no_overlap");
        assert_eq!(r.warnings[0].clips, ["A", "B"]);
        assert!(r.warnings[0].detail.contains("A, B"));
    }

    /// Écrit un exemple de chaque format dans `$VERIFLOW_EXPORTS_DUMP`
    /// (contrôle externe, voir `scripts/validate-exports.py`).
    #[test]
    fn dump_for_external_check() {
        let Some(dir) = std::env::var_os("VERIFLOW_EXPORTS_DUMP") else {
            return;
        };
        let mut a = clip("A001C001", FrameRate::new(25, 1), 900_000, false);
        a.meta.insert("scene".into(), "12A".into());
        a.meta.insert("take".into(), "3".into());
        a.log.markers = vec![
            testutil::marker(&a, 10, None, crate::player::logs::Color::Red),
            testutil::marker(&a, 20, Some((20, 29)), crate::player::logs::Color::Green),
        ];
        let mut b = clip("A001C002", FrameRate::new(25, 1), 905_000, false);
        b.sound = Some(testutil::sound(0.2));
        for f in ALL {
            export(
                f,
                &[a.clone(), b.clone()],
                &EditOptions::default(),
                Path::new(&dir),
            )
            .unwrap();
        }
    }
}
