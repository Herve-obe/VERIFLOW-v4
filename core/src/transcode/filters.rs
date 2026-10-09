//! Filtres FFmpeg : chaîne d'image, incrustations et graphes son.
//!
//! Les fichiers utilisés par les filtres (LUT, police, textes, sous-titres)
//! sont copiés dans un dossier de travail sous des noms simples : les chemins
//! Windows (« C:\... ») n'ont ainsi jamais à être échappés dans un filtre.

use std::path::PathBuf;

use super::settings::{
    Deinterlace, FitMode, FpsMethod, ImageOptions, OverlayOptions, Position, Rotate,
    SubtitleOptions, TextStyle,
};
use crate::media::probe::MediaInfo;
use crate::player::timecode::{FrameRate, Timecode};
use crate::{Error, Result};

/// Police des incrustations (Inter, licence SIL OFL), embarquée.
const FONT: &[u8] = include_bytes!("../../../templates/fonts/Inter-SemiBold.ttf");

/// Fichier à placer dans le dossier de travail.
#[derive(Debug, Clone)]
pub enum Asset {
    File(PathBuf),
    Bytes(&'static [u8]),
    Text(String),
}

/// Taille et cadence de l'image à une étape de la chaîne.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dims {
    pub w: u32,
    pub h: u32,
    pub rate: FrameRate,
}

fn even(v: u32) -> u32 {
    (v / 2 * 2).max(2)
}

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

/// Lit un rapport d'image : « 16:9 », « 2.39:1 », « 1.85 ».
pub fn parse_aspect(text: &str) -> Option<f64> {
    let (a, b) = match text.split_once([':', '/']) {
        Some((a, b)) => (
            a.trim().replace(',', ".").parse::<f64>().ok()?,
            b.trim().replace(',', ".").parse::<f64>().ok()?,
        ),
        None => (text.trim().replace(',', ".").parse::<f64>().ok()?, 1.0),
    };
    (a > 0.0 && b > 0.0).then_some(a / b)
}

/// Lit une cadence : « 25 », « 23.976 », « 24000/1001 », « 29,97 ».
pub fn parse_rate(text: &str) -> Option<FrameRate> {
    let t = text.trim().replace(',', ".");
    if t.contains('/') {
        return FrameRate::parse(&t);
    }
    let v: f64 = t.parse().ok()?;
    // Cadences NTSC écrites en décimal.
    for (approx, r) in [
        (23.976, FrameRate::new(24000, 1001)),
        (29.97, FrameRate::new(30000, 1001)),
        (47.952, FrameRate::new(48000, 1001)),
        (59.94, FrameRate::new(60000, 1001)),
    ] {
        if (v - approx).abs() < 0.01 {
            return Some(r);
        }
    }
    if (v - v.round()).abs() < 1e-6 && v >= 1.0 {
        return Some(FrameRate::new(v.round() as u32, 1));
    }
    Some(FrameRate::new((v * 1000.0).round() as u32, 1000))
}

/// Chaîne d'image : désentrelacement, cadence, recadrage, rotation, taille,
/// LUT et couleurs. Renvoie les filtres et la taille finale.
pub fn image_chain(
    img: &ImageOptions,
    scale: &str,
    frame: Option<(u32, u32)>,
    fit: FitMode,
    src: Dims,
    assets: &mut Vec<(String, Asset)>,
) -> Result<(Vec<String>, Dims)> {
    let mut f = Vec::new();
    let mut d = src;
    match img.deinterlace {
        Some(Deinterlace::Yadif) => f.push(s("yadif=mode=0")),
        Some(Deinterlace::Bwdif) => f.push(s("bwdif=mode=0")),
        Some(Deinterlace::BwdifField) => {
            f.push(s("bwdif=mode=1"));
            d.rate = FrameRate::new(d.rate.num * 2, d.rate.den);
        }
        None => {}
    }
    if img.decimate {
        f.push(s("mpdecimate,setpts=N/FRAME_RATE/TB"));
    }
    if let Some(text) = img.fps.as_deref().filter(|t| !t.trim().is_empty()) {
        let r = parse_rate(text)
            .ok_or_else(|| Error::Unsupported(format!("cadence illisible : {text}")))?;
        let rs = format!("{}/{}", r.num, r.den);
        f.push(match img.fps_method {
            FpsMethod::Duplicate => format!("fps={rs}"),
            FpsMethod::Blend => format!("framerate=fps={rs}"),
            FpsMethod::Interpolate => {
                format!("minterpolate=fps={rs}:mi_mode=mci:mc_mode=aobmc:vsbmc=1")
            }
        });
        d.rate = r;
    }
    let c = &img.crop;
    if !c.is_empty() {
        let w = d.w.saturating_sub(c.left + c.right);
        let h = d.h.saturating_sub(c.top + c.bottom);
        if w < 16 || h < 16 {
            return Err(Error::Unsupported(
                "recadrage plus grand que l'image".into(),
            ));
        }
        f.push(format!("crop={w}:{h}:{}:{}", c.left, c.top));
        d.w = w;
        d.h = h;
    }
    if let Some(ratio) = img.aspect.as_deref().filter(|a| !a.trim().is_empty()) {
        let r = parse_aspect(ratio)
            .ok_or_else(|| Error::Unsupported(format!("rapport d'image illisible : {ratio}")))?;
        let (w, h) = if d.w as f64 / d.h as f64 > r {
            (even((d.h as f64 * r).round() as u32), even(d.h))
        } else {
            (even(d.w), even((d.w as f64 / r).round() as u32))
        };
        f.push(format!("crop={w}:{h}"));
        d.w = w;
        d.h = h;
    }
    match img.rotate {
        Rotate::None => {}
        Rotate::Cw90 | Rotate::Ccw90 => {
            f.push(s(if img.rotate == Rotate::Cw90 {
                "transpose=1"
            } else {
                "transpose=2"
            }));
            std::mem::swap(&mut d.w, &mut d.h);
        }
        Rotate::Half => f.push(s("hflip,vflip")),
        Rotate::FlipH => f.push(s("hflip")),
        Rotate::FlipV => f.push(s("vflip")),
    }
    if let Some((w, h)) = img.frame.or(frame) {
        f.push(match fit {
            FitMode::Pad => format!(
                "scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2,setsar=1"
            ),
            FitMode::Crop => format!(
                "scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,crop={w}:{h},setsar=1"
            ),
        });
        d.w = w;
        d.h = h;
    } else {
        match scale {
            "" | "source" => {}
            "1/2" | "1/4" => {
                let k = if scale == "1/2" { 2 } else { 4 };
                d.w = even(d.w / k);
                d.h = even(d.h / k);
                f.push(format!("scale={}:{}:flags=lanczos", d.w, d.h));
            }
            h => {
                if let Ok(h) = h.parse::<u32>() {
                    let w = even((d.w as f64 * h as f64 / d.h as f64).round() as u32);
                    f.push(format!("scale={w}:{h}:flags=lanczos"));
                    d.w = w;
                    d.h = h;
                }
            }
        }
    }
    if let Some(lut) = &img.lut {
        let ext = lut
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| s("cube"));
        if !lut.is_file() {
            return Err(Error::NotFound(lut.display().to_string()));
        }
        let name = format!("lut.{ext}");
        assets.push((name.clone(), Asset::File(lut.clone())));
        f.push(format!("lut3d=file={name}"));
    }
    if img.has_color() {
        f.push(format!(
            "eq=brightness={:.3}:contrast={:.3}:saturation={:.3}:gamma={:.3}",
            img.brightness.clamp(-1.0, 1.0),
            img.contrast.clamp(0.0, 3.0),
            img.saturation.clamp(0.0, 3.0),
            img.gamma.clamp(0.1, 10.0)
        ));
    }
    Ok((f, d))
}

/// Coordonnées d'un élément de taille (`ew`, `eh`) dans l'image (`fw`, `fh`),
/// marge comprise. `stack` décale vers l'intérieur les éléments empilés.
fn place(
    pos: Position,
    ew: &str,
    eh: &str,
    fw: &str,
    fh: &str,
    margin: u32,
    stack: u32,
) -> (String, String) {
    let left = format!("{margin}");
    let center_x = format!("({fw}-{ew})/2");
    let right = format!("{fw}-{ew}-{margin}");
    let top = format!("{}", margin + stack);
    let middle = format!("({fh}-{eh})/2");
    let bottom = format!("{fh}-{eh}-{}", margin + stack);
    match pos {
        Position::TopLeft => (left, top),
        Position::Top => (center_x, top),
        Position::TopRight => (right, top),
        Position::Center => (center_x, middle),
        Position::BottomLeft => (left, bottom),
        Position::Bottom => (center_x, bottom),
        Position::BottomRight => (right, bottom),
    }
}

/// Logo en surimpression : entrée, préparation et superposition.
pub struct Logo {
    pub path: PathBuf,
    /// Filtres appliqués au logo (taille, opacité).
    pub prepare: String,
    /// Paramètres du filtre `overlay`.
    pub overlay: String,
}

/// Incrustations : timecode, nom du fichier, texte, sous-titres, logo.
pub fn overlay_filters(
    ov: &OverlayOptions,
    subs: &SubtitleOptions,
    dims: Dims,
    start_tc: Option<Timecode>,
    file_name: &str,
    assets: &mut Vec<(String, Asset)>,
) -> Result<(Vec<String>, Option<Logo>)> {
    let mut f = Vec::new();
    let margin = (dims.h as f64 * 0.04).round() as u32;
    let mut font = false;
    let mut stacks: Vec<(Position, u32)> = Vec::new();
    let mut text_box = |style: &TextStyle,
                        content: String,
                        f: &mut Vec<String>,
                        font: &mut bool| {
        *font = true;
        let px = ((dims.h as f64 * style.size.clamp(1.0, 30.0) / 100.0).round() as u32).max(8);
        let op = style.opacity.clamp(0.05, 1.0);
        let stack = stacks
            .iter()
            .filter(|(p, _)| *p == style.position)
            .map(|(_, h)| h)
            .sum::<u32>();
        stacks.push((style.position, px * 3 / 2 + px / 2));
        let (x, y) = place(style.position, "tw", "th", "w", "h", margin, stack);
        let mut d = format!("drawtext=fontfile=font.ttf:{content}:fontsize={px}:fontcolor=white@{op:.2}:x={x}:y={y}");
        if style.background {
            d += &format!(
                ":box=1:boxcolor=black@{:.2}:boxborderw={}",
                0.55 * op,
                (px / 4).max(2)
            );
        }
        f.push(d);
    };
    if let Some(style) = &ov.timecode {
        let tc = start_tc
            .map(|t| t.offset(ov.tc_offset))
            .map(|t| t.to_string());
        let tc = tc.unwrap_or_else(|| s("00:00:00:00"));
        // « . » marque le drop-frame pour FFmpeg, sans échappement de « ; ».
        let tc = tc.replace(';', ".").replace(':', "\\:");
        let r = dims.rate;
        text_box(
            style,
            format!("timecode='{tc}':rate={}/{}", r.num, r.den),
            &mut f,
            &mut font,
        );
    }
    let mut n = 0;
    let mut text_asset = |content: &str, assets: &mut Vec<(String, Asset)>| {
        n += 1;
        let name = format!("text{n}.txt");
        assets.push((name.clone(), Asset::Text(content.to_owned())));
        format!("textfile={name}:expansion=none")
    };
    if let Some(style) = &ov.filename {
        let t = text_asset(file_name, assets);
        text_box(style, t, &mut f, &mut font);
    }
    if let Some(style) = ov
        .text
        .as_ref()
        .filter(|_| !ov.text_value.trim().is_empty())
    {
        let t = text_asset(&ov.text_value, assets);
        text_box(style, t, &mut f, &mut font);
    }
    if let (Some(file), true) = (&subs.file, subs.burn) {
        if !file.is_file() {
            return Err(Error::NotFound(file.display().to_string()));
        }
        let ext = file
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_else(|| s("srt"));
        let name = format!("subs.{ext}");
        assets.push((name.clone(), Asset::File(file.clone())));
        font = true;
        let mut style = s("FontName=Inter");
        if subs.size > 0 {
            style += &format!(",FontSize={}", subs.size);
        }
        // Sous-titres avant les autres incrustations : le timecode reste lisible.
        f.insert(
            0,
            format!("subtitles=filename={name}:fontsdir=.:force_style='{style}'"),
        );
    }
    if font {
        assets.push((s("font.ttf"), Asset::Bytes(FONT)));
    }
    let logo = match &ov.logo {
        Some(l) if !l.path.as_os_str().is_empty() => {
            if !l.path.is_file() {
                return Err(Error::NotFound(l.path.display().to_string()));
            }
            let px = ((dims.h as f64 * l.size.clamp(1.0, 100.0) / 100.0).round() as u32).max(8);
            let op = l.opacity.clamp(0.05, 1.0);
            let (x, y) = place(l.position, "w", "h", "W", "H", margin, 0);
            Some(Logo {
                path: l.path.clone(),
                prepare: format!("scale=-2:{px},format=rgba,colorchannelmixer=aa={op:.2}"),
                overlay: format!("x={x}:y={y}:eof_action=repeat:format=auto"),
            })
        }
        _ => None,
    };
    Ok((f, logo))
}

/// Total des canaux des pistes son.
pub fn total_channels(info: &MediaInfo) -> u32 {
    info.audio.iter().map(|a| a.channels.max(1)).sum()
}

/// Entrée son : la piste, ou les pistes réunies (« amerge ») en une seule.
fn merged(input: usize, streams: usize) -> (String, Option<String>) {
    if streams <= 1 {
        (format!("[{input}:a:0]"), None)
    } else {
        let labels: String = (0..streams).map(|i| format!("[{input}:a:{i}]")).collect();
        (labels, Some(format!("amerge=inputs={streams}")))
    }
}

/// Pistes 1 et 2 en stéréo (piste mono dupliquée).
pub fn pan_first_two(channels: u32) -> String {
    if channels >= 2 {
        s("pan=stereo|c0=c0|c1=c1")
    } else {
        s("pan=stereo|c0=c0|c1=c0")
    }
}

/// Mix stéréo : pistes impaires à gauche, paires à droite, niveaux normalisés.
pub fn pan_mix(channels: u32) -> String {
    if channels < 2 {
        return pan_first_two(channels);
    }
    let left: Vec<String> = (0..channels).step_by(2).map(|c| format!("c{c}")).collect();
    let right: Vec<String> = (1..channels).step_by(2).map(|c| format!("c{c}")).collect();
    format!("pan=stereo|c0<{}|c1<{}", left.join("+"), right.join("+"))
}

/// Graphe son : pistes de l'entrée `input` réunies, `pan` éventuel, filtres,
/// sortie étiquetée `label`.
pub fn audio_chain(
    input: usize,
    info: &MediaInfo,
    pan: Option<String>,
    filters: &[String],
    label: &str,
) -> String {
    let (inputs, merge) = merged(input, info.audio.len());
    let mut chain: Vec<String> = merge.into_iter().collect();
    chain.extend(pan);
    chain.extend(filters.iter().cloned());
    if chain.is_empty() {
        chain.push(s("anull"));
    }
    format!("{inputs}{}[{label}]", chain.join(","))
}

/// Graphe son des formats son : deux premières pistes si le format n'accepte
/// pas autant de canaux. Sortie `[aout]`.
pub fn audio_graph(info: &MediaInfo, max_channels: Option<u32>, filters: &[String]) -> String {
    let channels = total_channels(info);
    let pan = match max_channels {
        // Pistes 1 et 2 : le mix gauche/droite des enregistreurs de tournage.
        Some(max) if channels > max && channels >= 2 => Some(s("pan=stereo|c0=c0|c1=c1")),
        _ => None,
    };
    audio_chain(0, info, pan, filters, "aout")
}

/// Une piste mono par canal (MXF) : `[a0]`, `[a1]`...
pub fn mono_tracks(
    input: usize,
    info: &MediaInfo,
    pan: Option<String>,
    filters: &[String],
    channels: u32,
) -> (String, Vec<String>) {
    if channels <= 1 {
        return (
            audio_chain(input, info, pan, filters, "a0"),
            vec![s("[a0]")],
        );
    }
    let split_labels: String = (0..channels).map(|i| format!("[s{i}]")).collect();
    let mut graph = audio_chain(input, info, pan, filters, "mix");
    graph += &format!(";[mix]asplit={channels}{split_labels}");
    let mut outs = Vec::new();
    for i in 0..channels {
        graph += &format!(";[s{i}]pan=mono|c0=c{i}[a{i}]");
        outs.push(format!("[a{i}]"));
    }
    (graph, outs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transcode::settings::Crop;

    fn hd() -> Dims {
        Dims {
            w: 1920,
            h: 1080,
            rate: FrameRate::new(25, 1),
        }
    }

    #[test]
    fn reads_rates_and_ratios() {
        assert_eq!(parse_rate("23.976"), Some(FrameRate::new(24000, 1001)));
        assert_eq!(parse_rate("29,97"), Some(FrameRate::new(30000, 1001)));
        assert_eq!(parse_rate("25"), Some(FrameRate::new(25, 1)));
        assert_eq!(parse_rate("24000/1001"), Some(FrameRate::new(24000, 1001)));
        assert!((parse_aspect("2.39:1").unwrap() - 2.39).abs() < 1e-9);
        assert!((parse_aspect("9:16").unwrap() - 0.5625).abs() < 1e-9);
    }

    #[test]
    fn image_chain_tracks_dimensions() {
        let mut assets = Vec::new();
        let img = ImageOptions {
            crop: Crop {
                top: 140,
                bottom: 140,
                left: 0,
                right: 0,
            },
            rotate: Rotate::Cw90,
            ..Default::default()
        };
        let (f, d) = image_chain(&img, "1/2", None, FitMode::Pad, hd(), &mut assets).unwrap();
        assert_eq!(f[0], "crop=1920:800:0:140");
        assert_eq!(f[1], "transpose=1");
        assert_eq!((d.w, d.h), (400, 960));
        // Vertical 9:16 par recadrage centré.
        let (f, d) = image_chain(
            &ImageOptions::default(),
            "source",
            Some((1080, 1920)),
            FitMode::Crop,
            hd(),
            &mut assets,
        )
        .unwrap();
        assert!(f[0].contains("force_original_aspect_ratio=increase"));
        assert_eq!((d.w, d.h), (1080, 1920));
        // Rapport 2.39:1 sur une image 16:9 : hauteur réduite.
        let img = ImageOptions {
            aspect: Some("2.39:1".into()),
            ..Default::default()
        };
        let (_, d) = image_chain(&img, "source", None, FitMode::Pad, hd(), &mut assets).unwrap();
        assert_eq!((d.w, d.h), (1920, 802));
    }

    #[test]
    fn pans_for_stereo() {
        assert_eq!(pan_mix(4), "pan=stereo|c0<c0+c2|c1<c1+c3");
        assert_eq!(pan_mix(3), "pan=stereo|c0<c0+c2|c1<c1");
        assert_eq!(pan_first_two(1), "pan=stereo|c0=c0|c1=c0");
    }

    #[test]
    fn overlays_stack_and_use_assets() {
        let mut assets = Vec::new();
        let ov = OverlayOptions {
            timecode: Some(TextStyle::default()),
            filename: Some(TextStyle::default()),
            ..Default::default()
        };
        let tc = Timecode::parse("10:00:00:00", FrameRate::new(25, 1)).ok();
        let (f, logo) = overlay_filters(
            &ov,
            &SubtitleOptions::default(),
            hd(),
            tc,
            "A001C001.mov",
            &mut assets,
        )
        .unwrap();
        assert!(logo.is_none());
        assert!(
            f[0].contains("timecode='10\\:00\\:00\\:00':rate=25/1"),
            "{}",
            f[0]
        );
        assert!(f[1].contains("textfile=text1.txt:expansion=none"));
        // Deux éléments en bas : le second est remonté.
        assert!(
            f[0].contains("y=h-th-43") && !f[1].contains("y=h-th-43"),
            "{f:?}"
        );
        assert!(assets.iter().any(|(n, _)| n == "font.ttf"));
        assert!(assets
            .iter()
            .any(|(n, a)| n == "text1.txt" && matches!(a, Asset::Text(t) if t == "A001C001.mov")));
    }
}
