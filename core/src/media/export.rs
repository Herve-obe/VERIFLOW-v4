//! Exports des métadonnées (charte §7.2 et §5.4) : les originaux ne sont jamais
//! modifiés. Les métadonnées sortent en CSV, en ALE (Avid, Resolve), en
//! sidecars XMP (Premiere, Resolve) ou dans une copie de travail du média.

use std::collections::BTreeMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use serde::Serialize;

use super::catalog::{MediaDetails, MediaKind};
use super::fields::{field, FieldKind, FIELDS, TRACK_PREFIX};
use crate::editorial::EditClip;
use crate::player::timecode::{FrameRate, Timecode};
use crate::{tools, Error, Result};

/// Un média et ses métadonnées effectives (fichier + éditions du projet).
#[derive(Debug, Clone, Serialize)]
pub struct MediaRecord {
    pub path: PathBuf,
    pub details: MediaDetails,
    pub values: BTreeMap<String, String>,
}

/// Fusionne les métadonnées du fichier et les éditions (les éditions priment).
pub fn merged(
    embedded: &BTreeMap<String, String>,
    edits: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut v = embedded.clone();
    v.extend(edits.iter().map(|(k, x)| (k.clone(), x.clone())));
    v
}

fn name(r: &MediaRecord) -> String {
    r.path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn duration(r: &MediaRecord) -> f64 {
    r.details
        .probe
        .as_ref()
        .map(|p| p.duration)
        .or_else(|| r.details.wav.as_ref().map(|w| w.duration()))
        .unwrap_or(0.0)
}

fn technical(r: &MediaRecord) -> Vec<(&'static str, String)> {
    let p = r.details.probe.as_ref();
    let v = p.and_then(|p| p.video.as_ref());
    let w = r.details.wav.as_ref();
    let a = p.and_then(|p| p.audio.first());
    vec![
        (
            "codec",
            v.map(|v| v.codec.clone())
                .or_else(|| a.map(|a| a.codec.clone()))
                .unwrap_or_default(),
        ),
        (
            "resolution",
            v.map(|v| format!("{}x{}", v.width, v.height))
                .unwrap_or_default(),
        ),
        (
            "fps",
            v.map(|v| format!("{:.3}", v.rate.as_f64()))
                .unwrap_or_default(),
        ),
        (
            "sample_rate",
            w.map(|w| w.sample_rate.to_string())
                .or_else(|| a.map(|a| a.sample_rate.to_string()))
                .unwrap_or_default(),
        ),
        (
            "channels",
            w.map(|w| w.channels.to_string())
                .or_else(|| a.map(|a| a.channels.to_string()))
                .unwrap_or_default(),
        ),
        ("duration_s", format!("{:.3}", duration(r))),
        (
            "start_tc",
            p.and_then(|p| p.start_timecode.clone()).unwrap_or_default(),
        ),
    ]
}

/// Numéros de pistes nommées présents dans l'ensemble des médias.
fn track_numbers(records: &[MediaRecord]) -> Vec<u16> {
    let mut n: Vec<u16> = records
        .iter()
        .flat_map(|r| r.values.keys())
        .filter_map(|k| k.strip_prefix(TRACK_PREFIX)?.parse().ok())
        .collect();
    n.sort();
    n.dedup();
    n
}

/// CSV (séparateur « ; », UTF-8 avec BOM) : technique + tous les champs.
pub fn to_csv(records: &[MediaRecord]) -> String {
    let q = |s: &str| {
        if s.contains([';', '"', '\n', '\r']) {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_owned()
        }
    };
    let tracks = track_numbers(records);
    let mut head: Vec<String> = vec!["fichier".into(), "chemin".into(), "type".into()];
    head.extend(
        technical(records.first().unwrap_or(&dummy()))
            .iter()
            .map(|(k, _)| k.to_string()),
    );
    head.extend(FIELDS.iter().map(|f| f.id.to_string()));
    head.extend(tracks.iter().map(|n| format!("piste_{n}")));
    let mut out = String::from("\u{feff}");
    out += &head.join(";");
    out += "\r\n";
    for r in records {
        let kind = match r.details.kind {
            Some(MediaKind::Video) => "video",
            Some(MediaKind::Audio) => "audio",
            Some(MediaKind::Image) => "image",
            None => "",
        };
        let mut row = vec![q(&name(r)), q(&r.path.display().to_string()), kind.into()];
        row.extend(technical(r).into_iter().map(|(_, v)| q(&v)));
        row.extend(
            FIELDS
                .iter()
                .map(|f| q(r.values.get(f.id).map(String::as_str).unwrap_or(""))),
        );
        row.extend(tracks.iter().map(|n| {
            q(r.values
                .get(&format!("{TRACK_PREFIX}{n}"))
                .map(String::as_str)
                .unwrap_or(""))
        }));
        out += &row.join(";");
        out += "\r\n";
    }
    out
}

fn dummy() -> MediaRecord {
    MediaRecord {
        path: PathBuf::new(),
        details: MediaDetails {
            kind: None,
            probe: None,
            wav: None,
            embedded: BTreeMap::new(),
            error: None,
        },
        values: BTreeMap::new(),
    }
}

/// Cadence d'un clip d'un fichier ALE : celle de la vidéo, ou 25 par défaut.
const ALE_DEFAULT_RATE: FrameRate = FrameRate::new(25, 1);

/// Début et durée d'un clip, en images à la cadence `rate` du fichier ALE. Un
/// clip d'une autre cadence (ou un son, noté sur la grille de 25 i/s) est
/// converti par les secondes. Le départ d'un son seul vient de son time
/// reference en échantillons, converti une seule fois.
fn ale_frames(c: &EditClip, rate: FrameRate) -> (i64, i64) {
    let l = &c.log;
    if l.has_video && l.rate == rate {
        return (l.start_frame, l.frame_count);
    }
    let k = rate.as_f64() / l.rate.as_f64();
    let start = match c.time_reference {
        Some(tr) if !l.has_video && l.audio_rate > 0 => {
            (tr as f64 / l.audio_rate as f64 * rate.as_f64()).round() as i64
        }
        _ => (l.start_frame as f64 * k).round() as i64,
    };
    // Durée d'un son seul : depuis le nombre d'échantillons, un seul arrondi.
    let count = match c.sample_count {
        Some(n) if !l.has_video && l.audio_rate > 0 => {
            (n as f64 / l.audio_rate as f64 * rate.as_f64()).round() as i64
        }
        _ => (l.frame_count as f64 * k).round() as i64,
    };
    (start, count)
}

/// Texte d'un fichier ALE pour des clips ramenés à la cadence `rate`.
fn ale_text(clips: &[&EditClip], rate: FrameRate, ascii: bool) -> String {
    let video_format = clips
        .iter()
        .find(|c| c.log.has_video)
        .map(|c| c.log.height)
        .map(|h| {
            if h >= 2160 {
                "CUSTOM".into()
            } else {
                h.to_string()
            }
        })
        .unwrap_or_else(|| "1080".into());
    let audio_rate = clips
        .iter()
        .map(|c| c.log.audio_rate)
        .find(|r| *r > 0)
        .unwrap_or(48_000);
    let with_sound = clips.iter().any(|c| c.sound.is_some());
    let ale_fields: Vec<&'static super::fields::FieldDef> = FIELDS
        .iter()
        .filter(|f| f.ale.is_some() && f.id != "clip_name" && f.id != "reel")
        .collect();
    let clean = |s: &str| {
        let s = s.replace(['\t', '\r', '\n'], " ");
        if ascii {
            crate::player::logs::ascii(&s)
        } else {
            s
        }
    };
    let tc = |frames: i64, drop: bool| Timecode::from_frames(frames, rate, drop).to_string();
    let mut out = String::new();
    out += "Heading\nFIELD_DELIM\tTABS\n";
    out += &format!(
        "VIDEO_FORMAT\t{video_format}\nAUDIO_FORMAT\t{}khz\nFPS\t{}\n\n",
        audio_rate / 1000,
        crate::player::logs::fps_text(rate)
    );
    // Les intitulés « Tape », « Camera », « Mark IN », « Mark OUT », « Duration »,
    // « Soundroll » et « Sound TC » sont ceux d'Avid Media Composer.
    let mut cols = vec![
        "Name",
        "Tracks",
        "Start",
        "End",
        "Source File",
        "Duration",
        "Tape",
        "Mark IN",
        "Mark OUT",
    ];
    if with_sound {
        cols.extend(["Soundroll", "Sound TC"]);
    }
    cols.extend(ale_fields.iter().filter_map(|f| f.ale));
    out += "Column\n";
    out += &cols.join("\t");
    out += "\n\nData\n";
    for c in clips {
        let l = &c.log;
        let (start, count) = ale_frames(c, rate);
        // Mode drop frame propre à chaque clip (un fichier peut mêler DF et NDF).
        let drop = l.has_video && l.rate == rate && l.drop_frame && rate.supports_drop_frame();
        let channels = match &c.sound {
            Some(s) => s.channels().len(),
            None => l.audio_channels as usize,
        };
        let tracks = format!(
            "{}{}",
            if l.has_video { "V" } else { "" },
            (1..=channels.min(24))
                .map(|i| format!("A{i}"))
                .collect::<String>()
        );
        let clip_name = c.meta.get("clip_name").unwrap_or(&l.name);
        let file = Path::new(&l.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| l.path.clone());
        let mut row = vec![
            clean(clip_name),
            tracks,
            tc(start, drop),
            tc(start + count, drop),
            clean(&file),
            Timecode::from_frames(count, rate, drop).to_string(),
            clean(l.reel.as_deref().unwrap_or(&l.name)),
            tc(start, drop),
            tc(start + count, drop),
        ];
        if with_sound {
            match &c.sound {
                Some(s) => {
                    let stem = s
                        .file
                        .path
                        .file_stem()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| s.file.name.clone());
                    row.push(clean(&stem));
                    row.push(
                        s.file
                            .start
                            .map(|t| tc((t * rate.as_f64()).round() as i64, drop))
                            .unwrap_or_default(),
                    );
                }
                None => row.extend([String::new(), String::new()]),
            }
        }
        for f in &ale_fields {
            let v = c.meta.get(f.id).cloned().unwrap_or_default();
            row.push(clean(&if f.kind == FieldKind::Bool {
                if v == "true" {
                    "Yes".into()
                } else {
                    String::new()
                }
            } else {
                v
            }));
        }
        out += &row.join("\t");
        out += "\n";
    }
    out
}

/// ALE (Avid Log Exchange), lu par Avid Media Composer et DaVinci Resolve.
/// La cadence est celle du premier média vidéo (25 par défaut) ; les autres
/// médias sont ramenés à cette cadence. Pour un fichier par cadence, voir
/// `ale_files`.
pub fn to_ale(records: &[MediaRecord]) -> String {
    let clips: Vec<EditClip> = records
        .iter()
        .map(|r| EditClip::from_record(r, Vec::new()))
        .collect();
    let rate = clips
        .iter()
        .find(|c| c.log.has_video)
        .map(|c| c.log.rate)
        .unwrap_or(ALE_DEFAULT_RATE);
    ale_text(&clips.iter().collect::<Vec<_>>(), rate, false)
}

/// Un fichier ALE par cadence de vidéo (un ALE n'a qu'une cadence). Les sons
/// seuls suivent la première cadence. Rend `(cadence, texte)` dans l'ordre
/// d'apparition des cadences.
pub fn ale_files(clips: &[EditClip], ascii: bool) -> Vec<(FrameRate, String)> {
    let mut rates: Vec<FrameRate> = Vec::new();
    for c in clips.iter().filter(|c| c.log.has_video) {
        if !rates.contains(&c.log.rate) {
            rates.push(c.log.rate);
        }
    }
    if rates.is_empty() {
        rates.push(ALE_DEFAULT_RATE);
    }
    rates
        .iter()
        .enumerate()
        .map(|(i, rate)| {
            let group: Vec<&EditClip> = clips
                .iter()
                .filter(|c| {
                    if c.log.has_video {
                        c.log.rate == *rate
                    } else {
                        i == 0
                    }
                })
                .collect();
            (*rate, ale_text(&group, *rate, ascii))
        })
        .collect()
}

fn xml_esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// Sidecar XMP (même nom que le média, extension .xmp).
pub fn to_xmp(r: &MediaRecord) -> String {
    let mut props = String::new();
    for f in FIELDS {
        let (Some(prop), Some(v)) = (f.xmp, r.values.get(f.id)) else {
            continue;
        };
        let v = if f.kind == FieldKind::Bool {
            if v == "true" { "True" } else { "False" }.to_owned()
        } else {
            v.clone()
        };
        props += &match prop {
            "dc:title" => format!("   <dc:title><rdf:Alt><rdf:li xml:lang=\"x-default\">{}</rdf:li></rdf:Alt></dc:title>\n", xml_esc(&v)),
            "dc:subject" => {
                let items: String = v
                    .split([',', ';'])
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(|s| format!("<rdf:li>{}</rdf:li>", xml_esc(s)))
                    .collect();
                format!("   <dc:subject><rdf:Bag>{items}</rdf:Bag></dc:subject>\n")
            }
            p => format!("   <{p}>{}</{p}>\n", xml_esc(&v)),
        };
    }
    format!(
        "<?xpacket begin=\"\u{feff}\" id=\"W5M0MpCehiHzreSzNTczkc9d\"?>\n\
<x:xmpmeta xmlns:x=\"adobe:ns:meta/\" x:xmptk=\"VERIFLOW {}\">\n\
 <rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\
  <rdf:Description rdf:about=\"\"\n\
    xmlns:dc=\"http://purl.org/dc/elements/1.1/\"\n\
    xmlns:xmp=\"http://ns.adobe.com/xap/1.0/\"\n\
    xmlns:xmpDM=\"http://ns.adobe.com/xmp/1.0/DynamicMedia/\">\n\
{props}  </rdf:Description>\n </rdf:RDF>\n</x:xmpmeta>\n<?xpacket end=\"w\"?>\n",
        crate::VERSION
    )
}

/// Écrit un sidecar XMP par média dans `out_dir` (arborescence relative à `base`).
pub fn write_xmp_sidecars(
    records: &[MediaRecord],
    base: &Path,
    out_dir: &Path,
) -> Result<Vec<PathBuf>> {
    let mut written = Vec::new();
    for r in records {
        let rel = r
            .path
            .strip_prefix(base)
            .unwrap_or(Path::new(r.path.file_name().unwrap_or_default()));
        let target = out_dir.join(rel).with_extension("xmp");
        if target == r.path {
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&target, to_xmp(r))?;
        written.push(target);
    }
    Ok(written)
}

// ---------------------------------------------------------------------------
// Copies de travail : média recopié avec ses métadonnées à jour.
// ---------------------------------------------------------------------------

/// Construit le bloc iXML d'une copie de travail.
fn ixml_for(r: &MediaRecord, channels: u16) -> String {
    let mut x = String::from(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<BWFXML>\n<IXML_VERSION>2.10</IXML_VERSION>\n",
    );
    for f in FIELDS {
        if let (Some(tag), Some(v)) = (f.ixml, r.values.get(f.id)) {
            let v = if f.kind == FieldKind::Bool {
                v.to_uppercase()
            } else {
                v.clone()
            };
            x += &format!("<{tag}>{}</{tag}>\n", xml_esc(&v));
        }
    }
    x += &format!("<TRACK_LIST>\n<TRACK_COUNT>{channels}</TRACK_COUNT>\n");
    for c in 1..=channels {
        let name = r
            .values
            .get(&format!("{TRACK_PREFIX}{c}"))
            .cloned()
            .unwrap_or_default();
        x += &format!(
            "<TRACK><CHANNEL_INDEX>{c}</CHANNEL_INDEX><INTERLEAVE_INDEX>{c}</INTERLEAVE_INDEX><NAME>{}</NAME></TRACK>\n",
            xml_esc(&name)
        );
    }
    x += "</TRACK_LIST>\n</BWFXML>\n";
    x
}

/// Recopie un WAV (RIFF) en remplaçant son bloc iXML ; les autres blocs
/// (fmt, bext, données...) sont conservés à l'identique.
fn wav_working_copy(r: &MediaRecord, target: &Path) -> Result<()> {
    let mut src = fs::File::open(&r.path)?;
    let mut header = [0u8; 12];
    src.read_exact(&mut header)?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" {
        return Err(Error::Unsupported(format!(
            "{} : copie de travail possible uniquement pour les WAV RIFF de moins de 4 Go (RF64 : à venir)",
            r.path.display()
        )));
    }
    let len = src.metadata()?.len();
    let channels = r.details.wav.as_ref().map(|w| w.channels).unwrap_or(1);
    let ixml = ixml_for(r, channels);
    let tmp = target.with_extension("vfpart");
    let mut out = std::io::BufWriter::new(fs::File::create(&tmp)?);
    out.write_all(b"RIFF\0\0\0\0WAVE")?;
    let mut written: u64 = 4;
    let mut pos = 12u64;
    let mut buf = vec![0u8; 1 << 20];
    while pos + 8 <= len {
        src.seek(SeekFrom::Start(pos))?;
        let mut ch = [0u8; 8];
        src.read_exact(&mut ch)?;
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        let padded = size + (size & 1);
        if &ch[0..4] != b"iXML" && &ch[0..4] != b"IXML" {
            out.write_all(&ch)?;
            let mut left = padded.min(len - pos - 8);
            while left > 0 {
                let n = left.min(buf.len() as u64) as usize;
                src.read_exact(&mut buf[..n])?;
                out.write_all(&buf[..n])?;
                left -= n as u64;
            }
            written += 8 + padded;
        }
        pos += 8 + padded;
    }
    let body = ixml.as_bytes();
    out.write_all(b"iXML")?;
    out.write_all(&(body.len() as u32).to_le_bytes())?;
    out.write_all(body)?;
    if body.len() % 2 == 1 {
        out.write_all(&[0])?;
    }
    written += 8 + body.len() as u64 + (body.len() as u64 & 1);
    let mut f = out.into_inner().map_err(|e| e.into_error())?;
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&(written as u32).to_le_bytes())?;
    f.sync_all()?;
    drop(f);
    fs::rename(&tmp, target)?;
    Ok(())
}

/// Recopie une vidéo sans réencodage en y inscrivant les métadonnées.
fn video_working_copy(r: &MediaRecord, target: &Path) -> Result<()> {
    let tmp = target.with_extension(format!(
        "vfpart.{}",
        target
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default()
    ));
    let mut cmd = tools::command("ffmpeg")?;
    cmd.args(["-v", "error", "-nostdin", "-y", "-i"])
        .arg(&r.path)
        .args([
            "-map",
            "0",
            "-c",
            "copy",
            "-map_metadata",
            "0",
            "-movflags",
            "use_metadata_tags",
        ]);
    for (k, v) in &r.values {
        let key = match k.as_str() {
            "clip_name" => "title".to_owned(),
            "comment" => "comment".to_owned(),
            "reel" => "reel_name".to_owned(),
            other if field(other).is_some() => format!("veriflow.{other}"),
            _ => continue,
        };
        cmd.arg("-metadata").arg(format!("{key}={v}"));
    }
    let out = cmd.arg(&tmp).stdin(Stdio::null()).output()?;
    if !out.status.success() {
        let _ = fs::remove_file(&tmp);
        return Err(Error::Tool {
            tool: "ffmpeg".into(),
            message: String::from_utf8_lossy(&out.stderr).trim().to_owned(),
        });
    }
    fs::rename(&tmp, target)?;
    Ok(())
}

/// Crée une copie de travail de chaque média dans `out_dir` (jamais sur l'original).
pub fn working_copies(
    records: &[MediaRecord],
    base: &Path,
    out_dir: &Path,
) -> Vec<(PathBuf, Result<PathBuf>)> {
    records
        .iter()
        .map(|r| {
            let rel = r
                .path
                .strip_prefix(base)
                .unwrap_or(Path::new(r.path.file_name().unwrap_or_default()));
            let target = out_dir.join(rel);
            let result = (|| {
                if target == r.path || target.exists() {
                    return Err(Error::AlreadyExists(target.display().to_string()));
                }
                if let Some(p) = target.parent() {
                    fs::create_dir_all(p)?;
                }
                match r.details.kind {
                    Some(MediaKind::Audio) if r.details.wav.is_some() => {
                        wav_working_copy(r, &target)
                    }
                    Some(MediaKind::Video) => video_working_copy(r, &target),
                    _ => Err(Error::Unsupported(format!(
                        "{} : copie de travail non prise en charge",
                        r.path.display()
                    ))),
                }?;
                Ok(target.clone())
            })();
            (r.path.clone(), result)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::catalog::describe;
    use crate::media::wav::{read_info, write_test_wav, WavReader};

    fn record(path: &Path, edits: &[(&str, &str)]) -> MediaRecord {
        let details = describe(path);
        let e: BTreeMap<String, String> = edits
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        MediaRecord {
            path: path.to_path_buf(),
            values: merged(&details.embedded, &e),
            details,
        }
    }

    #[test]
    fn wav_working_copy_rewrites_ixml_and_keeps_audio() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in/12A_T3.WAV");
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        let ixml = "<BWFXML><SCENE>12A</SCENE><TAKE>3</TAKE><TRACK_LIST><TRACK><CHANNEL_INDEX>1</CHANNEL_INDEX><NAME>Perche</NAME></TRACK></TRACK_LIST></BWFXML>";
        write_test_wav(&src, 2, 48_000, 24, false, 4_800, Some(ixml), |n, c| {
            ((n + c as u64) % 100) as f64 / 200.0
        })
        .unwrap();
        let original = fs::read(&src).unwrap();
        let r = record(
            &src,
            &[
                ("scene", "12B"),
                ("circled", "true"),
                ("track.2", "HF Marie & Paul"),
            ],
        );
        let results = working_copies(
            std::slice::from_ref(&r),
            &dir.path().join("in"),
            &dir.path().join("out"),
        );
        let copy = results[0].1.as_ref().unwrap().clone();
        assert_eq!(fs::read(&src).unwrap(), original, "original intact");
        let info = read_info(&copy).unwrap();
        assert_eq!(info.ixml.scene.as_deref(), Some("12B"));
        assert_eq!(info.ixml.take.as_deref(), Some("3"));
        assert_eq!(info.ixml.circled, Some(true));
        assert_eq!(info.track_name(0), "Perche");
        assert_eq!(info.track_name(1), "HF Marie & Paul");
        // Échantillons identiques.
        let (mut a, mut b) = (Vec::new(), Vec::new());
        WavReader::open(&src)
            .unwrap()
            .read(0, 4_800, &mut a)
            .unwrap();
        WavReader::open(&copy)
            .unwrap()
            .read(0, 4_800, &mut b)
            .unwrap();
        assert_eq!(a, b);
        // Une deuxième copie au même endroit est refusée (rien n'est écrasé).
        let again = working_copies(&[r], &dir.path().join("in"), &dir.path().join("out"));
        assert!(again[0].1.is_err());
    }

    #[test]
    fn csv_ale_and_xmp() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("T1.WAV");
        write_test_wav(&wav, 2, 48_000, 24, false, 48_000 * 2, None, |_, _| 0.0).unwrap();
        let r = record(
            &wav,
            &[
                ("scene", "4"),
                ("take", "2"),
                ("circled", "true"),
                ("comment", "bonne; prise"),
                ("keywords", "ext, nuit"),
            ],
        );
        let csv = to_csv(std::slice::from_ref(&r));
        assert!(csv.lines().nth(1).unwrap().contains("\"bonne; prise\""));
        let ale = to_ale(std::slice::from_ref(&r));
        assert!(ale.starts_with("Heading\nFIELD_DELIM\tTABS\n"));
        let data = ale.split("Data\n").nth(1).unwrap();
        let cols: Vec<&str> = data.lines().next().unwrap().split('\t').collect();
        assert_eq!(
            &cols[..5],
            ["T1", "A1A2", "00:00:00:00", "00:00:02:00", "T1.WAV"]
        );
        assert!(cols.contains(&"Yes"));
        let xmp = to_xmp(&r);
        assert!(
            xmp.contains("<xmpDM:scene>4</xmpDM:scene>")
                && xmp.contains("<xmpDM:good>True</xmpDM:good>")
        );
        assert!(xmp.contains("<rdf:li>ext</rdf:li><rdf:li>nuit</rdf:li>"));
        let written = write_xmp_sidecars(&[r], dir.path(), &dir.path().join("xmp")).unwrap();
        assert_eq!(written[0], dir.path().join("xmp/T1.xmp"));
    }

    fn ale_clip(name: &str, rate: FrameRate, drop: bool) -> EditClip {
        crate::editorial::testutil::clip(name, rate, 107_892, drop)
    }

    /// Valeur de l'en-tête (« FPS ») et colonnes de la première ligne.
    fn ale_head(ale: &str, key: &str) -> String {
        ale.lines()
            .find_map(|l| l.strip_prefix(key).map(|v| v.trim().to_owned()))
            .unwrap()
    }

    fn ale_rows(ale: &str) -> (Vec<&str>, Vec<Vec<&str>>) {
        let cols = ale
            .split("Column\n")
            .nth(1)
            .unwrap()
            .lines()
            .next()
            .unwrap();
        let data = ale.split("Data\n").nth(1).unwrap();
        (
            cols.split('\t').collect(),
            data.lines().map(|l| l.split('\t').collect()).collect(),
        )
    }

    #[test]
    fn ale_fps_ntsc_text() {
        for (rate, text) in [
            (FrameRate::new(24000, 1001), "23.976"),
            (FrameRate::new(30000, 1001), "29.97"),
            (FrameRate::new(60000, 1001), "59.94"),
            (FrameRate::new(25, 1), "25"),
        ] {
            let f = ale_files(&[ale_clip("A", rate, false)], false);
            assert_eq!(ale_head(&f[0].1, "FPS"), text);
        }
        let f = ale_files(&[ale_clip("A", FrameRate::new(30000, 1001), true)], false);
        let (cols, rows) = ale_rows(&f[0].1);
        let at = |n: &str| rows[0][cols.iter().position(|c| *c == n).unwrap()];
        assert_eq!(at("Start"), "01:00:00;00");
        assert_eq!(at("Duration"), "00:00:03;10");
        assert_eq!((at("Tape"), at("Mark IN")), ("A001", "01:00:00;00"));
    }

    #[test]
    fn ale_mixed_rates_split() {
        let mut w = ale_clip("W", FrameRate::new(25, 1), false);
        w.log.has_video = false;
        let clips = [
            ale_clip("A", FrameRate::new(25, 1), false),
            ale_clip("B", FrameRate::new(24000, 1001), false),
            ale_clip("C", FrameRate::new(25, 1), false),
            w,
        ];
        let f = ale_files(&clips, false);
        assert_eq!(f.len(), 2);
        assert_eq!(ale_head(&f[0].1, "FPS"), "25");
        assert_eq!(ale_head(&f[1].1, "FPS"), "23.976");
        let names = |t: &str| {
            ale_rows(t)
                .1
                .iter()
                .map(|r| r[0].to_owned())
                .collect::<Vec<_>>()
        };
        assert_eq!(names(&f[0].1), ["A", "C", "W"]);
        assert_eq!(names(&f[1].1), ["B"]);
    }

    #[test]
    fn ale_sound_columns() {
        let plain = ale_clip("A", FrameRate::new(25, 1), false);
        let files = ale_files(std::slice::from_ref(&plain), false);
        let (cols, _) = ale_rows(&files[0].1);
        assert!(!cols.contains(&"Soundroll"));
        let mut b = ale_clip("B", FrameRate::new(25, 1), false);
        b.sound = Some(crate::editorial::testutil::sound(0.0));
        let f = ale_files(&[plain, b], false);
        let (cols, rows) = ale_rows(&f[0].1);
        let sr = cols.iter().position(|c| *c == "Soundroll").unwrap();
        assert_eq!(cols[sr + 1], "Sound TC");
        assert_eq!(&rows[0][sr..sr + 2], ["", ""]);
        assert_eq!(&rows[1][sr..sr + 2], ["son", "01:00:00:00"]);
    }

    #[test]
    fn ale_ascii_option() {
        let mut a = ale_clip("Scène é", FrameRate::new(25, 1), false);
        a.meta.insert("comment".into(), "à vérifier".into());
        let utf8 = &ale_files(std::slice::from_ref(&a), false)[0].1;
        assert!(utf8.contains("Scène é") && utf8.contains("à vérifier"));
        let ascii = &ale_files(std::slice::from_ref(&a), true)[0].1;
        assert!(ascii.is_ascii());
        assert!(ascii.contains("Scene e") && ascii.contains("a verifier"));
    }

    #[test]
    fn video_working_copy_with_ffmpeg() {
        if tools::locate("ffmpeg").is_none() || tools::locate("ffprobe").is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in/A001.mov");
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        let status = tools::command("ffmpeg")
            .unwrap()
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc2=size=320x180:rate=25:duration=1",
                "-c:v",
                "mjpeg",
                "-timecode",
                "10:00:00:00",
            ])
            .arg(&src)
            .status()
            .unwrap();
        assert!(status.success());
        let r = record(&src, &[("clip_name", "Scène 4"), ("scene", "4")]);
        let res = working_copies(&[r], &dir.path().join("in"), &dir.path().join("out"));
        let copy = res[0].1.as_ref().unwrap();
        let d = describe(copy);
        assert_eq!(
            d.embedded.get("clip_name").map(String::as_str),
            Some("Scène 4")
        );
        assert_eq!(
            d.probe.unwrap().start_timecode.as_deref(),
            Some("10:00:00:00"),
            "timecode conservé"
        );
    }

    #[test]
    fn ale_sound_only_start_from_samples_2398() {
        let rate = FrameRate::new(24000, 1001);
        let video = ale_clip("V", rate, false);
        let mut w = ale_clip("W", FrameRate::new(25, 1), false);
        w.log.has_video = false;
        // 3600 s + 1000 échantillons : 25 i/s donnerait 90001 images puis 86315.
        w.log.start_frame = 90_001;
        w.log.audio_rate = 48_000;
        w.time_reference = Some(172_800_000 + 1000);
        let f = ale_files(&[video, w], false);
        let (cols, rows) = ale_rows(&f[0].1);
        let start = cols.iter().position(|c| *c == "Start").unwrap();
        let exact: f64 = (3600.0 + 1000.0 / 48_000.0) * 24000.0 / 1001.0;
        assert_eq!(exact.round() as i64, 86_314);
        assert_eq!(
            rows[1][start],
            Timecode::from_frames(86_314, rate, false).to_string()
        );
    }

    #[test]
    fn ale_drop_frame_per_clip() {
        let rate = FrameRate::new(30000, 1001);
        let clips = [ale_clip("DF", rate, true), ale_clip("NDF", rate, false)];
        let f = ale_files(&clips, false);
        let (cols, rows) = ale_rows(&f[0].1);
        let at = |r: usize, n: &str| rows[r][cols.iter().position(|c| *c == n).unwrap()];
        assert_eq!(at(0, "Start"), "01:00:00;00");
        assert_eq!(at(0, "End"), "01:00:03;10");
        assert_eq!(at(0, "Mark OUT"), "01:00:03;10");
        // 107892 images en NDF : 59:56:12 + ... pas de point-virgule.
        assert!(!at(1, "Start").contains(';') && !at(1, "End").contains(';'));
        assert!(!at(1, "Duration").contains(';'));
    }

    #[test]
    fn ale_sound_only_duration_from_samples_and_ltc_tracks() {
        let rate = FrameRate::new(24000, 1001);
        let mut w = ale_clip("W", FrameRate::new(25, 1), false);
        w.log.has_video = false;
        w.log.frame_count = 125; // 5 s sur la grille de 25 i/s
        w.log.audio_rate = 48_000;
        w.sample_count = Some(240_100);
        let v = ale_clip("V", rate, false);
        let mut s = ale_clip("S", rate, false);
        let mut snd = crate::editorial::testutil::sound(0.0);
        snd.file.channels = 3;
        snd.file.ltc_channel = Some(2);
        s.sound = Some(snd);
        let f = ale_files(&[v, w, s], false);
        let (cols, rows) = ale_rows(&f[0].1);
        let at = |r: usize, n: &str| rows[r][cols.iter().position(|c| *c == n).unwrap()];
        let exact: f64 = 240_100.0 / 48_000.0 * 24000.0 / 1001.0;
        assert_eq!(
            at(1, "Duration"),
            Timecode::from_frames(exact.round() as i64, rate, false).to_string()
        );
        // Trois canaux dont un de LTC : deux pistes son.
        assert_eq!(at(2, "Tracks"), "VA1A2");
    }
}
