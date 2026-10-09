//! Construction de la commande FFmpeg d'un fichier : entrées, filtres,
//! encodeurs et conteneur, pour chaque famille de préréglages.

use std::path::{Path, PathBuf};

use super::catalog::{format_for_ext, Kind, Preset};
use super::filters::{
    audio_chain, audio_graph, image_chain, mono_tracks, overlay_filters, pan_first_two, pan_mix,
    parse_rate, total_channels, Asset, Dims,
};
use super::settings::{AudioMode, BitDepth, Settings};
use super::video::{
    self, codec_args, default_kbps, video_audio, video_encoder, EncoderChoice, Frame,
};
use crate::media::probe::MediaInfo;
use crate::player::timecode::{FrameRate, Timecode};
use crate::{Error, Result};

fn s<T: ToString>(v: T) -> String {
    v.to_string()
}

/// Entrée supplémentaire (logo, son de remplacement, sous-titres).
#[derive(Debug, Clone)]
pub struct Input {
    pub args: Vec<String>,
    /// Chemin absolu, ou nom d'un fichier du dossier de travail.
    pub path: PathBuf,
}

/// Commande d'un fichier.
#[derive(Debug, Default)]
pub struct Plan {
    /// Entrée à la place de la source (liste « concat » du dossier de travail).
    pub input: Option<PathBuf>,
    /// Options placées avant `-i source` (points d'entrée et de sortie...).
    pub input_args: Vec<String>,
    pub extra_inputs: Vec<Input>,
    pub args: Vec<String>,
    pub format: String,
    pub encoder: Option<EncoderChoice>,
    /// Fichiers à placer dans le dossier de travail.
    pub assets: Vec<(String, Asset)>,
    /// Durée attendue du fichier produit (avancement).
    pub duration: f64,
    /// Séquence d'images : la sortie est un dossier.
    pub sequence: bool,
    /// Décalage du début par rapport à la source (secondes), pour le BWF.
    pub start: f64,
}

/// Ce qui est connu d'un fichier au moment de construire sa commande.
pub struct Job<'a> {
    pub preset: &'a Preset,
    pub settings: &'a Settings,
    pub info: &'a MediaInfo,
    pub source: &'a Path,
    /// Gain de normalisation déjà calculé.
    pub gain: Option<f64>,
}

impl Job<'_> {
    /// Format FFmpeg de sortie (celui de la source pour les préréglages « * »).
    pub fn format(&self) -> String {
        if self.preset.format.is_empty() {
            s(format_for_ext(&source_ext(self.source)))
        } else {
            s(self.preset.format)
        }
    }
}

pub fn source_ext(source: &Path) -> String {
    source
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_else(|| s("mov"))
}

fn rate_of(info: &MediaInfo) -> FrameRate {
    info.video
        .as_ref()
        .map(|v| v.rate)
        .unwrap_or(FrameRate::new(25, 1))
}

/// Position dans la source, en secondes depuis le début du fichier : timecode
/// (« 10:00:12:05 », comparé au timecode de début) ou secondes (« 12.5 »).
pub fn seconds_at(spec: &str, info: &MediaInfo) -> Result<f64> {
    let t = spec.trim();
    if t.contains(':') || t.contains(';') {
        let rate = rate_of(info);
        let tc = Timecode::parse(t, rate)
            .map_err(|_| Error::Unsupported(format!("timecode illisible : {t}")))?;
        let start = info.start_tc().map(|s| s.frames).unwrap_or(0);
        let frames = tc.frames - start;
        if frames < 0 {
            return Err(Error::Unsupported(format!(
                "{t} : avant le début du fichier"
            )));
        }
        Ok(frames as f64 * rate.den as f64 / rate.num as f64)
    } else {
        t.replace(',', ".")
            .parse::<f64>()
            .map_err(|_| Error::Unsupported(format!("position illisible : {t}")))
    }
}

/// Timecode de la source décalé de `seconds`.
pub fn tc_after(info: &MediaInfo, seconds: f64) -> Option<Timecode> {
    let tc = info.start_tc()?;
    let frames = (seconds * tc.rate.as_f64()).round() as i64;
    Some(tc.offset(frames))
}

/// Points d'entrée et de sortie : options d'entrée, début et durée.
pub(crate) fn trim(job: &Job) -> Result<(Vec<String>, f64, f64)> {
    let total = job.info.duration;
    let Some(t) = job.settings.trim_for(job.source) else {
        return Ok((Vec::new(), 0.0, total));
    };
    let start = match t.start.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(v) => seconds_at(v, job.info)?,
        None => 0.0,
    };
    let end = match t.end.as_deref().filter(|v| !v.trim().is_empty()) {
        Some(v) => seconds_at(v, job.info)?,
        None => total,
    };
    if end <= start {
        return Err(Error::Unsupported(
            "point de sortie avant le point d'entrée".into(),
        ));
    }
    let mut args = Vec::new();
    if start > 0.0 {
        args.extend([s("-ss"), format!("{start:.3}")]);
    }
    if end < total {
        args.extend([s("-to"), format!("{end:.3}")]);
    }
    Ok((args, start, end.min(total) - start))
}

/// Timecode de début du fichier produit.
fn timecode_args(info: &MediaInfo, start: f64, format: &str) -> Vec<String> {
    if !matches!(format, "mov" | "mp4" | "mxf") {
        return Vec::new();
    }
    match tc_after(info, start) {
        Some(tc) => vec![s("-timecode"), tc.to_string()],
        None => Vec::new(),
    }
}

/// Référence temporelle BWF (échantillons depuis minuit) tirée du timecode
/// de la source : le son extrait d'une vidéo garde sa position pour la synchro.
fn time_reference_args(info: &MediaInfo, start: f64, rate: u32) -> Vec<String> {
    let Some(tc) = tc_after(info, start) else {
        return Vec::new();
    };
    let r = tc.rate;
    let samples = (tc.frames.max(0) as u128 * r.den as u128 * rate as u128) / r.num as u128;
    vec![s("-metadata"), format!("time_reference={samples}")]
}

fn color_args(tags: Option<&str>) -> Vec<String> {
    let (space, prim, trc) = match tags {
        Some("bt709") => ("bt709", "bt709", "bt709"),
        Some("bt2020") => ("bt2020nc", "bt2020", "bt2020-10"),
        Some("bt601") => ("smpte170m", "smpte170m", "smpte170m"),
        _ => return Vec::new(),
    };
    vec![
        s("-colorspace"),
        s(space),
        s("-color_primaries"),
        s(prim),
        s("-color_trc"),
        s(trc),
    ]
}

/// Résolution du son source, d'après le codec et les bits annoncés.
pub fn source_depth(info: &MediaInfo) -> Option<BitDepth> {
    let a = info.audio.first()?;
    if a.codec.starts_with("pcm_f") {
        return Some(BitDepth::F32);
    }
    match a.bits {
        Some(b) if b <= 16 => Some(BitDepth::S16),
        Some(b) if b <= 24 => Some(BitDepth::S24),
        Some(_) => Some(BitDepth::F32),
        None => None,
    }
}

/// Résolution de sortie : choix de l'utilisateur, sinon celle de la source,
/// ramenée à ce que le format accepte (24 bits par défaut).
pub fn output_depth(preset: &Preset, settings: &Settings, info: &MediaInfo) -> BitDepth {
    let allowed = preset.bit_depths();
    let wanted = settings
        .bit_depth
        .or_else(|| source_depth(info))
        .unwrap_or(BitDepth::S24);
    if allowed.contains(&wanted) {
        wanted
    } else {
        BitDepth::S24
    }
}

/// Fréquence de sortie, ramenée à celles que le format accepte.
pub fn output_rate(preset: &Preset, settings: &Settings, info: &MediaInfo) -> u32 {
    let src = info
        .audio
        .first()
        .map(|a| a.sample_rate)
        .filter(|r| *r > 0)
        .unwrap_or(48_000);
    let rate = settings.sample_rate.unwrap_or(src);
    match preset.kind {
        Kind::Opus => 48_000,
        Kind::Mp3 | Kind::Ac3 if ![32_000, 44_100, 48_000].contains(&rate) => {
            if rate.is_multiple_of(44_100) {
                44_100
            } else {
                48_000
            }
        }
        _ => rate,
    }
}

/// Canaux maximum acceptés par un format son avec perte (5.1 gardé tel quel).
pub fn max_channels(kind: Kind, channels: u32) -> Option<u32> {
    match kind {
        Kind::Mp3 => Some(2),
        Kind::Aac | Kind::Opus | Kind::Vorbis | Kind::Ac3 if channels != 6 => Some(2),
        _ => None,
    }
}

/// Rééchantillonnage de qualité, avec dither si la résolution baisse.
fn resample_filter(rate: u32) -> String {
    format!(
        "aresample={rate}:filter_size=64:phase_shift=10:cutoff=0.97:dither_method=triangular_hp"
    )
}

fn pcm_codec(aiff: bool, depth: BitDepth) -> &'static str {
    match (aiff, depth) {
        (true, BitDepth::S16) => "pcm_s16be",
        (true, BitDepth::S24) => "pcm_s24be",
        (true, BitDepth::F32) => "pcm_f32be",
        (false, BitDepth::S16) => "pcm_s16le",
        (false, BitDepth::S24) => "pcm_s24le",
        (false, BitDepth::F32) => "pcm_f32le",
    }
}

fn no_audio(info: &MediaInfo) -> Error {
    Error::Unsupported(format!("{} : pas de son", info.path))
}

fn no_video(info: &MediaInfo) -> Error {
    Error::Unsupported(format!("{} : pas d'image", info.path))
}

/// Construit la commande d'un fichier.
pub fn build(job: &Job) -> Result<Plan> {
    let kind = job.preset.kind;
    if job.preset.is_video() || job.preset.is_image() {
        build_video(job)
    } else if job.preset.is_audio() {
        build_audio(job)
    } else {
        match kind {
            Kind::Rewrap | Kind::Cut | Kind::ExtractVideo => build_copy(job),
            Kind::ExtractAudio => build_extract_audio(job),
            Kind::Subtitles => build_subtitles(job),
            Kind::ReplaceAudio => build_replace_audio(job),
            Kind::Conform => build_conform(job),
            Kind::FrameMd5 => {
                let (input_args, start, duration) = trim(job)?;
                Ok(Plan {
                    input_args,
                    args: vec![s("-map"), s("0:v?"), s("-map"), s("0:a?")],
                    format: s("framemd5"),
                    duration,
                    start,
                    ..Default::default()
                })
            }
            _ => Err(Error::Unsupported(format!(
                "{} : traitement particulier",
                job.preset.id
            ))),
        }
    }
}

/// Son des fichiers vidéo : arguments et éventuel graphe, selon le mode choisi.
fn video_sound(job: &Job, format: &str, graph: &mut Vec<String>) -> Vec<String> {
    let info = job.info;
    if info.audio.is_empty() {
        return Vec::new();
    }
    let spec = video_audio(job.preset, format);
    let channels = total_channels(info);
    let mut mode = job.settings.audio_mode.unwrap_or(job.preset.audio_mode);
    if spec.max_channels == Some(2)
        && mode == AudioMode::Keep
        && (info.audio.len() > 1 || channels > 2)
    {
        mode = AudioMode::FirstTwo;
    }
    let mut filters = Vec::new();
    if let Some(g) = job.gain {
        filters.push(format!("volume={g:.2}dB"));
    }
    if let Some(r) = spec.rate {
        if info.audio.iter().any(|a| a.sample_rate != r) {
            filters.push(format!("aresample={r}"));
        }
    }
    let pan = match mode {
        AudioMode::None => return vec![s("-an")],
        AudioMode::Keep => None,
        AudioMode::FirstTwo => Some(pan_first_two(channels)),
        AudioMode::Mix => Some(pan_mix(channels)),
    };
    let mut a = Vec::new();
    if spec.mono_tracks {
        let n = if pan.is_some() { 2 } else { channels };
        let (g, outs) = mono_tracks(0, info, pan, &filters, n);
        graph.push(g);
        for o in outs {
            a.extend([s("-map"), o]);
        }
    } else if pan.is_some() {
        graph.push(audio_chain(0, info, pan, &filters, "aout"));
        a.extend([s("-map"), s("[aout]")]);
    } else if filters.is_empty() {
        a.extend([s("-map"), s("0:a?")]);
    } else {
        // Chaque piste gardée, même gain pour toutes.
        for i in 0..info.audio.len() {
            graph.push(format!("[0:a:{i}]{}[a{i}]", filters.join(",")));
            a.extend([s("-map"), format!("[a{i}]")]);
        }
    }
    a.extend(spec.args);
    a
}

/// Vidéo réencodée, ou images fixes.
fn build_video(job: &Job) -> Result<Plan> {
    let (preset, settings, info) = (job.preset, job.settings, job.info);
    let v = info.video.as_ref().ok_or_else(|| no_video(info))?;
    let format = job.format();
    let (input_args, start, duration) = trim(job)?;
    let mut plan = Plan {
        input_args,
        format: format.clone(),
        duration,
        start,
        ..Default::default()
    };
    let src = Dims {
        w: v.width,
        h: v.height,
        rate: v.rate,
    };
    // Tailles imposées par les formats broadcast.
    let mut frame = preset.frame;
    let mut forced_rate = None;
    match preset.kind {
        Kind::XavcIntra => {
            frame = Some(if v.height >= 2160 {
                (3840, 2160)
            } else {
                (1920, 1080)
            })
        }
        Kind::Dv => {
            let ntsc = v.rate.is_ntsc() || v.rate.nominal() == 30;
            frame = Some(if ntsc { (720, 480) } else { (720, 576) });
            forced_rate = Some(if ntsc {
                FrameRate::new(30000, 1001)
            } else {
                FrameRate::new(25, 1)
            });
        }
        _ => {}
    }
    let scale = settings.scale.as_deref().unwrap_or(preset.scale);
    let (mut filters, mut dims) = image_chain(
        &settings.image,
        scale,
        frame,
        preset.fit,
        src,
        &mut plan.assets,
    )?;
    if let Some(r) = forced_rate {
        if r != dims.rate {
            filters.push(format!("fps={}/{}", r.num, r.den));
            dims.rate = r;
        }
    }
    if matches!(preset.kind, Kind::Hap(_)) && (dims.w % 4 != 0 || dims.h % 4 != 0) {
        dims.w = dims.w / 4 * 4;
        dims.h = dims.h / 4 * 4;
        filters.push(format!("scale={}:{}", dims.w, dims.h));
    } else if dims.w % 2 == 1 || dims.h % 2 == 1 {
        dims.w = dims.w / 2 * 2;
        dims.h = dims.h / 2 * 2;
        filters.push(format!("scale={}:{}", dims.w, dims.h));
    }
    if matches!(preset.kind, Kind::Dv) {
        filters.push(s("setdar=16/9"));
    }
    let name = job
        .source
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let (overlays, logo) = overlay_filters(
        &settings.overlay,
        &settings.subtitles,
        dims,
        tc_after(info, start),
        &name,
        &mut plan.assets,
    )?;
    filters.extend(overlays);

    let mut graph: Vec<String> = Vec::new();
    let mut a: Vec<String> = vec![s("-map_metadata"), s("0")];
    // Image : chaîne simple, ou graphe avec logo.
    if let Some(l) = logo {
        let idx = 1 + plan.extra_inputs.len();
        plan.extra_inputs.push(Input {
            args: vec![s("-loop"), s("1")],
            path: l.path.clone(),
        });
        let base = if filters.is_empty() {
            s("null")
        } else {
            filters.join(",")
        };
        graph.push(format!("[0:v:0]{base}[vmain]"));
        graph.push(format!("[{idx}:v]{}[logo]", l.prepare));
        graph.push(format!(
            "[vmain][logo]overlay={}:shortest=1[vout]",
            l.overlay
        ));
        a.extend([s("-map"), s("[vout]")]);
    } else if !filters.is_empty() {
        graph.push(format!("[0:v:0]{}[vout]", filters.join(",")));
        a.extend([s("-map"), s("[vout]")]);
    } else {
        a.extend([s("-map"), s("0:v:0")]);
    }

    let ten_bit = ["10", "12", "16"].iter().any(|b| v.pix_fmt.contains(b));
    let encoder = video_encoder(preset, settings.software).ok_or_else(|| {
        Error::Unsupported("aucun encodeur disponible pour ce format sur ce poste".into())
    })?;
    let kbps = settings
        .video_mbps
        .filter(|m| *m > 0.0)
        .or_else(|| {
            preset
                .mbps
                .map(|(lo, hi)| if dims.rate.as_f64() > 31.0 { hi } else { lo })
        })
        .map(|m| (m * 1000.0).round() as u32)
        .unwrap_or_else(|| default_kbps(preset.kind, dims.h));
    let frame_info = Frame {
        width: dims.w,
        height: dims.h,
        rate: dims.rate,
        ten_bit,
    };
    a.extend(codec_args(preset.kind, &encoder, &frame_info, kbps)?);
    plan.encoder = Some(encoder);

    if preset.is_image() {
        a.push(s("-an"));
        let seq = &settings.sequence;
        if seq.single {
            let pos = match seq.position.as_deref().filter(|p| !p.trim().is_empty()) {
                Some(p) => (seconds_at(p, info)? - start).max(0.0),
                None => duration / 2.0,
            };
            plan.input_args = vec![s("-ss"), format!("{:.3}", start + pos)];
            a.extend([s("-frames:v"), s("1"), s("-update"), s("1")]);
            plan.duration = 0.0;
        } else {
            plan.sequence = true;
            if let Some(every) = seq.every.filter(|e| *e > 0.0) {
                let last = graph.pop().unwrap_or_else(|| s("[0:v:0]null[vout]"));
                let last = last.replacen("[vout]", &format!(",fps=1/{every}[vout]"), 1);
                graph.push(last);
                if !a.contains(&s("[vout]")) {
                    let pos = a.iter().position(|x| x == "0:v:0").unwrap();
                    a[pos] = s("[vout]");
                }
            }
            if seq.tc_numbering {
                if let Some(tc) = tc_after(info, start) {
                    a.extend([s("-start_number"), s(tc.frames.max(0))]);
                }
            }
        }
    } else {
        a.extend(video_sound(job, &format, &mut graph));
        // Sous-titres en piste.
        if let (Some(file), false) = (&settings.subtitles.file, settings.subtitles.burn) {
            let ext = file
                .extension()
                .map(|e| e.to_string_lossy().to_lowercase())
                .unwrap_or_default();
            let codec = video::subtitle_codec(&format, &ext)?;
            let idx = 1 + plan.extra_inputs.len();
            plan.extra_inputs.push(Input {
                args: Vec::new(),
                path: file.clone(),
            });
            a.extend([s("-map"), format!("{idx}:0"), s("-c:s"), s(codec)]);
        }
        a.extend(timecode_args(info, start, &format));
        a.extend(color_args(settings.image.color_tags.as_deref()));
        if format == "mp4" {
            a.extend([s("-movflags"), s("+faststart")]);
        }
    }
    if !graph.is_empty() {
        a.splice(2..2, [s("-filter_complex"), graph.join(";")]);
    }
    plan.args = a;
    Ok(plan)
}

/// Formats son.
fn build_audio(job: &Job) -> Result<Plan> {
    let (preset, settings, info) = (job.preset, job.settings, job.info);
    if info.audio.is_empty() {
        return Err(no_audio(info));
    }
    let (input_args, start, duration) = trim(job)?;
    let rate = output_rate(preset, settings, info);
    let mut filters = Vec::new();
    if let Some(g) = job.gain {
        filters.push(format!("volume={g:.2}dB"));
    }
    filters.push(resample_filter(rate));
    let graph = audio_graph(
        info,
        max_channels(preset.kind, total_channels(info)),
        &filters,
    );
    let mut a = vec![
        s("-map_metadata"),
        s("0"),
        s("-vn"),
        s("-filter_complex"),
        graph,
        s("-map"),
        s("[aout]"),
    ];
    let depth = output_depth(preset, settings, info);
    let (_, default_kbps) = preset.audio_bitrates();
    let kbps = format!("{}k", settings.audio_kbps.unwrap_or(default_kbps));
    match preset.kind {
        Kind::Wav => {
            a.extend([
                s("-c:a"),
                s(pcm_codec(false, depth)),
                s("-rf64"),
                s("auto"),
                s("-write_bext"),
                s("1"),
            ]);
            a.extend(time_reference_args(info, start, rate));
        }
        Kind::Aiff => a.extend([s("-c:a"), s(pcm_codec(true, depth))]),
        Kind::Flac => {
            let fmt = if depth == BitDepth::S16 { "s16" } else { "s32" };
            a.extend([s("-c:a"), s("flac"), s("-sample_fmt"), s(fmt)]);
            if depth != BitDepth::S16 {
                a.extend([s("-bits_per_raw_sample"), s("24")]);
            }
        }
        Kind::Alac => {
            let fmt = if depth == BitDepth::S16 {
                "s16p"
            } else {
                "s32p"
            };
            a.extend([s("-c:a"), s("alac"), s("-sample_fmt"), s(fmt)]);
            if depth != BitDepth::S16 {
                a.extend([s("-bits_per_raw_sample"), s("24")]);
            }
        }
        Kind::Mp3 => a.extend([
            s("-c:a"),
            s("libmp3lame"),
            s("-b:a"),
            kbps,
            s("-id3v2_version"),
            s("3"),
        ]),
        Kind::Aac => a.extend([s("-c:a"), video::aac_encoder(), s("-b:a"), kbps]),
        Kind::Opus => a.extend([s("-c:a"), s("libopus"), s("-b:a"), kbps]),
        // Vorbis : réglage par qualité (un débit imposé échoue au-delà de 48 kHz).
        Kind::Vorbis => {
            let q = match settings.audio_kbps.unwrap_or(default_kbps) {
                0..=160 => 4,
                161..=224 => 6,
                225..=288 => 8,
                _ => 9,
            };
            a.extend([s("-c:a"), s("libvorbis"), s("-q:a"), s(q)]);
        }
        Kind::Ac3 => a.extend([s("-c:a"), s("ac3"), s("-b:a"), kbps]),
        _ => unreachable!(),
    }
    Ok(Plan {
        input_args,
        args: a,
        format: job.format(),
        duration,
        start,
        ..Default::default()
    })
}

/// Changement de conteneur, découpe et extraction de l'image, sans réencodage.
fn build_copy(job: &Job) -> Result<Plan> {
    let info = job.info;
    let format = job.format();
    let (input_args, start, duration) = trim(job)?;
    let mut a = vec![s("-map_metadata"), s("0")];
    match job.preset.kind {
        Kind::ExtractVideo => {
            if info.video.is_none() {
                return Err(no_video(info));
            }
            a.extend([s("-map"), s("0:v:0"), s("-an"), s("-c"), s("copy")]);
        }
        Kind::Cut => a.extend([
            s("-map"),
            s("0:v?"),
            s("-map"),
            s("0:a?"),
            s("-map"),
            s("0:s?"),
            s("-c"),
            s("copy"),
        ]),
        _ => a.extend([
            s("-map"),
            s("0:v?"),
            s("-map"),
            s("0:a?"),
            s("-c"),
            s("copy"),
        ]),
    }
    if start > 0.0 {
        a.extend([s("-avoid_negative_ts"), s("make_zero")]);
    }
    a.extend(timecode_args(info, start, &format));
    Ok(Plan {
        input_args,
        args: a,
        format,
        duration,
        start,
        ..Default::default()
    })
}

/// Son d'une vidéo en un seul WAV polyphonique, sans perte.
fn build_extract_audio(job: &Job) -> Result<Plan> {
    let info = job.info;
    if info.audio.is_empty() {
        return Err(no_audio(info));
    }
    let (input_args, start, duration) = trim(job)?;
    let depth = source_depth(info).unwrap_or(BitDepth::S24);
    let rate = info.audio[0].sample_rate;
    let mut a = vec![
        s("-map_metadata"),
        s("0"),
        s("-filter_complex"),
        audio_graph(info, None, &[]),
    ];
    a.extend([
        s("-map"),
        s("[aout]"),
        s("-c:a"),
        s(pcm_codec(false, depth)),
    ]);
    a.extend([s("-rf64"), s("auto"), s("-write_bext"), s("1")]);
    a.extend(time_reference_args(info, start, rate));
    Ok(Plan {
        input_args,
        args: a,
        format: s("wav"),
        duration,
        start,
        ..Default::default()
    })
}

/// Sous-titres ajoutés en piste, sans réencodage.
fn build_subtitles(job: &Job) -> Result<Plan> {
    let file = job
        .settings
        .subtitles
        .file
        .clone()
        .ok_or_else(|| Error::Unsupported("aucun fichier de sous-titres choisi".into()))?;
    if !file.is_file() {
        return Err(Error::NotFound(file.display().to_string()));
    }
    let format = job.format();
    let ext = file
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let codec = video::subtitle_codec(&format, &ext)?;
    let (input_args, start, duration) = trim(job)?;
    let mut a = vec![
        s("-map_metadata"),
        s("0"),
        s("-map"),
        s("0:v?"),
        s("-map"),
        s("0:a?"),
        s("-map"),
        s("1:0"),
    ];
    a.extend([s("-c"), s("copy"), s("-c:s"), s(codec)]);
    a.extend(timecode_args(job.info, start, &format));
    Ok(Plan {
        input_args,
        extra_inputs: vec![Input {
            args: Vec::new(),
            path: file,
        }],
        args: a,
        format,
        duration,
        start,
        ..Default::default()
    })
}

/// Fichier son associé à une vidéo : même nom, sinon le seul proposé, sinon
/// un nom qui commence comme celui de la vidéo.
pub fn match_audio<'a>(source: &Path, files: &'a [PathBuf]) -> Option<&'a PathBuf> {
    let stem = |p: &Path| {
        p.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase()
    };
    let v = stem(source);
    files
        .iter()
        .find(|f| stem(f) == v)
        .or_else(|| (files.len() == 1).then(|| &files[0]))
        .or_else(|| {
            files
                .iter()
                .find(|f| stem(f).starts_with(&v) || v.starts_with(&stem(f)))
        })
}

/// Décalage du son de remplacement par timecode : BWF du son et timecode de l'image.
fn tc_offset(info: &MediaInfo, audio: &Path) -> Option<f64> {
    let wav = crate::media::wav::read_info(audio).ok()?;
    let refs = wav.time_reference? as f64 / wav.sample_rate.max(1) as f64;
    let tc = info.start_tc()?;
    let video = tc.frames as f64 * tc.rate.den as f64 / tc.rate.num as f64;
    Some(refs - video)
}

/// Son remplacé, image recopiée telle quelle.
fn build_replace_audio(job: &Job) -> Result<Plan> {
    let (settings, info) = (job.settings, job.info);
    if info.video.is_none() {
        return Err(no_video(info));
    }
    let audio = match_audio(job.source, &settings.audio_files)
        .ok_or_else(|| {
            Error::Unsupported("aucun fichier son associé à cette vidéo (même nom attendu)".into())
        })?
        .clone();
    let audio_info = crate::media::probe::probe(&audio)?;
    let mut offset = settings.audio_offset;
    if settings.sync_tc {
        offset += tc_offset(info, &audio).ok_or_else(|| {
            Error::Unsupported(
                "calage par timecode impossible : timecode de l'image ou BWF du son absent".into(),
            )
        })?;
    }
    let mut in_args = Vec::new();
    if offset > 0.0 {
        in_args.extend([s("-itsoffset"), format!("{offset:.4}")]);
    } else if offset < 0.0 {
        in_args.extend([s("-ss"), format!("{:.4}", -offset)]);
    }
    let format = job.format();
    let mut a = vec![
        s("-map_metadata"),
        s("0"),
        s("-map"),
        s("0:v:0"),
        s("-c:v"),
        s("copy"),
    ];
    let spec = video_audio(job.preset, &format);
    if spec.mono_tracks {
        let (g, outs) = mono_tracks(1, &audio_info, None, &[], total_channels(&audio_info));
        a.extend([s("-filter_complex"), g]);
        for o in outs {
            a.extend([s("-map"), o]);
        }
    } else {
        a.extend([s("-map"), s("1:a")]);
    }
    a.extend(spec.args);
    a.extend([s("-t"), format!("{:.3}", info.duration)]);
    a.extend(timecode_args(info, 0.0, &format));
    Ok(Plan {
        extra_inputs: vec![Input {
            args: in_args,
            path: audio,
        }],
        args: a,
        format,
        duration: info.duration,
        ..Default::default()
    })
}

/// Nouvelle cadence sans réencoder l'image (25 vers 24 i/s, par exemple) ;
/// le son suit, ralenti ou accéléré.
fn build_conform(job: &Job) -> Result<Plan> {
    let (settings, info) = (job.settings, job.info);
    let v = info.video.as_ref().ok_or_else(|| no_video(info))?;
    let target = settings
        .conform_rate
        .as_deref()
        .and_then(parse_rate)
        .ok_or_else(|| Error::Unsupported("cadence de conformation à choisir".into()))?;
    let k = v.rate.as_f64() / target.as_f64();
    let format = job.format();
    let mut a = vec![
        s("-map_metadata"),
        s("0"),
        s("-map"),
        s("0:v:0"),
        s("-c:v"),
        s("copy"),
    ];
    a.extend([s("-r"), format!("{}/{}", target.num, target.den)]);
    if !info.audio.is_empty() {
        let sr = info.audio[0].sample_rate.max(1);
        let speed = 1.0 / k;
        let f = if settings.keep_pitch {
            format!("atempo={speed:.6}")
        } else {
            format!("asetrate={:.0},aresample={sr}", sr as f64 * speed)
        };
        let spec = video_audio(job.preset, &format);
        if spec.mono_tracks {
            let (g, outs) = mono_tracks(0, info, None, &[f], total_channels(info));
            a.extend([s("-filter_complex"), g]);
            for o in outs {
                a.extend([s("-map"), o]);
            }
        } else {
            a.extend([
                s("-filter_complex"),
                audio_chain(0, info, None, &[f], "aout"),
                s("-map"),
                s("[aout]"),
            ]);
        }
        a.extend(spec.args);
    }
    // Même heure de début, images ramenées à la nouvelle cadence.
    if let Some(tc) = info.start_tc() {
        if matches!(format.as_str(), "mov" | "mp4" | "mxf") {
            let (h, m, sec, f) = tc.components();
            let f = f.min(target.nominal() as i64 - 1);
            let tc = Timecode::from_components(h, m, sec, f, target, tc.drop_frame);
            a.extend([s("-timecode"), tc.to_string()]);
        }
    }
    Ok(Plan {
        input_args: vec![s("-itsscale:v"), format!("{k:.10}")],
        args: a,
        format,
        duration: info.duration * k,
        ..Default::default()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::probe::{AudioStream, VideoStream};
    use crate::transcode::catalog::find;
    use crate::transcode::settings::Trim;

    pub fn info(audio: &[(u32, u32, Option<u32>)], video: bool) -> MediaInfo {
        MediaInfo {
            path: "clip.mov".into(),
            format: "mov".into(),
            duration: 10.0,
            size: 0,
            video: video.then(|| VideoStream {
                index: 0,
                codec: "h264".into(),
                width: 1920,
                height: 1080,
                rate: FrameRate::new(25, 1),
                pix_fmt: "yuv420p".into(),
                frame_count: 250,
                start_time: 0.0,
            }),
            audio: audio
                .iter()
                .enumerate()
                .map(|(i, (rate, ch, bits))| AudioStream {
                    index: i as u32 + 1,
                    codec: "pcm_s24le".into(),
                    sample_rate: *rate,
                    channels: *ch,
                    bits: *bits,
                })
                .collect(),
            start_timecode: Some("10:00:00:00".into()),
            tags: Default::default(),
        }
    }

    fn settings(preset: &str) -> Settings {
        Settings {
            preset: preset.into(),
            software: true,
            ..Default::default()
        }
    }

    fn plan(preset: &str, settings: &Settings, info: &MediaInfo) -> Result<Plan> {
        build(&Job {
            preset: find(preset).unwrap(),
            settings,
            info,
            source: Path::new("/rushes/clip.mov"),
            gain: None,
        })
    }

    #[test]
    fn positions_read_timecode_or_seconds() {
        let i = info(&[], true);
        assert_eq!(seconds_at("10:00:02:00", &i).unwrap(), 2.0);
        assert_eq!(seconds_at("1,5", &i).unwrap(), 1.5);
        assert!(seconds_at("09:59:59:00", &i).is_err());
    }

    #[test]
    fn trim_shifts_timecode_and_bwf() {
        let i = info(&[(48_000, 2, Some(24))], true);
        let mut st = settings("extract_audio");
        st.trims.insert(
            "/rushes/clip.mov".into(),
            Trim {
                start: Some("10:00:02:00".into()),
                end: Some("8".into()),
            },
        );
        let p = plan("extract_audio", &st, &i).unwrap();
        assert_eq!(p.input_args, ["-ss", "2.000", "-to", "8.000"]);
        assert_eq!(p.duration, 6.0);
        // 10:00:02:00 = 36 002 s × 48 000.
        assert!(
            p.args.contains(&"time_reference=1728096000".to_string()),
            "{:?}",
            p.args
        );
        let mut st = settings("prores_hq");
        st.trims = settings("x").trims;
        st.trims.insert(
            "/rushes/clip.mov".into(),
            Trim {
                start: Some("1".into()),
                end: None,
            },
        );
        let p = plan("prores_hq", &st, &i).unwrap();
        assert!(p.args.join(" ").contains("-timecode 10:00:01:00"));
    }

    #[test]
    fn mxf_gets_one_mono_track_per_channel() {
        let i = info(&[(48_000, 2, Some(24))], true);
        let p = plan("xdcam_hd422", &settings("xdcam_hd422"), &i).unwrap();
        let args = p.args.join(" ");
        assert!(args.contains("asplit=2[s0][s1]"), "{args}");
        assert!(args.contains("-map [a0] -map [a1]"), "{args}");
        assert!(
            args.contains("scale=1920:1080:force_original_aspect_ratio=decrease"),
            "{args}"
        );
    }

    #[test]
    fn web_preset_forces_stereo_and_bitrate() {
        let i = info(
            &[
                (48_000, 1, Some(24)),
                (48_000, 1, Some(24)),
                (48_000, 1, Some(24)),
            ],
            true,
        );
        let p = plan("youtube_1080", &settings("youtube_1080"), &i).unwrap();
        let args = p.args.join(" ");
        assert!(
            args.contains("amerge=inputs=3,pan=stereo|c0=c0|c1=c1[aout]"),
            "{args}"
        );
        assert!(args.contains("-b:v 8000k"), "{args}");
        assert!(args.contains("-b:a 384k"), "{args}");
    }

    #[test]
    fn normalization_gain_comes_before_resampling() {
        let wav = info(&[(48_000, 2, Some(24))], false);
        let p = build(&Job {
            preset: find("wav").unwrap(),
            settings: &settings("wav"),
            info: &wav,
            source: Path::new("/rushes/a.wav"),
            gain: Some(-3.5),
        })
        .unwrap();
        let graph = &p.args[p.args.iter().position(|a| a == "-filter_complex").unwrap() + 1];
        assert!(
            graph.starts_with("[0:a:0]volume=-3.50dB,aresample=48000:"),
            "{graph}"
        );
    }

    #[test]
    fn rates_and_depths_follow_the_format() {
        let hi = info(&[(96_000, 2, Some(24))], false);
        assert_eq!(
            output_rate(find("mp3").unwrap(), &settings("mp3"), &hi),
            48_000
        );
        let hi441 = info(&[(88_200, 2, Some(24))], false);
        assert_eq!(
            output_rate(find("mp3").unwrap(), &settings("mp3"), &hi441),
            44_100
        );
        assert_eq!(
            output_rate(find("opus").unwrap(), &settings("opus"), &hi441),
            48_000
        );
        let mut st = settings("flac");
        st.bit_depth = Some(BitDepth::F32);
        assert_eq!(output_depth(find("flac").unwrap(), &st, &hi), BitDepth::S24);
        let cd = info(&[(44_100, 2, Some(16))], false);
        assert_eq!(
            output_depth(find("wav").unwrap(), &settings("wav"), &cd),
            BitDepth::S16
        );
    }

    #[test]
    fn audio_files_match_videos_by_name() {
        let files = vec![
            PathBuf::from("/son/A001C001.WAV"),
            PathBuf::from("/son/A001C002.wav"),
        ];
        assert_eq!(
            match_audio(Path::new("/v/a001c002.mov"), &files),
            Some(&files[1])
        );
        assert_eq!(match_audio(Path::new("/v/B001.mov"), &files), None);
        let one = vec![PathBuf::from("/son/mix.wav")];
        assert_eq!(match_audio(Path::new("/v/B001.mov"), &one), Some(&one[0]));
    }
}
