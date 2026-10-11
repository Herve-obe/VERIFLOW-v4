//! Encodeurs vidéo : choix de l'encodeur sur le poste (charte §6.2) et
//! paramètres de chaque codec, ainsi que le son des fichiers vidéo.

use serde::Serialize;

use super::catalog::{ImageFormat, Kind, Preset};
use super::encoders;
use crate::player::timecode::FrameRate;
use crate::{Error, Result};

/// Encodeur retenu pour un préréglage sur ce poste.
#[derive(Debug, Clone, Serialize)]
pub struct EncoderChoice {
    pub name: String,
    /// Encodeur du système ou de la carte graphique.
    pub hardware: bool,
    /// ProRes produit par un encodeur non certifié par Apple.
    pub uncertified_prores: bool,
}

impl EncoderChoice {
    fn soft(name: &str) -> Self {
        Self {
            name: name.to_owned(),
            hardware: false,
            uncertified_prores: false,
        }
    }
}

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

/// Encodeurs matériels ou du système à essayer, du préféré au moins préféré.
pub fn hardware_candidates(codec: &str) -> &'static [&'static str] {
    match (codec, std::env::consts::OS) {
        ("h264", "macos") => &["h264_videotoolbox"],
        ("h264", "windows") => &["h264_nvenc", "h264_qsv", "h264_amf", "h264_mf"],
        ("h264", _) => &["h264_nvenc", "h264_qsv"],
        ("hevc", "macos") => &["hevc_videotoolbox"],
        ("hevc", "windows") => &["hevc_nvenc", "hevc_qsv", "hevc_amf", "hevc_mf"],
        ("hevc", _) => &["hevc_nvenc", "hevc_qsv"],
        ("av1", "windows") => &["av1_nvenc", "av1_qsv", "av1_amf"],
        ("av1", "linux") => &["av1_nvenc", "av1_qsv"],
        ("aac", "macos") => &["aac_at"],
        ("aac", "windows") => &["aac_mf"],
        ("prores", "macos") => &["prores_videotoolbox"],
        _ => &[],
    }
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

/// Débit (kbit/s) avec plafond et mémoire tampon.
fn rate_args(kbps: u32) -> Vec<String> {
    vec![
        s("-b:v"),
        format!("{kbps}k"),
        s("-maxrate"),
        format!("{}k", kbps * 3 / 2),
        s("-bufsize"),
        format!("{}k", kbps * 2),
    ]
}

/// Arguments d'un encodeur H.264, HEVC ou AV1 au débit donné (kbit/s).
fn long_gop_args(encoder: &str, kbps: u32, ten_bit: bool) -> Vec<String> {
    let mut a = vec![s("-c:v"), s(encoder)];
    let pix10 = if ten_bit { "yuv420p10le" } else { "yuv420p" };
    match encoder {
        "libx264" => {
            a.extend([s("-preset"), s("medium"), s("-pix_fmt"), s("yuv420p")]);
            a.extend(rate_args(kbps));
        }
        "libx265" => {
            a.extend([s("-preset"), s("medium"), s("-pix_fmt"), s(pix10)]);
            a.extend([s("-x265-params"), s("log-level=error")]);
            a.extend(rate_args(kbps));
        }
        "libsvtav1" => {
            a.extend([
                s("-preset"),
                s("8"),
                s("-pix_fmt"),
                s(pix10),
                s("-b:v"),
                format!("{kbps}k"),
            ]);
        }
        "libaom-av1" => {
            a.extend([
                s("-cpu-used"),
                s("6"),
                s("-row-mt"),
                s("1"),
                s("-pix_fmt"),
                s(pix10),
            ]);
            a.extend([s("-b:v"), format!("{kbps}k")]);
        }
        e if e.ends_with("_videotoolbox") => {
            a.extend([s("-b:v"), format!("{kbps}k"), s("-allow_sw"), s("1")]);
        }
        e if e.ends_with("_nvenc") => {
            a.extend([s("-preset"), s("p5"), s("-rc"), s("vbr")]);
            a.extend(rate_args(kbps));
        }
        e if e.ends_with("_amf") => {
            a.extend([s("-rc"), s("vbr_peak")]);
            a.extend(rate_args(kbps));
        }
        _ => a.extend(rate_args(kbps)),
    }
    if encoder.starts_with("hevc") || encoder == "libx265" {
        // Étiquette lue par QuickTime et les appareils Apple.
        a.extend([s("-tag:v"), s("hvc1")]);
    }
    a
}

/// Encodeur AAC : celui du système s'il fonctionne, sinon celui de FFmpeg.
pub fn aac_encoder() -> String {
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
        Kind::Av1 => "av1",
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
        kind => return fixed_encoder(kind).map(EncoderChoice::soft),
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
    let soft: &[&str] = match codec {
        "h264" => &["libx264"],
        "hevc" => &["libx265"],
        _ => &["libsvtav1", "libaom-av1"],
    };
    soft.iter()
        .find(|e| encoders::compiled().contains(**e))
        .map(|e| EncoderChoice::soft(e))
}

/// Encodeur unique des autres codecs.
fn fixed_encoder(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::DnxHr(..) | Kind::DnxHd(_) => "dnxhd",
        Kind::CineForm => "cfhd",
        Kind::Animation => "qtrle",
        Kind::Uncompressed => "v210",
        Kind::Ffv1 => "ffv1",
        Kind::Vvc => "libvvenc",
        Kind::Vp8 => "libvpx",
        Kind::Vp9 => "libvpx-vp9",
        Kind::Xdcam422 | Kind::Xdcam35 | Kind::Mpeg2 => "mpeg2video",
        Kind::AvcIntra100 | Kind::XavcIntra | Kind::XavcLongGop => "libx264",
        Kind::Hap(_) => "hap",
        Kind::Mpeg1 => "mpeg1video",
        Kind::Mjpeg => "mjpeg",
        Kind::Xvid => "libxvid",
        Kind::Dv => "dvvideo",
        Kind::Wmv => "wmv2",
        Kind::Theora => "libtheora",
        Kind::Image(f) => match f {
            ImageFormat::Jpeg => "mjpeg",
            ImageFormat::Png => "png",
            ImageFormat::Tiff => "tiff",
            ImageFormat::Dpx => "dpx",
            ImageFormat::Exr => "exr",
            ImageFormat::Jxl => "libjxl",
            ImageFormat::Webp => "libwebp",
        },
        _ => return None,
    })
}

/// Débit par défaut (kbit/s) selon le codec et la hauteur de l'image.
pub fn default_kbps(kind: Kind, height: u32) -> u32 {
    let h264 = match height {
        0..=576 => 5_000,
        577..=720 => 10_000,
        721..=1080 => 20_000,
        1081..=1440 => 30_000,
        _ => 50_000,
    };
    match kind {
        Kind::Hevc | Kind::Vp9 => h264 * 6 / 10,
        Kind::Av1 => h264 / 2,
        Kind::Vvc => h264 * 4 / 10,
        Kind::Mpeg2 | Kind::Mpeg1 => h264 * 3 / 2,
        _ => h264,
    }
}

/// Débits DNxHD 1080 (Mbit/s) par cadence : LB, SQ, HQ, HQX.
fn dnxhd_rate(rate: FrameRate, level: u8) -> Option<u32> {
    let fps100 = (rate.as_f64() * 100.0).round() as u32;
    let table: [u32; 4] = match fps100 {
        2397 | 2398 | 2400 => [36, 115, 175, 175],
        2500 => [36, 120, 185, 185],
        2997 | 3000 => [45, 145, 220, 220],
        5000 => [75, 240, 365, 365],
        5994 | 6000 => [90, 290, 440, 440],
        _ => return None,
    };
    Some(table[level as usize])
}

/// Caractéristiques de l'image au moment de l'encodage.
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rate: FrameRate,
    pub ten_bit: bool,
}

/// Arguments de l'encodeur vidéo.
pub fn codec_args(
    kind: Kind,
    encoder: &EncoderChoice,
    frame: &Frame,
    kbps: u32,
) -> Result<Vec<String>> {
    let mut a = vec![];
    match kind {
        Kind::ProRes(profile) => a = prores_args(profile, &encoder.name),
        Kind::DnxHr(profile, pix) => a.extend([
            s("-c:v"),
            s("dnxhd"),
            s("-profile:v"),
            s(profile),
            s("-pix_fmt"),
            s(pix),
        ]),
        Kind::DnxHd(level) => {
            let mbps = dnxhd_rate(frame.rate, level).ok_or_else(|| {
                Error::Unsupported(format!(
                    "DNxHD : cadence {:.3} i/s non prise en charge (23,976 à 59,94 i/s)",
                    frame.rate.as_f64()
                ))
            })?;
            let pix = if level == 3 { "yuv422p10le" } else { "yuv422p" };
            a.extend([
                s("-c:v"),
                s("dnxhd"),
                s("-b:v"),
                format!("{mbps}M"),
                s("-pix_fmt"),
                s(pix),
            ]);
        }
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
        Kind::H264 | Kind::Hevc | Kind::Av1 => {
            a = long_gop_args(&encoder.name, kbps, frame.ten_bit)
        }
        Kind::Vvc => a.extend([
            s("-c:v"),
            s("libvvenc"),
            s("-preset"),
            s("faster"),
            s("-b:v"),
            format!("{kbps}k"),
        ]),
        Kind::Vp9 => {
            a.extend([
                s("-c:v"),
                s("libvpx-vp9"),
                s("-row-mt"),
                s("1"),
                s("-deadline"),
                s("good"),
            ]);
            a.extend([
                s("-cpu-used"),
                s("4"),
                s("-pix_fmt"),
                s("yuv420p"),
                s("-b:v"),
                format!("{kbps}k"),
            ]);
        }
        Kind::Vp8 => {
            a.extend([
                s("-c:v"),
                s("libvpx"),
                s("-deadline"),
                s("good"),
                s("-cpu-used"),
                s("4"),
            ]);
            a.extend([s("-pix_fmt"), s("yuv420p"), s("-b:v"), format!("{kbps}k")]);
        }
        Kind::Xdcam422 => a.extend(
            [
                "-c:v",
                "mpeg2video",
                "-pix_fmt",
                "yuv422p",
                "-b:v",
                "50M",
                "-minrate",
                "50M",
                "-maxrate",
                "50M",
                "-bufsize",
                "17825792",
                "-rc_init_occupancy",
                "17825792",
                "-g",
                "12",
                "-bf",
                "2",
                "-intra_vlc",
                "1",
                "-non_linear_quant",
                "1",
                "-dc",
                "10",
                "-qmin",
                "1",
                "-qmax",
                "12",
                "-profile:v",
                "0",
                "-level:v",
                "2",
                "-sc_threshold",
                "1000000000",
            ]
            .map(s),
        ),
        Kind::Xdcam35 => a.extend(
            [
                "-c:v",
                "mpeg2video",
                "-pix_fmt",
                "yuv420p",
                "-b:v",
                "35M",
                "-maxrate",
                "35M",
                "-bufsize",
                "17825792",
                "-g",
                "12",
                "-bf",
                "2",
                "-profile:v",
                "4",
                "-level:v",
                "6",
            ]
            .map(s),
        ),
        Kind::AvcIntra100 => a.extend(
            [
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv422p10le",
                "-x264-params",
                "avcintra-class=100",
            ]
            .map(s),
        ),
        Kind::XavcIntra => {
            let class = if frame.height >= 2160 { 300 } else { 100 };
            a.extend([s("-c:v"), s("libx264"), s("-pix_fmt"), s("yuv422p10le")]);
            a.extend([
                s("-x264-params"),
                format!("avcintra-class={class}:avcintra-flavor=sony"),
            ]);
        }
        Kind::XavcLongGop => {
            let mbps = if frame.height >= 2160 { 150 } else { 50 };
            a.extend(
                [
                    "-c:v",
                    "libx264",
                    "-pix_fmt",
                    "yuv422p10le",
                    "-profile:v",
                    "high422",
                ]
                .map(s),
            );
            a.extend([
                s("-b:v"),
                format!("{mbps}M"),
                s("-maxrate"),
                format!("{mbps}M"),
                s("-bufsize"),
                format!("{mbps}M"),
            ]);
            a.extend([s("-g"), s(frame.rate.nominal().max(1) / 2)]);
        }
        Kind::Hap(variant) => a.extend([s("-c:v"), s("hap"), s("-format"), s(variant)]),
        Kind::Mpeg2 => {
            a.extend([
                s("-c:v"),
                s("mpeg2video"),
                s("-pix_fmt"),
                s("yuv420p"),
                s("-g"),
                s("15"),
                s("-bf"),
                s("2"),
            ]);
            a.extend(rate_args(kbps));
        }
        Kind::Mpeg1 => {
            a.extend([s("-c:v"), s("mpeg1video"), s("-pix_fmt"), s("yuv420p")]);
            a.extend(rate_args(kbps));
        }
        Kind::Mjpeg => a.extend([
            s("-c:v"),
            s("mjpeg"),
            s("-q:v"),
            s("2"),
            s("-pix_fmt"),
            s("yuvj422p"),
        ]),
        Kind::Xvid => a.extend([
            s("-c:v"),
            s("libxvid"),
            s("-b:v"),
            format!("{kbps}k"),
            s("-vtag"),
            s("XVID"),
        ]),
        Kind::Dv => {
            let pix = if frame.height == 480 {
                "yuv411p"
            } else {
                "yuv420p"
            };
            a.extend([s("-c:v"), s("dvvideo"), s("-pix_fmt"), s(pix)]);
        }
        Kind::Wmv => a.extend([s("-c:v"), s("wmv2"), s("-b:v"), format!("{kbps}k")]),
        Kind::Theora => a.extend([s("-c:v"), s("libtheora"), s("-q:v"), s("7")]),
        Kind::Image(f) => match f {
            ImageFormat::Jpeg => a.extend([
                s("-c:v"),
                s("mjpeg"),
                s("-q:v"),
                s("2"),
                s("-pix_fmt"),
                s("yuvj444p"),
            ]),
            ImageFormat::Png => {
                let pix = if frame.ten_bit { "rgb48be" } else { "rgb24" };
                a.extend([s("-c:v"), s("png"), s("-pix_fmt"), s(pix)]);
            }
            ImageFormat::Tiff => {
                let pix = if frame.ten_bit { "rgb48le" } else { "rgb24" };
                a.extend([
                    s("-c:v"),
                    s("tiff"),
                    s("-pix_fmt"),
                    s(pix),
                    s("-compression_algo"),
                    s("lzw"),
                ]);
            }
            ImageFormat::Dpx => a.extend([s("-c:v"), s("dpx"), s("-pix_fmt"), s("gbrp10le")]),
            ImageFormat::Exr => a.extend([
                s("-c:v"),
                s("exr"),
                s("-pix_fmt"),
                s("gbrpf32le"),
                s("-compression"),
                s("zip1"),
            ]),
            ImageFormat::Jxl => a.extend([s("-c:v"), s("libjxl"), s("-distance"), s("1.0")]),
            ImageFormat::Webp => a.extend([s("-c:v"), s("libwebp"), s("-quality"), s("90")]),
        },
        _ => {
            return Err(Error::Unsupported("préréglage sans encodage vidéo".into()));
        }
    }
    Ok(a)
}

/// Son d'un fichier vidéo : codec, canaux et fréquence imposés par le format.
pub struct AudioSpec {
    pub args: Vec<String>,
    /// Canaux maximum par piste (MP2, MP3, WMA, DV : stéréo).
    pub max_channels: Option<u32>,
    pub rate: Option<u32>,
    /// Une piste mono par canal (MXF).
    pub mono_tracks: bool,
}

pub fn video_audio(preset: &Preset, format: &str) -> AudioSpec {
    let pcm = |codec: &str| AudioSpec {
        args: vec![s("-c:a"), s(codec)],
        max_channels: None,
        rate: None,
        mono_tracks: false,
    };
    let kbps = format!("{}k", preset.audio_kbps);
    match (preset.kind, format) {
        (Kind::Xdcam35, _) => AudioSpec {
            rate: Some(48_000),
            mono_tracks: true,
            ..pcm("pcm_s16le")
        },
        (_, "mxf") => AudioSpec {
            rate: Some(48_000),
            mono_tracks: true,
            ..pcm("pcm_s24le")
        },
        (_, "mp4") => AudioSpec {
            args: vec![s("-c:a"), aac_encoder(), s("-b:a"), kbps],
            ..pcm("")
        },
        (Kind::Vp8, _) => AudioSpec {
            args: vec![s("-c:a"), s("libvorbis"), s("-b:a"), s("256k")],
            max_channels: Some(2),
            ..pcm("")
        },
        (_, "webm") => AudioSpec {
            args: vec![s("-c:a"), s("libopus"), s("-b:a"), s("256k")],
            rate: Some(48_000),
            ..pcm("")
        },
        (_, "mpeg") => AudioSpec {
            args: vec![s("-c:a"), s("mp2"), s("-b:a"), s("256k")],
            max_channels: Some(2),
            rate: Some(48_000),
            mono_tracks: false,
        },
        (_, "dv") => AudioSpec {
            max_channels: Some(2),
            rate: Some(48_000),
            ..pcm("pcm_s16le")
        },
        (_, "avi") => AudioSpec {
            args: vec![s("-c:a"), s("libmp3lame"), s("-b:a"), s("320k")],
            max_channels: Some(2),
            rate: Some(48_000),
            mono_tracks: false,
        },
        (_, "asf") => AudioSpec {
            args: vec![s("-c:a"), s("wmav2"), s("-b:a"), s("192k")],
            max_channels: Some(2),
            ..pcm("")
        },
        (_, "ogg") => AudioSpec {
            args: vec![s("-c:a"), s("libvorbis"), s("-b:a"), s("256k")],
            ..pcm("")
        },
        _ => pcm("pcm_s24le"),
    }
}

/// Codec de sous-titres accepté par le conteneur.
pub fn subtitle_codec(format: &str, sub_ext: &str) -> Result<&'static str> {
    Ok(match format {
        "mov" | "mp4" => "mov_text",
        "matroska" if sub_ext.eq_ignore_ascii_case("ass") => "ass",
        "matroska" => "srt",
        "webm" => "webvtt",
        _ => {
            return Err(Error::Unsupported(
                "sous-titres en piste : MOV, MP4, MKV ou WebM uniquement (sinon, les incruster)"
                    .into(),
            ))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dnxhd_rates_follow_frame_rate() {
        assert_eq!(dnxhd_rate(FrameRate::new(25, 1), 1), Some(120));
        assert_eq!(dnxhd_rate(FrameRate::new(24000, 1001), 2), Some(175));
        assert_eq!(dnxhd_rate(FrameRate::new(30000, 1001), 0), Some(45));
        assert_eq!(dnxhd_rate(FrameRate::new(15, 1), 0), None);
    }

    #[test]
    fn default_bitrate_follows_height_and_codec() {
        assert_eq!(default_kbps(Kind::H264, 1080), 20_000);
        assert_eq!(default_kbps(Kind::H264, 540), 5_000);
        assert_eq!(default_kbps(Kind::Hevc, 2160), 30_000);
        assert_eq!(default_kbps(Kind::Av1, 1080), 10_000);
    }
}
