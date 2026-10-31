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

fn stem(r: &MediaRecord) -> String {
    r.path
        .file_stem()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn rate(r: &MediaRecord) -> Option<FrameRate> {
    r.details.probe.as_ref()?.video.as_ref().map(|v| v.rate)
}

fn duration(r: &MediaRecord) -> f64 {
    r.details
        .probe
        .as_ref()
        .map(|p| p.duration)
        .or_else(|| r.details.wav.as_ref().map(|w| w.duration()))
        .unwrap_or(0.0)
}

/// Timecode de début et de fin (exclusif) d'un média, à la cadence donnée.
fn tc_range(r: &MediaRecord, rate: FrameRate) -> (Timecode, Timecode) {
    let df = r
        .details
        .probe
        .as_ref()
        .and_then(|p| p.start_timecode.as_deref())
        .is_some_and(|t| t.contains(';'));
    let start = r
        .details
        .probe
        .as_ref()
        .and_then(|p| p.start_timecode.as_deref())
        .and_then(|t| Timecode::parse(t, rate).ok())
        .or_else(|| {
            // BWF : time reference en échantillons depuis minuit.
            let w = r.details.wav.as_ref()?;
            let secs = w.time_reference? as f64 / w.sample_rate as f64;
            Some(Timecode::from_frames(
                (secs * rate.as_f64()).round() as i64,
                rate,
                df,
            ))
        })
        .unwrap_or_else(|| Timecode::from_frames(0, rate, df));
    let frames = (duration(r) * rate.as_f64()).round() as i64;
    (start, start.offset(frames))
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

/// ALE (Avid Log Exchange), lu par Avid Media Composer et DaVinci Resolve.
/// La cadence est celle du premier média vidéo (25 par défaut).
pub fn to_ale(records: &[MediaRecord]) -> String {
    let rate = records
        .iter()
        .find_map(rate)
        .unwrap_or(FrameRate::new(25, 1));
    let fps = if rate.den == 1 {
        rate.num.to_string()
    } else {
        format!("{:.2}", rate.as_f64())
    };
    let video_format = records
        .iter()
        .find_map(|r| r.details.probe.as_ref()?.video.as_ref().map(|v| v.height))
        .map(|h| {
            if h >= 2160 {
                "CUSTOM".into()
            } else {
                h.to_string()
            }
        })
        .unwrap_or_else(|| "1080".into());
    let audio_rate = records
        .iter()
        .find_map(|r| {
            r.details.wav.as_ref().map(|w| w.sample_rate).or_else(|| {
                r.details
                    .probe
                    .as_ref()?
                    .audio
                    .first()
                    .map(|a| a.sample_rate)
            })
        })
        .unwrap_or(48_000);
    let ale_fields: Vec<&'static super::fields::FieldDef> = FIELDS
        .iter()
        .filter(|f| f.ale.is_some() && f.id != "clip_name")
        .collect();
    let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ");
    let mut out = String::new();
    out += "Heading\nFIELD_DELIM\tTABS\n";
    out += &format!(
        "VIDEO_FORMAT\t{video_format}\nAUDIO_FORMAT\t{}khz\nFPS\t{fps}\n\n",
        audio_rate / 1000
    );
    let mut cols = vec!["Name", "Tracks", "Start", "End", "Source File"];
    cols.extend(ale_fields.iter().filter_map(|f| f.ale));
    out += "Column\n";
    out += &cols.join("\t");
    out += "\n\nData\n";
    for r in records {
        let (start, end) = tc_range(r, rate);
        let has_video = r.details.probe.as_ref().is_some_and(|p| p.video.is_some());
        let channels = r
            .details
            .wav
            .as_ref()
            .map(|w| w.channels as usize)
            .or_else(|| {
                r.details
                    .probe
                    .as_ref()
                    .map(|p| p.audio.iter().map(|a| a.channels as usize).sum())
            })
            .unwrap_or(0);
        let tracks = format!(
            "{}{}",
            if has_video { "V" } else { "" },
            (1..=channels.min(24))
                .map(|i| format!("A{i}"))
                .collect::<String>()
        );
        let clip_name = r
            .values
            .get("clip_name")
            .cloned()
            .unwrap_or_else(|| stem(r));
        let mut row = vec![
            clean(&clip_name),
            tracks,
            start.to_string(),
            end.to_string(),
            clean(&name(r)),
        ];
        for f in &ale_fields {
            let v = r.values.get(f.id).cloned().unwrap_or_default();
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

/// Taille au-delà de laquelle la copie de travail est écrite en RF64 : la
/// taille d'un fichier RIFF tient sur 32 bits (4 Gio).
const RIFF_LIMIT: u64 = u32::MAX as u64;

/// Recopie un WAV (RIFF, RF64 ou BW64) en remplaçant son bloc iXML ; les
/// autres blocs (fmt, bext, données...) sont conservés à l'identique.
fn wav_working_copy(r: &MediaRecord, target: &Path) -> Result<()> {
    wav_copy_with_limit(r, target, RIFF_LIMIT)
}

/// Copie de travail WAV ; au-delà de `limit` octets (ou si la source est
/// déjà en RF64), le fichier produit est un RF64 (EBU Tech 3306) : bloc
/// `ds64` en tête, tailles 64 bits, taille du bloc `data` à 0xFFFFFFFF.
fn wav_copy_with_limit(r: &MediaRecord, target: &Path, limit: u64) -> Result<()> {
    let mut src = fs::File::open(&r.path)?;
    let mut header = [0u8; 12];
    src.read_exact(&mut header)?;
    let src_rf64 = match &header[0..4] {
        b"RIFF" => false,
        b"RF64" | b"BW64" => true,
        _ => {
            return Err(Error::Unsupported(format!(
                "{} : ce n'est pas un fichier WAV",
                r.path.display()
            )))
        }
    };
    if &header[8..12] != b"WAVE" {
        return Err(Error::Unsupported(format!(
            "{} : ce n'est pas un fichier WAV",
            r.path.display()
        )));
    }
    let len = src.metadata()?.len();

    // Inventaire des blocs à recopier (ds64 et iXML exceptés), taille réelle
    // du bloc data lue dans ds64 pour un RF64.
    let mut chunks: Vec<([u8; 4], u64, u64)> = Vec::new();
    let mut ds64_data: Option<u64> = None;
    let mut block_align = 0u64;
    let mut pos = 12u64;
    while pos + 8 <= len {
        src.seek(SeekFrom::Start(pos))?;
        let mut ch = [0u8; 8];
        src.read_exact(&mut ch)?;
        let id = [ch[0], ch[1], ch[2], ch[3]];
        let mut size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        let body = pos + 8;
        match &id {
            b"ds64" => {
                let mut b = [0u8; 16];
                if size >= 16 {
                    src.read_exact(&mut b)?;
                    ds64_data = Some(u64::from_le_bytes(b[8..16].try_into().unwrap()));
                }
            }
            b"fmt " => {
                let mut b = [0u8; 14];
                if size >= 14 {
                    src.read_exact(&mut b)?;
                    block_align = u16::from_le_bytes([b[12], b[13]]) as u64;
                }
            }
            b"data" if src_rf64 && size == 0xFFFF_FFFF => {
                size = ds64_data.unwrap_or(len - body);
            }
            _ => {}
        }
        let size = size.min(len - body);
        if &id != b"ds64" && &id != b"iXML" && &id != b"IXML" {
            chunks.push((id, body, size));
        }
        pos = body + size + (size & 1);
    }

    let channels = r.details.wav.as_ref().map(|w| w.channels).unwrap_or(1);
    let ixml = ixml_for(r, channels);
    let ixml_len = ixml.len() as u64;
    let padded = |s: u64| s + (s & 1);
    let payload: u64 =
        chunks.iter().map(|(_, _, s)| 8 + padded(*s)).sum::<u64>() + 8 + padded(ixml_len);
    let data_size = chunks
        .iter()
        .find(|(id, _, _)| id == b"data")
        .map(|(_, _, s)| *s)
        .unwrap_or(0);
    let rf64 = src_rf64 || 4 + payload > limit;

    let tmp = target.with_extension("vfpart");
    let mut out = std::io::BufWriter::new(fs::File::create(&tmp)?);
    if rf64 {
        // RF64 : taille du fichier dans ds64 (riffSize, dataSize, sampleCount).
        let riff_size = 4 + 8 + 28 + payload;
        out.write_all(b"RF64\xff\xff\xff\xffWAVE")?;
        out.write_all(b"ds64")?;
        out.write_all(&28u32.to_le_bytes())?;
        out.write_all(&riff_size.to_le_bytes())?;
        out.write_all(&data_size.to_le_bytes())?;
        let samples = data_size.checked_div(block_align).unwrap_or(0);
        out.write_all(&samples.to_le_bytes())?;
        out.write_all(&0u32.to_le_bytes())?; // aucune autre taille 64 bits
    } else {
        out.write_all(b"RIFF")?;
        out.write_all(&((4 + payload) as u32).to_le_bytes())?;
        out.write_all(b"WAVE")?;
    }
    let mut buf = vec![0u8; 1 << 20];
    for (id, body, size) in &chunks {
        let field = if rf64 && id == b"data" {
            0xFFFF_FFFF
        } else {
            u32::try_from(*size).map_err(|_| {
                Error::Unsupported(format!("{} : bloc trop grand", r.path.display()))
            })?
        };
        out.write_all(id)?;
        out.write_all(&field.to_le_bytes())?;
        src.seek(SeekFrom::Start(*body))?;
        let mut left = *size;
        while left > 0 {
            let n = left.min(buf.len() as u64) as usize;
            src.read_exact(&mut buf[..n])?;
            out.write_all(&buf[..n])?;
            left -= n as u64;
        }
        if size & 1 == 1 {
            out.write_all(&[0])?;
        }
    }
    out.write_all(b"iXML")?;
    out.write_all(&(ixml_len as u32).to_le_bytes())?;
    out.write_all(ixml.as_bytes())?;
    if ixml_len & 1 == 1 {
        out.write_all(&[0])?;
    }
    let f = out.into_inner().map_err(|e| e.into_error())?;
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
    fn wav_working_copy_rf64() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("in/BIG.WAV");
        fs::create_dir_all(src.parent().unwrap()).unwrap();
        write_test_wav(&src, 3, 48_000, 24, false, 9_601, None, |n, c| {
            ((n * 7 + c as u64) % 97) as f64 / 120.0
        })
        .unwrap();
        let samples = |p: &Path| {
            let mut v = Vec::new();
            WavReader::open(p).unwrap().read(0, 9_601, &mut v).unwrap();
            v
        };
        // Copie « trop grande » pour un RIFF (seuil abaissé) : écrite en RF64.
        let r = record(&src, &[("scene", "40"), ("track.3", "Ambiance")]);
        let rf64 = dir.path().join("out/BIG.WAV");
        fs::create_dir_all(rf64.parent().unwrap()).unwrap();
        wav_copy_with_limit(&r, &rf64, 1000).unwrap();
        let bytes = fs::read(&rf64).unwrap();
        assert_eq!(&bytes[0..4], b"RF64");
        assert_eq!(&bytes[4..8], &[0xff; 4]);
        assert_eq!(&bytes[12..16], b"ds64", "ds64 en premier bloc");
        let riff_size = u64::from_le_bytes(bytes[20..28].try_into().unwrap());
        assert_eq!(riff_size, bytes.len() as u64 - 8);
        let data_size = u64::from_le_bytes(bytes[28..36].try_into().unwrap());
        assert_eq!(data_size, 9_601 * 9);
        assert_eq!(u64::from_le_bytes(bytes[36..44].try_into().unwrap()), 9_601);
        let info = read_info(&rf64).unwrap();
        assert_eq!((info.channels, info.frames), (3, 9_601));
        assert_eq!(info.ixml.scene.as_deref(), Some("40"));
        assert_eq!(info.track_name(2), "Ambiance");
        assert_eq!(samples(&rf64), samples(&src), "échantillons identiques");
        // FFmpeg lit le RF64 produit (si présent).
        if crate::tools::locate("ffprobe").is_some() {
            let out = crate::tools::command("ffprobe")
                .unwrap()
                .args([
                    "-v",
                    "error",
                    "-show_entries",
                    "stream=channels,duration_ts",
                ])
                .args(["-of", "csv=p=0"])
                .arg(&rf64)
                .output()
                .unwrap();
            assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "3,9601");
        }

        // Source RF64 : la copie reste en RF64, données identiques.
        let r2 = record(&rf64, &[("take", "7")]);
        let again = dir.path().join("out2/BIG.WAV");
        fs::create_dir_all(again.parent().unwrap()).unwrap();
        wav_working_copy(&r2, &again).unwrap();
        assert_eq!(&fs::read(&again).unwrap()[0..4], b"RF64");
        let info = read_info(&again).unwrap();
        assert_eq!(info.ixml.take.as_deref(), Some("7"));
        assert_eq!(info.ixml.scene.as_deref(), Some("40"));
        assert_eq!(samples(&again), samples(&src));
        // Sous le seuil, une source RIFF reste un RIFF.
        let small = dir.path().join("out3/BIG.WAV");
        fs::create_dir_all(small.parent().unwrap()).unwrap();
        wav_working_copy(&r, &small).unwrap();
        assert_eq!(&fs::read(&small).unwrap()[0..4], b"RIFF");
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
}
