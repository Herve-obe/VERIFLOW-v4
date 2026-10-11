//! Report des métadonnées de tournage (blocs `bext` et `iXML`) d'un WAV
//! source vers le WAV produit. FFmpeg ne recopie pas l'iXML (scène, prise,
//! noms de pistes) et ne garde qu'une partie du `bext` : les blocs d'origine
//! sont donc recopiés tels quels, avec la référence temporelle recalculée si
//! la fréquence change.

use std::fs;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;

use crate::Result;

/// Bloc RIFF : identifiant, position des données, taille.
type Chunk = ([u8; 4], u64, u64);

/// Blocs d'un fichier RIFF/WAVE.
fn chunks(file: &mut fs::File) -> Result<Option<Vec<Chunk>>> {
    let len = file.metadata()?.len();
    let mut header = [0u8; 12];
    file.seek(SeekFrom::Start(0))?;
    if file.read_exact(&mut header).is_err()
        || &header[0..4] != b"RIFF"
        || &header[8..12] != b"WAVE"
    {
        return Ok(None);
    }
    let mut list = Vec::new();
    let mut pos = 12u64;
    while pos + 8 <= len {
        file.seek(SeekFrom::Start(pos))?;
        let mut ch = [0u8; 8];
        file.read_exact(&mut ch)?;
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        let id = [ch[0], ch[1], ch[2], ch[3]];
        list.push((id, pos + 8, size.min(len - pos - 8)));
        pos += 8 + size + (size & 1);
    }
    Ok(Some(list))
}

fn read_chunk(file: &mut fs::File, at: u64, size: u64) -> Result<Vec<u8>> {
    let mut buf = vec![0u8; size as usize];
    file.seek(SeekFrom::Start(at))?;
    file.read_exact(&mut buf)?;
    Ok(buf)
}

/// Remplace le contenu d'une balise XML simple (`<TAG>...</TAG>`), si présente.
fn set_tag(xml: &str, tag: &str, value: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let Some(a) = xml.find(&open) else {
        return xml.to_owned();
    };
    let start = a + open.len();
    let Some(b) = xml[start..].find(&close) else {
        return xml.to_owned();
    };
    format!("{}{value}{}", &xml[..start], &xml[start + b..])
}

fn tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{tag}>"))?;
    Some(xml[start..start + end].trim().to_owned())
}

/// Changement de fréquence et décalage du début (fichier découpé).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retime {
    pub old_rate: u32,
    pub new_rate: u32,
    /// Secondes retirées au début du fichier.
    pub shift: f64,
}

impl Retime {
    fn changes(&self) -> bool {
        self.old_rate != self.new_rate || self.shift > 0.0
    }

    /// Position (échantillons depuis minuit) dans le fichier produit.
    fn apply(&self, samples: u64) -> u64 {
        rescale(samples, self.old_rate, self.new_rate)
            + (self.shift.max(0.0) * self.new_rate as f64).round() as u64
    }
}

/// Met à jour l'iXML pour la fréquence, la résolution et le début du fichier produit.
pub fn update_ixml(xml: &str, t: &Retime, bits: u16) -> String {
    let new_rate = t.new_rate;
    let mut x = set_tag(xml, "FILE_SAMPLE_RATE", &new_rate.to_string());
    x = set_tag(&x, "AUDIO_BIT_DEPTH", &bits.to_string());
    if t.changes() {
        let hi = tag_value(&x, "TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_HI")
            .and_then(|v| v.parse::<u64>().ok());
        let lo = tag_value(&x, "TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_LO")
            .and_then(|v| v.parse::<u64>().ok());
        if let (Some(hi), Some(lo)) = (hi, lo) {
            let samples = t.apply((hi << 32) | lo);
            x = set_tag(
                &x,
                "TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_HI",
                &(samples >> 32).to_string(),
            );
            x = set_tag(
                &x,
                "TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_LO",
                &(samples & 0xFFFF_FFFF).to_string(),
            );
        }
        x = set_tag(&x, "TIMESTAMP_SAMPLE_RATE", &new_rate.to_string());
    }
    x
}

fn rescale(samples: u64, old_rate: u32, new_rate: u32) -> u64 {
    if old_rate == 0 {
        return samples;
    }
    ((samples as u128 * new_rate as u128 + old_rate as u128 / 2) / old_rate as u128) as u64
}

/// Recalcule la référence temporelle d'un bloc `bext` (octets 338 à 345).
pub fn update_bext(bext: &mut [u8], t: &Retime) {
    if bext.len() < 346 || !t.changes() {
        return;
    }
    let lo = u32::from_le_bytes(bext[338..342].try_into().unwrap()) as u64;
    let hi = u32::from_le_bytes(bext[342..346].try_into().unwrap()) as u64;
    let samples = t.apply((hi << 32) | lo);
    bext[338..342].copy_from_slice(&(samples as u32).to_le_bytes());
    bext[342..346].copy_from_slice(&((samples >> 32) as u32).to_le_bytes());
}

fn write_chunk(out: &mut impl Write, id: &[u8], body: &[u8]) -> Result<u64> {
    out.write_all(id)?;
    out.write_all(&(body.len() as u32).to_le_bytes())?;
    out.write_all(body)?;
    if body.len() % 2 == 1 {
        out.write_all(&[0])?;
    }
    Ok(8 + body.len() as u64 + (body.len() as u64 & 1))
}

/// Recopie `bext` et `iXML` de `source` dans `output` (WAV RIFF produit).
/// Renvoie faux si rien n'a été fait (source sans ces blocs, fichier RF64).
pub fn carry(source: &Path, output: &Path, t: &Retime, bits: u16) -> Result<bool> {
    let mut src = fs::File::open(source)?;
    let Some(src_chunks) = chunks(&mut src)? else {
        return Ok(false);
    };
    let find = |id: &[u8; 4]| src_chunks.iter().find(|c| c.0.eq_ignore_ascii_case(id));
    let mut bext = match find(b"bext") {
        Some(&(_, at, size)) => Some(read_chunk(&mut src, at, size)?),
        None => None,
    };
    let ixml = match find(b"iXML") {
        Some(&(_, at, size)) => {
            let raw = read_chunk(&mut src, at, size)?;
            let text = String::from_utf8_lossy(&raw)
                .trim_end_matches('\0')
                .to_owned();
            Some(update_ixml(&text, t, bits))
        }
        None => None,
    };
    if bext.is_none() && ixml.is_none() {
        return Ok(false);
    }
    if let Some(b) = bext.as_mut() {
        update_bext(b, t);
    }
    let mut out_file = fs::File::open(output)?;
    let Some(out_chunks) = chunks(&mut out_file)? else {
        return Ok(false);
    };
    let tmp = output.with_extension("vfbwf");
    let mut w = BufWriter::new(fs::File::create(&tmp)?);
    w.write_all(b"RIFF\0\0\0\0WAVE")?;
    let mut written = 4u64;
    let mut buf = vec![0u8; 1 << 20];
    let (replace_bext, replace_ixml) = (bext.is_some(), ixml.is_some());
    for (id, at, size) in out_chunks {
        if (replace_bext && id.eq_ignore_ascii_case(b"bext"))
            || (replace_ixml && id.eq_ignore_ascii_case(b"iXML"))
        {
            continue;
        }
        w.write_all(&id)?;
        w.write_all(&(size as u32).to_le_bytes())?;
        out_file.seek(SeekFrom::Start(at))?;
        let mut left = size;
        while left > 0 {
            let n = left.min(buf.len() as u64) as usize;
            out_file.read_exact(&mut buf[..n])?;
            w.write_all(&buf[..n])?;
            left -= n as u64;
        }
        if size & 1 == 1 {
            w.write_all(&[0])?;
        }
        written += 8 + size + (size & 1);
        // Le bext se place juste après le bloc de format, comme les enregistreurs.
        if &id == b"fmt " {
            if let Some(b) = bext.take() {
                written += write_chunk(&mut w, b"bext", &b)?;
            }
        }
    }
    if let Some(b) = bext {
        written += write_chunk(&mut w, b"bext", &b)?;
    }
    if let Some(x) = ixml {
        written += write_chunk(&mut w, b"iXML", x.as_bytes())?;
    }
    if written > u32::MAX as u64 {
        drop(w);
        let _ = fs::remove_file(&tmp);
        return Ok(false);
    }
    let mut f = w.into_inner().map_err(|e| e.into_error())?;
    f.seek(SeekFrom::Start(4))?;
    f.write_all(&(written as u32).to_le_bytes())?;
    f.sync_all()?;
    drop(f);
    drop(out_file);
    fs::rename(&tmp, output)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ixml_timestamp_follows_new_rate() {
        let xml = "<BWFXML><SPEED><FILE_SAMPLE_RATE>48000</FILE_SAMPLE_RATE><AUDIO_BIT_DEPTH>24</AUDIO_BIT_DEPTH><TIMESTAMP_SAMPLE_RATE>48000</TIMESTAMP_SAMPLE_RATE><TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_HI>0</TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_HI><TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_LO>1728000000</TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_LO></SPEED><SCENE>12A</SCENE></BWFXML>";
        let x = update_ixml(
            xml,
            &Retime {
                old_rate: 48_000,
                new_rate: 96_000,
                shift: 0.0,
            },
            16,
        );
        assert_eq!(tag_value(&x, "FILE_SAMPLE_RATE").as_deref(), Some("96000"));
        assert_eq!(tag_value(&x, "AUDIO_BIT_DEPTH").as_deref(), Some("16"));
        assert_eq!(
            tag_value(&x, "TIMESTAMP_SAMPLE_RATE").as_deref(),
            Some("96000")
        );
        // 1 728 000 000 × 2 = 3 456 000 000, encore sur 32 bits.
        assert_eq!(
            tag_value(&x, "TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_HI").as_deref(),
            Some("0")
        );
        assert_eq!(
            tag_value(&x, "TIMESTAMP_SAMPLES_SINCE_MIDNIGHT_LO").as_deref(),
            Some("3456000000")
        );
        assert_eq!(tag_value(&x, "SCENE").as_deref(), Some("12A"));
    }

    #[test]
    fn bext_time_reference_is_rescaled() {
        let mut bext = vec![0u8; 602];
        let samples: u64 = 5_000_000_000; // au-delà de 32 bits
        bext[338..342].copy_from_slice(&(samples as u32).to_le_bytes());
        bext[342..346].copy_from_slice(&((samples >> 32) as u32).to_le_bytes());
        update_bext(
            &mut bext,
            &Retime {
                old_rate: 48_000,
                new_rate: 44_100,
                shift: 2.0,
            },
        );
        let lo = u32::from_le_bytes(bext[338..342].try_into().unwrap()) as u64;
        let hi = u32::from_le_bytes(bext[342..346].try_into().unwrap()) as u64;
        // 5e9 × 44 100 / 48 000 = 4 593 750 000, plus 2 s à 44,1 kHz.
        assert_eq!((hi << 32) | lo, 4_593_750_000 + 88_200);
    }
}
