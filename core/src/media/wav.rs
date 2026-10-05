//! Lecture des fichiers WAV / BWF / RF64 (charte §7.2 et §7.3).
//!
//! Gère les fichiers polyphoniques des enregistreurs de tournage : PCM 8 à
//! 32 bits, flottant 32/64 bits, WAVE_FORMAT_EXTENSIBLE, RF64 (> 4 Go),
//! métadonnées BWF (bext) et iXML (noms de pistes, scène, prise...).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use quick_xml::events::Event;
use quick_xml::Reader;
use serde::Serialize;

use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SampleFormat {
    /// Entier signé (non signé pour 8 bits).
    Int,
    Float,
}

/// Métadonnées iXML utiles au tournage.
#[derive(Debug, Clone, Default, Serialize)]
pub struct IxmlInfo {
    pub project: Option<String>,
    pub scene: Option<String>,
    pub take: Option<String>,
    pub tape: Option<String>,
    pub note: Option<String>,
    pub circled: Option<bool>,
    /// Noms de pistes, indexés par canal (0 = premier canal).
    pub track_names: Vec<Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WavInfo {
    pub path: PathBuf,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits: u16,
    pub format: SampleFormat,
    pub frames: u64,
    /// Position du timecode de début en échantillons depuis minuit (BWF).
    pub time_reference: Option<u64>,
    pub ixml: IxmlInfo,
    #[serde(skip)]
    data_offset: u64,
    #[serde(skip)]
    block_align: u16,
}

impl WavInfo {
    pub fn duration(&self) -> f64 {
        self.frames as f64 / self.sample_rate as f64
    }

    /// Nom de la piste `channel`, ou "Piste N" par défaut.
    pub fn track_name(&self, channel: usize) -> String {
        self.ixml
            .track_names
            .get(channel)
            .cloned()
            .flatten()
            .filter(|n| !n.trim().is_empty())
            .unwrap_or_else(|| format!("Piste {}", channel + 1))
    }
}

fn u16le(b: &[u8]) -> u16 {
    u16::from_le_bytes([b[0], b[1]])
}
fn u32le(b: &[u8]) -> u32 {
    u32::from_le_bytes([b[0], b[1], b[2], b[3]])
}
fn u64le(b: &[u8]) -> u64 {
    u64::from_le_bytes(b[..8].try_into().unwrap())
}

fn bad(path: &Path, why: &str) -> Error {
    Error::Unsupported(format!("{} : {why}", path.display()))
}

/// Lit l'en-tête et les métadonnées d'un fichier WAV.
pub fn read_info(path: &Path) -> Result<WavInfo> {
    let mut f = BufReader::new(File::open(path)?);
    let mut header = [0u8; 12];
    f.read_exact(&mut header)?;
    let rf64 = match &header[0..4] {
        b"RIFF" => false,
        b"RF64" | b"BW64" => true,
        _ => return Err(bad(path, "ce n'est pas un fichier WAV")),
    };
    if &header[8..12] != b"WAVE" {
        return Err(bad(path, "ce n'est pas un fichier WAV"));
    }

    let mut fmt: Option<(u16, u16, u32, u16, u16)> = None;
    let mut data: Option<(u64, u64)> = None;
    let mut ds64_data_size: Option<u64> = None;
    let mut time_reference = None;
    let mut ixml = IxmlInfo::default();
    let file_len = f.get_ref().metadata()?.len();
    let mut pos = 12u64;

    while pos + 8 <= file_len {
        f.seek(SeekFrom::Start(pos))?;
        let mut ch = [0u8; 8];
        if f.read_exact(&mut ch).is_err() {
            break;
        }
        let id = [ch[0], ch[1], ch[2], ch[3]];
        let mut size = u32le(&ch[4..8]) as u64;
        let body = pos + 8;
        match &id {
            b"ds64" => {
                let mut b = vec![0u8; size.min(64) as usize];
                f.read_exact(&mut b)?;
                if b.len() >= 16 {
                    ds64_data_size = Some(u64le(&b[8..16]));
                }
            }
            b"fmt " => {
                let mut b = vec![0u8; size.min(64) as usize];
                f.read_exact(&mut b)?;
                if b.len() < 16 {
                    return Err(bad(path, "bloc fmt invalide"));
                }
                let mut tag = u16le(&b[0..2]);
                if tag == 0xFFFE && b.len() >= 26 {
                    tag = u16le(&b[24..26]); // sous-format de WAVE_FORMAT_EXTENSIBLE
                }
                fmt = Some((
                    tag,
                    u16le(&b[2..4]),
                    u32le(&b[4..8]),
                    u16le(&b[12..14]),
                    u16le(&b[14..16]),
                ));
            }
            b"data" => {
                if rf64 && size == 0xFFFF_FFFF {
                    size = ds64_data_size.unwrap_or(file_len - body);
                }
                let size = size.min(file_len - body);
                data = Some((body, size));
            }
            b"bext" if size >= 346 => {
                let mut b = vec![0u8; 346];
                f.read_exact(&mut b)?;
                time_reference = Some(u64le(&b[338..346]));
            }
            b"iXML" | b"IXML" => {
                let mut b = vec![0u8; size.min(4 << 20) as usize];
                f.read_exact(&mut b)?;
                ixml = parse_ixml(&String::from_utf8_lossy(&b));
            }
            _ => {}
        }
        pos = body + size + (size & 1);
    }

    let (tag, channels, sample_rate, block_align, bits) =
        fmt.ok_or_else(|| bad(path, "bloc fmt absent"))?;
    let (data_offset, data_len) = data.ok_or_else(|| bad(path, "bloc data absent"))?;
    let format = match (tag, bits) {
        (1, 8 | 16 | 24 | 32) => SampleFormat::Int,
        (3, 32 | 64) => SampleFormat::Float,
        _ => {
            return Err(bad(
                path,
                &format!("format audio {tag} / {bits} bits non géré"),
            ))
        }
    };
    if channels == 0 || sample_rate == 0 || block_align != channels * bits.div_ceil(8) {
        return Err(bad(path, "en-tête incohérent"));
    }
    Ok(WavInfo {
        path: path.to_path_buf(),
        channels,
        sample_rate,
        bits,
        format,
        frames: data_len / block_align as u64,
        time_reference,
        ixml,
        data_offset,
        block_align,
    })
}

/// Extrait les champs iXML utiles. Tolérant : les champs illisibles sont ignorés.
pub fn parse_ixml(xml: &str) -> IxmlInfo {
    let mut info = IxmlInfo::default();
    let mut reader = Reader::from_str(xml);
    let mut path: Vec<String> = Vec::new();
    let mut track_index: Option<usize> = None;
    let mut track_name: Option<String> = None;
    let mut text = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => {
                path.push(String::from_utf8_lossy(e.name().as_ref()).to_uppercase());
                text.clear();
                if path.last().map(String::as_str) == Some("TRACK") {
                    track_index = None;
                    track_name = None;
                }
            }
            Ok(Event::Text(t)) => text.push_str(&t.xml_content().unwrap_or_default()),
            Ok(Event::CData(t)) => text.push_str(&t.decode().unwrap_or_default()),
            Ok(Event::GeneralRef(r)) => {
                let name = r.decode().unwrap_or_default();
                match name.as_ref() {
                    "amp" => text.push('&'),
                    "lt" => text.push('<'),
                    "gt" => text.push('>'),
                    "quot" => text.push('"'),
                    "apos" => text.push('\''),
                    _ => {
                        if let Ok(Some(c)) = r.resolve_char_ref() {
                            text.push(c);
                        }
                    }
                }
            }
            Ok(Event::End(_)) => {
                let value = text.trim().to_owned();
                text.clear();
                let tag = path.last().map(String::as_str).unwrap_or("");
                let parent = path
                    .len()
                    .checked_sub(2)
                    .map(|i| path[i].as_str())
                    .unwrap_or("");
                if !value.is_empty() {
                    match (parent, tag) {
                        ("BWFXML", "PROJECT") => info.project = Some(value),
                        ("BWFXML", "SCENE") => info.scene = Some(value),
                        ("BWFXML", "TAKE") => info.take = Some(value),
                        ("BWFXML", "TAPE") => info.tape = Some(value),
                        ("BWFXML", "NOTE") => info.note = Some(value),
                        ("BWFXML", "CIRCLED") => {
                            info.circled = Some(value.eq_ignore_ascii_case("TRUE"))
                        }
                        ("TRACK", "CHANNEL_INDEX") => track_index = value.parse().ok(),
                        ("TRACK", "NAME") => track_name = Some(value),
                        _ => {}
                    }
                }
                if tag == "TRACK" {
                    if let (Some(i), Some(n)) = (track_index.take(), track_name.take()) {
                        if (1..=512).contains(&i) {
                            if info.track_names.len() < i {
                                info.track_names.resize(i, None);
                            }
                            info.track_names[i - 1] = Some(n);
                        }
                    }
                }
                path.pop();
            }
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
    }
    info
}

/// Lecteur d'échantillons : renvoie des blocs entrelacés en f32 (-1.0..1.0).
pub struct WavReader {
    info: WavInfo,
    file: BufReader<File>,
    raw: Vec<u8>,
}

impl WavReader {
    pub fn open(path: &Path) -> Result<Self> {
        let info = read_info(path)?;
        let file = BufReader::with_capacity(1 << 20, File::open(path)?);
        Ok(Self {
            info,
            file,
            raw: Vec::new(),
        })
    }

    pub fn info(&self) -> &WavInfo {
        &self.info
    }

    /// Lit jusqu'à `frames` images à partir de `start` dans `out` (entrelacé).
    /// Renvoie le nombre d'images lues (0 en fin de fichier).
    pub fn read(&mut self, start: u64, frames: usize, out: &mut Vec<f32>) -> Result<usize> {
        out.clear();
        if start >= self.info.frames {
            return Ok(0);
        }
        let frames = frames.min((self.info.frames - start) as usize);
        let ba = self.info.block_align as usize;
        self.raw.resize(frames * ba, 0);
        self.file
            .seek(SeekFrom::Start(self.info.data_offset + start * ba as u64))?;
        self.file.read_exact(&mut self.raw)?;
        out.reserve(frames * self.info.channels as usize);
        decode(&self.raw, self.info.bits, self.info.format, out);
        Ok(frames)
    }
}

/// Convertit des échantillons bruts en f32.
fn decode(raw: &[u8], bits: u16, format: SampleFormat, out: &mut Vec<f32>) {
    match (format, bits) {
        (SampleFormat::Int, 8) => out.extend(raw.iter().map(|&b| (b as f32 - 128.0) / 128.0)),
        (SampleFormat::Int, 16) => out.extend(
            raw.chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32_768.0),
        ),
        (SampleFormat::Int, 24) => out.extend(
            raw.chunks_exact(3)
                .map(|c| (i32::from_le_bytes([0, c[0], c[1], c[2]]) >> 8) as f32 / 8_388_608.0),
        ),
        (SampleFormat::Int, 32) => out.extend(
            raw.chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2_147_483_648.0),
        ),
        (SampleFormat::Float, 32) => out.extend(
            raw.chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])),
        ),
        (SampleFormat::Float, 64) => out.extend(
            raw.chunks_exact(8)
                .map(|c| f64::from_le_bytes(c.try_into().unwrap()) as f32),
        ),
        _ => {}
    }
}

/// Écrit un WAV de test (utilisé par les tests et les outils de démonstration).
#[allow(clippy::too_many_arguments)]
#[doc(hidden)]
pub fn write_test_wav(
    path: &Path,
    channels: u16,
    sample_rate: u32,
    bits: u16,
    float: bool,
    frames: u64,
    ixml: Option<&str>,
    sample: impl Fn(u64, u16) -> f64,
) -> Result<()> {
    use std::io::Write;
    let ba = channels * bits / 8;
    let mut data = Vec::with_capacity((frames * ba as u64) as usize);
    for n in 0..frames {
        for c in 0..channels {
            let v = sample(n, c).clamp(-1.0, 1.0);
            match (float, bits) {
                (true, 32) => data.extend((v as f32).to_le_bytes()),
                (false, 16) => data.extend(((v * 32_767.0) as i16).to_le_bytes()),
                (false, 24) => data.extend(&((v * 8_388_607.0) as i32).to_le_bytes()[..3]),
                (false, 32) => data.extend(((v * 2_147_483_647.0) as i32).to_le_bytes()),
                _ => unimplemented!("format de test"),
            }
        }
    }
    let mut chunks = Vec::new();
    let mut push = |id: &[u8; 4], body: &[u8]| {
        chunks.extend(id);
        chunks.extend((body.len() as u32).to_le_bytes());
        chunks.extend(body);
        if body.len() % 2 == 1 {
            chunks.push(0);
        }
    };
    let mut fmt = Vec::new();
    fmt.extend((if float { 3u16 } else { 1u16 }).to_le_bytes());
    fmt.extend(channels.to_le_bytes());
    fmt.extend(sample_rate.to_le_bytes());
    fmt.extend((sample_rate * ba as u32).to_le_bytes());
    fmt.extend(ba.to_le_bytes());
    fmt.extend(bits.to_le_bytes());
    push(b"fmt ", &fmt);
    if let Some(x) = ixml {
        push(b"iXML", x.as_bytes());
    }
    push(b"data", &data);
    let mut f = File::create(path)?;
    f.write_all(b"RIFF")?;
    f.write_all(&((chunks.len() + 4) as u32).to_le_bytes())?;
    f.write_all(b"WAVE")?;
    f.write_all(&chunks)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const IXML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<BWFXML><PROJECT>Court m&#233;trage &amp; essai</PROJECT><SCENE>12A</SCENE><TAKE>3</TAKE>
<CIRCLED>TRUE</CIRCLED><TRACK_LIST><TRACK_COUNT>3</TRACK_COUNT>
<TRACK><CHANNEL_INDEX>1</CHANNEL_INDEX><INTERLEAVE_INDEX>1</INTERLEAVE_INDEX><NAME>Perche</NAME></TRACK>
<TRACK><CHANNEL_INDEX>3</CHANNEL_INDEX><NAME>HF Marie</NAME></TRACK>
</TRACK_LIST></BWFXML>"#;

    #[test]
    fn parses_ixml_fields() {
        let i = parse_ixml(IXML);
        assert_eq!(i.scene.as_deref(), Some("12A"));
        assert_eq!(i.take.as_deref(), Some("3"));
        assert_eq!(i.circled, Some(true));
        assert_eq!(i.project.as_deref(), Some("Court métrage & essai"));
        assert_eq!(
            i.track_names,
            vec![Some("Perche".into()), None, Some("HF Marie".into())]
        );
    }

    #[test]
    fn reads_every_supported_format() {
        let dir = tempfile::tempdir().unwrap();
        for (bits, float) in [(16, false), (24, false), (32, false), (32, true)] {
            let p = dir.path().join(format!("t{bits}{float}.wav"));
            let sig = |n: u64, c: u16| ((n as f64 * 0.01) + c as f64 * 0.1).sin() * 0.5;
            write_test_wav(&p, 3, 48_000, bits, float, 1000, Some(IXML), sig).unwrap();
            let mut r = WavReader::open(&p).unwrap();
            let info = r.info().clone();
            assert_eq!(
                (info.channels, info.sample_rate, info.bits, info.frames),
                (3, 48_000, bits, 1000)
            );
            assert_eq!(info.track_name(0), "Perche");
            assert_eq!(info.track_name(1), "Piste 2");
            let mut buf = Vec::new();
            assert_eq!(r.read(500, 10, &mut buf).unwrap(), 10);
            let tolerance = if bits == 16 { 1e-4 } else { 1e-6 };
            for (k, v) in buf.iter().enumerate() {
                let expected = sig(500 + (k / 3) as u64, (k % 3) as u16) as f32;
                assert!(
                    (v - expected).abs() < tolerance as f32,
                    "{bits} bits : {v} != {expected}"
                );
            }
            assert_eq!(r.read(995, 10, &mut buf).unwrap(), 5);
            assert_eq!(r.read(2000, 10, &mut buf).unwrap(), 0);
        }
    }

    #[test]
    fn rejects_non_wav() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.wav");
        std::fs::write(&p, b"pas un wav du tout").unwrap();
        assert!(read_info(&p).is_err());
    }
}
