//! Logs du PLAYER (charte §7.3) : marqueurs avec couleur, commentaire,
//! points d'entrée et de sortie, scène et prise ; exports EDL CMX3600, ALE,
//! CSV, FCPXML et OTIO.
//!
//! Un marqueur « plage » (entrée et sortie) devient un événement de la
//! timeline exportée ; les marqueurs « point » sont attachés à l'événement qui
//! les contient. Un clip sans plage est exporté en entier. Les positions sont
//! en images depuis le début du clip, à la cadence du clip.

use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::timecode::{FrameRate, Timecode};
use crate::Result;

/// Cadence de la grille des marqueurs posés sur un fichier son (sans image).
pub const AUDIO_LOG_RATE: FrameRate = FrameRate::new(25, 1);

/// Couleurs disponibles : celles des marqueurs Avid, reconnues aussi par
/// Premiere, Resolve et OpenTimelineIO.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Color {
    #[default]
    Red,
    Green,
    Blue,
    Cyan,
    Magenta,
    Yellow,
    Black,
    White,
}

impl Color {
    pub const ALL: [Color; 8] = [
        Color::Red,
        Color::Green,
        Color::Blue,
        Color::Cyan,
        Color::Magenta,
        Color::Yellow,
        Color::Black,
        Color::White,
    ];

    /// Nom en majuscules (EDL Avid, OTIO).
    pub fn upper(self) -> &'static str {
        match self {
            Color::Red => "RED",
            Color::Green => "GREEN",
            Color::Blue => "BLUE",
            Color::Cyan => "CYAN",
            Color::Magenta => "MAGENTA",
            Color::Yellow => "YELLOW",
            Color::Black => "BLACK",
            Color::White => "WHITE",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Color::Red => "red",
            Color::Green => "green",
            Color::Blue => "blue",
            Color::Cyan => "cyan",
            Color::Magenta => "magenta",
            Color::Yellow => "yellow",
            Color::Black => "black",
            Color::White => "white",
        }
    }

    pub fn from_id(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.id() == s)
    }
}

/// Marqueur posé dans le PLAYER.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marker {
    /// Identifiant dans le projet (0 tant qu'il n'est pas enregistré).
    #[serde(default)]
    pub id: i64,
    /// Média concerné.
    pub path: String,
    /// Position du marqueur (images depuis le début du clip).
    pub frame: i64,
    /// Plage : première image et dernière image incluses.
    #[serde(default)]
    pub in_frame: Option<i64>,
    #[serde(default)]
    pub out_frame: Option<i64>,
    #[serde(default)]
    pub color: Color,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub scene: String,
    #[serde(default)]
    pub take: String,
}

impl Marker {
    /// Plage `[début, fin[` si le marqueur a une entrée et une sortie.
    pub fn range(&self) -> Option<(i64, i64)> {
        match (self.in_frame, self.out_frame) {
            (Some(a), Some(b)) => Some((a.min(b), a.max(b) + 1)),
            _ => None,
        }
    }
}

/// Clip à exporter, avec ses marqueurs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogClip {
    pub path: String,
    /// Nom affiché (nom du fichier sans extension par défaut).
    pub name: String,
    /// Bobine (nom de carte ou de bande), sinon le nom du clip.
    #[serde(default)]
    pub reel: Option<String>,
    /// Timecode de la première image, en images.
    pub start_frame: i64,
    pub frame_count: i64,
    pub rate: FrameRate,
    #[serde(default)]
    pub drop_frame: bool,
    #[serde(default)]
    pub width: u32,
    #[serde(default)]
    pub height: u32,
    #[serde(default)]
    pub has_video: bool,
    #[serde(default)]
    pub audio_channels: u32,
    #[serde(default)]
    pub audio_rate: u32,
    #[serde(default)]
    pub markers: Vec<Marker>,
}

impl LogClip {
    /// Décrit un média pour l'export : cadence, timecode de départ et durée
    /// lus dans le fichier (vidéo), ou grille de 25 i/s et timecode BWF (son).
    pub fn from_media(path: &Path, markers: Vec<Marker>) -> Result<Self> {
        let info = crate::media::probe::probe(path)?;
        let name = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.display().to_string());
        let audio_channels = info.audio.iter().map(|a| a.channels).sum();
        let audio_rate = info.audio.first().map(|a| a.sample_rate).unwrap_or(0);
        let reel = ["reel_name", "com.apple.quicktime.reelname", "tape_name"]
            .iter()
            .find_map(|k| info.tags.get(*k).cloned())
            .filter(|r| !r.trim().is_empty());
        let mut clip = LogClip {
            path: path.display().to_string(),
            name,
            reel,
            start_frame: 0,
            frame_count: 0,
            rate: AUDIO_LOG_RATE,
            drop_frame: false,
            width: 0,
            height: 0,
            has_video: false,
            audio_channels,
            audio_rate,
            markers,
        };
        if let Some(v) = &info.video {
            clip.rate = v.rate;
            clip.frame_count = v.frame_count;
            clip.width = v.width;
            clip.height = v.height;
            clip.has_video = true;
            clip.drop_frame = info
                .start_timecode
                .as_deref()
                .is_some_and(|t| t.contains(';'));
            clip.start_frame = info.start_tc().map(|t| t.frames).unwrap_or(0);
        } else {
            clip.frame_count = (info.duration * AUDIO_LOG_RATE.as_f64()).round() as i64;
            if let Ok(w) = crate::media::wav::read_info(path) {
                if let Some(tr) = w.time_reference {
                    let secs = tr as f64 / w.sample_rate.max(1) as f64;
                    clip.start_frame = (secs * AUDIO_LOG_RATE.as_f64()).round() as i64;
                }
                if clip.reel.is_none() {
                    clip.reel = w.ixml.tape.clone().filter(|t| !t.trim().is_empty());
                }
            }
        }
        Ok(clip)
    }

    fn tc(&self, frame: i64) -> Timecode {
        Timecode::from_frames(self.start_frame + frame, self.rate, self.drop_frame)
    }
}

/// Événement de la timeline exportée.
#[derive(Debug, Clone, PartialEq)]
pub struct Event<'a> {
    pub clip: &'a LogClip,
    /// Plage source `[src_in, src_out[`, en images depuis le début du clip.
    pub src_in: i64,
    pub src_out: i64,
    /// Marqueur plage à l'origine de l'événement (aucun : clip entier).
    pub range: Option<&'a Marker>,
    /// Marqueurs point contenus dans la plage.
    pub points: Vec<&'a Marker>,
    /// Position et durée dans la timeline, à la cadence de la séquence.
    pub rec_in: i64,
    pub rec_dur: i64,
}

/// Cadence de la séquence exportée : celle du premier clip (25 par défaut).
fn sequence_rate(clips: &[LogClip]) -> (FrameRate, bool) {
    clips
        .first()
        .map(|c| (c.rate, c.drop_frame && c.rate.supports_drop_frame()))
        .unwrap_or((FrameRate::new(25, 1), false))
}

/// Début de la timeline : 01:00:00:00, usage des salles de montage.
fn sequence_start(rate: FrameRate, drop: bool) -> i64 {
    Timecode::from_components(1, 0, 0, 0, rate, drop).frames
}

/// Découpe les clips en événements, dans l'ordre des clips puis des plages.
pub fn events(clips: &[LogClip]) -> Vec<Event<'_>> {
    let (seq_rate, seq_drop) = sequence_rate(clips);
    let mut rec = sequence_start(seq_rate, seq_drop);
    let mut out = Vec::new();
    for clip in clips {
        let mut ranges: Vec<&Marker> = clip
            .markers
            .iter()
            .filter(|m| m.range().is_some())
            .collect();
        ranges.sort_by_key(|m| m.range());
        let spans: Vec<(i64, i64, Option<&Marker>)> = if ranges.is_empty() {
            vec![(0, clip.frame_count.max(1), None)]
        } else {
            ranges
                .iter()
                .map(|m| {
                    let (a, b) = m.range().expect("filtré");
                    {
                        let end = if clip.frame_count > 0 {
                            b.min(clip.frame_count)
                        } else {
                            b
                        };
                        (a.max(0), end.max(a.max(0) + 1), Some(*m))
                    }
                })
                .collect()
        };
        for (src_in, src_out, range) in spans {
            let mut points: Vec<&Marker> = clip
                .markers
                .iter()
                .filter(|m| m.range().is_none() && m.frame >= src_in && m.frame < src_out)
                .collect();
            points.sort_by_key(|m| m.frame);
            let rec_dur = ((src_out - src_in) as f64 * seq_rate.as_f64() / clip.rate.as_f64())
                .round()
                .max(1.0) as i64;
            out.push(Event {
                clip,
                src_in,
                src_out,
                range,
                points,
                rec_in: rec,
                rec_dur,
            });
            rec += rec_dur;
        }
    }
    out
}

fn one_line(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Texte d'un marqueur : commentaire, puis scène et prise.
fn label(m: &Marker) -> String {
    let mut parts = Vec::new();
    if !m.comment.trim().is_empty() {
        parts.push(one_line(&m.comment));
    }
    if !m.scene.trim().is_empty() {
        parts.push(format!("Sc {}", one_line(&m.scene)));
    }
    if !m.take.trim().is_empty() {
        parts.push(format!("Pr {}", one_line(&m.take)));
    }
    parts.join(" / ")
}

/// Nom de bobine EDL : 8 caractères alphanumériques au plus (norme CMX3600).
fn edl_reel(clip: &LogClip) -> String {
    let src = clip.reel.as_deref().unwrap_or(&clip.name);
    let r: String = src
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() {
                c.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .take(8)
        .collect();
    if r.is_empty() {
        "AX".into()
    } else {
        r
    }
}

/// Texte EDL en ASCII (norme CMX3600) : accents retirés, autres signes remplacés.
fn ascii(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' | 'á' => 'a',
            'À' | 'Â' | 'Ä' | 'Á' => 'A',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'É' | 'È' | 'Ê' | 'Ë' => 'E',
            'î' | 'ï' | 'í' => 'i',
            'Î' | 'Ï' | 'Í' => 'I',
            'ô' | 'ö' | 'ó' => 'o',
            'Ô' | 'Ö' | 'Ó' => 'O',
            'ù' | 'û' | 'ü' | 'ú' => 'u',
            'Ù' | 'Û' | 'Ü' | 'Ú' => 'U',
            'ç' => 'c',
            'Ç' => 'C',
            c if c.is_ascii() => c,
            _ => '_',
        })
        .collect()
}

/// EDL CMX3600 : un événement par plage (ou par clip), marqueurs en `* LOC:`
/// au timecode source (convention Avid, reprise par OpenTimelineIO).
pub fn to_edl(title: &str, clips: &[LogClip]) -> String {
    let (seq_rate, seq_drop) = sequence_rate(clips);
    let rec_tc = |f: i64| Timecode::from_frames(f, seq_rate, seq_drop);
    let mut out = format!(
        "TITLE: {}\nFCM: {}\n",
        one_line(title),
        if seq_drop {
            "DROP FRAME"
        } else {
            "NON-DROP FRAME"
        }
    );
    for (n, e) in events(clips).iter().enumerate() {
        let c = e.clip;
        let channel = if c.has_video { "V" } else { "A" };
        out += &format!(
            "\n{:03}  {:<8} {:<5} C        {} {} {} {}\n",
            n + 1,
            edl_reel(c),
            channel,
            c.tc(e.src_in),
            c.tc(e.src_out),
            rec_tc(e.rec_in),
            rec_tc(e.rec_in + e.rec_dur),
        );
        out += &format!("* FROM CLIP NAME: {}\n", ascii(&one_line(&c.name)));
        out += &format!("* SOURCE FILE: {}\n", ascii(&c.path));
        if let Some(r) = e.range {
            let text = ascii(&label(r));
            if !text.is_empty() {
                out += &format!("* COMMENT: {text}\n");
            }
            out += &format!(
                "* LOC: {} {:<7} {}\n",
                c.tc(e.src_in),
                r.color.upper(),
                text
            );
        }
        for m in &e.points {
            out += &format!(
                "* LOC: {} {:<7} {}\n",
                c.tc(m.frame),
                m.color.upper(),
                ascii(&label(m))
            );
        }
    }
    out
}

fn fps_text(rate: FrameRate) -> String {
    if rate.den == 1 {
        rate.num.to_string()
    } else {
        format!("{:.2}", rate.as_f64())
    }
}

/// ALE : une ligne par événement (sous-clip), lisible par Avid et Resolve.
pub fn to_ale(clips: &[LogClip]) -> String {
    let (seq_rate, _) = sequence_rate(clips);
    let height = clips
        .iter()
        .find(|c| c.has_video)
        .map(|c| c.height)
        .unwrap_or(1080);
    let video_format = if height >= 2160 {
        "CUSTOM".into()
    } else {
        height.to_string()
    };
    let audio_rate = clips
        .iter()
        .map(|c| c.audio_rate)
        .find(|r| *r > 0)
        .unwrap_or(48_000);
    let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ");
    let mut out = String::from("Heading\nFIELD_DELIM\tTABS\n");
    out += &format!(
        "VIDEO_FORMAT\t{video_format}\nAUDIO_FORMAT\t{}khz\nFPS\t{}\n\n",
        audio_rate / 1000,
        fps_text(seq_rate)
    );
    out += "Column\nName\tTracks\tStart\tEnd\tTape\tSource File\tScene\tTake\tComments\tMarker Color\n\nData\n";
    let evs = events(clips);
    for (i, e) in evs.iter().enumerate() {
        let c = e.clip;
        let siblings = evs.iter().filter(|x| std::ptr::eq(x.clip, c)).count();
        let name = if siblings > 1 {
            let rank = evs[..=i].iter().filter(|x| std::ptr::eq(x.clip, c)).count();
            format!("{}_{rank:02}", c.name)
        } else {
            c.name.clone()
        };
        let tracks = format!(
            "{}{}",
            if c.has_video { "V" } else { "" },
            (1..=c.audio_channels.min(24))
                .map(|i| format!("A{i}"))
                .collect::<String>()
        );
        let mut comments: Vec<String> = e.range.map(|r| one_line(&r.comment)).into_iter().collect();
        comments.extend(
            e.points
                .iter()
                .map(|m| format!("{} {}", c.tc(m.frame), label(m))),
        );
        let row = [
            clean(&name),
            tracks,
            c.tc(e.src_in).to_string(),
            c.tc(e.src_out).to_string(),
            clean(c.reel.as_deref().unwrap_or("")),
            clean(file_name(&c.path)),
            clean(e.range.map(|r| r.scene.as_str()).unwrap_or("")),
            clean(e.range.map(|r| r.take.as_str()).unwrap_or("")),
            clean(
                &comments
                    .into_iter()
                    .filter(|s| !s.trim().is_empty())
                    .collect::<Vec<_>>()
                    .join(" | "),
            ),
            e.range.map(|r| r.color.id().to_owned()).unwrap_or_default(),
        ];
        out += &row.join("\t");
        out += "\n";
    }
    out
}

fn file_name(path: &str) -> &str {
    path.rsplit(['/', '\\']).next().unwrap_or(path)
}

fn csv_cell(s: &str) -> String {
    if s.contains([';', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_owned()
    }
}

/// CSV (séparateur « ; », UTF-8 avec BOM) : un marqueur par ligne.
pub fn to_csv(clips: &[LogClip]) -> String {
    let mut out = String::from("\u{feff}");
    out +=
        "Clip;Fichier;Type;TC;TC entrée;TC sortie;Durée (images);Couleur;Scène;Prise;Commentaire\n";
    for c in clips {
        let mut markers: Vec<&Marker> = c.markers.iter().collect();
        markers.sort_by_key(|m| (m.range().map(|r| r.0).unwrap_or(m.frame), m.id));
        for m in markers {
            let (kind, tin, tout, dur) = match m.range() {
                Some((a, b)) => (
                    "plage",
                    c.tc(a).to_string(),
                    c.tc(b).to_string(),
                    (b - a).to_string(),
                ),
                None => ("point", String::new(), String::new(), String::new()),
            };
            let row = [
                c.name.clone(),
                c.path.clone(),
                kind.into(),
                c.tc(m.range().map(|r| r.0).unwrap_or(m.frame)).to_string(),
                tin,
                tout,
                dur,
                m.color.id().into(),
                m.scene.clone(),
                m.take.clone(),
                m.comment.clone(),
            ];
            out += &row
                .iter()
                .map(|s| csv_cell(s))
                .collect::<Vec<_>>()
                .join(";");
            out += "\n";
        }
    }
    out
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn gcd(a: u64, b: u64) -> u64 {
    if b == 0 {
        a.max(1)
    } else {
        gcd(b, a % b)
    }
}

/// Durée FCPXML : fraction exacte de secondes (« 1001/30000s », « 4s »).
fn fcp_time(frames: i64, rate: FrameRate) -> String {
    let num = frames.unsigned_abs() * rate.den as u64;
    let den = rate.num as u64;
    let g = gcd(num, den);
    let sign = if frames < 0 { "-" } else { "" };
    if num == 0 {
        "0s".into()
    } else if den / g == 1 {
        format!("{sign}{}s", num / g)
    } else {
        format!("{sign}{}/{}s", num / g, den / g)
    }
}

/// Chemin local en URL `file://` (espaces et caractères spéciaux encodés).
pub fn file_url(path: &str) -> String {
    let p = path.replace('\\', "/");
    let p = if p.starts_with('/') {
        p
    } else {
        format!("/{p}")
    };
    let mut out = String::from("file://");
    for b in p.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'/' | b'-' | b'_' | b'.' | b'~' | b':' => {
                out.push(b as char)
            }
            _ => out += &format!("%{b:02X}"),
        }
    }
    out
}

/// FCPXML 1.10 (Final Cut Pro, Resolve, Premiere via import XML).
pub fn to_fcpxml(title: &str, clips: &[LogClip]) -> String {
    let (seq_rate, seq_drop) = sequence_rate(clips);
    // Un format par couple (taille, cadence).
    let mut formats: Vec<(u32, u32, FrameRate)> = Vec::new();
    let fmt_id = |formats: &Vec<(u32, u32, FrameRate)>, c: &LogClip| {
        formats
            .iter()
            .position(|f| *f == (c.width, c.height, c.rate))
            .map(|i| format!("r{}", i + 1))
    };
    for c in clips {
        if !formats.contains(&(c.width, c.height, c.rate)) {
            formats.push((c.width, c.height, c.rate));
        }
    }
    let mut res = String::new();
    for (i, (w, h, r)) in formats.iter().enumerate() {
        res += &format!(
            "    <format id=\"r{}\" frameDuration=\"{}\" width=\"{}\" height=\"{}\"/>\n",
            i + 1,
            fcp_time(1, *r),
            if *w > 0 { *w } else { 1920 },
            if *h > 0 { *h } else { 1080 }
        );
    }
    let asset_id = |i: usize| format!("a{}", i + 1);
    for (i, c) in clips.iter().enumerate() {
        let audio = if c.audio_channels > 0 {
            format!(
                " hasAudio=\"1\" audioSources=\"1\" audioChannels=\"{}\" audioRate=\"{}\"",
                c.audio_channels, c.audio_rate
            )
        } else {
            String::new()
        };
        res += &format!(
            "    <asset id=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\" hasVideo=\"{}\" format=\"{}\"{audio}>\n      <media-rep kind=\"original-media\" src=\"{}\"/>\n    </asset>\n",
            asset_id(i),
            xml_esc(&c.name),
            fcp_time(c.start_frame, c.rate),
            fcp_time(c.frame_count.max(1), c.rate),
            u8::from(c.has_video),
            fmt_id(&formats, c).unwrap_or_else(|| "r1".into()),
            xml_esc(&file_url(&c.path)),
        );
    }
    let evs = events(clips);
    let seq_start = sequence_start(seq_rate, seq_drop);
    let total: i64 = evs.iter().map(|e| e.rec_dur).sum();
    let tc_format = if seq_drop { "DF" } else { "NDF" };
    let mut spine = String::new();
    for e in &evs {
        let c = e.clip;
        let idx = clips.iter().position(|x| std::ptr::eq(x, c)).unwrap_or(0);
        let name = e
            .range
            .map(label)
            .filter(|s| !s.is_empty())
            .map(|s| format!("{} ({s})", c.name))
            .unwrap_or_else(|| c.name.clone());
        spine += &format!(
            "            <asset-clip ref=\"{}\" offset=\"{}\" name=\"{}\" start=\"{}\" duration=\"{}\" tcFormat=\"{}\">\n",
            asset_id(idx),
            fcp_time(e.rec_in, seq_rate),
            xml_esc(&name),
            fcp_time(c.start_frame + e.src_in, c.rate),
            fcp_time(e.rec_dur, seq_rate),
            if c.drop_frame && c.rate.supports_drop_frame() { "DF" } else { "NDF" },
        );
        let mut marks: Vec<(&Marker, i64)> = e.range.map(|r| (r, e.src_in)).into_iter().collect();
        marks.extend(e.points.iter().map(|m| (*m, m.frame)));
        for (m, frame) in marks {
            let note = format!("Couleur : {}", m.color.id());
            spine += &format!(
                "              <marker start=\"{}\" duration=\"{}\" value=\"{}\" note=\"{}\"/>\n",
                fcp_time(c.start_frame + frame, c.rate),
                fcp_time(1, c.rate),
                xml_esc(&label(m)),
                xml_esc(&note),
            );
        }
        spine += "            </asset-clip>\n";
    }
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE fcpxml>\n<fcpxml version=\"1.10\">\n  <resources>\n{res}  </resources>\n  <library>\n    <event name=\"VERIFLOW\">\n      <project name=\"{}\">\n        <sequence format=\"{}\" duration=\"{}\" tcStart=\"{}\" tcFormat=\"{tc_format}\">\n          <spine>\n{spine}          </spine>\n        </sequence>\n      </project>\n    </event>\n  </library>\n</fcpxml>\n",
        xml_esc(title),
        clips.first().and_then(|c| fmt_id(&formats, c)).unwrap_or_else(|| "r1".into()),
        fcp_time(total, seq_rate),
        fcp_time(seq_start, seq_rate),
    )
}

fn rt(value: i64, rate: FrameRate) -> Value {
    json!({ "OTIO_SCHEMA": "RationalTime.1", "rate": rate.as_f64(), "value": value as f64 })
}

fn range_json(start: i64, dur: i64, rate: FrameRate) -> Value {
    json!({ "OTIO_SCHEMA": "TimeRange.1", "start_time": rt(start, rate), "duration": rt(dur, rate) })
}

fn otio_marker(m: &Marker, start: i64, dur: i64, rate: FrameRate) -> Value {
    json!({
        "OTIO_SCHEMA": "Marker.2",
        "name": label(m),
        "color": m.color.upper(),
        "marked_range": range_json(start, dur, rate),
        "comment": m.comment,
        "metadata": { "veriflow": { "scene": m.scene, "take": m.take } },
    })
}

/// OpenTimelineIO (JSON `.otio`), lu par Resolve, Premiere (extension), Avid, Nuke.
pub fn to_otio(title: &str, clips: &[LogClip]) -> String {
    let (seq_rate, seq_drop) = sequence_rate(clips);
    let mut video = Vec::new();
    let mut audio = Vec::new();
    for e in events(clips) {
        let c = e.clip;
        let mut markers: Vec<Value> = Vec::new();
        if let Some(r) = e.range {
            markers.push(otio_marker(
                r,
                c.start_frame + e.src_in,
                e.src_out - e.src_in,
                c.rate,
            ));
        }
        markers.extend(
            e.points
                .iter()
                .map(|m| otio_marker(m, c.start_frame + m.frame, 0, c.rate)),
        );
        let clip = json!({
            "OTIO_SCHEMA": "Clip.2",
            "name": c.name,
            "source_range": range_json(c.start_frame + e.src_in, e.src_out - e.src_in, c.rate),
            "media_references": {
                "DEFAULT_MEDIA": {
                    "OTIO_SCHEMA": "ExternalReference.1",
                    "name": file_name(&c.path),
                    "target_url": file_url(&c.path),
                    "available_range": range_json(c.start_frame, c.frame_count.max(1), c.rate),
                    "available_image_bounds": null,
                    "metadata": {},
                }
            },
            "active_media_reference_key": "DEFAULT_MEDIA",
            "markers": markers,
            "effects": [],
            "enabled": true,
            "metadata": { "veriflow": { "reel": c.reel } },
        });
        if c.has_video {
            video.push(clip);
        } else {
            audio.push(clip);
        }
    }
    let track = |name: &str, kind: &str, children: Vec<Value>| {
        json!({
            "OTIO_SCHEMA": "Track.1",
            "name": name,
            "kind": kind,
            "children": children,
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
        })
    };
    let mut tracks = Vec::new();
    if !video.is_empty() || audio.is_empty() {
        tracks.push(track("V1", "Video", video));
    }
    if !audio.is_empty() {
        tracks.push(track("A1", "Audio", audio));
    }
    let timeline = json!({
        "OTIO_SCHEMA": "Timeline.1",
        "name": title,
        "global_start_time": rt(sequence_start(seq_rate, seq_drop), seq_rate),
        "tracks": {
            "OTIO_SCHEMA": "Stack.1",
            "name": "tracks",
            "children": tracks,
            "source_range": null,
            "effects": [],
            "markers": [],
            "enabled": true,
            "metadata": {},
        },
        "metadata": { "veriflow": { "version": crate::VERSION } },
    });
    serde_json::to_string_pretty(&timeline).unwrap_or_default()
}

/// Formats d'export disponibles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    Edl,
    Ale,
    Csv,
    Fcpxml,
    Otio,
}

impl LogFormat {
    pub fn extension(self) -> &'static str {
        match self {
            LogFormat::Edl => "edl",
            LogFormat::Ale => "ale",
            LogFormat::Csv => "csv",
            LogFormat::Fcpxml => "fcpxml",
            LogFormat::Otio => "otio",
        }
    }

    pub fn render(self, title: &str, clips: &[LogClip]) -> String {
        match self {
            LogFormat::Edl => to_edl(title, clips),
            LogFormat::Ale => to_ale(clips),
            LogFormat::Csv => to_csv(clips),
            LogFormat::Fcpxml => to_fcpxml(title, clips),
            LogFormat::Otio => to_otio(title, clips),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub fn sample() -> Vec<LogClip> {
        let m = |path: &str,
                 frame,
                 range: Option<(i64, i64)>,
                 color,
                 comment: &str,
                 scene: &str,
                 take: &str| Marker {
            id: 0,
            path: path.into(),
            frame,
            in_frame: range.map(|r| r.0),
            out_frame: range.map(|r| r.1),
            color,
            comment: comment.into(),
            scene: scene.into(),
            take: take.into(),
        };
        let a = "/rushes/A001/A001C001.mov";
        let b = "/rushes/A001/A001 C002 & co.mov";
        vec![
            LogClip {
                path: a.into(),
                name: "A001C001".into(),
                reel: Some("A001".into()),
                start_frame: Timecode::from_components(10, 0, 0, 0, FrameRate::new(25, 1), false)
                    .frames,
                frame_count: 250,
                rate: FrameRate::new(25, 1),
                drop_frame: false,
                width: 1920,
                height: 1080,
                has_video: true,
                audio_channels: 2,
                audio_rate: 48_000,
                markers: vec![
                    m(
                        a,
                        25,
                        Some((25, 74)),
                        Color::Green,
                        "Bonne prise",
                        "12A",
                        "3",
                    ),
                    m(a, 50, None, Color::Red, "Micro dans le champ", "", ""),
                    m(a, 100, Some((100, 149)), Color::Blue, "", "12A", "4"),
                    m(a, 200, None, Color::Yellow, "hors plage", "", ""),
                ],
            },
            LogClip {
                path: b.into(),
                name: "A001C002".into(),
                reel: None,
                start_frame: Timecode::from_components(10, 5, 0, 0, FrameRate::new(25, 1), false)
                    .frames,
                frame_count: 100,
                rate: FrameRate::new(25, 1),
                drop_frame: false,
                width: 1920,
                height: 1080,
                has_video: true,
                audio_channels: 2,
                audio_rate: 48_000,
                markers: vec![m(b, 10, None, Color::Cyan, "Flou; à vérifier", "", "")],
            },
        ]
    }

    #[test]
    fn events_follow_ranges_and_whole_clips() {
        let clips = sample();
        let ev = events(&clips);
        assert_eq!(ev.len(), 3);
        assert_eq!((ev[0].src_in, ev[0].src_out), (25, 75));
        assert_eq!(
            ev[0].points.len(),
            1,
            "le marqueur à l'image 50 est dans la plage"
        );
        assert_eq!((ev[1].src_in, ev[1].src_out), (100, 150));
        assert!(ev[1].points.is_empty());
        assert_eq!(
            (ev[2].src_in, ev[2].src_out),
            (0, 100),
            "clip sans plage : en entier"
        );
        assert_eq!(ev[0].rec_in, 90_000, "01:00:00:00 à 25 i/s");
        assert_eq!(ev[1].rec_in, 90_050);
        assert_eq!(ev[2].rec_in, 90_100);
    }

    #[test]
    fn edl_lines_are_cmx3600() {
        let edl = to_edl("Jour 1", &sample());
        assert!(edl.starts_with("TITLE: Jour 1\nFCM: NON-DROP FRAME\n"));
        assert!(edl.contains(
            "001  A001     V     C        10:00:01:00 10:00:03:00 01:00:00:00 01:00:02:00\n"
        ));
        assert!(edl.contains("* LOC: 10:00:02:00 RED     Micro dans le champ\n"));
        assert!(
            edl.contains("* LOC: 10:05:00:10 CYAN    Flou; a verifier\n"),
            "ASCII"
        );
        assert!(edl.contains(
            "003  A001C002 V     C        10:05:00:00 10:05:04:00 01:00:04:00 01:00:08:00\n"
        ));
    }

    #[test]
    fn csv_lists_every_marker() {
        let csv = to_csv(&sample());
        assert!(csv.starts_with('\u{feff}'));
        assert_eq!(csv.lines().count(), 1 + 5);
        assert!(csv.contains("A001C001;/rushes/A001/A001C001.mov;plage;10:00:01:00;10:00:01:00;10:00:03:00;50;green;12A;3;Bonne prise\n"));
        assert!(csv.contains("\"Flou; à vérifier\""));
    }

    #[test]
    fn fcp_times_are_exact_fractions() {
        assert_eq!(fcp_time(25, FrameRate::new(25, 1)), "1s");
        assert_eq!(fcp_time(1, FrameRate::new(25, 1)), "1/25s");
        assert_eq!(fcp_time(1, FrameRate::new(30000, 1001)), "1001/30000s");
        assert_eq!(fcp_time(0, FrameRate::new(24, 1)), "0s");
        assert_eq!(file_url("/a b/c&d.mov"), "file:///a%20b/c%26d.mov");
        assert_eq!(file_url("C:\\Rushes\\x.mov"), "file:///C:/Rushes/x.mov");
    }

    #[test]
    fn otio_is_valid_json_with_markers() {
        let v: Value = serde_json::from_str(&to_otio("Jour 1", &sample())).unwrap();
        let clips = &v["tracks"]["children"][0]["children"];
        assert_eq!(clips.as_array().unwrap().len(), 3);
        assert_eq!(clips[0]["markers"].as_array().unwrap().len(), 2);
        assert_eq!(clips[0]["markers"][1]["color"], "RED");
        assert_eq!(clips[0]["source_range"]["start_time"]["value"], 900_025.0);
    }

    #[test]
    fn clip_description_from_real_files() {
        use crate::tools;
        if tools::locate("ffmpeg").is_none() || tools::locate("ffprobe").is_none() {
            eprintln!("FFmpeg absent : test ignoré");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let mov = dir.path().join("A001C007.mov");
        let out = tools::command("ffmpeg")
            .unwrap()
            .args([
                "-v",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=640x360:rate=25:duration=2",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=48000:cl=stereo:d=2",
                "-shortest",
            ])
            .args([
                "-c:v",
                "mjpeg",
                "-c:a",
                "pcm_s24le",
                "-timecode",
                "10:00:00:00",
            ])
            .arg(&mov)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let c = LogClip::from_media(&mov, vec![]).unwrap();
        assert_eq!(c.name, "A001C007");
        assert_eq!(c.rate, FrameRate::new(25, 1));
        assert_eq!(c.start_frame, 900_000);
        assert_eq!(c.frame_count, 50);
        assert_eq!((c.width, c.height, c.has_video), (640, 360, true));
        assert_eq!((c.audio_channels, c.audio_rate), (2, 48_000));

        // Son BWF : grille de 25 i/s et TC lu dans le time reference (01:00:00:00).
        let wav = dir.path().join("12A_T3.wav");
        crate::media::wav::write_test_wav(&wav, 2, 48_000, 24, false, 96_000, None, |_, _| 0.0)
            .unwrap();
        let mut bytes = std::fs::read(&wav).unwrap();
        // Insère un bloc bext minimal avec time reference = 3600 s.
        let mut bext = vec![0u8; 602];
        bext[338..346].copy_from_slice(&(3600u64 * 48_000).to_le_bytes());
        let mut chunk = b"bext".to_vec();
        chunk.extend_from_slice(&(bext.len() as u32).to_le_bytes());
        chunk.extend_from_slice(&bext);
        bytes.splice(12..12, chunk);
        let riff = (bytes.len() - 8) as u32;
        bytes[4..8].copy_from_slice(&riff.to_le_bytes());
        std::fs::write(&wav, bytes).unwrap();
        let a = LogClip::from_media(&wav, vec![]).unwrap();
        assert!(!a.has_video);
        assert_eq!(a.rate, AUDIO_LOG_RATE);
        assert_eq!(a.start_frame, 90_000);
        assert_eq!(a.frame_count, 50);
    }

    #[test]
    fn colors_roundtrip() {
        for c in Color::ALL {
            assert_eq!(Color::from_id(c.id()), Some(c));
        }
    }
}

/// Écrit les cinq exports de l'exemple dans `$VERIFLOW_LOGS_DUMP` (contrôle
/// externe avec OpenTimelineIO, voir `scripts/validate-logs.py`).
#[cfg(test)]
#[test]
fn dump_for_external_check() {
    let Some(dir) = std::env::var_os("VERIFLOW_LOGS_DUMP") else {
        return;
    };
    let dir = std::path::PathBuf::from(dir);
    std::fs::create_dir_all(&dir).unwrap();
    let clips = tests::sample();
    for f in [
        LogFormat::Edl,
        LogFormat::Ale,
        LogFormat::Csv,
        LogFormat::Fcpxml,
        LogFormat::Otio,
    ] {
        std::fs::write(
            dir.join(format!("logs.{}", f.extension())),
            f.render("Jour 1", &clips),
        )
        .unwrap();
    }
}
