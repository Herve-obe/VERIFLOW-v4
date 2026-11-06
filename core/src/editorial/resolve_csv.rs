//! CSV de métadonnées pour DaVinci Resolve (Import Metadata from File) : une
//! ligne par clip, appariée au média par « File Name ». Séparateur virgule,
//! UTF-8 sans BOM.
//!
//! Les intitulés ci-dessous sont ceux de l'export CSV de Resolve tels qu'on
//! les connaît ; ils restent à confirmer sur un vrai export de Resolve
//! (voir docs/TESTS_EN_ATTENTE.md).

use super::model::EditClip;

const COLUMNS: [&str; 10] = [
    "File Name",
    "Clip Directory",
    "Reel Name",
    "Scene",
    "Shot",
    "Take",
    "Camera #",
    "Good Take",
    "Comments",
    "Keywords",
];

fn cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// CSV Resolve : en-tête puis un clip par ligne, fins de ligne CRLF.
pub fn to_resolve_csv(clips: &[EditClip]) -> String {
    let mut out = COLUMNS.join(",");
    out += "\r\n";
    for c in clips {
        let meta = |k: &str| c.meta.get(k).map(|s| s.trim()).unwrap_or("");
        // Les chemins Windows se coupent aussi sous Linux (tests).
        let (dir, file) = match c.log.path.rfind(['/', '\\']) {
            Some(i) => (&c.log.path[..i], &c.log.path[i + 1..]),
            None => ("", c.log.path.as_str()),
        };
        let good = matches!(
            meta("circled").to_lowercase().as_str(),
            "true" | "1" | "oui" | "yes" | "vrai"
        );
        let row = [
            file,
            dir,
            c.log.reel.as_deref().unwrap_or(&c.log.name),
            meta("scene"),
            meta("shot"),
            meta("take"),
            meta("camera"),
            if good { "true" } else { "false" },
            meta("comment"),
            meta("keywords"),
        ];
        out += &row.iter().map(|s| cell(s)).collect::<Vec<_>>().join(",");
        out += "\r\n";
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editorial::testutil::clip;
    use crate::player::timecode::FrameRate;

    #[test]
    fn resolve_csv_no_bom_header() {
        let mut a = clip("A", FrameRate::new(25, 1), 0, false);
        a.meta.insert("scene".into(), "12".into());
        a.meta.insert("circled".into(), "true".into());
        a.meta.insert("comment".into(), "bonne, \"nette\"".into());
        let csv = to_resolve_csv(&[a]);
        assert!(!csv.starts_with('\u{feff}'));
        let mut lines = csv.lines();
        assert_eq!(
            lines.next().unwrap(),
            "File Name,Clip Directory,Reel Name,Scene,Shot,Take,Camera #,Good Take,Comments,Keywords"
        );
        assert_eq!(
            lines.next().unwrap(),
            "A.mov,/m,A001,12,,,,true,\"bonne, \"\"nette\"\"\","
        );
    }
}
