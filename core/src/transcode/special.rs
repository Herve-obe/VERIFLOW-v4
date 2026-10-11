//! Traitements sans réencodage qui ne suivent pas le modèle « un fichier en
//! entrée, un fichier en sortie » : fusion, insert, une piste par fichier.

use std::path::{Path, PathBuf};

use super::build::{seconds_at, source_depth, tc_after, Plan};
use super::filters::{audio_chain, total_channels, Asset};
use super::settings::BitDepth;
use crate::media::probe::MediaInfo;
use crate::{Error, Result};

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

/// Ligne d'une liste du démultiplexeur « concat ».
fn concat_line(path: &Path) -> String {
    let p = path
        .to_string_lossy()
        .replace('\\', "/")
        .replace('\'', "'\\''");
    format!("file '{p}'\n")
}

/// Mêmes codecs et taille d'image : condition d'un assemblage sans réencodage.
fn compatible(a: &MediaInfo, b: &MediaInfo) -> Result<()> {
    let key = |i: &MediaInfo| {
        i.video.as_ref().map(|v| {
            (
                v.codec.clone(),
                v.width,
                v.height,
                v.rate,
                v.pix_fmt.clone(),
            )
        })
    };
    let audio = |i: &MediaInfo| {
        i.audio
            .iter()
            .map(|x| (x.codec.clone(), x.sample_rate, x.channels))
            .collect::<Vec<_>>()
    };
    if key(a) != key(b) || audio(a) != audio(b) {
        return Err(Error::Unsupported(format!(
            "{} et {} : codecs, taille, cadence ou pistes son différents (assemblage sans réencodage impossible)",
            a.path, b.path
        )));
    }
    Ok(())
}

fn tc_args(info: &MediaInfo, format: &str) -> Vec<String> {
    match (tc_after(info, 0.0), matches!(format, "mov" | "mp4" | "mxf")) {
        (Some(tc), true) => vec![s("-timecode"), tc.to_string()],
        _ => Vec::new(),
    }
}

/// Fichiers mis bout à bout, dans l'ordre de la liste.
pub fn merge(sources: &[(PathBuf, MediaInfo)], format: &str) -> Result<Plan> {
    let (_, first) = sources
        .first()
        .ok_or_else(|| Error::Unsupported("aucun fichier".into()))?;
    let mut list = String::new();
    for (path, info) in sources {
        compatible(first, info)?;
        list += &concat_line(path);
    }
    let mut args = vec![
        s("-map"),
        s("0:v?"),
        s("-map"),
        s("0:a?"),
        s("-c"),
        s("copy"),
    ];
    args.extend(tc_args(first, format));
    Ok(Plan {
        input: Some(PathBuf::from("list.txt")),
        input_args: vec![s("-f"), s("concat"), s("-safe"), s("0")],
        args,
        format: s(format),
        assets: vec![(s("list.txt"), Asset::Text(list))],
        duration: sources.iter().map(|(_, i)| i.duration).sum(),
        ..Default::default()
    })
}

/// Passage remplacé par un autre plan, à partir du point d'insertion.
/// Coupes exactes avec les codecs intra-image (ProRes, DNx...) ; avec les
/// codecs à groupe d'images (H.264...), les coupes tombent sur les images clés.
pub fn insert(
    source: &Path,
    info: &MediaInfo,
    insert: &Path,
    insert_info: &MediaInfo,
    at: &str,
    format: &str,
) -> Result<Plan> {
    compatible(info, insert_info)?;
    let start = seconds_at(at, info)?;
    let end = start + insert_info.duration;
    if end > info.duration + 0.001 {
        return Err(Error::Unsupported(
            "l'insert dépasse la fin du fichier".into(),
        ));
    }
    let mut list = concat_line(source);
    list += &format!("outpoint {start:.6}\n");
    list += &concat_line(insert);
    if end < info.duration - 0.001 {
        list += &concat_line(source);
        list += &format!("inpoint {end:.6}\n");
    }
    let mut args = vec![
        s("-map"),
        s("0:v?"),
        s("-map"),
        s("0:a?"),
        s("-c"),
        s("copy"),
    ];
    args.extend(tc_args(info, format));
    Ok(Plan {
        input: Some(PathBuf::from("list.txt")),
        input_args: vec![s("-f"), s("concat"), s("-safe"), s("0")],
        args,
        format: s(format),
        assets: vec![(s("list.txt"), Asset::Text(list))],
        duration: info.duration,
        ..Default::default()
    })
}

/// Une commande par canal : WAV mono, nommé d'après le numéro et le nom de
/// la piste (iXML des WAV d'enregistreur).
pub fn tracks(
    source: &Path,
    info: &MediaInfo,
    input_args: &[String],
    start: f64,
    duration: f64,
) -> Result<Vec<(String, Plan)>> {
    if info.audio.is_empty() {
        return Err(Error::Unsupported(format!("{} : pas de son", info.path)));
    }
    let names = crate::media::wav::read_info(source).ok();
    let depth = source_depth(info).unwrap_or(BitDepth::S24);
    let codec = match depth {
        BitDepth::S16 => "pcm_s16le",
        BitDepth::S24 => "pcm_s24le",
        BitDepth::F32 => "pcm_f32le",
    };
    let rate = info.audio[0].sample_rate;
    let mut out = Vec::new();
    for c in 0..total_channels(info) {
        let mut suffix = format!("_A{:02}", c + 1);
        let name = names
            .as_ref()
            .and_then(|w| w.ixml.track_names.get(c as usize).cloned().flatten())
            .filter(|n| !n.trim().is_empty());
        if let Some(n) = name {
            let clean: String = n
                .chars()
                .map(|ch| {
                    if ch.is_alphanumeric() || ch == '-' {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect();
            suffix += &format!("_{}", clean.trim_matches('_'));
        }
        let graph = audio_chain(0, info, Some(format!("pan=mono|c0=c{c}")), &[], "aout");
        let mut args = vec![
            s("-map_metadata"),
            s("0"),
            s("-filter_complex"),
            graph,
            s("-map"),
            s("[aout]"),
        ];
        args.extend([
            s("-c:a"),
            s(codec),
            s("-rf64"),
            s("auto"),
            s("-write_bext"),
            s("1"),
        ]);
        if let Some(tc) = tc_after(info, start) {
            let r = tc.rate;
            let samples = (tc.frames.max(0) as u128 * r.den as u128 * rate as u128) / r.num as u128;
            args.extend([s("-metadata"), format!("time_reference={samples}")]);
        }
        out.push((
            suffix,
            Plan {
                input_args: input_args.to_vec(),
                args,
                format: s("wav"),
                duration,
                start,
                ..Default::default()
            },
        ));
    }
    Ok(out)
}
