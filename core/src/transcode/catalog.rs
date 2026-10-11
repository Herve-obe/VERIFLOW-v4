//! Catalogue des préréglages (charte §7.6) : codecs, conteneurs, et ce que
//! chaque préréglage permet de régler.

use serde::{Deserialize, Serialize};

use super::encoders;
use super::settings::{AudioMode, BitDepth, FitMode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    /// Codecs de montage et d'étalonnage.
    Intermediate,
    /// Codecs broadcast (XDCAM, AVC-Intra, XAVC, HAP).
    Broadcast,
    /// Fichiers de diffusion (H.264, HEVC, AV1, VP9, H.266).
    Delivery,
    /// Préréglages pour les plateformes web.
    Web,
    /// Copies légères pour le montage.
    Proxy,
    /// Anciens codecs.
    Legacy,
    /// Images fixes et séquences.
    Image,
    /// Sans réencodage.
    NoReencode,
    /// Conversion de fichiers son.
    Audio,
    /// Analyses.
    Analysis,
}

/// Mode (VIDEO ou AUDIO) dans lequel le préréglage est proposé.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Domain {
    Video,
    Audio,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    Tiff,
    Dpx,
    Exr,
    Jxl,
    Webp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Profil ProRes : 0 Proxy, 1 LT, 2 422, 3 HQ, 4 4444, 5 4444 XQ.
    ProRes(u8),
    /// Profil DNxHR et format des pixels.
    DnxHr(&'static str, &'static str),
    /// DNxHD 1080 : 0 LB (36), 1 SQ (115/120), 2 HQ (175/185), 3 HQX 10 bits.
    DnxHd(u8),
    CineForm,
    Animation,
    Uncompressed,
    Ffv1,
    H264,
    Hevc,
    Av1,
    Vvc,
    Vp8,
    Vp9,
    Xdcam422,
    Xdcam35,
    AvcIntra100,
    XavcIntra,
    XavcLongGop,
    /// Variante HAP : "hap", "hap_alpha", "hap_q".
    Hap(&'static str),
    Mpeg2,
    Mpeg1,
    Mjpeg,
    Xvid,
    Dv,
    Wmv,
    Theora,
    Image(ImageFormat),
    Rewrap,
    Cut,
    ExtractAudio,
    ExtractTracks,
    ExtractVideo,
    ReplaceAudio,
    Conform,
    Merge,
    Insert,
    Subtitles,
    Wav,
    Aiff,
    Flac,
    Alac,
    Mp3,
    Aac,
    Opus,
    Vorbis,
    Ac3,
    Loudness,
    CutDetect,
    BlackDetect,
    OfflineDetect,
    SilenceDetect,
    FrameMd5,
    Vmaf,
}

pub struct Preset {
    pub id: &'static str,
    pub category: Category,
    pub domain: Domain,
    pub label_fr: &'static str,
    pub label_en: &'static str,
    /// Extension du fichier produit ; "*" : celle de la source ; vide : aucun fichier.
    pub ext: &'static str,
    /// Format de sortie FFmpeg (`-f`) ; vide : d'après l'extension de la source.
    pub format: &'static str,
    pub kind: Kind,
    /// Taille d'image proposée : "source", "1/2", "1/4" ou une hauteur ("1080").
    pub scale: &'static str,
    /// Suffixe ajouté au nom du fichier produit.
    pub suffix: &'static str,
    /// Taille imposée (plateformes web, formats broadcast).
    pub frame: Option<(u32, u32)>,
    pub fit: FitMode,
    /// Débit vidéo imposé (Mbit/s) jusqu'à 30 i/s, et au-delà.
    pub mbps: Option<(f64, f64)>,
    /// Son par défaut des fichiers vidéo.
    pub audio_mode: AudioMode,
    /// Débit du son (kbit/s) des formats vidéo à son compressé.
    pub audio_kbps: u32,
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
        frame: None,
        fit: FitMode::Pad,
        mbps: None,
        audio_mode: AudioMode::Keep,
        audio_kbps: 320,
    }
}

const fn proxy(mut preset: Preset) -> Preset {
    preset.scale = "1/2";
    preset.suffix = "_proxy";
    preset
}

/// Plateforme web : taille, recadrage, débits, son stéréo.
const fn web(
    mut preset: Preset,
    w: u32,
    h: u32,
    fit: FitMode,
    mbps: (f64, f64),
    audio_kbps: u32,
) -> Preset {
    preset.frame = Some((w, h));
    preset.fit = fit;
    preset.mbps = Some(mbps);
    preset.audio_mode = AudioMode::FirstTwo;
    preset.audio_kbps = audio_kbps;
    preset
}

const fn sized(mut preset: Preset, w: u32, h: u32) -> Preset {
    preset.frame = Some((w, h));
    preset
}

const fn stereo(mut preset: Preset) -> Preset {
    preset.audio_mode = AudioMode::FirstTwo;
    preset
}

use Category as C;
use Domain::{Audio as A, Both as B, Video as V};
use FitMode::{Crop as FC, Pad as FP};
use ImageFormat as I;

#[rustfmt::skip]
pub static PRESETS: &[Preset] = &[
    // Montage et étalonnage.
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
    sized(p("dnxhd_lb", C::Intermediate, V, "DNxHD LB 1080 (36 Mbit/s, MXF Avid)", "DNxHD LB 1080 (36 Mbit/s, Avid MXF)", "mxf", "mxf", Kind::DnxHd(0)), 1920, 1080),
    sized(p("dnxhd_sq", C::Intermediate, V, "DNxHD SQ 1080 (MXF Avid)", "DNxHD SQ 1080 (Avid MXF)", "mxf", "mxf", Kind::DnxHd(1)), 1920, 1080),
    sized(p("dnxhd_hq", C::Intermediate, V, "DNxHD HQ 1080 (MXF Avid)", "DNxHD HQ 1080 (Avid MXF)", "mxf", "mxf", Kind::DnxHd(2)), 1920, 1080),
    sized(p("dnxhd_hqx", C::Intermediate, V, "DNxHD HQX 1080 10 bits (MXF Avid)", "DNxHD HQX 1080 10-bit (Avid MXF)", "mxf", "mxf", Kind::DnxHd(3)), 1920, 1080),
    p("cineform", C::Intermediate, V, "GoPro CineForm", "GoPro CineForm", "mov", "mov", Kind::CineForm),
    p("animation", C::Intermediate, V, "QuickTime Animation", "QuickTime Animation", "mov", "mov", Kind::Animation),
    p("uncompressed", C::Intermediate, V, "Non compressé 10 bits 4:2:2 (v210)", "Uncompressed 10-bit 4:2:2 (v210)", "mov", "mov", Kind::Uncompressed),
    p("ffv1", C::Intermediate, V, "FFV1 (archivage sans perte, MKV)", "FFV1 (lossless archive, MKV)", "mkv", "matroska", Kind::Ffv1),
    // Broadcast.
    sized(p("xdcam_hd422", C::Broadcast, V, "XDCAM HD422 (MPEG-2 50 Mbit/s, MXF)", "XDCAM HD422 (MPEG-2 50 Mbit/s, MXF)", "mxf", "mxf", Kind::Xdcam422), 1920, 1080),
    sized(p("xdcam_hd35", C::Broadcast, V, "XDCAM HD 35 (MPEG-2 35 Mbit/s, MXF)", "XDCAM HD 35 (MPEG-2 35 Mbit/s, MXF)", "mxf", "mxf", Kind::Xdcam35), 1440, 1080),
    sized(p("avc_intra100", C::Broadcast, V, "AVC-Intra 100 (MXF)", "AVC-Intra 100 (MXF)", "mxf", "mxf", Kind::AvcIntra100), 1920, 1080),
    p("xavc_intra", C::Broadcast, V, "XAVC Intra (MXF)", "XAVC Intra (MXF)", "mxf", "mxf", Kind::XavcIntra),
    p("xavc_longgop", C::Broadcast, V, "XAVC Long GOP 4:2:2 10 bits (MXF)", "XAVC Long GOP 4:2:2 10-bit (MXF)", "mxf", "mxf", Kind::XavcLongGop),
    p("hap", C::Broadcast, V, "HAP (régie, VJing)", "HAP (live playback, VJing)", "mov", "mov", Kind::Hap("hap")),
    p("hap_alpha", C::Broadcast, V, "HAP Alpha", "HAP Alpha", "mov", "mov", Kind::Hap("hap_alpha")),
    p("hap_q", C::Broadcast, V, "HAP Q", "HAP Q", "mov", "mov", Kind::Hap("hap_q")),
    // Diffusion.
    p("h264", C::Delivery, V, "H.264 (MP4)", "H.264 (MP4)", "mp4", "mp4", Kind::H264),
    p("hevc", C::Delivery, V, "HEVC / H.265 (MP4)", "HEVC / H.265 (MP4)", "mp4", "mp4", Kind::Hevc),
    p("av1", C::Delivery, V, "AV1 (MP4)", "AV1 (MP4)", "mp4", "mp4", Kind::Av1),
    p("vvc", C::Delivery, V, "H.266 / VVC (MP4)", "H.266 / VVC (MP4)", "mp4", "mp4", Kind::Vvc),
    p("vp9", C::Delivery, V, "VP9 (WebM)", "VP9 (WebM)", "webm", "webm", Kind::Vp9),
    p("vp8", C::Delivery, V, "VP8 (WebM)", "VP8 (WebM)", "webm", "webm", Kind::Vp8),
    // Plateformes web (débits YouTube : recommandations publiées par YouTube).
    web(p("youtube_1080", C::Web, V, "YouTube 1080p", "YouTube 1080p", "mp4", "mp4", Kind::H264), 1920, 1080, FP, (8.0, 12.0), 384),
    web(p("youtube_2160", C::Web, V, "YouTube 4K (2160p)", "YouTube 4K (2160p)", "mp4", "mp4", Kind::H264), 3840, 2160, FP, (40.0, 60.0), 384),
    web(p("vimeo_1080", C::Web, V, "Vimeo 1080p", "Vimeo 1080p", "mp4", "mp4", Kind::H264), 1920, 1080, FP, (20.0, 20.0), 320),
    web(p("web_720", C::Web, V, "Web léger 720p", "Light web 720p", "mp4", "mp4", Kind::H264), 1280, 720, FP, (5.0, 7.5), 192),
    web(p("social_vertical", C::Web, V, "Vertical 9:16 (Reels, TikTok, Shorts)", "Vertical 9:16 (Reels, TikTok, Shorts)", "mp4", "mp4", Kind::H264), 1080, 1920, FC, (10.0, 12.0), 256),
    web(p("social_square", C::Web, V, "Carré 1:1 (réseaux sociaux)", "Square 1:1 (social media)", "mp4", "mp4", Kind::H264), 1080, 1080, FC, (8.0, 10.0), 256),
    web(p("social_portrait", C::Web, V, "Portrait 4:5 (réseaux sociaux)", "Portrait 4:5 (social media)", "mp4", "mp4", Kind::H264), 1080, 1350, FC, (8.0, 10.0), 256),
    // Proxies.
    proxy(p("proxy_prores", C::Proxy, V, "Proxy ProRes 422 Proxy", "ProRes 422 Proxy proxy", "mov", "mov", Kind::ProRes(0))),
    proxy(p("proxy_dnxhr", C::Proxy, V, "Proxy DNxHR LB", "DNxHR LB proxy", "mov", "mov", Kind::DnxHr("dnxhr_lb", "yuv422p"))),
    proxy(p("proxy_h264", C::Proxy, V, "Proxy H.264", "H.264 proxy", "mov", "mov", Kind::H264)),
    // Anciens codecs.
    stereo(p("mpeg2", C::Legacy, V, "MPEG-2 (MPG)", "MPEG-2 (MPG)", "mpg", "mpeg", Kind::Mpeg2)),
    stereo(p("mpeg1", C::Legacy, V, "MPEG-1 (MPG)", "MPEG-1 (MPG)", "mpg", "mpeg", Kind::Mpeg1)),
    p("mjpeg", C::Legacy, V, "Motion JPEG (MOV)", "Motion JPEG (MOV)", "mov", "mov", Kind::Mjpeg),
    stereo(p("dv", C::Legacy, V, "DV (PAL ou NTSC)", "DV (PAL or NTSC)", "dv", "dv", Kind::Dv)),
    stereo(p("xvid", C::Legacy, V, "Xvid (AVI)", "Xvid (AVI)", "avi", "avi", Kind::Xvid)),
    stereo(p("wmv", C::Legacy, V, "Windows Media Video (WMV)", "Windows Media Video (WMV)", "wmv", "asf", Kind::Wmv)),
    stereo(p("theora", C::Legacy, V, "Theora (OGV)", "Theora (OGV)", "ogv", "ogg", Kind::Theora)),
    // Images.
    p("image_jpeg", C::Image, V, "JPEG", "JPEG", "jpg", "image2", Kind::Image(I::Jpeg)),
    p("image_png", C::Image, V, "PNG", "PNG", "png", "image2", Kind::Image(I::Png)),
    p("image_tiff", C::Image, V, "TIFF", "TIFF", "tif", "image2", Kind::Image(I::Tiff)),
    p("image_dpx", C::Image, V, "DPX 10 bits", "DPX 10-bit", "dpx", "image2", Kind::Image(I::Dpx)),
    p("image_exr", C::Image, V, "OpenEXR", "OpenEXR", "exr", "image2", Kind::Image(I::Exr)),
    p("image_jxl", C::Image, V, "JPEG XL", "JPEG XL", "jxl", "image2", Kind::Image(I::Jxl)),
    p("image_webp", C::Image, V, "WebP", "WebP", "webp", "image2", Kind::Image(I::Webp)),
    // Sans réencodage.
    p("rewrap_mov", C::NoReencode, V, "Changer de conteneur : MOV", "Rewrap to MOV", "mov", "mov", Kind::Rewrap),
    p("rewrap_mp4", C::NoReencode, V, "Changer de conteneur : MP4", "Rewrap to MP4", "mp4", "mp4", Kind::Rewrap),
    p("rewrap_mxf", C::NoReencode, V, "Changer de conteneur : MXF OP1a", "Rewrap to MXF OP1a", "mxf", "mxf", Kind::Rewrap),
    p("rewrap_mkv", C::NoReencode, V, "Changer de conteneur : MKV", "Rewrap to MKV", "mkv", "matroska", Kind::Rewrap),
    p("cut", C::NoReencode, V, "Découper sans réencodage", "Cut without re-encoding", "*", "", Kind::Cut),
    p("replace_audio", C::NoReencode, V, "Remplacer le son", "Replace sound", "*", "", Kind::ReplaceAudio),
    p("conform", C::NoReencode, V, "Conformer la cadence", "Conform frame rate", "*", "", Kind::Conform),
    p("merge", C::NoReencode, V, "Fusionner les fichiers", "Merge files", "*", "", Kind::Merge),
    p("insert", C::NoReencode, V, "Insert (remplacer un passage)", "Insert (replace a section)", "*", "", Kind::Insert),
    p("subtitles", C::NoReencode, V, "Ajouter des sous-titres", "Add subtitles", "*", "", Kind::Subtitles),
    p("extract_video", C::NoReencode, V, "Extraire l'image (sans son)", "Extract picture (no sound)", "*", "", Kind::ExtractVideo),
    p("extract_audio", C::NoReencode, B, "Extraire le son (WAV polyphonique)", "Extract sound (polyphonic WAV)", "wav", "wav", Kind::ExtractAudio),
    p("extract_tracks", C::NoReencode, B, "Extraire chaque piste (WAV mono)", "Extract each track (mono WAV)", "wav", "wav", Kind::ExtractTracks),
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
    // Analyses.
    p("analyze", C::Analysis, B, "Loudness et True Peak", "Loudness and True Peak", "", "", Kind::Loudness),
    p("silence_detect", C::Analysis, A, "Détection de silences", "Silence detection", "csv", "", Kind::SilenceDetect),
    p("cut_detect", C::Analysis, V, "Détection de plans (EDL)", "Cut detection (EDL)", "edl", "", Kind::CutDetect),
    p("black_detect", C::Analysis, V, "Détection de noir", "Black detection", "csv", "", Kind::BlackDetect),
    p("offline_detect", C::Analysis, V, "Détection de médias hors ligne", "Offline media detection", "csv", "", Kind::OfflineDetect),
    p("vmaf", C::Analysis, V, "Qualité VMAF (comparaison à l'original)", "VMAF quality (against the original)", "", "", Kind::Vmaf),
    p("framemd5", C::Analysis, B, "Empreintes par image (FrameMD5)", "Per-frame checksums (FrameMD5)", "framemd5", "framemd5", Kind::FrameMd5),
];

pub fn find(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

impl Preset {
    /// Produit une image réencodée (réglages d'image disponibles).
    pub fn is_video(&self) -> bool {
        matches!(
            self.kind,
            Kind::ProRes(_)
                | Kind::DnxHr(..)
                | Kind::DnxHd(_)
                | Kind::CineForm
                | Kind::Animation
                | Kind::Uncompressed
                | Kind::Ffv1
                | Kind::H264
                | Kind::Hevc
                | Kind::Av1
                | Kind::Vvc
                | Kind::Vp8
                | Kind::Vp9
                | Kind::Xdcam422
                | Kind::Xdcam35
                | Kind::AvcIntra100
                | Kind::XavcIntra
                | Kind::XavcLongGop
                | Kind::Hap(_)
                | Kind::Mpeg2
                | Kind::Mpeg1
                | Kind::Mjpeg
                | Kind::Xvid
                | Kind::Dv
                | Kind::Wmv
                | Kind::Theora
        )
    }

    pub fn is_image(&self) -> bool {
        matches!(self.kind, Kind::Image(_))
    }

    /// Fichier son produit (fréquence, résolution, normalisation).
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

    pub fn is_analysis(&self) -> bool {
        self.category == Category::Analysis
    }

    pub fn bit_depths(&self) -> &'static [BitDepth] {
        match self.kind {
            Kind::Wav | Kind::Aiff => &[BitDepth::S16, BitDepth::S24, BitDepth::F32],
            Kind::Flac | Kind::Alac => &[BitDepth::S16, BitDepth::S24],
            _ => &[],
        }
    }

    /// Débits proposés (kbit/s) et débit par défaut, pour les formats son avec perte.
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

    /// Débit vidéo réglable.
    pub fn has_video_bitrate(&self) -> bool {
        matches!(
            self.kind,
            Kind::H264
                | Kind::Hevc
                | Kind::Av1
                | Kind::Vvc
                | Kind::Vp8
                | Kind::Vp9
                | Kind::Mpeg2
                | Kind::Mpeg1
                | Kind::Xvid
                | Kind::Wmv
        )
    }

    /// Le choix entre encodeur du système et encodeur logiciel a un sens.
    pub fn has_encoder_choice(&self) -> bool {
        match self.kind {
            Kind::H264 | Kind::Hevc => true,
            Kind::Av1 => !super::video::hardware_candidates("av1").is_empty(),
            Kind::ProRes(_) => !super::video::hardware_candidates("prores").is_empty(),
            _ => false,
        }
    }

    /// Taille d'image libre (les formats broadcast imposent la leur).
    pub fn has_scale(&self) -> bool {
        (self.is_video() || self.is_image())
            && self.frame.is_none()
            && !matches!(self.kind, Kind::Dv | Kind::XavcIntra)
    }

    /// Le son des fichiers vidéo peut être réglé (pistes, normalisation).
    pub fn has_video_audio(&self) -> bool {
        self.is_video() && !matches!(self.kind, Kind::Hap(_))
    }

    /// Ce que ce préréglage demande au FFmpeg du poste : encodeurs (un au moins
    /// de chaque groupe) et filtres.
    fn requirements(&self) -> (&'static [&'static [&'static str]], &'static [&'static str]) {
        match self.kind {
            Kind::Av1 => (
                &[&["libsvtav1", "libaom-av1", "av1_nvenc", "av1_qsv", "av1_amf"]],
                &[],
            ),
            Kind::Vvc => (&[&["libvvenc"]], &[]),
            Kind::Vp8 => (&[&["libvpx"]], &[]),
            Kind::Vp9 => (&[&["libvpx-vp9"]], &[]),
            Kind::Hap(_) => (&[&["hap"]], &[]),
            Kind::Xvid => (&[&["libxvid"]], &[]),
            Kind::Theora => (&[&["libtheora"]], &[]),
            Kind::H264 | Kind::AvcIntra100 | Kind::XavcIntra | Kind::XavcLongGop => (
                &[&[
                    "libx264",
                    "h264_videotoolbox",
                    "h264_nvenc",
                    "h264_qsv",
                    "h264_amf",
                    "h264_mf",
                ]],
                &[],
            ),
            Kind::Hevc => (
                &[&[
                    "libx265",
                    "hevc_videotoolbox",
                    "hevc_nvenc",
                    "hevc_qsv",
                    "hevc_amf",
                    "hevc_mf",
                ]],
                &[],
            ),
            Kind::Image(ImageFormat::Jxl) => (&[&["libjxl"]], &[]),
            Kind::Image(ImageFormat::Webp) => (&[&["libwebp"]], &[]),
            Kind::Mp3 => (&[&["libmp3lame"]], &[]),
            Kind::Opus => (&[&["libopus"]], &[]),
            Kind::Vorbis => (&[&["libvorbis"]], &[]),
            Kind::Vmaf => (&[], &["libvmaf"]),
            Kind::Loudness => (&[], &["ebur128"]),
            Kind::CutDetect => (&[], &["scdet"]),
            _ => (&[], &[]),
        }
    }

    /// Raison pour laquelle le préréglage est indisponible sur ce poste.
    pub fn unavailable(&self) -> Option<String> {
        let (encs, filters) = self.requirements();
        let compiled = encoders::compiled();
        if compiled.is_empty() {
            return Some("FFmpeg introuvable".into());
        }
        // AVC-Intra et XAVC : x264 obligatoire (classes Intra).
        if matches!(
            self.kind,
            Kind::AvcIntra100 | Kind::XavcIntra | Kind::XavcLongGop
        ) && !compiled.contains("libx264")
        {
            return Some("encodeur libx264 absent du FFmpeg intégré".into());
        }
        for group in encs {
            if !group.iter().any(|e| compiled.contains(*e)) {
                return Some(format!("encodeur {} absent du FFmpeg intégré", group[0]));
            }
        }
        let available = encoders::filters();
        for f in filters.iter() {
            if !available.contains(*f) {
                return Some(format!("filtre {f} absent du FFmpeg intégré"));
            }
        }
        None
    }
}

impl Kind {
    /// Identifiant de la famille, pour l'interface (réglages à afficher).
    pub fn id(self) -> &'static str {
        match self {
            Kind::Image(_) => "image",
            Kind::Rewrap => "rewrap",
            Kind::Cut => "cut",
            Kind::ExtractAudio => "extract_audio",
            Kind::ExtractTracks => "extract_tracks",
            Kind::ExtractVideo => "extract_video",
            Kind::ReplaceAudio => "replace_audio",
            Kind::Conform => "conform",
            Kind::Merge => "merge",
            Kind::Insert => "insert",
            Kind::Subtitles => "subtitles",
            Kind::Loudness => "loudness",
            Kind::CutDetect => "cut_detect",
            Kind::BlackDetect => "black_detect",
            Kind::OfflineDetect => "offline_detect",
            Kind::SilenceDetect => "silence_detect",
            Kind::FrameMd5 => "framemd5",
            Kind::Vmaf => "vmaf",
            Kind::Wav
            | Kind::Aiff
            | Kind::Flac
            | Kind::Alac
            | Kind::Mp3
            | Kind::Aac
            | Kind::Opus
            | Kind::Vorbis
            | Kind::Ac3 => "audio",
            _ => "video",
        }
    }
}

/// Format FFmpeg d'après l'extension (préréglages qui gardent le conteneur).
pub fn format_for_ext(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "mov" | "qt" => "mov",
        "mp4" | "m4v" => "mp4",
        "mxf" => "mxf",
        "mkv" => "matroska",
        "webm" => "webm",
        "avi" => "avi",
        "mts" | "m2ts" | "ts" => "mpegts",
        "mpg" | "mpeg" => "mpeg",
        "wmv" => "asf",
        "dv" => "dv",
        "ogv" => "ogg",
        "3gp" => "3gp",
        _ => "mov",
    }
}
