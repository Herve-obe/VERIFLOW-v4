//! Champs de métadonnées éditables (charte §7.2).
//!
//! Chaque champ indique sa correspondance dans les formats d'échange :
//! colonne ALE (Avid, Resolve), élément iXML (son de tournage) et propriété
//! XMP (sidecar lu par Premiere, Resolve, Lightroom).

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Text,
    Bool,
    Number,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Applies {
    Video,
    Audio,
    Both,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct FieldDef {
    pub id: &'static str,
    pub group: &'static str,
    pub kind: FieldKind,
    pub applies: Applies,
    /// Colonne ALE.
    pub ale: Option<&'static str>,
    /// Élément iXML (sous BWFXML).
    pub ixml: Option<&'static str>,
    /// Propriété XMP (préfixe:nom).
    pub xmp: Option<&'static str>,
}

const fn f(
    id: &'static str,
    group: &'static str,
    kind: FieldKind,
    applies: Applies,
    ale: Option<&'static str>,
    ixml: Option<&'static str>,
    xmp: Option<&'static str>,
) -> FieldDef {
    FieldDef {
        id,
        group,
        kind,
        applies,
        ale,
        ixml,
        xmp,
    }
}

use Applies::*;
use FieldKind::*;

/// Liste ordonnée des champs (l'ordre est celui de l'inspecteur).
pub const FIELDS: &[FieldDef] = &[
    // Production
    f(
        "project",
        "production",
        Text,
        Both,
        Some("Project"),
        Some("PROJECT"),
        Some("xmpDM:projectName"),
    ),
    f(
        "shoot_day",
        "production",
        Text,
        Both,
        Some("Shoot Day"),
        None,
        Some("xmpDM:shotDay"),
    ),
    f(
        "shoot_date",
        "production",
        Text,
        Both,
        Some("Shoot Date"),
        None,
        Some("xmpDM:shotDate"),
    ),
    f(
        "production_company",
        "production",
        Text,
        Both,
        None,
        None,
        None,
    ),
    f(
        "director",
        "production",
        Text,
        Both,
        Some("Director"),
        None,
        Some("xmpDM:director"),
    ),
    f(
        "dop",
        "production",
        Text,
        Video,
        Some("DOP"),
        None,
        Some("xmpDM:directorPhotography"),
    ),
    f(
        "sound_mixer",
        "production",
        Text,
        Audio,
        Some("Sound Mixer"),
        None,
        Some("xmpDM:engineer"),
    ),
    f("dit", "production", Text, Video, None, None, None),
    // Identification
    f(
        "reel",
        "identification",
        Text,
        Both,
        Some("Tape"),
        Some("TAPE"),
        Some("xmpDM:tapeName"),
    ),
    f(
        "clip_name",
        "identification",
        Text,
        Both,
        Some("Name"),
        None,
        Some("dc:title"),
    ),
    f(
        "camera",
        "identification",
        Text,
        Video,
        Some("Camera"),
        None,
        Some("xmpDM:cameraLabel"),
    ),
    f("recorder", "identification", Text, Audio, None, None, None),
    // Découpage
    f("sequence", "breakdown", Text, Both, None, None, None),
    f(
        "scene",
        "breakdown",
        Text,
        Both,
        Some("Scene"),
        Some("SCENE"),
        Some("xmpDM:scene"),
    ),
    f(
        "shot",
        "breakdown",
        Text,
        Both,
        Some("Shot"),
        None,
        Some("xmpDM:shotName"),
    ),
    f(
        "take",
        "breakdown",
        Text,
        Both,
        Some("Take"),
        Some("TAKE"),
        Some("xmpDM:takeNumber"),
    ),
    f(
        "circled",
        "breakdown",
        Bool,
        Both,
        Some("Circled"),
        Some("CIRCLED"),
        Some("xmpDM:good"),
    ),
    f("false_start", "breakdown", Bool, Both, None, None, None),
    f("wild_track", "breakdown", Bool, Audio, None, None, None),
    f("mos", "breakdown", Bool, Video, Some("MOS"), None, None),
    // Image
    f(
        "lens",
        "image",
        Text,
        Video,
        Some("Lens"),
        None,
        Some("xmpDM:lens"),
    ),
    f(
        "focal_length",
        "image",
        Text,
        Video,
        Some("Focal Length"),
        None,
        None,
    ),
    f("t_stop", "image", Text, Video, Some("T-Stop"), None, None),
    f(
        "focus_distance",
        "image",
        Text,
        Video,
        Some("Focus Distance"),
        None,
        None,
    ),
    f("iso", "image", Text, Video, Some("ISO"), None, None),
    f("shutter", "image", Text, Video, Some("Shutter"), None, None),
    f(
        "white_balance",
        "image",
        Text,
        Video,
        Some("White Balance"),
        None,
        None,
    ),
    f("tint", "image", Text, Video, None, None, None),
    f("filters", "image", Text, Video, Some("Filter"), None, None),
    f("lut", "image", Text, Video, Some("LUT"), None, None),
    f(
        "capture_fps",
        "image",
        Text,
        Video,
        Some("Sensor FPS"),
        None,
        None,
    ),
    // Son
    f("mics", "sound", Text, Audio, None, None, None),
    f("tc_rate", "sound", Text, Audio, None, None, None),
    // Timecode
    f("user_bits", "timecode", Text, Both, None, None, None),
    // Notes
    f(
        "comment",
        "notes",
        Text,
        Both,
        Some("Comments"),
        Some("NOTE"),
        Some("xmpDM:logComment"),
    ),
    f(
        "rating",
        "notes",
        Number,
        Both,
        None,
        None,
        Some("xmp:Rating"),
    ),
    f(
        "keywords",
        "notes",
        Text,
        Both,
        Some("Keywords"),
        None,
        Some("dc:subject"),
    ),
    f(
        "color_label",
        "notes",
        Text,
        Both,
        None,
        None,
        Some("xmp:Label"),
    ),
];

/// Préfixe des champs de noms de pistes : « track.1 », « track.2 »...
pub const TRACK_PREFIX: &str = "track.";

pub fn field(id: &str) -> Option<&'static FieldDef> {
    FIELDS.iter().find(|f| f.id == id)
}

/// Vrai pour un identifiant de champ accepté (champ connu ou nom de piste).
pub fn is_valid(id: &str) -> bool {
    field(id).is_some()
        || id
            .strip_prefix(TRACK_PREFIX)
            .and_then(|n| n.parse::<u16>().ok())
            .is_some_and(|n| (1..=512).contains(&n))
}

/// Normalise une valeur selon le type du champ (booléens « true »/« false »).
pub fn normalize(id: &str, value: &str) -> String {
    let v = value.trim();
    match field(id).map(|f| f.kind) {
        Some(FieldKind::Bool) => {
            let yes = matches!(
                v.to_lowercase().as_str(),
                "true" | "1" | "oui" | "yes" | "x" | "vrai"
            );
            if v.is_empty() {
                String::new()
            } else if yes {
                "true".into()
            } else {
                "false".into()
            }
        }
        _ => v.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_validated() {
        let mut ids: Vec<_> = FIELDS.iter().map(|f| f.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), FIELDS.len());
        assert!(
            is_valid("scene")
                && is_valid("track.32")
                && !is_valid("track.0")
                && !is_valid("inconnu")
        );
        assert_eq!(normalize("circled", "Oui"), "true");
        assert_eq!(normalize("circled", "non"), "false");
        assert_eq!(normalize("scene", " 12A "), "12A");
    }
}
