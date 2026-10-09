//! Rapports d'exemple pour contrôler la mise en page :
//! `cargo run -p veriflow-core --example report_demo -- <dossier>`.
use std::collections::BTreeMap;
use veriflow_core::report::pdf::{render, Branding};
use veriflow_core::report::{Lang, Report, ReportKind, Row};

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("dossier de sortie"));
    for kind in [ReportKind::Image, ReportKind::Sound] {
        for full in [false, true] {
            let mut r = Report::new(kind);
            if full {
                r.columns = veriflow_core::report::catalog(kind)
                    .iter()
                    .map(|c| c.key.to_string())
                    .collect();
            }
            r.number = 3;
            r.tracks = if kind == ReportKind::Sound { 10 } else { 8 };
            for (k, v) in [
                ("date", "2026-10-09"),
                ("title", "La traversée"),
                ("director", "Camille Martin"),
                ("dop", "Léa Robert"),
                ("operator", "Hugo Petit"),
                ("camera", "Sony FX6"),
                ("image_format", "3840x2160 XAVC S-I"),
                ("sound_ref", "-20"),
                ("definition", "4K"),
                ("fps", "25"),
                ("media", "CFexpress"),
                ("backup", "SSD-02"),
                ("sound_engineer", "Noé Bernard"),
                ("boom", "Inès Garcia"),
                ("film", ""),
                ("recorder", "Sound Devices 888"),
                ("timecode", "25 NDF, rec run"),
                ("sample_rate", "48"),
                ("bits", "24"),
                (
                    "remarks",
                    "Vent fort l'après-midi, bonnette Rycote sur la perche.",
                ),
            ] {
                r.header.insert(k.into(), v.into());
            }
            for i in 0..9 {
                let mut f = BTreeMap::new();
                f.insert("id".to_string(), format!("T{:03}", i + 1));
                f.insert("file".to_string(), format!("A001C{:03}_261009.MXF", i + 1));
                f.insert("scene".to_string(), format!("12/{}", i / 3 + 1));
                f.insert("take".to_string(), format!("{}", i % 3 + 1));
                f.insert("tc_in".to_string(), format!("10:{:02}:12:05", 10 + i));
                f.insert("tc_out".to_string(), format!("10:{:02}:48:17", 10 + i));
                f.insert("duration".to_string(), "00:00:36:12".to_string());
                f.insert("audio".to_string(), "Audio".to_string());
                if i % 3 == 2 {
                    f.insert("circled".to_string(), "●".to_string());
                }
                f.insert("track_1".to_string(), "Mix".to_string());
                f.insert("track_2".to_string(), "Perche".to_string());
                f.insert("track_3".to_string(), "HF Léa".to_string());
                f.insert("track_10".to_string(), "Ambiance".to_string());
                f.insert(
                    "notes".to_string(),
                    if i == 4 {
                        "Avion au début, à refaire si possible".to_string()
                    } else {
                        String::new()
                    },
                );
                r.rows.push(Row {
                    clip: None,
                    fields: f,
                });
            }
            let pdf = render(
                &r,
                Lang::Fr,
                &Branding {
                    organization: "Nom de la production".into(),
                    ..Default::default()
                },
            )
            .unwrap();
            std::fs::write(
                out.join(format!(
                    "{kind:?}-{}.pdf",
                    if full { "Toutes" } else { "Base" }
                )),
                pdf,
            )
            .unwrap();
        }
    }
}
