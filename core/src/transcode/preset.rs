//! Préréglages de conversion (charte §7.6) et arguments FFmpeg correspondants.

use serde::{Deserialize, Serialize};

use super::encoders::{self, hardware_candidates};
use crate::media::probe::MediaInfo;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Codecs de montage et d'étalonnage (ProRes, DNxHR...).
    Intermediate,
    /// Fichiers de diffusion (H.264, HEVC).
    Delivery,
    /// Copies légères pour le montage.
    Proxy,
    /// Changement de conteneur ou extraction, sans réencodage.
    NoReencode,
    /// Conversion de fichiers son.
    Audio,
    /// Mesure du niveau, sans fichier produit.
    Analysis,
}

/// Mode (VIDEO ou AUDIO) dans lequel le préréglage est proposé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Video,
    Audio,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Profil ProRes : 0 Proxy, 1 LT, 2 422, 3 HQ, 4 4444, 5 4444 XQ.
    ProRes(u8),
    /// Profil DNxHR et format des pixels.
    DnxHr(&'static str, &'static str),
    CineForm,
    Animation,
    Uncompressed,
    Ffv1,
    H264,
    Hevc,
    Rewrap,
    ExtractAudio,
    Wav,
    Aiff,
    Flac,
    Alac,
    Mp3,
    Aac,
    Opus,
    Vorbis,
    Ac3,
    Analyze,
}

pub struct Preset {
    pub id: &'static str,
    pub category: Category,
    pub domain: Domain,
    pub label_fr: &'static str,
    pub label_en: &'static str,
    /// Extension du fichier produit (vide pour une analyse).
    pub ext: &'static str,
    /// Format de sortie FFmpeg (`-f`).
    pub format: &'static str,
    pub kind: Kind,
    /// Taille d'image proposée : "source", "1/2", "1/4" ou une hauteur ("1080").
    pub scale: &'static str,
    /// Suffixe ajouté au nom du fichier produit.
    pub suffix: &'static str,
}

#[allow(clippy::too_many_arguments)]
const fn p(
    id: &'static str,
    category: Category,
    domain: Domain,
    label_fr: &'static str,
    label_en: &'static str,
    ext: &'static str,
    format: &'static str,
    kind: Kind,
) -> Preset {
    Preset {
        id,
        category,
        domain,
        label_fr,
        label_en,
        ext,
        format,
        kind,
        scale: "source",
        suffix: "",
    }
}

const fn proxy(mut preset: Preset) -> Preset {
    preset.scale = "1/2";
    preset.suffix = "_proxy";
    preset
}

use Category as C;
use Domain::{Audio as A, Video as V};

#[rustfmt::skip]
pub static PRESETS: &[Preset] = &[
    // Vidéo intermédiaire.
    p("prores_proxy", C::Intermediate, V, "ProRes 422 Proxy", "ProRes 422 Proxy", "mov", "mov", Kind::ProRes(0)),
    p("prores_lt", C::Intermediate, V, "ProRes 422 LT", "ProRes 422 LT", "mov", "mov", Kind::ProRes(1)),
    p("prores_422", C::Intermediate, V, "ProRes 422", "ProRes 422", "mov", "mov", Kind::ProRes(2)),
    p("prores_hq", C::Intermediate, V, "ProRes 422 HQ", "ProRes 422 HQ", "mov", "mov", Kind::ProRes(3)),
    p("prores_4444", C::Intermediate, V, "ProRes 4444", "ProRes 4444", "mov", "mov", Kind::ProRes(4)),
    p("prores_4444xq", C::Intermediate, V, "ProRes 4444 XQ", "ProRes 4444 XQ", "mov", "mov", Kind::ProRes(5)),
    p("dnxhr_lb", C::Intermediate, V, "DNxHR LB", "DNxHR LB", "mov", "mov", Kind::DnxHr("dnxhr_lb", "yuv422p")),
    p("dnxhr_sq", C::Intermediate, V, "DNxHR SQ", "DNxHR SQ", "mov", "mov", Kind::DnxHr("dnxhr_sq", "yuv422p")),
    p("dnxhr_hq", C::Intermediate, V, "DNxHR HQ", "DNxHR HQ", "mov", "mov", Kind::DnxHr("dnxhr_hq", "yuv422p")),
    p("dnxhr_hqx", C::Intermediate, V, "DNxHR HQX (10 bits)", "DNxHR HQX (10-bit)", "mov", "mov", Kind::DnxHr("dnxhr_hqx", "yuv422p10le")),
    p("dnxhr_444", C::Intermediate, V, "DNxHR 444 (10 bits)", "DNxHR 444 (10-bit)", "mov", "mov", Kind::DnxHr("dnxhr_444", "yuv444p10le")),
    p("cineform", C::Intermediate, V, "GoPro CineForm", "GoPro CineForm", "mov", "mov", Kind::CineForm),
    p("animation", C::Intermediate, V, "QuickTime Animation", "QuickTime Animation", "mov", "mov", Kind::Animation),
    p("uncompressed", C::Intermediate, V, "Non compressé 10 bits 4:2:2 (v210)", "Uncompressed 10-bit 4:2:2 (v210)", "mov", "mov", Kind::Uncompressed),
    p("ffv1", C::Intermediate, V, "FFV1 (archivage sans perte, MKV)", "FFV1 (lossless archive, MKV)", "mkv", "matroska", Kind::Ffv1),
    // Diffusion.
    p("h264", C::Delivery, V, "H.264 (MP4)", "H.264 (MP4)", "mp4", "mp4", Kind::H264),
    p("hevc", C::Delivery, V, "HEVC / H.265 (MP4)", "HEVC / H.265 (MP4)", "mp4", "mp4", Kind::Hevc),
    // Proxies.
    proxy(p("proxy_prores", C::Proxy, V, "Proxy ProRes 422 Proxy", "ProRes 422 Proxy proxy", "mov", "mov", Kind::ProRes(0))),
    proxy(p("proxy_dnxhr", C::Proxy, V, "Proxy DNxHR LB", "DNxHR LB proxy", "mov", "mov", Kind::DnxHr("dnxhr_lb", "yuv422p"))),
    proxy(p("proxy_h264", C::Proxy, V, "Proxy H.264", "H.264 proxy", "mov", "mov", Kind::H264)),
    // Sans réencodage.
    p("rewrap_mov", C::NoReencode, V, "Changer de conteneur : MOV", "Rewrap to MOV", "mov", "mov", Kind::Rewrap),
    p("rewrap_mp4", C::NoReencode, V, "Changer de conteneur : MP4", "Rewrap to MP4", "mp4", "mp4", Kind::Rewrap),
    p("rewrap_mxf", C::NoReencode, V, "Changer de conteneur : MXF OP1a", "Rewrap to MXF OP1a", "mxf", "mxf", Kind::Rewrap),
    p("rewrap_mkv", C::NoReencode, V, "Changer de conteneur : MKV", "Rewrap to MKV", "mkv", "matroska", Kind::Rewrap),
    p("extract_audio", C::NoReencode, V, "Extraire le son (WAV, sans perte)", "Extract sound (WAV, lossless)", "wav", "wav", Kind::ExtractAudio),
    // Son.
    p("wav", C::Audio, A, "WAV / BWF", "WAV / BWF", "wav", "wav", Kind::Wav),
    p("aiff", C::Audio, A, "AIFF", "AIFF", "aif", "aiff", Kind::Aiff),
    p("flac", C::Audio, A, "FLAC (sans perte)", "FLAC (lossless)", "flac", "flac", Kind::Flac),
    p("alac", C::Audio, A, "ALAC (Apple Lossless, M4A)", "ALAC (Apple Lossless, M4A)", "m4a", "ipod", Kind::Alac),
    p("mp3", C::Audio, A, "MP3", "MP3", "mp3", "mp3", Kind::Mp3),
    p("aac", C::Audio, A, "AAC (M4A)", "AAC (M4A)", "m4a", "ipod", Kind::Aac),
    p("opus", C::Audio, A, "Opus", "Opus", "opus", "ogg", Kind::Opus),
    p("vorbis", C::Audio, A, "Ogg Vorbis", "Ogg Vorbis", "ogg", "ogg", Kind::Vorbis),
    p("ac3", C::Audio, A, "AC-3 (Dolby Digital)", "AC-3 (Dolby Digital)", "ac3", "ac3", Kind::Ac3),
    p("analyze", C::Analysis, A, "Mesure loudness et True Peak", "Loudness and True Peak measurement", "", "", Kind::Analyze),
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

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

impl Preset {
    pub fn is_video(&self) -> bool {
        matches!(
            self.kind,
            Kind::ProRes(_)
                | Kind::DnxHr(..)
                | Kind::CineForm
                | Kind::Animation
                | Kind::Uncompressed
                | Kind::Ffv1
                | Kind::H264
                | Kind::Hevc
        )
    }

    pub fn is_audio(&self) -> bool {
        matches!(
            self.kind,
            Kind::Wav
                | Kind::Aiff
                | Kind::Flac
                | Kind::Alac
                | Kind::Mp3
                | Kind::Aac
                | Kind::Opus
                | Kind::Vorbis
                | Kind::Ac3
        )
    }

    pub fn bit_depths(&self) -> &'static [BitDepth] {
        match self.kind {
            Kind::Wav | Kind::Aiff => &[BitDepth::S16, BitDepth::S24, BitDepth::F32],
            Kind::Flac | Kind::Alac => &[BitDepth::S16, BitDepth::S24],
            _ => &[],
        }
    }

    /// Débits proposés (kbit/s) et débit par défaut, pour les formats avec perte.
    pub fn audio_bitrates(&self) -> (&'static [u32], u32) {
        match self.kind {
            Kind::Mp3 => (&[128, 192, 256, 320], 320),
            Kind::Aac => (&[128, 192, 256, 320], 256),
            Kind::Opus => (&[64, 96, 128, 160, 256], 160),
            Kind::Vorbis => (&[128, 192, 256, 320], 256),
            Kind::Ac3 => (&[192, 384, 448, 640], 640),
            _ => (&[], 0),
        }
    }

    pub fn has_video_bitrate(&self) -> bool {
        matches!(self.kind, Kind::H264 | Kind::Hevc)
    }

    /// Le choix entre encodeur du système et encodeur logiciel a un sens.
    pub fn has_encoder_choice(&self) -> bool {
        matches!(self.kind, Kind::H264 | Kind::Hevc)
            || (matches!(self.kind, Kind::ProRes(_)) && !hardware_candidates("prores").is_empty())
    }
}

/// Réglages choisis par l'utilisateur pour un préréglage.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    pub preset: String,
    /// "source", "1/2", "1/4" ou une hauteur en pixels ("1080").
    #[serde(default)]
    pub scale: Option<String>,
    /// Débit vidéo en Mbit/s (H.264, HEVC) ; automatique selon la taille sinon.
    #[serde(default)]
    pub video_mbps: Option<f64>,
    /// Encodeur logiciel de FFmpeg plutôt que celui du système ou de la carte graphique.
    #[serde(default)]
    pub software: bool,
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default)]
    pub bit_depth: Option<BitDepth>,
    #[serde(default)]
    pub audio_kbps: Option<u32>,
    /// Normalisation du niveau (formats son).
    #[serde(default)]
    pub loudness: Option<LoudnessTarget>,
}

/// Cible de normalisation : niveau intégré (LUFS) et True Peak maximal (dBTP).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LoudnessTarget {
    pub integrated: f64,
    pub true_peak: f64,
}

/// Encodeur retenu pour un préréglage sur ce poste.
#[derive(Debug, Clone, Serialize)]
pub struct EncoderChoice {
    pub name: String,
    /// Encodeur du système ou de la carte graphique.
    pub hardware: bool,
    /// ProRes produit par un encodeur non certifié par Apple.
    pub uncertified_prores: bool,
}

/// Arguments FFmpeg de sortie (entre l'entrée et le fichier produit).
pub struct Plan {
    pub args: Vec<String>,
    pub encoder: Option<EncoderChoice>,
}

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

const PRORES_VT: [&str; 6] = ["proxy", "lt", "standard", "hq", "4444", "xq"];

fn prores_args(profile: u8, encoder: &str) -> Vec<String> {
    if encoder == "prores_videotoolbox" {
        vec![
            s("-c:v"),
            s(encoder),
            s("-profile:v"),
            s(PRORES_VT[profile as usize]),
        ]
    } else {
        let pix = if profile >= 4 {
            "yuv444p10le"
        } else {
            "yuv422p10le"
        };
        vec![
            s("-c:v"),
            s("prores_ks"),
            s("-profile:v"),
            s(profile),
            s("-vendor"),
            s("apl0"),
            s("-pix_fmt"),
            s(pix),
        ]
    }
}

/// Arguments d'un encodeur H.264 ou HEVC au débit donné (kbit/s).
fn long_gop_args(encoder: &str, kbps: u32, ten_bit: bool) -> Vec<String> {
    let mut a = vec![s("-c:v"), s(encoder)];
    let rate = |a: &mut Vec<String>| {
        a.extend([
            s("-b:v"),
            format!("{kbps}k"),
            s("-maxrate"),
            format!("{}k", kbps * 3 / 2),
            s("-bufsize"),
            format!("{}k", kbps * 2),
        ])
    };
    match encoder {
        "libx264" => {
            a.extend([s("-preset"), s("medium"), s("-pix_fmt"), s("yuv420p")]);
            rate(&mut a);
        }
        "libx265" => {
            let pix = if ten_bit { "yuv420p10le" } else { "yuv420p" };
            a.extend([
                s("-preset"),
                s("medium"),
                s("-pix_fmt"),
                s(pix),
                s("-x265-params"),
                s("log-level=error"),
            ]);
            rate(&mut a);
        }
        e if e.ends_with("_videotoolbox") => {
            a.extend([s("-b:v"), format!("{kbps}k"), s("-allow_sw"), s("1")]);
        }
        e if e.ends_with("_nvenc") => {
            a.extend([s("-preset"), s("p5"), s("-rc"), s("vbr")]);
            rate(&mut a);
        }
        e if e.ends_with("_amf") => {
            a.extend([s("-rc"), s("vbr_peak")]);
            rate(&mut a);
        }
        _ => rate(&mut a),
    }
    if encoder.starts_with("hevc") || encoder == "libx265" {
        // Étiquette lue par QuickTime et les appareils Apple.
        a.extend([s("-tag:v"), s("hvc1")]);
    }
    a
}

/// Encodeur AAC : celui du système s'il fonctionne, sinon celui de FFmpeg.
fn aac_encoder() -> String {
    for c in hardware_candidates("aac") {
        let args = vec![s("-c:a"), s(*c), s("-b:a"), s("192k")];
        if encoders::works(&args, true) {
            return s(*c);
        }
    }
    s("aac")
}

/// Choisit l'encodeur vidéo d'un préréglage sur ce poste (essais compris).
pub fn video_encoder(preset: &Preset, software: bool) -> Option<EncoderChoice> {
    let codec = match preset.kind {
        Kind::H264 => "h264",
        Kind::Hevc => "hevc",
        Kind::ProRes(profile) => {
            if !software {
                for c in hardware_candidates("prores") {
                    if encoders::works(&prores_args(profile, c), false) {
                        return Some(EncoderChoice {
                            name: s(*c),
                            hardware: true,
                            uncertified_prores: false,
                        });
                    }
                }
            }
            return Some(EncoderChoice {
                name: s("prores_ks"),
                hardware: false,
                uncertified_prores: true,
            });
        }
        _ => return None,
    };
    if !software {
        for c in hardware_candidates(codec) {
            if encoders::works(&long_gop_args(c, 8000, false), false) {
                return Some(EncoderChoice {
                    name: s(*c),
                    hardware: true,
                    uncertified_prores: false,
                });
            }
        }
    }
    let soft = if codec == "h264" {
        "libx264"
    } else {
        "libx265"
    };
    encoders::compiled().contains(soft).then(|| EncoderChoice {
        name: s(soft),
        hardware: false,
        uncertified_prores: false,
    })
}

/// Filtre de redimensionnement, et hauteur de l'image produite.
fn scale_filter(scale: &str, src_height: u32) -> (Option<String>, u32) {
    match scale {
        "" | "source" => (None, src_height),
        "1/2" => (
            Some(s("scale=trunc(iw/4)*2:trunc(ih/4)*2:flags=lanczos")),
            src_height / 2,
        ),
        "1/4" => (
            Some(s("scale=trunc(iw/8)*2:trunc(ih/8)*2:flags=lanczos")),
            src_height / 4,
        ),
        h => match h.parse::<u32>() {
            Ok(h) if h > 0 => (Some(format!("scale=-2:{h}:flags=lanczos")), h),
            _ => (None, src_height),
        },
    }
}

/// Débit H.264 par défaut (kbit/s) selon la hauteur de l'image ; HEVC : 60 %.
pub fn default_kbps(height: u32, hevc: bool) -> u32 {
    let h264 = match height {
        0..=576 => 5_000,
        577..=720 => 10_000,
        721..=1080 => 20_000,
        1081..=1440 => 30_000,
        _ => 50_000,
    };
    if hevc {
        h264 * 6 / 10
    } else {
        h264
    }
}

/// Résolution du son source, d'après le codec et les bits annoncés.
fn source_depth(info: &MediaInfo) -> Option<BitDepth> {
    let a = info.audio.first()?;
    if a.codec.starts_with("pcm_f") {
        return Some(BitDepth::F32);
    }
    match a.bits {
        Some(b) if b <= 16 => Some(BitDepth::S16),
        Some(b) if b <= 24 => Some(BitDepth::S24),
        Some(_) => Some(BitDepth::F32),
        None => None,
    }
}

/// Résolution de sortie : choix de l'utilisateur, sinon celle de la source,
/// ramenée à ce que le format accepte (24 bits par défaut).
pub fn output_depth(preset: &Preset, settings: &Settings, info: &MediaInfo) -> BitDepth {
    let allowed = preset.bit_depths();
    let wanted = settings
        .bit_depth
        .or_else(|| source_depth(info))
        .unwrap_or(BitDepth::S24);
    if allowed.contains(&wanted) {
        wanted
    } else {
        BitDepth::S24
    }
}

/// Fréquence de sortie, ramenée à celles que le format accepte.
pub fn output_rate(preset: &Preset, settings: &Settings, info: &MediaInfo) -> u32 {
    let src = info
        .audio
        .first()
        .map(|a| a.sample_rate)
        .filter(|r| *r > 0)
        .unwrap_or(48_000);
    let rate = settings.sample_rate.unwrap_or(src);
    match preset.kind {
        Kind::Opus => 48_000,
        Kind::Mp3 | Kind::Ac3 if ![32_000, 44_100, 48_000].contains(&rate) => {
            if rate.is_multiple_of(44_100) {
                44_100
            } else {
                48_000
            }
        }
        _ => rate,
    }
}

/// Nombre total de canaux des pistes son de la source.
pub fn total_channels(info: &MediaInfo) -> u32 {
    info.audio.iter().map(|a| a.channels.max(1)).sum()
}

/// Graphe de filtres son : pistes réunies en une seule (fichier polyphonique),
/// limitées aux deux premières si le format n'accepte pas autant de canaux,
/// puis filtres demandés. Sortie étiquetée `[aout]`.
pub fn audio_graph(info: &MediaInfo, max_channels: Option<u32>, filters: &[String]) -> String {
    let n = info.audio.len();
    let mut chain: Vec<String> = Vec::new();
    let inputs = if n <= 1 {
        s("[0:a:0]")
    } else {
        chain.push(format!("amerge=inputs={n}"));
        (0..n).map(|i| format!("[0:a:{i}]")).collect::<String>()
    };
    let channels = total_channels(info);
    if let Some(max) = max_channels {
        if channels > max && channels >= 2 {
            // Pistes 1 et 2 : le mix gauche/droite des enregistreurs de tournage.
            chain.push(s("pan=stereo|c0=c0|c1=c1"));
        }
    }
    chain.extend(filters.iter().cloned());
    if chain.is_empty() {
        chain.push(s("anull"));
    }
    format!("{inputs}{}[aout]", chain.join(","))
}

/// Canaux maximum acceptés par un format avec perte (5.1 gardé tel quel).
pub(crate) fn max_channels(kind: Kind, channels: u32) -> Option<u32> {
    match kind {
        Kind::Mp3 => Some(2),
        Kind::Aac | Kind::Opus | Kind::Vorbis | Kind::Ac3 if channels != 6 => Some(2),
        _ => None,
    }
}

/// Filtre de rééchantillonnage de qualité, avec dither si la résolution baisse.
fn resample_filter(rate: u32) -> String {
    format!(
        "aresample={rate}:filter_size=64:phase_shift=10:cutoff=0.97:dither_method=triangular_hp"
    )
}

fn pcm_codec(kind: Kind, depth: BitDepth) -> &'static str {
    match (kind, depth) {
        (Kind::Aiff, BitDepth::S16) => "pcm_s16be",
        (Kind::Aiff, BitDepth::S24) => "pcm_s24be",
        (Kind::Aiff, BitDepth::F32) => "pcm_f32be",
        (_, BitDepth::S16) => "pcm_s16le",
        (_, BitDepth::S24) => "pcm_s24le",
        (_, BitDepth::F32) => "pcm_f32le",
    }
}

/// Timecode de début de la source, repris dans le fichier produit.
fn timecode_args(info: &MediaInfo) -> Vec<String> {
    match info.start_tc() {
        Some(tc) => vec![s("-timecode"), tc.to_string()],
        None => Vec::new(),
    }
}

/// Construit les arguments de sortie d'un fichier. `gain_db` : gain de
/// normalisation déjà calculé (formats son).
pub fn build(
    preset: &Preset,
    settings: &Settings,
    info: &MediaInfo,
    gain_db: Option<f64>,
) -> Result<Plan> {
    let mut a: Vec<String> = vec![s("-map_metadata"), s("0")];
    let mut encoder = None;
    let has_audio = !info.audio.is_empty();
    if preset.is_video() {
        let v = info
            .video
            .as_ref()
            .ok_or_else(|| Error::Unsupported(format!("{} : pas d'image", info.path)))?;
        let scale = settings.scale.as_deref().unwrap_or(preset.scale);
        let (filter, out_h) = scale_filter(scale, v.height);
        a.extend([s("-map"), s("0:v:0"), s("-map"), s("0:a?")]);
        if let Some(f) = filter {
            a.extend([s("-vf"), f]);
        }
        let ten_bit = v.pix_fmt.contains("10") || v.pix_fmt.contains("12");
        match preset.kind {
            Kind::ProRes(profile) => {
                let choice = video_encoder(preset, settings.software).ok_or_else(no_encoder)?;
                a.extend(prores_args(profile, &choice.name));
                encoder = Some(choice);
            }
            Kind::DnxHr(profile, pix) => a.extend([
                s("-c:v"),
                s("dnxhd"),
                s("-profile:v"),
                s(profile),
                s("-pix_fmt"),
                s(pix),
            ]),
            Kind::CineForm => a.extend([s("-c:v"), s("cfhd"), s("-pix_fmt"), s("yuv422p10le")]),
            Kind::Animation => a.extend([s("-c:v"), s("qtrle"), s("-pix_fmt"), s("rgb24")]),
            Kind::Uncompressed => a.extend([s("-c:v"), s("v210")]),
            Kind::Ffv1 => a.extend([
                s("-c:v"),
                s("ffv1"),
                s("-level"),
                s("3"),
                s("-g"),
                s("1"),
                s("-slicecrc"),
                s("1"),
                s("-slices"),
                s("16"),
            ]),
            Kind::H264 | Kind::Hevc => {
                let hevc = preset.kind == Kind::Hevc;
                let choice = video_encoder(preset, settings.software).ok_or_else(no_encoder)?;
                let kbps = settings
                    .video_mbps
                    .filter(|m| *m > 0.0)
                    .map(|m| (m * 1000.0).round() as u32)
                    .unwrap_or_else(|| default_kbps(out_h, hevc));
                a.extend(long_gop_args(&choice.name, kbps, ten_bit));
                encoder = Some(choice);
            }
            _ => unreachable!(),
        }
        if has_audio {
            if preset.format == "mp4" {
                a.extend([s("-c:a"), aac_encoder(), s("-b:a"), s("320k")]);
            } else {
                a.extend([s("-c:a"), s("pcm_s24le")]);
            }
        }
        if preset.format == "mp4" {
            a.extend([s("-movflags"), s("+faststart")]);
        }
        a.extend(timecode_args(info));
        return Ok(Plan { args: a, encoder });
    }

    match preset.kind {
        Kind::Rewrap => {
            a.extend([
                s("-map"),
                s("0:v?"),
                s("-map"),
                s("0:a?"),
                s("-c"),
                s("copy"),
            ]);
            a.extend(timecode_args(info));
        }
        Kind::ExtractAudio => {
            if !has_audio {
                return Err(no_audio(info));
            }
            let depth = source_depth(info).unwrap_or(BitDepth::S24);
            a.extend([s("-filter_complex"), audio_graph(info, None, &[])]);
            a.extend([
                s("-map"),
                s("[aout]"),
                s("-c:a"),
                s(pcm_codec(Kind::Wav, depth)),
            ]);
            a.extend([s("-rf64"), s("auto"), s("-write_bext"), s("1")]);
            let rate = info.audio[0].sample_rate;
            a.extend(time_reference_args(info, rate));
        }
        kind if preset.is_audio() => {
            if !has_audio {
                return Err(no_audio(info));
            }
            let rate = output_rate(preset, settings, info);
            let mut filters = Vec::new();
            if let Some(g) = gain_db {
                filters.push(format!("volume={g:.2}dB"));
            }
            filters.push(resample_filter(rate));
            let graph = audio_graph(info, max_channels(kind, total_channels(info)), &filters);
            a.extend([
                s("-vn"),
                s("-filter_complex"),
                graph,
                s("-map"),
                s("[aout]"),
            ]);
            let depth = output_depth(preset, settings, info);
            let (_, default_kbps) = preset.audio_bitrates();
            let kbps = format!("{}k", settings.audio_kbps.unwrap_or(default_kbps));
            match kind {
                Kind::Wav => {
                    a.extend([s("-c:a"), s(pcm_codec(kind, depth))]);
                    a.extend([s("-rf64"), s("auto"), s("-write_bext"), s("1")]);
                    a.extend(time_reference_args(info, rate));
                }
                Kind::Aiff => a.extend([s("-c:a"), s(pcm_codec(kind, depth))]),
                Kind::Flac => {
                    let fmt = if depth == BitDepth::S16 { "s16" } else { "s32" };
                    a.extend([s("-c:a"), s("flac"), s("-sample_fmt"), s(fmt)]);
                    if depth != BitDepth::S16 {
                        a.extend([s("-bits_per_raw_sample"), s("24")]);
                    }
                }
                Kind::Alac => {
                    let fmt = if depth == BitDepth::S16 {
                        "s16p"
                    } else {
                        "s32p"
                    };
                    a.extend([s("-c:a"), s("alac"), s("-sample_fmt"), s(fmt)]);
                    if depth != BitDepth::S16 {
                        a.extend([s("-bits_per_raw_sample"), s("24")]);
                    }
                }
                Kind::Mp3 => a.extend([
                    s("-c:a"),
                    s("libmp3lame"),
                    s("-b:a"),
                    kbps,
                    s("-id3v2_version"),
                    s("3"),
                ]),
                Kind::Aac => a.extend([s("-c:a"), aac_encoder(), s("-b:a"), kbps]),
                Kind::Opus => a.extend([s("-c:a"), s("libopus"), s("-b:a"), kbps]),
                Kind::Vorbis => a.extend([s("-c:a"), s("libvorbis"), s("-b:a"), kbps]),
                Kind::Ac3 => a.extend([s("-c:a"), s("ac3"), s("-b:a"), kbps]),
                _ => unreachable!(),
            }
        }
        _ => {
            return Err(Error::Unsupported(format!(
                "{} : préréglage sans fichier produit",
                preset.id
            )))
        }
    }
    Ok(Plan { args: a, encoder })
}

/// Référence temporelle BWF (échantillons depuis minuit) tirée du timecode
/// de la source : le son extrait d'une vidéo garde sa position pour la synchro.
fn time_reference_args(info: &MediaInfo, rate: u32) -> Vec<String> {
    let Some(tc) = info.start_tc() else {
        return Vec::new();
    };
    let r = tc.rate;
    let samples = (tc.frames as u128 * r.den as u128 * rate as u128) / r.num as u128;
    vec![s("-metadata"), format!("time_reference={samples}")]
}

fn no_encoder() -> Error {
    Error::Unsupported("aucun encodeur disponible pour ce format sur ce poste".into())
}

fn no_audio(info: &MediaInfo) -> Error {
    Error::Unsupported(format!("{} : pas de son", info.path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::probe::{AudioStream, VideoStream};
    use crate::player::timecode::FrameRate;

    fn info(audio: &[(u32, u32, Option<u32>)], video: bool) -> MediaInfo {
        MediaInfo {
            path: "clip".into(),
            format: "mov".into(),
            duration: 10.0,
            size: 0,
            video: video.then(|| VideoStream {
                index: 0,
                codec: "h264".into(),
                width: 1920,
                height: 1080,
                rate: FrameRate::new(25, 1),
                pix_fmt: "yuv420p".into(),
                frame_count: 250,
                start_time: 0.0,
            }),
            audio: audio
                .iter()
                .enumerate()
                .map(|(i, (rate, ch, bits))| AudioStream {
                    index: i as u32 + 1,
                    codec: "pcm_s24le".into(),
                    sample_rate: *rate,
                    channels: *ch,
                    bits: *bits,
                })
                .collect(),
            start_timecode: Some("10:00:00:00".into()),
            tags: Default::default(),
        }
    }

    fn settings(preset: &str) -> Settings {
        Settings {
            preset: preset.into(),
            ..Default::default()
        }
    }

    #[test]
    fn ids_are_unique_and_extensions_set() {
        for (i, p) in PRESETS.iter().enumerate() {
            assert!(PRESETS[i + 1..].iter().all(|q| q.id != p.id), "{}", p.id);
            assert_eq!(p.ext.is_empty(), p.kind == Kind::Analyze, "{}", p.id);
        }
    }

    #[test]
    fn audio_graph_merges_tracks_and_limits_channels() {
        let two_mono = info(&[(48_000, 1, Some(24)), (48_000, 1, Some(24))], true);
        assert_eq!(
            audio_graph(&two_mono, None, &[]),
            "[0:a:0][0:a:1]amerge=inputs=2[aout]"
        );
        let poly = info(&[(48_000, 8, Some(24))], false);
        assert_eq!(
            audio_graph(&poly, Some(2), &["volume=1.00dB".into()]),
            "[0:a:0]pan=stereo|c0=c0|c1=c1,volume=1.00dB[aout]"
        );
        assert_eq!(audio_graph(&poly, None, &[]), "[0:a:0]anull[aout]");
    }

    #[test]
    fn rates_and_depths_follow_the_format() {
        let hi = info(&[(96_000, 2, Some(24))], false);
        let mp3 = find("mp3").unwrap();
        assert_eq!(output_rate(mp3, &settings("mp3"), &hi), 48_000);
        let hi441 = info(&[(88_200, 2, Some(24))], false);
        assert_eq!(output_rate(mp3, &settings("mp3"), &hi441), 44_100);
        let opus = find("opus").unwrap();
        assert_eq!(output_rate(opus, &settings("opus"), &hi441), 48_000);
        let wav = find("wav").unwrap();
        assert_eq!(output_rate(wav, &settings("wav"), &hi), 96_000);
        let mut s = settings("flac");
        s.bit_depth = Some(BitDepth::F32);
        assert_eq!(output_depth(find("flac").unwrap(), &s, &hi), BitDepth::S24);
        let cd = info(&[(44_100, 2, Some(16))], false);
        assert_eq!(output_depth(wav, &settings("wav"), &cd), BitDepth::S16);
    }

    #[test]
    fn wav_from_video_keeps_time_reference() {
        let clip = info(&[(48_000, 1, Some(24)), (48_000, 1, Some(24))], true);
        let plan = build(
            find("extract_audio").unwrap(),
            &settings("extract_audio"),
            &clip,
            None,
        )
        .unwrap();
        // 10:00:00:00 à 25 i/s = 36 000 s = 1 728 000 000 échantillons à 48 kHz.
        assert!(plan.args.contains(&"time_reference=1728000000".to_string()));
        assert!(plan.args.contains(&"pcm_s24le".to_string()));
    }

    #[test]
    fn normalization_gain_comes_before_resampling() {
        let wav = info(&[(48_000, 2, Some(24))], false);
        let plan = build(find("wav").unwrap(), &settings("wav"), &wav, Some(-3.5)).unwrap();
        let graph = &plan.args[plan
            .args
            .iter()
            .position(|a| a == "-filter_complex")
            .unwrap()
            + 1];
        assert!(
            graph.starts_with("[0:a:0]volume=-3.50dB,aresample=48000:"),
            "{graph}"
        );
    }

    #[test]
    fn video_presets_keep_timecode_and_audio() {
        let clip = info(&[(48_000, 2, Some(24))], true);
        let plan = build(
            find("dnxhr_hq").unwrap(),
            &settings("dnxhr_hq"),
            &clip,
            None,
        )
        .unwrap();
        let args = plan.args.join(" ");
        assert!(args.contains("-timecode 10:00:00:00"), "{args}");
        assert!(args.contains("-map 0:a? "), "{args}");
        assert!(args.contains("-c:a pcm_s24le"), "{args}");
        let mut proxy = settings("proxy_dnxhr");
        proxy.scale = None;
        let plan = build(find("proxy_dnxhr").unwrap(), &proxy, &clip, None).unwrap();
        assert!(plan.args.join(" ").contains("scale=trunc(iw/4)*2"));
    }

    #[test]
    fn default_bitrate_follows_height() {
        assert_eq!(default_kbps(1080, false), 20_000);
        assert_eq!(default_kbps(540, false), 5_000);
        assert_eq!(default_kbps(2160, true), 30_000);
    }
}
