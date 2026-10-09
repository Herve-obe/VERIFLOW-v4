//! REPORT : rapports image et son (charte §7.4).
//!
//! Un rapport est un en-tête (production, réglages, supports) et un tableau
//! d'une ligne par prise. Deux modèles :
//! - **École** : reproduction des rapports papier de l'école (rapport Image,
//!   rapport Son 8 pistes), mêmes cases, même ordre ;
//! - **Pro** : mêmes en-têtes, colonnes étendues (fichier, durée, prise
//!   cerclée, faux départ, son seul, MOS, réglages caméra...).
//!
//! Les lignes sont pré-remplies depuis les médias (timecodes, iXML, champs
//! saisis dans MEDIA) puis éditées à la main. Exports : PDF (Typst, voir
//! `templates/reports/`), CSV, XLSX, HTML.

pub mod export;
pub mod pdf;

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::media::probe::probe;
use crate::media::wav::read_info;
use crate::player::timecode::{FrameRate, Timecode};
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportKind {
    Image,
    Sound,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Template {
    #[default]
    School,
    Pro,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lang {
    #[default]
    Fr,
    En,
}

/// Une ligne du rapport (une prise). Clés : voir [`columns`].
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// Média d'origine (chemin), s'il y en a un.
    #[serde(default)]
    pub clip: Option<String>,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

impl Row {
    pub fn get(&self, key: &str) -> &str {
        self.fields.get(key).map(String::as_str).unwrap_or("")
    }
}

/// Rapport enregistré dans le projet.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Report {
    #[serde(default)]
    pub id: i64,
    pub kind: ReportKind,
    /// Ancien choix École / Pro, lu dans les rapports créés avant le choix
    /// des colonnes (Pro : toutes les colonnes).
    #[serde(default)]
    pub template: Template,
    /// Numéro du rapport (attribué automatiquement, modifiable).
    #[serde(default)]
    pub number: u32,
    /// Champs d'en-tête. Clés : voir [`header_fields`].
    #[serde(default)]
    pub header: BTreeMap<String, String>,
    #[serde(default)]
    pub rows: Vec<Row>,
    /// Nombre de pistes affichées (rapport son, 1 à 32).
    #[serde(default = "default_tracks")]
    pub tracks: u8,
    /// Colonnes affichées, dans l'ordre choisi (vide : colonnes de base).
    #[serde(default)]
    pub columns: Vec<String>,
}

fn default_tracks() -> u8 {
    8
}

/// Nombre maximal de pistes d'un rapport son.
pub const MAX_TRACKS: u8 = 32;
/// Pistes par feuillet imprimé (format du rapport papier de l'école).
pub const TRACKS_PER_SHEET: usize = 8;

impl Report {
    pub fn new(kind: ReportKind) -> Self {
        Self {
            id: 0,
            kind,
            template: Template::School,
            number: 0,
            header: BTreeMap::new(),
            rows: Vec::new(),
            tracks: default_tracks(),
            columns: base_columns(kind),
        }
    }

    /// Colonnes affichées, dans l'ordre choisi.
    pub fn column_keys(&self) -> Vec<String> {
        if self.columns.is_empty() {
            match self.template {
                Template::Pro => all_columns_keys(self.kind),
                Template::School => base_columns(self.kind),
            }
        } else {
            normalize_columns(self.kind, &self.columns)
        }
    }

    pub fn header(&self, key: &str) -> &str {
        self.header.get(key).map(String::as_str).unwrap_or("")
    }

    /// Ajoute des lignes et complète les champs d'en-tête encore vides.
    pub fn add(&mut self, rows: Vec<Row>, header: BTreeMap<String, String>) {
        for (k, v) in header {
            let e = self.header.entry(k).or_default();
            if e.trim().is_empty() {
                *e = v;
            }
        }
        if self.kind == ReportKind::Sound {
            let used = rows
                .iter()
                .flat_map(|r| r.fields.keys())
                .filter_map(|k| k.strip_prefix("track_")?.parse::<u8>().ok())
                .max()
                .unwrap_or(0);
            self.tracks = self.tracks.max(used).clamp(1, MAX_TRACKS);
        }
        self.rows.extend(rows);
    }
}

// ---------- Colonnes ----------

/// Colonne du tableau : clé du champ, libellé, largeur relative.
#[derive(Debug, Clone, Serialize)]
pub struct Column {
    pub key: String,
    pub label: String,
    pub width: f32,
}

/// Colonne disponible pour un type de rapport. Les colonnes de base (celles
/// des rapports papier de l'école) sont toujours affichées ; les autres se
/// cochent dans l'éditeur. La clé « tracks » représente les colonnes de
/// pistes du rapport son.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct ColumnDef {
    pub key: &'static str,
    pub label_fr: &'static str,
    pub label_en: &'static str,
    pub width: f32,
    pub base: bool,
}

const fn def(
    key: &'static str,
    fr: &'static str,
    en: &'static str,
    width: f32,
    base: bool,
) -> ColumnDef {
    ColumnDef {
        key,
        label_fr: fr,
        label_en: en,
        width,
        base,
    }
}

/// Colonnes du rapport image, dans l'ordre proposé par défaut.
const IMAGE_COLUMNS: &[ColumnDef] = &[
    def("file", "Fichier", "File", 1.8, false),
    def("scene", "SEQ/Plan", "Scene/Shot", 0.95, true),
    def("take", "Prise", "Take", 0.75, true),
    def("tc_in", "TC IN", "TC IN", 1.55, true),
    def("tc_out", "TC OUT", "TC OUT", 1.55, true),
    def("duration", "Durée", "Duration", 1.55, false),
    def("audio", "Audio/Muet", "Sound/MOS", 0.95, true),
    def("sound_tc", "TC son", "Sound TC", 1.55, false),
    def("lens", "Objectif", "Lens", 1.15, false),
    def("focal", "Focale", "Focal", 0.85, false),
    def("tstop", "T-stop", "T-stop", 0.8, false),
    def("iso", "ISO", "ISO", 0.6, false),
    def("shutter", "Obtur.", "Shutter", 0.85, false),
    def("nd", "ND", "ND", 0.55, false),
    def("wb", "Bal. blancs", "WB", 0.9, false),
    def("flags", "FD/Seul/MOS", "FS/Wild/MOS", 0.95, false),
    def("notes", "Effets/Observations", "Effects/Notes", 3.0, true),
];

/// Colonnes du rapport son, dans l'ordre proposé par défaut.
const SOUND_COLUMNS: &[ColumnDef] = &[
    def("id", "ID", "ID", 0.9, true),
    def("file", "Fichier", "File", 1.5, false),
    def("scene", "Plan", "Scene/Shot", 0.9, true),
    def("take", "Prise", "Take", 0.6, true),
    def("tc_in", "TC IN", "TC IN", 1.35, false),
    def("duration", "Durée", "Duration", 1.35, false),
    def("flags", "FD/Seul", "FS/Wild", 0.7, false),
    def("tracks", "Pistes", "Tracks", 1.0, true),
    def("notes", "Observations", "Notes", 2.2, true),
];

/// Colonnes disponibles pour un type de rapport.
pub fn catalog(kind: ReportKind) -> &'static [ColumnDef] {
    match kind {
        ReportKind::Image => IMAGE_COLUMNS,
        ReportKind::Sound => SOUND_COLUMNS,
    }
}

/// Colonnes de base, dans l'ordre du rapport papier.
pub fn base_columns(kind: ReportKind) -> Vec<String> {
    catalog(kind)
        .iter()
        .filter(|c| c.base)
        .map(|c| c.key.to_string())
        .collect()
}

/// Colonnes de l'ancien modèle « Pro » (rapports créés avant le choix des colonnes).
fn all_columns_keys(kind: ReportKind) -> Vec<String> {
    catalog(kind).iter().map(|c| c.key.to_string()).collect()
}

/// Ordre des colonnes nettoyé : clés inconnues et doublons retirés, colonnes
/// de base manquantes ajoutées à leur place par défaut.
pub fn normalize_columns(kind: ReportKind, keys: &[String]) -> Vec<String> {
    let cat = catalog(kind);
    let mut out: Vec<String> = Vec::new();
    for k in keys {
        if cat.iter().any(|c| c.key == k) && !out.contains(k) {
            out.push(k.clone());
        }
    }
    for (i, c) in cat.iter().enumerate() {
        if c.base && !out.iter().any(|k| k == c.key) {
            // Après la colonne affichée qui la précède dans le catalogue.
            let at = cat[..i]
                .iter()
                .rev()
                .find_map(|p| out.iter().position(|k| k == p.key))
                .map_or(0, |p| p + 1);
            out.insert(at, c.key.to_string());
        }
    }
    out
}

/// Colonnes à imprimer, dans l'ordre choisi. `tracks` : numéros des pistes
/// de ce feuillet (rapport son), qui remplacent la clé « tracks ». La prise
/// cerclée n'est pas une colonne : son numéro de prise est entouré sur le PDF.
pub fn columns(kind: ReportKind, keys: &[String], tracks: &[usize], lang: Lang) -> Vec<Column> {
    let cat = catalog(kind);
    let mut out = Vec::new();
    for k in normalize_columns(kind, keys) {
        let Some(c) = cat.iter().find(|c| c.key == k) else {
            continue;
        };
        if c.key == "tracks" {
            for &n in tracks {
                out.push(Column {
                    key: format!("track_{n}"),
                    label: if lang == Lang::Fr {
                        format!("Piste {n}")
                    } else {
                        format!("Track {n}")
                    },
                    width: c.width,
                });
            }
        } else {
            out.push(Column {
                key: c.key.into(),
                label: (if lang == Lang::Fr {
                    c.label_fr
                } else {
                    c.label_en
                })
                .into(),
                width: c.width,
            });
        }
    }
    out
}

// ---------- En-tête ----------

/// Champ d'en-tête : texte libre, ou choix parmi des cases (avec « autre »).
#[derive(Debug, Clone, Serialize)]
pub struct HeaderField {
    pub key: &'static str,
    pub label_fr: &'static str,
    pub label_en: &'static str,
    /// Cases du rapport papier (vide : texte libre).
    pub options: &'static [&'static str],
    /// Valeurs supplémentaires proposées dans l'interface (« autre » sur papier).
    pub extra: &'static [&'static str],
    /// Unité affichée après les cases (mm, i/s, kHz, bits, dB FS).
    pub unit: &'static str,
}

const fn text(key: &'static str, fr: &'static str, en: &'static str) -> HeaderField {
    HeaderField {
        key,
        label_fr: fr,
        label_en: en,
        options: &[],
        extra: &[],
        unit: "",
    }
}

const fn choice(
    key: &'static str,
    fr: &'static str,
    en: &'static str,
    options: &'static [&'static str],
    extra: &'static [&'static str],
    unit: &'static str,
) -> HeaderField {
    HeaderField {
        key,
        label_fr: fr,
        label_en: en,
        options,
        extra,
        unit,
    }
}

const FPS_EXTRA: &[&str] = &["23.976", "29.97", "30", "50", "59.94", "60"];

/// Formats d'enregistrement proposés pour « Format image » (« Autre » permet
/// d'en saisir un autre).
const IMAGE_FORMATS: &[&str] = &[
    "ProRes 422 Proxy",
    "ProRes 422 LT",
    "ProRes 422",
    "ProRes 422 HQ",
    "ProRes 4444",
    "ProRes 4444 XQ",
    "ProRes RAW",
    "DNxHD",
    "DNxHR LB",
    "DNxHR SQ",
    "DNxHR HQ",
    "DNxHR HQX",
    "DNxHR 444",
    "XAVC S",
    "XAVC S-I",
    "XAVC HS",
    "XAVC Intra",
    "XAVC Long GOP",
    "XF-AVC",
    "AVC-Intra",
    "MPEG-2 (XDCAM)",
    "H.264",
    "H.265 (HEVC)",
    "Blackmagic RAW",
    "REDCODE RAW",
    "ARRIRAW",
    "Cinema DNG",
];

/// Nom courant d'un codec vidéo lu par FFprobe (« h264 » → « H.264 »).
fn codec_label(codec: &str) -> String {
    match codec {
        "h264" => "H.264".into(),
        "hevc" => "H.265 (HEVC)".into(),
        "prores" => "ProRes".into(),
        "dnxhd" => "DNxHD".into(),
        "mpeg2video" => "MPEG-2 (XDCAM)".into(),
        "mjpeg" => "MJPEG".into(),
        "av1" => "AV1".into(),
        "vp9" => "VP9".into(),
        other => other.to_uppercase(),
    }
}

const IMAGE_HEADER: &[HeaderField] = &[
    text("date", "Date", "Date"),
    text("backup", "Support de sauvegarde N°", "Backup media #"),
    text("title", "Titre du film", "Film title"),
    text("director", "Réalisateur", "Director"),
    text("dop", "Dir. Phot.", "DoP"),
    text("operator", "OPV", "Camera operator"),
    text("camera", "Caméra", "Camera"),
    choice(
        "image_format",
        "Format image",
        "Image format",
        &[],
        IMAGE_FORMATS,
        "",
    ),
    choice(
        "sound_ref",
        "Référence Son",
        "Sound reference",
        &[],
        &[],
        "dB FS",
    ),
    choice(
        "definition",
        "Image",
        "Image",
        &["HD", "4K"],
        &["2K", "6K", "8K"],
        "",
    ),
    choice("fps", "Cadence", "Frame rate", &["24", "25"], FPS_EXTRA, ""),
    choice(
        "media",
        "Media",
        "Media",
        &["SD", "P2", "SSD", "CF"],
        &["CFexpress", "SxS"],
        "",
    ),
    text("remarks", "Remarques", "Remarks"),
];

const SOUND_HEADER: &[HeaderField] = &[
    text("date", "Date", "Date"),
    text("backup", "Support de sauvegarde N°", "Backup media #"),
    text("title", "Titre du film", "Film title"),
    text("director", "Réalisateur", "Director"),
    text("sound_engineer", "Ingénieur du Son", "Sound mixer"),
    text("boom", "Perchman", "Boom operator"),
    choice("film", "Image", "Image", &["35", "16"], &[], "mm"),
    choice(
        "fps",
        "Cadence",
        "Frame rate",
        &["24", "25"],
        FPS_EXTRA,
        "i/s",
    ),
    text("recorder", "Enregistreur", "Recorder"),
    text("timecode", "Time-Code", "Timecode"),
    choice("sound_ref", "Référence", "Reference", &[], &[], "dB FS"),
    text("recorder_other", "Autre", "Other"),
    choice(
        "sample_rate",
        "Numérisation",
        "Sample rate",
        &["48", "96"],
        &["44.1", "88.2", "192"],
        "kHz",
    ),
    choice(
        "bits",
        "Résolution",
        "Bit depth",
        &["16", "24"],
        &["32 float"],
        "bits",
    ),
    text("remarks", "Remarques", "Remarks"),
];

/// Champs d'en-tête d'un type de rapport, dans l'ordre du rapport papier.
pub fn header_fields(kind: ReportKind) -> &'static [HeaderField] {
    match kind {
        ReportKind::Image => IMAGE_HEADER,
        ReportKind::Sound => SOUND_HEADER,
    }
}

// ---------- Pré-remplissage depuis les médias ----------

fn tc_string(frames: i64, rate: FrameRate, df: bool) -> String {
    Timecode::from_frames(frames, rate, df).to_string()
}

/// Cadence affichée dans un rapport (« 25 », « 23.976 »).
pub fn fps_label(rate: FrameRate) -> String {
    let v = rate.as_f64();
    if rate.den == 1 {
        rate.num.to_string()
    } else {
        format!("{:.3}", v)
            .trim_end_matches('0')
            .trim_end_matches('.')
            .into()
    }
}

/// Champs saisis dans MEDIA → champs de ligne du rapport.
fn apply_meta(row: &mut Row, meta: &BTreeMap<String, String>) {
    let get = |k: &str| meta.get(k).filter(|v| !v.trim().is_empty()).cloned();
    let mut set = |key: &str, v: Option<String>| {
        if let Some(v) = v {
            row.fields.insert(key.into(), v);
        }
    };
    // SEQ/Plan : « séquence/plan » ou scène seule, comme sur une claquette.
    let scene = match (get("scene"), get("shot")) {
        (Some(s), Some(p)) => Some(format!("{s}/{p}")),
        (s, p) => s.or(p),
    };
    set("scene", scene);
    set("take", get("take"));
    set("lens", get("lens"));
    set("focal", get("focal_length"));
    set("tstop", get("t_stop"));
    set("iso", get("iso"));
    set("shutter", get("shutter"));
    set("nd", get("filters"));
    set("wb", get("white_balance"));
    set("notes", get("comment"));
    let yes = |k: &str| get(k).is_some_and(|v| v == "true" || v == "1");
    if yes("circled") {
        row.fields.insert("circled".into(), "●".into());
    }
    let mut flags = Vec::new();
    if yes("false_start") {
        flags.push("FD");
    }
    if yes("wild_track") {
        flags.push("Seul");
    }
    if yes("mos") {
        flags.push("MOS");
        row.fields.insert("audio".into(), "Muet".into());
    }
    if !flags.is_empty() {
        row.fields.insert("flags".into(), flags.join(" "));
    }
}

/// Champs d'en-tête proposés par les métadonnées saisies dans MEDIA.
fn header_from_meta(
    kind: ReportKind,
    meta: &BTreeMap<String, String>,
    out: &mut BTreeMap<String, String>,
) {
    let mut put = |key: &str, field: &str| {
        if let Some(v) = meta.get(field).filter(|v| !v.trim().is_empty()) {
            out.entry(key.into()).or_insert_with(|| v.clone());
        }
    };
    put("title", "project");
    put("director", "director");
    put("date", "shoot_date");
    match kind {
        ReportKind::Image => {
            put("dop", "dop");
            put("camera", "camera");
        }
        ReportKind::Sound => {
            put("sound_engineer", "sound_mixer");
            put("recorder", "recorder");
        }
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Ligne et en-tête d'un clip vidéo.
fn video_row(path: &Path, header: &mut BTreeMap<String, String>) -> Result<Row> {
    let info = probe(path)?;
    let mut row = Row {
        clip: Some(path.display().to_string()),
        fields: BTreeMap::new(),
    };
    row.fields.insert("file".into(), file_name(path));
    if let Some(v) = &info.video {
        let df = info
            .start_timecode
            .as_deref()
            .is_some_and(|s| s.contains(';'));
        let start = info.start_tc().map(|t| t.frames).unwrap_or(0);
        row.fields
            .insert("tc_in".into(), tc_string(start, v.rate, df));
        row.fields.insert(
            "tc_out".into(),
            tc_string(start + v.frame_count, v.rate, df),
        );
        row.fields
            .insert("duration".into(), tc_string(v.frame_count, v.rate, df));
        header
            .entry("fps".into())
            .or_insert_with(|| fps_label(v.rate));
        let definition = match v.height {
            h if h >= 2160 => "4K".to_string(),
            h if h >= 720 => "HD".to_string(),
            _ => format!("{}x{}", v.width, v.height),
        };
        header.entry("definition".into()).or_insert(definition);
        header
            .entry("image_format".into())
            .or_insert_with(|| codec_label(&v.codec));
    }
    row.fields.insert(
        "audio".into(),
        if info.audio.is_empty() {
            "Muet"
        } else {
            "Audio"
        }
        .into(),
    );
    let tag = |k: &str| info.tags.get(k).filter(|v| !v.trim().is_empty()).cloned();
    if let Some(model) = tag("com.apple.quicktime.model").or_else(|| tag("model")) {
        header.entry("camera".into()).or_insert(model);
    }
    Ok(row)
}

/// Ligne et en-tête d'un fichier son (WAV/BWF).
fn sound_row(path: &Path, fps: FrameRate, header: &mut BTreeMap<String, String>) -> Result<Row> {
    let info = read_info(path)?;
    let mut row = Row {
        clip: Some(path.display().to_string()),
        fields: BTreeMap::new(),
    };
    let stem = path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    row.fields.insert("id".into(), stem);
    row.fields.insert("file".into(), file_name(path));
    let ix = &info.ixml;
    if let Some(s) = ix.scene.clone().filter(|s| !s.is_empty()) {
        row.fields.insert("scene".into(), s);
    }
    if let Some(t) = ix.take.clone().filter(|s| !s.is_empty()) {
        row.fields.insert("take".into(), t);
    }
    if let Some(n) = ix.note.clone().filter(|s| !s.is_empty()) {
        row.fields.insert("notes".into(), n);
    }
    if ix.circled == Some(true) {
        row.fields.insert("circled".into(), "●".into());
    }
    for c in 0..info.channels as usize {
        row.fields
            .insert(format!("track_{}", c + 1), info.track_name(c));
    }
    let sr = info.sample_rate.max(1) as f64;
    if let Some(tr) = info.time_reference {
        let frames = (tr as f64 / sr * fps.as_f64()).round() as i64;
        row.fields
            .insert("tc_in".into(), tc_string(frames, fps, false));
    }
    let dur = (info.frames as f64 / sr * fps.as_f64()).round() as i64;
    row.fields
        .insert("duration".into(), tc_string(dur, fps, false));
    let khz = info.sample_rate as f64 / 1000.0;
    let khz = if khz.fract() == 0.0 {
        format!("{khz:.0}")
    } else {
        format!("{khz}")
    };
    header.entry("sample_rate".into()).or_insert(khz);
    header.entry("bits".into()).or_insert_with(|| {
        if info.format == crate::media::wav::SampleFormat::Float {
            format!("{} float", info.bits)
        } else {
            info.bits.to_string()
        }
    });
    Ok(row)
}

/// Lignes d'un rapport pour une liste de médias, et champs d'en-tête
/// proposés. `meta` : champs saisis dans MEDIA pour un chemin. Un média
/// illisible donne une ligne avec son seul nom de fichier.
pub fn rows_from_media(
    kind: ReportKind,
    paths: &[impl AsRef<Path>],
    fps: Option<FrameRate>,
    meta: impl Fn(&str) -> BTreeMap<String, String>,
) -> (Vec<Row>, BTreeMap<String, String>) {
    let mut header = BTreeMap::new();
    let fps = fps.unwrap_or(FrameRate::new(25, 1));
    let rows = paths
        .iter()
        .map(|p| {
            let path = p.as_ref();
            let row = match kind {
                ReportKind::Image => video_row(path, &mut header),
                ReportKind::Sound => sound_row(path, fps, &mut header),
            };
            let mut row = row.unwrap_or_else(|_| Row {
                clip: Some(path.display().to_string()),
                fields: BTreeMap::from([
                    ("file".into(), file_name(path)),
                    ("id".into(), file_name(path)),
                ]),
            });
            let m = meta(&path.display().to_string());
            apply_meta(&mut row, &m);
            header_from_meta(kind, &m, &mut header);
            row
        })
        .collect();
    (rows, header)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn school_columns_follow_the_paper_reports() {
        let img: Vec<_> = columns(
            ReportKind::Image,
            &base_columns(ReportKind::Image),
            &[],
            Lang::Fr,
        )
        .into_iter()
        .map(|c| c.label)
        .collect();
        assert_eq!(
            img,
            [
                "SEQ/Plan",
                "Prise",
                "TC IN",
                "TC OUT",
                "Audio/Muet",
                "Effets/Observations"
            ]
        );
        let son: Vec<_> = columns(
            ReportKind::Sound,
            &base_columns(ReportKind::Sound),
            &[1, 2, 3, 4, 5, 6, 7, 8],
            Lang::Fr,
        )
        .into_iter()
        .map(|c| c.label)
        .collect();
        assert_eq!(son.len(), 3 + 8 + 1);
        assert_eq!(son[0], "ID");
        assert_eq!(son[3], "Piste 1");
        assert_eq!(son[11], "Observations");
    }

    #[test]
    fn media_fields_fill_rows_and_header() {
        let mut row = Row::default();
        let meta = BTreeMap::from([
            ("scene".to_string(), "12".to_string()),
            ("shot".to_string(), "3".to_string()),
            ("take".to_string(), "4".to_string()),
            ("circled".to_string(), "true".to_string()),
            ("mos".to_string(), "true".to_string()),
        ]);
        apply_meta(&mut row, &meta);
        assert_eq!(row.get("scene"), "12/3");
        assert_eq!(row.get("take"), "4");
        assert_eq!(row.get("circled"), "●");
        assert_eq!(row.get("audio"), "Muet");
        assert_eq!(row.get("flags"), "MOS");
    }

    #[test]
    fn adding_rows_keeps_user_header_and_counts_tracks() {
        let mut r = Report::new(ReportKind::Sound);
        r.header.insert("title".into(), "Mon film".into());
        let row = Row {
            clip: None,
            fields: BTreeMap::from([("track_12".to_string(), "Boom".to_string())]),
        };
        r.add(
            vec![row],
            BTreeMap::from([
                ("title".to_string(), "Autre".to_string()),
                ("sample_rate".to_string(), "48".to_string()),
            ]),
        );
        assert_eq!(r.header("title"), "Mon film");
        assert_eq!(r.header("sample_rate"), "48");
        assert_eq!(r.tracks, 12);
    }

    #[test]
    fn column_order_is_kept_and_base_columns_restored() {
        let keys: Vec<String> = ["notes", "iso", "take", "bogus", "iso"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        let n = normalize_columns(ReportKind::Image, &keys);
        // Ordre choisi conservé, clé inconnue et doublon retirés, colonnes de
        // base manquantes remises à leur place.
        assert_eq!(
            n,
            ["scene", "notes", "iso", "take", "tc_in", "tc_out", "audio"]
        );
        let cols = columns(
            ReportKind::Sound,
            &base_columns(ReportKind::Sound),
            &[9, 10],
            Lang::En,
        );
        let labels: Vec<_> = cols.iter().map(|c| c.label.as_str()).collect();
        assert_eq!(
            labels,
            ["ID", "Scene/Shot", "Take", "Track 9", "Track 10", "Notes"]
        );
        // Ancien rapport « Pro » : toutes les colonnes.
        let mut old = Report::new(ReportKind::Image);
        old.columns.clear();
        old.template = Template::Pro;
        assert_eq!(old.column_keys().len(), IMAGE_COLUMNS.len());
    }

    #[test]
    fn fps_labels() {
        assert_eq!(fps_label(FrameRate::new(25, 1)), "25");
        assert_eq!(fps_label(FrameRate::new(24000, 1001)), "23.976");
        assert_eq!(fps_label(FrameRate::new(30000, 1001)), "29.97");
    }
}
