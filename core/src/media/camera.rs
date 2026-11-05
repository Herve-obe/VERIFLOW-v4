//! Réglages caméra lus dans les fichiers : sidecar Sony (`<clip>M01.XML`), Panasonic P2
//! (`CONTENTS/CLIP/<clip>.XML`) et étiquettes QuickTime (Blackmagic, Apple).
//! Lecture seule, sans erreur bloquante : une source absente ou illisible est ignorée.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

/// Taille maximale lue dans un sidecar XML (contenu non fiable).
const MAX_XML: u64 = 1 << 20;
/// Garde-fous sur le nombre d'entrées et la longueur des valeurs.
const MAX_ENTRIES: usize = 400;
const MAX_VALUE: usize = 200;

/// Groupes dont le contenu est retenu.
const GROUPS: [&str; 3] = ["Device", "CameraUnitMetadata", "LensUnitMetadata"];

/// Réglages caméra d'un fichier.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CameraMeta {
    /// Champs normalisés, avec les identifiants de `fields` (iso, shutter, camera...).
    pub fields: BTreeMap<String, String>,
    /// Valeurs brutes telles que lues (`CameraUnitMetadata.ISOSensitivity`, étiquettes QuickTime).
    pub raw: BTreeMap<String, String>,
}

/// Lit les réglages caméra d'un média. `tags` vient de FFprobe (vide si inconnu).
/// Au plus deux ouvertures de fichier, aucun parcours de dossier.
pub fn camera_meta(path: &Path, tags: &BTreeMap<String, String>) -> CameraMeta {
    let mut raw = BTreeMap::new();
    for p in sidecar_candidates(path) {
        if let Some(xml) = read_capped(&p) {
            parse_sidecar(&xml, &mut raw);
            if !raw.is_empty() {
                break;
            }
        }
    }
    // Les étiquettes alimentent les champs normalisés, mais pas `raw` (réservé aux sidecars).
    let mut all = raw.clone();
    for (k, v) in tags {
        let key = k.to_lowercase();
        let v = v.trim();
        if v.is_empty() {
            continue;
        }
        if key.starts_with("com.blackmagic-design.camera.")
            || key == "com.apple.quicktime.make"
            || key == "com.apple.quicktime.model"
        {
            all.entry(key).or_insert_with(|| clip(v));
        }
    }
    let fields = map_fields(&all);
    CameraMeta { fields, raw }
}

/// Emplacements possibles du sidecar : Sony à côté du clip, P2 dans `CONTENTS/CLIP`.
fn sidecar_candidates(path: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let (Some(dir), Some(stem)) = (path.parent(), path.file_stem()) else {
        return out;
    };
    let mut sony = stem.to_os_string();
    sony.push("M01.XML");
    out.push(dir.join(sony));
    let in_video = dir
        .file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("VIDEO"));
    if let (true, Some(contents)) = (in_video, dir.parent()) {
        if contents
            .file_name()
            .is_some_and(|n| n.eq_ignore_ascii_case("CONTENTS"))
        {
            out.push(
                contents
                    .join("CLIP")
                    .join(Path::new(stem).with_extension("XML")),
            );
        }
    }
    out
}

/// Lit au plus 1 Mo d'un fichier texte ; `None` si absent ou illisible.
fn read_capped(path: &Path) -> Option<String> {
    let mut buf = Vec::new();
    File::open(path)
        .ok()?
        .take(MAX_XML)
        .read_to_end(&mut buf)
        .ok()?;
    Some(String::from_utf8_lossy(&buf).into_owned())
}

fn clip(v: &str) -> String {
    v.chars().take(MAX_VALUE).collect()
}

fn local(name: &[u8]) -> String {
    let s = String::from_utf8_lossy(name);
    s.rsplit(':').next().unwrap_or_default().to_owned()
}

fn attr(e: &BytesStart, name: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| local(a.key.as_ref()).eq_ignore_ascii_case(name))
        .and_then(|a| a.unescape_value().ok())
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty())
}

/// Nom de groupe normalisé (`CameraUnitMetadataSet` devient `CameraUnitMetadata`).
fn group_name(n: &str) -> Option<&'static str> {
    let n = n.strip_suffix("Set").unwrap_or(n);
    GROUPS.iter().find(|g| **g == n).copied()
}

/// Lit un sidecar Sony ou P2 dans `raw`. Tolérant : s'arrête silencieusement à la première erreur.
fn parse_sidecar(xml: &str, raw: &mut BTreeMap<String, String>) {
    let mut reader = Reader::from_str(xml);
    // Pile des éléments (nom local, ou nom du groupe Sony).
    let mut stack: Vec<String> = Vec::new();
    let mut text = String::new();
    while let Ok(ev) = reader.read_event() {
        match ev {
            Event::Start(e) => {
                open(&e, &mut stack, raw);
                text.clear();
            }
            Event::Empty(e) => {
                open(&e, &mut stack, raw);
                stack.pop();
                text.clear();
            }
            Event::Text(t) => text.push_str(&t.xml_content().unwrap_or_default()),
            Event::GeneralRef(r) => match r.decode().unwrap_or_default().as_ref() {
                "amp" => text.push('&'),
                "lt" => text.push('<'),
                "gt" => text.push('>'),
                "quot" => text.push('"'),
                "apos" => text.push('\''),
                _ => {}
            },
            Event::End(_) => {
                // Feuille P2 : chemin sous le groupe, ex. `CameraUnitMetadata.ShutterSpeed.Angle`.
                if !text.trim().is_empty() {
                    if let Some(i) = stack.iter().rposition(|s| group_name(s).is_some()) {
                        let g = group_name(&stack[i]).unwrap_or_default();
                        let mut key = g.to_owned();
                        for s in &stack[i + 1..] {
                            key.push('.');
                            key.push_str(s);
                        }
                        put(raw, key, &text);
                    }
                }
                text.clear();
                stack.pop();
            }
            Event::Eof => break,
            _ => {}
        }
    }
}

fn put(raw: &mut BTreeMap<String, String>, k: String, v: &str) {
    let v = v.trim();
    if !v.is_empty() && raw.len() < MAX_ENTRIES {
        raw.entry(k).or_insert_with(|| clip(v));
    }
}

/// Ouverture d'un élément : empile son nom et lit les attributs Sony utiles.
fn open(e: &BytesStart, stack: &mut Vec<String>, raw: &mut BTreeMap<String, String>) {
    let name = local(e.name().as_ref());
    match name.as_str() {
        "Group" => {
            let g = attr(e, "name").unwrap_or_else(|| name.clone());
            stack.push(g);
            return;
        }
        "Item" => {
            let group = stack.iter().rev().find_map(|s| group_name(s));
            if let (Some(g), Some(n), Some(v)) = (group, attr(e, "name"), attr(e, "value")) {
                put(raw, format!("{g}.{n}"), &v);
            }
        }
        "Device" | "VideoFrame" => {
            for a in e.attributes().flatten() {
                let k = local(a.key.as_ref());
                if let (false, Ok(v)) = (k.starts_with("xmlns"), a.unescape_value()) {
                    put(raw, format!("{name}.{k}"), &v);
                }
            }
        }
        _ => {}
    }
    stack.push(name);
}

/// Première valeur non vide parmi les clés données (casse ignorée).
fn first<'a>(raw: &'a BTreeMap<String, String>, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|k| {
        raw.iter()
            .find(|(rk, _)| rk.eq_ignore_ascii_case(k))
            .map(|(_, v)| v.as_str())
    })
}

/// Ajoute l'unité si la valeur est un nombre seul.
fn with_unit(v: &str, unit: &str) -> String {
    if v.parse::<f64>().is_ok() {
        format!("{v}{unit}")
    } else {
        v.to_owned()
    }
}

/// Angle d'obturation : `180.0deg` ou `180` donnent `180°`.
fn angle(v: &str) -> String {
    let n = v.trim_end_matches("deg").trim_end_matches('°').trim();
    match n.parse::<f64>() {
        Ok(a) if a.fract() == 0.0 => format!("{a:.0}°"),
        Ok(a) => format!("{a}°"),
        Err(_) => v.to_owned(),
    }
}

/// Traduit les valeurs brutes vers les identifiants de `fields`.
fn map_fields(raw: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut f = BTreeMap::new();
    let mut set = |k: &str, v: Option<String>| {
        if let Some(v) = v.filter(|v| !v.is_empty()) {
            f.insert(k.to_owned(), v);
        }
    };
    let model = first(
        raw,
        &[
            "Device.modelName",
            "Device.ModelName",
            "com.apple.quicktime.model",
            // Noms Blackmagic supposés, à confirmer sur un fichier réel.
            "com.blackmagic-design.camera.model",
            "com.blackmagic-design.camera.camera_type",
            "com.blackmagic-design.camera.camera_id",
        ],
    );
    let make = first(raw, &["com.apple.quicktime.make"]);
    set(
        "camera",
        match (make, model) {
            (Some(ma), Some(mo)) if !mo.starts_with(ma) => Some(format!("{ma} {mo}")),
            (_, Some(mo)) => Some(mo.to_owned()),
            _ => None,
        },
    );
    set(
        "iso",
        first(
            raw,
            &[
                "CameraUnitMetadata.ISOSensitivity",
                "CameraUnitMetadata.ExposureIndexOfPhotoMeter",
                "com.blackmagic-design.camera.iso",
            ],
        )
        .map(str::to_owned),
    );
    let ang = first(
        raw,
        &[
            "CameraUnitMetadata.ShutterSpeed_Angle",
            "CameraUnitMetadata.ShutterSpeedAngle",
            "CameraUnitMetadata.ShutterSpeed.Angle",
            "com.blackmagic-design.camera.shutter_angle",
        ],
    )
    .map(angle);
    let time = first(
        raw,
        &[
            "CameraUnitMetadata.ShutterSpeed_Time",
            "CameraUnitMetadata.ShutterSpeedTime",
            "CameraUnitMetadata.ShutterSpeed.Time",
            "com.blackmagic-design.camera.shutter_speed",
        ],
    )
    .map(str::to_owned);
    set("shutter", ang.or(time));
    set(
        "white_balance",
        first(
            raw,
            &[
                "CameraUnitMetadata.WhiteBalance",
                "CameraUnitMetadata.ColorTemperature",
                "com.blackmagic-design.camera.white_balance",
            ],
        )
        .map(|v| with_unit(v, "K")),
    );
    set(
        "lens",
        first(
            raw,
            &[
                "LensUnitMetadata.LensModelName",
                "LensUnitMetadata.LensAttributes",
                "com.blackmagic-design.camera.lens_type",
            ],
        )
        .map(str::to_owned),
    );
    set(
        "focal_length",
        first(
            raw,
            &[
                "LensUnitMetadata.LensZoomActualFocalLength",
                "LensUnitMetadata.FocalLength",
                "com.blackmagic-design.camera.lens_focal_length",
            ],
        )
        .map(|v| with_unit(v, " mm")),
    );
    set(
        "focus_distance",
        first(
            raw,
            &[
                "LensUnitMetadata.FocusPositionFromImagePlane",
                "LensUnitMetadata.FocusPositionFromFrontLensVertex",
                "com.blackmagic-design.camera.focus_distance",
            ],
        )
        .map(str::to_owned),
    );
    set(
        "tint",
        first(raw, &["com.blackmagic-design.camera.tint"]).map(str::to_owned),
    );
    // Cadence de capture Sony : `25p` devient `25`.
    set(
        "capture_fps",
        first(raw, &["VideoFrame.captureFps"])
            .map(|v| v.trim_end_matches(['p', 'P', 'i', 'I']).to_owned()),
    );
    f
}

#[cfg(test)]
mod tests {
    use super::*;

    const SONY: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<NonRealTimeMeta xmlns="urn:schemas-professionalDisc:nonRealTimeMeta:ver.2.20">
 <VideoFrame videoCodec="AVC_3840_2160_HP@L51" captureFps="25p" formatFps="25p"/>
 <Device manufacturer="Sony" modelName="ILME-FX6" serialNo="12345"/>
 <AcquisitionRecord>
  <Group name="CameraUnitMetadataSet">
   <Item name="ISOSensitivity" value="800"/>
   <Item name="ShutterSpeed_Angle" value="180.0deg"/>
   <Item name="WhiteBalance" value="5600K"/>
  </Group>
  <Group name="LensUnitMetadataSet">
   <Item name="LensZoomActualFocalLength" value="35"/>
  </Group>
 </AcquisitionRecord>
</NonRealTimeMeta>"#;

    fn get<'a>(m: &'a CameraMeta, k: &str) -> Option<&'a str> {
        m.fields.get(k).map(String::as_str)
    }

    #[test]
    fn reads_sony_m01_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("C0001.MP4"), b"x").unwrap();
        std::fs::write(dir.path().join("C0001M01.XML"), SONY).unwrap();
        let m = camera_meta(&dir.path().join("C0001.MP4"), &BTreeMap::new());
        assert_eq!(get(&m, "iso"), Some("800"));
        assert_eq!(get(&m, "white_balance"), Some("5600K"));
        assert_eq!(get(&m, "shutter"), Some("180°"));
        assert_eq!(get(&m, "capture_fps"), Some("25"));
        assert_eq!(get(&m, "camera"), Some("ILME-FX6"));
        assert_eq!(get(&m, "focal_length"), Some("35 mm"));
        assert_eq!(
            m.raw
                .get("CameraUnitMetadata.ISOSensitivity")
                .map(String::as_str),
            Some("800")
        );
        assert_eq!(
            m.raw.get("Device.serialNo").map(String::as_str),
            Some("12345")
        );
    }

    #[test]
    fn reads_p2_clip_xml() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("CONTENTS");
        std::fs::create_dir_all(root.join("VIDEO")).unwrap();
        std::fs::create_dir_all(root.join("CLIP")).unwrap();
        std::fs::write(root.join("VIDEO/0001AB.MXF"), b"x").unwrap();
        let xml = "<P2Main:ClipMain xmlns:P2Main=\"urn:x\"><ClipContent><Device>\
            <Manufacturer>Panasonic</Manufacturer><ModelName>AU-EVA1</ModelName></Device>\
            <Camera><CameraUnitMetadata><ISOSensitivity>1600</ISOSensitivity>\
            <ShutterSpeed><Angle>172.8</Angle></ShutterSpeed>\
            <WhiteBalance>3200</WhiteBalance></CameraUnitMetadata>\
            <LensUnitMetadata><LensModelName>CN-E 24mm</LensModelName></LensUnitMetadata>\
            </Camera></ClipContent></P2Main:ClipMain>";
        std::fs::write(root.join("CLIP/0001AB.XML"), xml).unwrap();
        let m = camera_meta(&root.join("VIDEO/0001AB.MXF"), &BTreeMap::new());
        assert_eq!(get(&m, "iso"), Some("1600"));
        assert_eq!(get(&m, "shutter"), Some("172.8°"));
        assert_eq!(get(&m, "white_balance"), Some("3200K"));
        assert_eq!(get(&m, "camera"), Some("AU-EVA1"));
        assert_eq!(get(&m, "lens"), Some("CN-E 24mm"));
    }

    #[test]
    fn maps_blackmagic_tags() {
        let mut tags = BTreeMap::new();
        tags.insert(
            "com.blackmagic-design.camera.iso".to_owned(),
            "400".to_owned(),
        );
        tags.insert(
            "com.blackmagic-design.camera.white_balance".to_owned(),
            "5600".to_owned(),
        );
        tags.insert("com.apple.quicktime.make".to_owned(), "Apple".to_owned());
        tags.insert(
            "com.apple.quicktime.model".to_owned(),
            "iPhone 15 Pro".to_owned(),
        );
        tags.insert("encoder".to_owned(), "x".to_owned());
        let m = camera_meta(Path::new("/nulle/part/A.mov"), &tags);
        assert_eq!(get(&m, "iso"), Some("400"));
        assert_eq!(get(&m, "white_balance"), Some("5600K"));
        assert_eq!(get(&m, "camera"), Some("Apple iPhone 15 Pro"));
        // Les étiquettes ne sont pas recopiées dans raw.
        assert!(m.raw.is_empty());
        tags.insert(
            "com.blackmagic-design.camera.model".to_owned(),
            "Pocket 6K".to_owned(),
        );
        tags.remove("com.apple.quicktime.make");
        tags.remove("com.apple.quicktime.model");
        let m = camera_meta(Path::new("/nulle/part/A.mov"), &tags);
        assert_eq!(get(&m, "camera"), Some("Pocket 6K"));
    }

    #[test]
    fn oversized_sidecar_is_capped() {
        let dir = tempfile::tempdir().unwrap();
        let mut xml = String::from("<NonRealTimeMeta><!--");
        xml.push_str(&"x".repeat(MAX_XML as usize));
        xml.push_str("--><Device modelName=\"ILME-FX6\"/></NonRealTimeMeta>");
        std::fs::write(dir.path().join("C0005M01.XML"), xml).unwrap();
        let m = camera_meta(&dir.path().join("C0005.MP4"), &BTreeMap::new());
        assert_eq!(m, CameraMeta::default());
    }

    #[test]
    fn missing_or_broken_sidecar_is_silent() {
        let dir = tempfile::tempdir().unwrap();
        let clip = dir.path().join("C0002.MP4");
        assert_eq!(camera_meta(&clip, &BTreeMap::new()), CameraMeta::default());
        std::fs::write(
            dir.path().join("C0002M01.XML"),
            "<NonRealTimeMeta><Device mod",
        )
        .unwrap();
        let m = camera_meta(&clip, &BTreeMap::new());
        assert!(m.fields.is_empty());
        // Dossier à la place du fichier, DOCTYPE à entité externe : rien ne plante.
        std::fs::create_dir(dir.path().join("C0003M01.XML")).unwrap();
        assert_eq!(
            camera_meta(&dir.path().join("C0003.MP4"), &BTreeMap::new()),
            CameraMeta::default()
        );
        std::fs::write(
            dir.path().join("C0004M01.XML"),
            "<!DOCTYPE x [<!ENTITY e SYSTEM \"file:///etc/passwd\">]><NonRealTimeMeta>\
             <Device modelName=\"&e;\"/></NonRealTimeMeta>",
        )
        .unwrap();
        let m = camera_meta(&dir.path().join("C0004.MP4"), &BTreeMap::new());
        assert!(!m.raw.values().any(|v| v.contains("root:")));
    }
}
