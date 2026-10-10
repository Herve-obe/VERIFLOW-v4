//! Corrélation de formes d'onde : retrouve le décalage entre le son témoin
//! d'une caméra et le son de l'enregistreur (claps, voix, ambiance).
//!
//! Méthode GCC-PHAT (corrélation croisée à phase normalisée, calculée par
//! FFT) : le pic est net même quand les deux micros sont différents.

use std::path::Path;
use std::process::Stdio;

use realfft::num_complex::Complex;
use realfft::RealFftPlanner;

use crate::media::probe::MediaInfo;
use crate::transcode::filters::audio_chain;
use crate::{tools, Error, Result};

/// Son d'un fichier, mono, à la fréquence `rate`, de `start` (s) pendant
/// `duration` (s). `skip` : canal à ignorer (piste LTC).
pub fn decode_mono(
    path: &Path,
    info: &MediaInfo,
    start: f64,
    duration: f64,
    rate: u32,
    skip: Option<usize>,
) -> Result<Vec<f32>> {
    if info.audio.is_empty() {
        return Err(Error::Unsupported(format!("{} : pas de son", info.path)));
    }
    let channels: u32 = info.audio.iter().map(|a| a.channels.max(1)).sum();
    let used: Vec<String> = (0..channels as usize)
        .filter(|c| Some(*c) != skip || channels == 1)
        .map(|c| format!("c{c}"))
        .collect();
    let pan = format!("pan=mono|c0<{}", used.join("+"));
    // Passe-haut : le grondement et le vent pèsent peu dans la corrélation.
    let graph = audio_chain(
        0,
        info,
        Some(pan),
        &["highpass=f=120".into(), format!("aresample={rate}")],
        "aout",
    );
    let out = tools::command("ffmpeg")?
        .args(["-hide_banner", "-nostdin", "-v", "error"])
        .args([
            "-ss",
            &format!("{:.3}", start.max(0.0)),
            "-t",
            &format!("{:.3}", duration.max(0.1)),
        ])
        .arg("-i")
        .arg(path)
        .args([
            "-vn",
            "-filter_complex",
            &graph,
            "-map",
            "[aout]",
            "-f",
            "f32le",
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()?;
    if !out.status.success() {
        return Err(Error::Tool {
            tool: "ffmpeg".into(),
            message: crate::transcode::run::useful_error(&String::from_utf8_lossy(&out.stderr)),
        });
    }
    Ok(out
        .stdout
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| f32::from_le_bytes(*b))
        .collect())
}

/// Résultat d'une corrélation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lag {
    /// Décalage (échantillons) : `b[n + lag]` correspond à `a[n]`.
    pub samples: i64,
    /// Netteté du pic : 0 (aucune ressemblance) à 1 (pic isolé).
    pub confidence: f64,
}

/// Décalage de `b` par rapport à `a`, cherché entre `min_lag` et `max_lag`
/// échantillons.
pub fn gcc_phat(a: &[f32], b: &[f32], min_lag: i64, max_lag: i64) -> Option<Lag> {
    if a.len() < 64 || b.len() < 64 || min_lag > max_lag {
        return None;
    }
    let n = (a.len() + b.len()).next_power_of_two();
    let mut planner = RealFftPlanner::<f32>::new();
    let fwd = planner.plan_fft_forward(n);
    let inv = planner.plan_fft_inverse(n);
    let spectrum = |x: &[f32]| -> Vec<Complex<f32>> {
        let mut buf = vec![0f32; n];
        // Fenêtre de Hann : les bords de l'extrait ne créent pas de faux pic.
        let len = x.len();
        for (i, v) in x.iter().enumerate() {
            let w = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * i as f32 / (len - 1) as f32).cos();
            buf[i] = v * w;
        }
        let mut out = fwd.make_output_vec();
        fwd.process(&mut buf, &mut out).ok();
        out
    };
    let fa = spectrum(a);
    let fb = spectrum(b);
    let mean = fa
        .iter()
        .zip(&fb)
        .map(|(x, y)| (x.conj() * y).norm())
        .sum::<f32>()
        / fa.len() as f32;
    let eps = mean * 1e-3 + 1e-20;
    let mut cross: Vec<Complex<f32>> = fa
        .iter()
        .zip(&fb)
        .map(|(x, y)| {
            let r = x.conj() * y;
            r / (r.norm() + eps)
        })
        .collect();
    let mut corr = inv.make_output_vec();
    inv.process(&mut cross, &mut corr).ok()?;
    // corr[k] : ressemblance pour un décalage k (négatif : indices en fin de tableau).
    let at = |lag: i64| -> f32 {
        let idx = if lag >= 0 {
            lag as usize
        } else {
            (n as i64 + lag) as usize
        };
        corr.get(idx).copied().unwrap_or(0.0)
    };
    let lo = min_lag.max(-(a.len() as i64) + 1);
    let hi = max_lag.min(b.len() as i64 - 1);
    if lo > hi {
        return None;
    }
    let (mut best, mut best_v) = (lo, f32::MIN);
    for lag in lo..=hi {
        let v = at(lag);
        if v > best_v {
            best_v = v;
            best = lag;
        }
    }
    // Deuxième pic, hors du voisinage du premier (2 ms à 16 kHz).
    let guard = 32;
    let mut second = 0f32;
    for lag in lo..=hi {
        if (lag - best).abs() > guard {
            second = second.max(at(lag));
        }
    }
    if best_v <= 0.0 {
        return None;
    }
    Some(Lag {
        samples: best,
        confidence: (1.0 - (second / best_v) as f64).clamp(0.0, 1.0),
    })
}

/// Enveloppe (valeur crête par bloc), pour l'affichage des formes d'onde.
pub fn envelope(x: &[f32], points: usize) -> Vec<f32> {
    if x.is_empty() || points == 0 {
        return Vec::new();
    }
    let step = (x.len() as f64 / points as f64).max(1.0);
    (0..points)
        .map(|i| {
            let a = (i as f64 * step) as usize;
            let b = (((i + 1) as f64 * step) as usize).min(x.len());
            x.get(a..b.max(a + 1).min(x.len()))
                .unwrap_or(&[])
                .iter()
                .fold(0f32, |m, v| m.max(v.abs()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bruit pseudo-aléatoire reproductible.
    fn noise(n: usize, seed: u64) -> Vec<f32> {
        let mut s = seed;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s % 2000) as f32 / 1000.0 - 1.0
            })
            .collect()
    }

    #[test]
    fn finds_the_offset_of_a_copy_with_other_noise() {
        let sig = noise(40_000, 7);
        // b : même signal décalé de 1 234 échantillons, plus du bruit d'un autre micro.
        let lag = 1234;
        let other = noise(40_000, 99);
        let b: Vec<f32> = (0..36_000)
            .map(|i| sig.get(i + 4000 - lag).copied().unwrap_or(0.0) * 0.6 + other[i] * 0.3)
            .collect();
        let a: Vec<f32> = sig[4000..30_000].to_vec();
        let r = gcc_phat(&a, &b, -5000, 5000).unwrap();
        assert_eq!(r.samples, lag as i64);
        assert!(r.confidence > 0.3, "{}", r.confidence);
        // Aucun rapport entre les deux : confiance faible.
        let r = gcc_phat(&a, &noise(36_000, 5), -5000, 5000).unwrap();
        assert!(r.confidence < 0.3, "{}", r.confidence);
    }

    #[test]
    fn envelope_keeps_peaks() {
        let x = [0.0, 0.5, -0.9, 0.1, 0.2, 0.0];
        assert_eq!(envelope(&x, 2), vec![0.9, 0.2]);
    }
}
