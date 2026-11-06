//! Modèle commun des exports vers le montage : un clip de chutier, ses
//! métadonnées, son éventuel son synchronisé, et les options d'export.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::media::export::MediaRecord;
use crate::player::logs::{LogClip, Marker, AUDIO_LOG_RATE};
use crate::sync::SyncFile;

/// Son d'enregistreur synchronisé à un clip vidéo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncedSound {
    pub file: SyncFile,
    /// Départ du son par rapport au début de la vidéo, en secondes (positif :
    /// le son commence après la première image).
    pub offset: f64,
    /// Bobine du son : sound roll (iXML TAPE) du fichier de l'enregistreur, à
    /// défaut le nom du fichier. Ce n'est jamais la bobine de la caméra.
    #[serde(default)]
    pub roll: Option<String>,
    /// Profondeur des échantillons en bits, si connue (16 par défaut à l'export).
    #[serde(default)]
    pub bits: Option<u16>,
}

impl SyncedSound {
    /// Le son recouvre-t-il l'image du clip ? Sinon aucun format ne l'écrit
    /// (`sound_no_overlap`).
    pub fn overlaps(&self, clip: &LogClip) -> bool {
        let v_dur = clip.frame_count.max(1) as f64 / clip.rate.as_f64();
        let lead = self.offset.max(0.0);
        let skip = (-self.offset).max(0.0);
        (v_dur - lead).min(self.file.duration - skip) > 0.0
    }

    /// Canaux du son à exporter (numérotés depuis 1), sans la piste de LTC.
    pub fn channels(&self) -> Vec<u32> {
        (1..=self.file.channels.max(1))
            .filter(|ch| self.file.ltc_channel != Some(*ch as usize - 1))
            .collect()
    }

    /// Nom de la bobine du son : sound roll, à défaut le nom du fichier.
    pub fn roll_name(&self) -> String {
        self.roll
            .as_deref()
            .map(str::trim)
            .filter(|r| !r.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| self.file.name.clone())
    }

    /// Son à écrire dans les formats qui le rangent avec l'image : il
    /// recouvre l'image et a au moins un canal hors LTC.
    pub fn attachable(&self, clip: &LogClip) -> bool {
        self.overlaps(clip) && !self.channels().is_empty()
    }
}

/// Clip à exporter : sa description technique et ses marqueurs (`log`), les
/// métadonnées de MEDIA (identifiants de `media::fields`) et le son synchronisé.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditClip {
    pub log: LogClip,
    #[serde(default)]
    pub meta: BTreeMap<String, String>,
    #[serde(default)]
    pub sound: Option<SyncedSound>,
    /// Son seul : time reference BWF en échantillons (à la cadence
    /// `log.audio_rate`). `log.start_frame` n'en garde qu'un arrondi à 25 i/s ;
    /// ALE et FCPXML convertissent depuis ce nombre, une seule fois.
    #[serde(default)]
    pub time_reference: Option<u64>,
}

fn filled(s: Option<&String>) -> Option<String> {
    s.filter(|s| !s.trim().is_empty()).cloned()
}

impl EditClip {
    /// Décrit un média déjà analysé, sans relancer FFprobe (`details.probe`).
    /// Bobine : champ « reel » des métadonnées, puis étiquettes `reel_name`,
    /// `com.apple.quicktime.reelname`, `tape_name`, puis iXML TAPE, puis le nom.
    pub fn from_record(r: &MediaRecord, markers: Vec<Marker>) -> Self {
        let path = &r.path;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let probe = r.details.probe.as_ref();
        let wav = r.details.wav.as_ref();
        let tag = |k: &str| probe.and_then(|p| filled(p.tags.get(k)));
        let reel = filled(r.values.get("reel"))
            .or_else(|| tag("reel_name"))
            .or_else(|| tag("com.apple.quicktime.reelname"))
            .or_else(|| tag("tape_name"))
            .or_else(|| wav.and_then(|w| filled(w.ixml.tape.as_ref())))
            .unwrap_or_else(|| name.clone());
        let mut log = LogClip {
            path: path.display().to_string(),
            name,
            reel: Some(reel),
            start_frame: 0,
            frame_count: 0,
            rate: AUDIO_LOG_RATE,
            drop_frame: false,
            width: 0,
            height: 0,
            has_video: false,
            audio_channels: probe
                .map(|p| p.audio.iter().map(|a| a.channels).sum())
                .or_else(|| wav.map(|w| w.channels as u32))
                .unwrap_or(0),
            audio_rate: probe
                .and_then(|p| p.audio.first().map(|a| a.sample_rate))
                .or_else(|| wav.map(|w| w.sample_rate))
                .unwrap_or(0),
            markers,
        };
        let mut time_reference = None;
        if let Some(v) = probe.and_then(|p| p.video.as_ref()) {
            let p = probe.expect("vidéo sans analyse");
            log.rate = v.rate;
            log.frame_count = v.frame_count;
            log.width = v.width;
            log.height = v.height;
            log.has_video = true;
            log.drop_frame = p.start_timecode.as_deref().is_some_and(|t| t.contains(';'));
            log.start_frame = p.start_tc().map(|t| t.frames).unwrap_or(0);
        } else {
            time_reference = wav.and_then(|w| w.time_reference);
            let secs = probe
                .map(|p| p.duration)
                .or_else(|| wav.map(|w| w.duration()))
                .unwrap_or(0.0);
            log.frame_count = (secs * AUDIO_LOG_RATE.as_f64()).round() as i64;
            if let Some(w) = wav {
                if let Some(tr) = w.time_reference {
                    let s = tr as f64 / w.sample_rate.max(1) as f64;
                    log.start_frame = (s * AUDIO_LOG_RATE.as_f64()).round() as i64;
                }
            }
        }
        EditClip {
            log,
            meta: r.values.clone(),
            sound: None,
            time_reference,
        }
    }
}

/// Format de sortie.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditFormat {
    Fcpxml,
    Xmeml,
    Edl,
    Ale,
    Otio,
    ResolveCsv,
}

impl EditFormat {
    /// Extension du fichier écrit (sans point).
    pub fn extension(self) -> &'static str {
        match self {
            EditFormat::Fcpxml => "fcpxml",
            EditFormat::Xmeml => "xml",
            EditFormat::Edl => "edl",
            EditFormat::Ale => "ale",
            EditFormat::Otio => "otio",
            EditFormat::ResolveCsv => "csv",
        }
    }
}

/// Chemins des médias dans le fichier exporté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum PathMode {
    #[default]
    Absolute,
    /// Relatifs au dossier `EditOptions::base`.
    Relative,
}

/// Version du DTD FCPXML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FcpxmlVersion {
    #[serde(rename = "1.9")]
    V1_9,
    #[default]
    #[serde(rename = "1.10")]
    V1_10,
    #[serde(rename = "1.11")]
    V1_11,
}

impl FcpxmlVersion {
    /// Valeur de l'attribut `version` de `<fcpxml>`.
    pub fn as_str(self) -> &'static str {
        match self {
            FcpxmlVersion::V1_9 => "1.9",
            FcpxmlVersion::V1_10 => "1.10",
            FcpxmlVersion::V1_11 => "1.11",
        }
    }
}

/// Options communes aux formats (celles qui ne concernent pas un format sont ignorées).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct EditOptions {
    /// Nom de la bibliothèque, du bin ou de la séquence EDL.
    pub title: String,
    pub paths: PathMode,
    /// Dossier du fichier écrit, base des chemins relatifs.
    pub base: Option<PathBuf>,
    /// Exporter les marqueurs du PLAYER.
    pub markers: bool,
    pub fcpxml_version: FcpxmlVersion,
    /// ALE en ASCII (Avid ancien) au lieu d'UTF-8.
    pub ale_ascii: bool,
}

impl Default for EditOptions {
    fn default() -> Self {
        Self {
            title: "VERIFLOW".into(),
            paths: PathMode::Absolute,
            base: None,
            markers: true,
            fcpxml_version: FcpxmlVersion::default(),
            ale_ascii: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::catalog::MediaDetails;
    use crate::media::probe::{MediaInfo, VideoStream};
    use crate::media::wav::{IxmlInfo, WavInfo};
    use crate::player::timecode::FrameRate;

    fn record(tags: &[(&str, &str)], values: &[(&str, &str)], tape: Option<&str>) -> MediaRecord {
        let video = VideoStream {
            index: 0,
            codec: "h264".into(),
            width: 1920,
            height: 1080,
            rate: FrameRate::new(30000, 1001),
            pix_fmt: "yuv420p".into(),
            color_space: String::new(),
            frame_count: 300,
            start_time: 0.0,
        };
        let probe = MediaInfo {
            path: "/m/A001C002.mov".into(),
            format: "mov".into(),
            duration: 10.0,
            size: 0,
            video: Some(video),
            audio: vec![],
            start_timecode: Some("01:00:00;02".into()),
            tags: tags
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        };
        let wav = tape.map(|t| {
            let mut w = WavInfo::decoded(std::path::Path::new("/m/A001C002.mov"), 2, 48000, 480000);
            w.ixml = IxmlInfo {
                tape: Some(t.into()),
                ..Default::default()
            };
            w
        });
        MediaRecord {
            path: "/m/A001C002.mov".into(),
            details: MediaDetails {
                kind: None,
                probe: Some(probe),
                wav,
                embedded: BTreeMap::new(),
                error: None,
            },
            values: values
                .iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
        }
    }

    #[test]
    fn reel_priority() {
        let reel = |r: &MediaRecord| EditClip::from_record(r, vec![]).log.reel.unwrap();
        let all = record(
            &[("reel_name", "TAGREEL"), ("tape_name", "TAGTAPE")],
            &[("reel", "CHAMP")],
            Some("IXML"),
        );
        assert_eq!(reel(&all), "CHAMP");
        let no_field = record(
            &[("reel_name", "TAGREEL"), ("tape_name", "TAGTAPE")],
            &[("reel", "  ")],
            Some("IXML"),
        );
        assert_eq!(reel(&no_field), "TAGREEL");
        let tape_tag = record(&[("tape_name", "TAGTAPE")], &[], Some("IXML"));
        assert_eq!(reel(&tape_tag), "TAGTAPE");
        assert_eq!(reel(&record(&[], &[], Some("IXML"))), "IXML");
        assert_eq!(reel(&record(&[], &[], None)), "A001C002");
    }

    #[test]
    fn from_record_reads_probe_without_ffprobe() {
        let c = EditClip::from_record(&record(&[], &[("scene", "12")], None), vec![]);
        assert!(c.log.has_video && c.log.drop_frame);
        assert_eq!(c.log.rate, FrameRate::new(30000, 1001));
        assert_eq!(c.log.frame_count, 300);
        assert_eq!(c.log.start_frame, 107_894);
        assert_eq!(c.meta["scene"], "12");
        assert!(c.sound.is_none());
    }

    #[test]
    fn options_serde_snake_case() {
        let o: EditOptions = serde_json::from_str(
            r#"{"paths":"relative","fcpxml_version":"1.11","ale_ascii":true}"#,
        )
        .unwrap();
        assert_eq!(o.paths, PathMode::Relative);
        assert_eq!(o.fcpxml_version, FcpxmlVersion::V1_11);
        assert!(o.ale_ascii && o.markers);
        let f: Vec<EditFormat> = serde_json::from_str(r#"["resolve_csv","fcpxml"]"#).unwrap();
        assert_eq!(f, [EditFormat::ResolveCsv, EditFormat::Fcpxml]);
    }
}
