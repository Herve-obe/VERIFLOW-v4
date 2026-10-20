//! Reconnaissance de la carte source : type de caméra d'après l'arborescence
//! de la carte, bobine et lettre de caméra d'après le nom des clips. Sert aux
//! variables `{bobine}`, `{support}` et `{camera}` du modèle d'arborescence.
//!
//! Seuls la structure des dossiers et le nom des fichiers sont lus : aucune
//! métadonnée propriétaire. Un support inconnu n'a ni type ni bobine.

use std::path::Path;

use serde::Serialize;

/// Ce qui a été reconnu sur la carte.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct CardInfo {
    /// Type de support (« Sony XAVC », « RED »...).
    pub kind: Option<String>,
    /// Bobine (« A001 ») d'après le nom des clips.
    pub reel: Option<String>,
    /// Lettre de la caméra (« A »), première lettre de la bobine.
    pub camera: Option<String>,
}

/// Dossiers parcourus au plus (profondeur 4) : la reconnaissance reste
/// instantanée, même sur un disque entier choisi par erreur.
const MAX_ENTRIES: usize = 2000;

/// Marqueurs d'arborescence, du plus précis au plus général. Chemin relatif à
/// la racine de la carte (casse ignorée).
const MARKERS: &[(&str, &str)] = &[
    ("PRIVATE/M4ROOT", "Sony XAVC"),
    ("PRIVATE/XDROOT", "Sony XDCAM"),
    ("XDROOT", "Sony XDCAM"),
    ("CONTENTS/CLIPS001", "Canon XF"),
    ("CONTENTS/VIDEO", "Panasonic P2"),
    ("PRIVATE/AVCHD", "AVCHD"),
];

pub fn detect(root: &Path) -> CardInfo {
    let mut kind = MARKERS
        .iter()
        .find(|(rel, _)| exists_nocase(root, rel))
        .map(|(_, k)| k.to_string());
    let mut reel = None;
    let walker = walkdir::WalkDir::new(root)
        .max_depth(4)
        .sort_by_file_name()
        .into_iter()
        .filter_map(|e| e.ok())
        .take(MAX_ENTRIES);
    for e in walker {
        let name = e.file_name().to_string_lossy().to_string();
        let upper = name.to_ascii_uppercase();
        let ext = upper.rsplit_once('.').map(|(_, x)| x).unwrap_or("");
        if kind.is_none() {
            kind = match ext {
                "RDM" | "RDC" | "R3D" => Some("RED"),
                "BRAW" => Some("Blackmagic RAW"),
                "ARI" | "ARX" => Some("ARRI"),
                _ if is_arri_clip(&upper) => Some("ARRI"),
                _ if upper.ends_with("GOPRO") && e.file_type().is_dir() => Some("GoPro"),
                _ if upper.starts_with("DJI_") => Some("DJI"),
                _ => None,
            }
            .map(str::to_owned);
        }
        if reel.is_none() {
            reel = reel_of(&upper);
        }
        if kind.is_some() && reel.is_some() {
            break;
        }
    }
    let camera = reel.as_ref().map(|r| r[..1].to_owned());
    CardInfo { kind, reel, camera }
}

/// Le chemin relatif existe, sans tenir compte de la casse (cartes FAT ou
/// exFAT lues depuis Linux, dossiers en minuscules).
fn exists_nocase(root: &Path, rel: &str) -> bool {
    let mut dir = root.to_path_buf();
    for part in rel.split('/') {
        let Ok(rd) = std::fs::read_dir(&dir) else {
            return false;
        };
        let found = rd
            .filter_map(|e| e.ok())
            .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case(part));
        match found {
            Some(e) if e.path().is_dir() => dir = e.path(),
            _ => return false,
        }
    }
    true
}

/// Bobine au début d'un nom de clip ou de dossier : une lettre et trois
/// chiffres, suivis de `C` et trois chiffres (A001C001 : ARRI, Sony Venice et
/// FX, Canon) ou d'un tiret bas (A001_C001 et A001_0101XX.RDM : RED ;
/// A001_08151230_C001 : Blackmagic).
fn reel_of(upper: &str) -> Option<String> {
    let b = upper.as_bytes();
    if b.len() < 5 || !b[0].is_ascii_uppercase() || !b[1..4].iter().all(u8::is_ascii_digit) {
        return None;
    }
    let clip = b.len() >= 8 && b[4] == b'C' && b[5..8].iter().all(u8::is_ascii_digit);
    (clip || b[4] == b'_').then(|| upper[..4].to_owned())
}

/// Nom de clip ARRI : A001C001_AAMMJJ_XXXX (lettre, bobine, clip, date, code).
fn is_arri_clip(upper: &str) -> bool {
    let b = upper.as_bytes();
    b.len() >= 20
        && reel_of(upper).is_some()
        && b[4] == b'C'
        && b[8] == b'_'
        && b[9..15].iter().all(u8::is_ascii_digit)
        && b[15] == b'_'
        && b[16..20].iter().all(u8::is_ascii_alphanumeric)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(files: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for f in files {
            let p = dir.path().join(f);
            if f.ends_with('/') {
                std::fs::create_dir_all(&p).unwrap();
            } else {
                std::fs::create_dir_all(p.parent().unwrap()).unwrap();
                std::fs::write(&p, b"x").unwrap();
            }
        }
        dir
    }

    fn info(kind: Option<&str>, reel: Option<&str>) -> CardInfo {
        CardInfo {
            kind: kind.map(str::to_owned),
            reel: reel.map(str::to_owned),
            camera: reel.map(|r| r[..1].to_owned()),
        }
    }

    #[test]
    fn recognises_common_cards() {
        let cases: &[(&[&str], CardInfo)] = &[
            (
                &["PRIVATE/M4ROOT/CLIP/C0001.MP4"],
                info(Some("Sony XAVC"), None),
            ),
            (
                &["XDROOT/Clip/B012C003_220101AB.MXF"],
                info(Some("Sony XDCAM"), Some("B012")),
            ),
            (
                &["CONTENTS/CLIPS001/A001C001_221010_ABCD.MXF"],
                info(Some("Canon XF"), Some("A001")),
            ),
            (
                &["CONTENTS/VIDEO/0001AB.MXF", "CONTENTS/CLIP/0001AB.XML"],
                info(Some("Panasonic P2"), None),
            ),
            (
                &["A002_1010AB.RDM/A002_C001_1010XY.RDC/A002_C001_1010XY_001.R3D"],
                info(Some("RED"), Some("A002")),
            ),
            (
                &["C003_10201234_C001.braw"],
                info(Some("Blackmagic RAW"), Some("C003")),
            ),
            (
                &["A001C004_261020_R1AB.mxf"],
                info(Some("ARRI"), Some("A001")),
            ),
            (&["DCIM/100GOPRO/GX010001.MP4"], info(Some("GoPro"), None)),
            (&["DCIM/100MEDIA/DJI_0001.MP4"], info(Some("DJI"), None)),
            (&["notes.txt", "IMG_0001.MOV"], info(None, None)),
        ];
        for (files, expected) in cases {
            let dir = card(files);
            assert_eq!(&detect(dir.path()), expected, "{files:?}");
        }
    }

    #[test]
    fn markers_ignore_case() {
        let dir = card(&["private/m4root/clip/"]);
        assert_eq!(detect(dir.path()).kind.as_deref(), Some("Sony XAVC"));
    }

    #[test]
    fn reel_needs_the_full_pattern() {
        assert_eq!(reel_of("A001C001.MOV").as_deref(), Some("A001"));
        assert_eq!(reel_of("A001_C001").as_deref(), Some("A001"));
        assert_eq!(reel_of("A001.MOV"), None);
        assert_eq!(reel_of("A0012C01"), None);
        assert_eq!(reel_of("IMG_0001"), None);
    }
}
