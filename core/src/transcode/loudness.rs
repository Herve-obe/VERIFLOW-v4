//! Mesure du niveau (EBU R128 / ITU-R BS.1770) et normalisation par gain
//! simple : la dynamique n'est jamais modifiée. Si le gain nécessaire ferait
//! dépasser le True Peak maximal, il est réduit et le niveau cible n'est pas
//! atteint (signalé dans le résultat), plutôt que de passer par un limiteur.

use std::path::Path;
use std::process::Stdio;

use serde::Serialize;

use super::filters::audio_graph;
use super::settings::LoudnessTarget;
use crate::media::probe::MediaInfo;
use crate::{tools, Error, Result};

/// Mesures d'un fichier, et gain appliqué en cas de normalisation.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Loudness {
    /// Niveau intégré (LUFS).
    pub integrated: f64,
    /// Plage de loudness (LU).
    pub range: f64,
    /// True Peak (dBTP).
    pub true_peak: f64,
    /// Crête échantillon (dBFS).
    pub sample_peak: f64,
    /// Gain appliqué (dB), si normalisation.
    pub gain: Option<f64>,
    /// Le gain a été réduit pour respecter le True Peak maximal.
    pub peak_limited: bool,
}

impl Loudness {
    /// Gain à appliquer pour atteindre la cible sans dépasser le True Peak.
    pub fn plan(&mut self, target: LoudnessTarget) -> f64 {
        let wanted = target.integrated - self.integrated;
        let headroom = target.true_peak - self.true_peak;
        let gain = if wanted > headroom {
            self.peak_limited = true;
            headroom
        } else {
            wanted
        };
        let gain = (gain * 100.0).round() / 100.0;
        self.gain = Some(gain);
        gain
    }
}

/// Lit le résumé du filtre `ebur128` dans la sortie d'erreur de FFmpeg.
pub fn parse_summary(text: &str) -> Option<Loudness> {
    let start = text.rfind("Summary:")?;
    let mut l = Loudness::default();
    let mut section = "";
    let (mut i, mut lra, mut tp, mut sp) = (None, None, None, None);
    for line in text[start..].lines() {
        let line = line.trim();
        let value = |line: &str| -> Option<f64> {
            let v = line.split_once(':')?.1.split_whitespace().next()?;
            // Silence complet : « -inf ».
            if v.eq_ignore_ascii_case("-inf") {
                Some(f64::NEG_INFINITY)
            } else {
                v.parse().ok()
            }
        };
        if line.starts_with("Integrated loudness") {
            section = "i";
        } else if line.starts_with("Loudness range") {
            section = "lra";
        } else if line.starts_with("True peak") {
            section = "tp";
        } else if line.starts_with("Sample peak") {
            section = "sp";
        } else if line.starts_with("I:") && section == "i" {
            i = value(line);
        } else if line.starts_with("LRA:") && section == "lra" {
            lra = value(line);
        } else if line.starts_with("Peak:") {
            match section {
                "tp" => tp = value(line),
                "sp" => sp = value(line),
                _ => {}
            }
        }
    }
    l.integrated = i?;
    l.range = lra.unwrap_or(0.0);
    l.true_peak = tp?;
    l.sample_peak = sp.unwrap_or(l.true_peak);
    Some(l)
}

/// Mesure un fichier : toutes ses pistes son réunies, limitées aux deux
/// premières si `max_channels` le demande (comme le fichier produit).
pub fn measure(path: &Path, info: &MediaInfo, max_channels: Option<u32>) -> Result<Loudness> {
    if info.audio.is_empty() {
        return Err(Error::Unsupported(format!("{} : pas de son", info.path)));
    }
    let graph = audio_graph(
        info,
        max_channels,
        &["ebur128=peak=sample+true:framelog=quiet".to_owned()],
    );
    let out = tools::command("ffmpeg")?
        .args(["-hide_banner", "-nostats", "-nostdin", "-i"])
        .arg(path)
        .args([
            "-filter_complex",
            &graph,
            "-map",
            "[aout]",
            "-f",
            "null",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .output()?;
    let text = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(Error::Tool {
            tool: "ffmpeg".into(),
            message: text.lines().last().unwrap_or_default().to_owned(),
        });
    }
    parse_summary(&text).ok_or_else(|| Error::Tool {
        tool: "ffmpeg".into(),
        message: "mesure de loudness illisible".into(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUMMARY: &str = "[Parsed_ebur128_0 @ 0x1] Summary:

  Integrated loudness:
    I:         -18.4 LUFS
    Threshold: -28.6 LUFS

  Loudness range:
    LRA:         6.2 LU
    Threshold: -38.7 LUFS
    LRA low:   -22.1 LUFS
    LRA high:  -15.9 LUFS

  Sample peak:
    Peak:       -2.1 dBFS

  True peak:
    Peak:       -1.7 dBFS
";

    #[test]
    fn reads_ebur128_summary() {
        let l = parse_summary(SUMMARY).unwrap();
        assert_eq!(l.integrated, -18.4);
        assert_eq!(l.range, 6.2);
        assert_eq!(l.sample_peak, -2.1);
        assert_eq!(l.true_peak, -1.7);
    }

    #[test]
    fn gain_is_reduced_to_respect_true_peak() {
        let mut l = parse_summary(SUMMARY).unwrap();
        // EBU R128 : -23 LUFS, -1 dBTP : baisse de 4,6 dB, sans contrainte.
        assert_eq!(
            l.plan(LoudnessTarget {
                integrated: -23.0,
                true_peak: -1.0
            }),
            -4.6
        );
        assert!(!l.peak_limited);
        // Streaming -14 LUFS : +4,4 dB voulus, mais le True Peak passerait à
        // +2,7 dBTP : gain limité à +0,7 dB.
        let mut l = parse_summary(SUMMARY).unwrap();
        assert_eq!(
            l.plan(LoudnessTarget {
                integrated: -14.0,
                true_peak: -1.0
            }),
            0.7
        );
        assert!(l.peak_limited);
    }
}
