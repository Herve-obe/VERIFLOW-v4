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
use crate::player::logs::{fps_text, to_edl, LogClip};
use crate::Result;

/// Avertissement structuré : `code` permet à l'interface de le traduire
/// (`ale_mixed_rates`, `edl_mixed_rates`, `xmeml_rounding`), `detail` garde le
/// texte français complet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Warning {
    pub code: String,
    pub detail: String,
}

impl Warning {
    pub fn new(code: &str, detail: impl Into<String>) -> Self {
        Self {
            code: code.to_owned(),
            detail: detail.into(),
        }
    }
}

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
    let s = s.trim_matches(['.', ' ']).to_owned();
    if s.is_empty() {
        "VERIFLOW".into()
    } else {
        s
    }
}

fn mixed_rates(clips: &[EditClip]) -> bool {
    let mut rates = clips.iter().filter(|c| c.log.has_video).map(|c| c.log.rate);
    rates.next().is_some_and(|first| rates.any(|r| r != first))
}

/// Écrit `clips` au `format` voulu dans le dossier `out` (créé au besoin),
/// sous le nom du titre. Les chemins relatifs le sont par rapport à
/// `opts.base`, à défaut au dossier `out`. Un ALE n'ayant qu'une cadence, des
/// cadences mêlées donnent un fichier par cadence.
pub fn export(
    format: EditFormat,
    clips: &[EditClip],
    opts: &EditOptions,
    out: &Path,
) -> Result<EditResult> {
    std::fs::create_dir_all(out)?;
    let mut opts = opts.clone();
    if opts.base.is_none() {
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
        EditFormat::Fcpxml => files.push((stem, fcpxml::to_fcpxml(&clips, &opts))),
        EditFormat::Xmeml => {
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
            files.push((stem, to_edl(&opts.title, &logs)));
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
        EditFormat::Otio => files.push((stem, otio::to_otio_bin(&clips, &opts))),
        EditFormat::ResolveCsv => files.push((stem, resolve_csv::to_resolve_csv(&clips))),
    }
    for (name, text) in files {
        let path = out.join(format!("{name}.{ext}"));
        std::fs::write(&path, text)?;
        res.written.push(path);
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
