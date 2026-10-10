//! Réglages d'une conversion, tels que choisis dans l'interface.
//!
//! Tous les champs ont une valeur par défaut : une demande ne contient que ce
//! que l'utilisateur a changé.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Résolution de la sortie audio (bits par échantillon).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BitDepth {
    #[serde(rename = "16")]
    S16,
    #[serde(rename = "24")]
    S24,
    #[serde(rename = "32f")]
    F32,
}

impl BitDepth {
    pub fn bits(self) -> u16 {
        match self {
            BitDepth::S16 => 16,
            BitDepth::S24 => 24,
            BitDepth::F32 => 32,
        }
    }
}

/// Cible de normalisation : niveau intégré (LUFS) et True Peak maximal (dBTP).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoudnessTarget {
    pub integrated: f64,
    pub true_peak: f64,
}

/// Son des fichiers vidéo produits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioMode {
    /// Toutes les pistes, telles quelles.
    #[default]
    Keep,
    /// Pistes 1 et 2 en stéréo.
    FirstTwo,
    /// Mix stéréo : pistes impaires à gauche, paires à droite.
    Mix,
    /// Sans son.
    None,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Rotate {
    #[default]
    None,
    Cw90,
    Ccw90,
    Half,
    FlipH,
    FlipV,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Deinterlace {
    /// Rapide, une image par image entrelacée.
    Yadif,
    /// Meilleure qualité.
    Bwdif,
    /// Une image par trame (double la cadence : 50i devient 50p).
    BwdifField,
}

/// Conversion de cadence.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FpsMethod {
    /// Images dupliquées ou supprimées.
    #[default]
    Duplicate,
    /// Fondu entre images voisines.
    Blend,
    /// Images intermédiaires calculées (compensation de mouvement, lent).
    Interpolate,
}

/// Image ramenée à une taille imposée.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FitMode {
    /// Image entière, bandes noires si besoin.
    #[default]
    Pad,
    /// Image recadrée au centre pour remplir le cadre.
    Crop,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Position {
    TopLeft,
    Top,
    TopRight,
    Center,
    BottomLeft,
    #[default]
    Bottom,
    BottomRight,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Crop {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

impl Crop {
    pub fn is_empty(&self) -> bool {
        self.top == 0 && self.bottom == 0 && self.left == 0 && self.right == 0
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ImageOptions {
    /// Pixels retirés sur chaque bord.
    pub crop: Crop,
    /// Recadrage centré à un rapport d'image (« 16:9 », « 1:1 », « 9:16 », « 2.39:1 »).
    pub aspect: Option<String>,
    /// Taille imposée (largeur, hauteur).
    pub frame: Option<(u32, u32)>,
    pub fit: FitMode,
    pub rotate: Rotate,
    pub deinterlace: Option<Deinterlace>,
    /// Cadence de sortie (« 25 », « 24000/1001 »...).
    pub fps: Option<String>,
    pub fps_method: FpsMethod,
    /// Suppression des images dupliquées.
    pub decimate: bool,
    /// LUT 3D (.cube, .3dl).
    pub lut: Option<PathBuf>,
    /// Réglages couleur : luminosité -1 à 1, contraste, saturation et gamma autour de 1.
    pub brightness: f64,
    pub contrast: f64,
    pub saturation: f64,
    pub gamma: f64,
    /// Étiquettes couleur écrites dans le fichier (« bt709 », « bt2020 », « bt601 »).
    pub color_tags: Option<String>,
}

impl Default for ImageOptions {
    fn default() -> Self {
        Self {
            crop: Crop::default(),
            aspect: None,
            frame: None,
            fit: FitMode::Pad,
            rotate: Rotate::None,
            deinterlace: None,
            fps: None,
            fps_method: FpsMethod::Duplicate,
            decimate: false,
            lut: None,
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            gamma: 1.0,
            color_tags: None,
        }
    }
}

impl ImageOptions {
    pub fn has_color(&self) -> bool {
        self.brightness != 0.0
            || self.contrast != 1.0
            || self.saturation != 1.0
            || self.gamma != 1.0
    }
}

/// Texte incrusté : position, taille en % de la hauteur d'image, opacité, fond.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub position: Position,
    pub size: f64,
    pub opacity: f64,
    pub background: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            position: Position::Bottom,
            size: 4.0,
            opacity: 1.0,
            background: true,
        }
    }
}

/// Logo ou image en surimpression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LogoStyle {
    pub path: PathBuf,
    pub position: Position,
    /// Hauteur en % de la hauteur d'image.
    pub size: f64,
    pub opacity: f64,
}

impl Default for LogoStyle {
    fn default() -> Self {
        Self {
            path: PathBuf::new(),
            position: Position::TopRight,
            size: 12.0,
            opacity: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OverlayOptions {
    pub timecode: Option<TextStyle>,
    /// Timecode de départ imposé ; sinon celui de la source (00:00:00:00 à défaut).
    pub tc_start: Option<String>,
    /// Décalage du timecode incrusté, en images.
    pub tc_offset: i64,
    pub filename: Option<TextStyle>,
    pub text: Option<TextStyle>,
    pub text_value: String,
    pub logo: Option<LogoStyle>,
}

impl OverlayOptions {
    pub fn is_empty(&self) -> bool {
        self.timecode.is_none()
            && self.filename.is_none()
            && self.text.is_none()
            && self.logo.is_none()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SubtitleOptions {
    /// Fichier .srt, .vtt ou .ass.
    pub file: Option<PathBuf>,
    /// Incrustés dans l'image plutôt qu'ajoutés en piste.
    pub burn: bool,
    /// Taille du texte incrusté (0 : taille du fichier).
    pub size: u32,
}

/// Points d'entrée et de sortie : timecode (« 10:00:12:05 ») ou secondes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Trim {
    pub start: Option<String>,
    pub end: Option<String>,
}

/// Images fixes.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SequenceOptions {
    /// Une seule image, à la position donnée (timecode ou secondes ; milieu du plan sinon).
    pub single: bool,
    pub position: Option<String>,
    /// Une image toutes les N secondes ; toutes les images sinon.
    pub every: Option<f64>,
    /// Numérotation des images d'après le timecode (séquences DPX, EXR).
    pub tc_numbering: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub preset: String,
    /// "source", "1/2", "1/4" ou une hauteur en pixels ("1080").
    pub scale: Option<String>,
    /// Débit vidéo en Mbit/s ; automatique selon la taille sinon.
    pub video_mbps: Option<f64>,
    /// Encodeur logiciel de FFmpeg plutôt que celui du système ou de la carte graphique.
    pub software: bool,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<BitDepth>,
    pub audio_kbps: Option<u32>,
    /// Normalisation du niveau.
    pub loudness: Option<LoudnessTarget>,
    /// Son des fichiers vidéo (choix du préréglage par défaut).
    pub audio_mode: Option<AudioMode>,
    pub image: ImageOptions,
    pub overlay: OverlayOptions,
    pub subtitles: SubtitleOptions,
    /// Points d'entrée et de sortie, par fichier source.
    pub trims: BTreeMap<String, Trim>,
    pub sequence: SequenceOptions,
    /// Conformation : nouvelle cadence, et son ralenti/accéléré sans changer de hauteur.
    pub conform_rate: Option<String>,
    pub keep_pitch: bool,
    /// Remplacement du son : fichiers candidats, associés aux vidéos par leur nom.
    pub audio_files: Vec<PathBuf>,
    /// Décalage du son de remplacement (secondes, positif : son plus tard).
    pub audio_offset: f64,
    /// Calage du son de remplacement par timecode (BWF et timecode vidéo).
    pub sync_tc: bool,
    /// Insert : plan à insérer et point d'insertion dans chaque source.
    pub insert_file: Option<PathBuf>,
    pub insert_at: Option<String>,
    /// VMAF : fichier ou dossier des originaux.
    pub reference: Option<PathBuf>,
    /// Seuil des détections (plans, noir, silence...).
    pub threshold: Option<f64>,
    /// Qualité VMAF du fichier produit, comparé à la source.
    pub vmaf_after: bool,
}

impl Settings {
    pub fn trim_for(&self, source: &std::path::Path) -> Option<&Trim> {
        self.trims
            .get(&source.to_string_lossy().to_string())
            .filter(|t| t.start.is_some() || t.end.is_some())
    }
}
