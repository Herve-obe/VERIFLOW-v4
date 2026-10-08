//! Analyse d'un fichier média via FFprobe (charte §7.2).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

use serde::{Deserialize, Serialize};

use crate::player::timecode::{FrameRate, Timecode};
use crate::{tools, Error, Result};

#[derive(Debug, Clone, Serialize)]
pub struct VideoStream {
    pub index: u32,
    pub codec: String,
    pub width: u32,
    pub height: u32,
    pub rate: FrameRate,
    pub pix_fmt: String,
    /// Nombre d'images, mesuré ou estimé à partir de la durée.
    pub frame_count: i64,
    /// Horodatage de la première image, en secondes (souvent 0).
    pub start_time: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AudioStream {
    pub index: u32,
    pub codec: String,
    pub sample_rate: u32,
    pub channels: u32,
    pub bits: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MediaInfo {
    pub path: String,
    pub format: String,
    pub duration: f64,
    pub size: u64,
    pub video: Option<VideoStream>,
    pub audio: Vec<AudioStream>,
    /// Timecode de début ("HH:MM:SS:FF"), si présent dans le fichier.
    pub start_timecode: Option<String>,
    /// Étiquettes du conteneur et de la piste vidéo (clés en minuscules).
    pub tags: std::collections::BTreeMap<String, String>,
}

impl MediaInfo {
    /// Timecode de début converti en nombre d'images (0 si absent).
    pub fn start_tc(&self) -> Option<Timecode> {
        let v = self.video.as_ref()?;
        let s = self.start_timecode.as_deref()?;
        Timecode::parse(s, v.rate).ok()
    }
}

// Structures brutes du JSON FFprobe.
#[derive(Deserialize)]
struct RawProbe {
    #[serde(default)]
    streams: Vec<RawStream>,
    format: Option<RawFormat>,
}

#[derive(Deserialize, Default)]
struct RawTags {
    timecode: Option<String>,
    #[serde(flatten)]
    other: std::collections::BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
struct RawStream {
    index: u32,
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
    r_frame_rate: Option<String>,
    pix_fmt: Option<String>,
    nb_frames: Option<String>,
    duration: Option<String>,
    start_time: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    bits_per_raw_sample: Option<String>,
    bits_per_sample: Option<u32>,
    #[serde(default)]
    tags: RawTags,
    #[serde(default)]
    disposition: RawDisposition,
}

#[derive(Deserialize, Default)]
struct RawDisposition {
    #[serde(default)]
    attached_pic: u8,
}

#[derive(Deserialize)]
struct RawFormat {
    format_name: Option<String>,
    duration: Option<String>,
    size: Option<String>,
    #[serde(default)]
    tags: RawTags,
}

fn num<T: std::str::FromStr>(s: &Option<String>) -> Option<T> {
    s.as_deref().and_then(|v| v.parse().ok())
}

/// Interprète la sortie JSON de FFprobe.
pub fn parse_ffprobe_json(path: &str, json: &[u8]) -> Result<MediaInfo> {
    let raw: RawProbe = serde_json::from_slice(json).map_err(|e| Error::Tool {
        tool: "ffprobe".into(),
        message: e.to_string(),
    })?;
    let format = raw
        .format
        .ok_or_else(|| Error::Unsupported(path.to_owned()))?;
    let duration: f64 = num(&format.duration).unwrap_or(0.0);

    let mut start_timecode = format.tags.timecode.clone();
    let mut tags = std::collections::BTreeMap::new();
    let mut add_tags = |t: &RawTags| {
        for (k, v) in &t.other {
            if let Some(s) = v.as_str() {
                tags.entry(k.to_lowercase()).or_insert_with(|| s.to_owned());
            }
        }
    };
    add_tags(&format.tags);
    let mut video = None;
    let mut audio = Vec::new();
    for s in &raw.streams {
        if start_timecode.is_none() {
            start_timecode = s.tags.timecode.clone();
        }
        match s.codec_type.as_deref() {
            Some("video") if video.is_none() && s.disposition.attached_pic == 0 => {
                add_tags(&s.tags);
                let rate = s
                    .avg_frame_rate
                    .as_deref()
                    .and_then(FrameRate::parse)
                    .or_else(|| s.r_frame_rate.as_deref().and_then(FrameRate::parse))
                    .ok_or_else(|| Error::Unsupported(format!("{path} : cadence inconnue")))?;
                let stream_duration: f64 = num(&s.duration).unwrap_or(duration);
                let frame_count = num::<i64>(&s.nb_frames)
                    .unwrap_or_else(|| (stream_duration * rate.as_f64()).round() as i64);
                video = Some(VideoStream {
                    index: s.index,
                    codec: s.codec_name.clone().unwrap_or_default(),
                    width: s.width.unwrap_or(0),
                    height: s.height.unwrap_or(0),
                    rate,
                    pix_fmt: s.pix_fmt.clone().unwrap_or_default(),
                    frame_count,
                    start_time: num(&s.start_time).unwrap_or(0.0),
                });
            }
            // Piste timecode (tmcd) : les caméras y rangent souvent le nom de bobine.
            Some("data") => add_tags(&s.tags),
            Some("audio") => audio.push(AudioStream {
                index: s.index,
                codec: s.codec_name.clone().unwrap_or_default(),
                sample_rate: num(&s.sample_rate).unwrap_or(0),
                channels: s.channels.unwrap_or(0),
                bits: num(&s.bits_per_raw_sample).or(s.bits_per_sample.filter(|b| *b > 0)),
            }),
            _ => {}
        }
    }

    Ok(MediaInfo {
        path: path.to_owned(),
        format: format.format_name.unwrap_or_default(),
        duration,
        size: num(&format.size).unwrap_or(0),
        video,
        audio,
        start_timecode,
        tags,
    })
}

/// Analyse un fichier avec FFprobe.
pub fn probe(path: &Path) -> Result<MediaInfo> {
    // Un même média est analysé plusieurs fois à l'ouverture (image, son,
    // recherche de LTC) : FFprobe ne tourne qu'une fois tant que le fichier
    // ne change pas (taille et date de modification).
    let key = std::fs::metadata(path)
        .ok()
        .map(|m| (path.to_path_buf(), m.len(), m.modified().ok()));
    if let Some(k) = &key {
        if let Some(info) = cache().lock().ok().and_then(|c| c.get(k).cloned()) {
            return Ok(info);
        }
    }
    let info = run_ffprobe(path)?;
    if let (Some(k), Ok(mut c)) = (key, cache().lock()) {
        if c.len() >= PROBE_CACHE {
            c.clear();
        }
        c.insert(k, info.clone());
    }
    Ok(info)
}

/// Nombre maximal d'analyses gardées en mémoire.
const PROBE_CACHE: usize = 512;

type ProbeKey = (PathBuf, u64, Option<SystemTime>);

fn cache() -> &'static Mutex<HashMap<ProbeKey, MediaInfo>> {
    static CACHE: OnceLock<Mutex<HashMap<ProbeKey, MediaInfo>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn run_ffprobe(path: &Path) -> Result<MediaInfo> {
    let output = tools::command("ffprobe")?
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(Error::Tool {
            tool: "ffprobe".into(),
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    parse_ffprobe_json(&path.display().to_string(), &output.stdout)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
      "streams": [
        {"index":0,"codec_type":"video","codec_name":"prores","width":1920,"height":1080,
         "avg_frame_rate":"25/1","r_frame_rate":"25/1","pix_fmt":"yuv422p10le","nb_frames":"500",
         "tags":{"timecode":"10:00:00:00"}},
        {"index":1,"codec_type":"audio","codec_name":"pcm_s24le","sample_rate":"48000","channels":2,
         "bits_per_raw_sample":"24"},
        {"index":2,"codec_type":"data","tags":{"timecode":"10:00:00:00"}}
      ],
      "format":{"format_name":"mov,mp4,m4a,3gp,3g2,mj2","duration":"20.000000","size":"167155457",
                "tags":{"Reel_Name":"A001","encoder":"Lavf"}}
    }"#;

    #[test]
    fn parses_prores_sample() {
        let info = parse_ffprobe_json("a.mov", SAMPLE.as_bytes()).unwrap();
        let v = info.video.as_ref().unwrap();
        assert_eq!((v.width, v.height, v.frame_count), (1920, 1080, 500));
        assert_eq!(v.rate, FrameRate::new(25, 1));
        assert_eq!(info.audio.len(), 1);
        assert_eq!(info.audio[0].bits, Some(24));
        assert_eq!(info.start_tc().unwrap().frames, 10 * 3600 * 25);
        assert_eq!(info.tags.get("reel_name").map(String::as_str), Some("A001"));
    }

    #[test]
    fn estimates_frame_count_from_duration() {
        let json = r#"{"streams":[{"index":0,"codec_type":"video","codec_name":"h264",
          "width":1280,"height":720,"avg_frame_rate":"30000/1001","duration":"10.01"}],
          "format":{"duration":"10.01"}}"#;
        let info = parse_ffprobe_json("b.mp4", json.as_bytes()).unwrap();
        assert_eq!(info.video.unwrap().frame_count, 300);
        assert!(info.start_timecode.is_none());
    }
}
