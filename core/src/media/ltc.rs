//! Timecode linéaire (LTC, SMPTE 12M) enregistré sur une piste son : par
//! exemple, un générateur Tentacle Sync branché sur l'entrée droite d'une
//! caméra. Détection (pour couper cette piste à l'écoute) et lecture du
//! timecode (réutilisée par l'onglet SYNC).
//!
//! Le signal est un code biphase : une transition à chaque début de bit, et
//! une transition supplémentaire au milieu des bits à 1. Une trame compte
//! 80 bits et se termine par le mot de synchronisation 0011 1111 1111 1101.

use serde::Serialize;

/// Mot de synchronisation (bits 64 à 79, dans l'ordre d'émission).
const SYNC_WORD: u128 = 0x3FFD;
/// Cadences possibles, pour estimer la durée d'un bit.
const RATES: [f64; 5] = [23.976, 24.0, 25.0, 29.97, 30.0];

/// Trame LTC décodée.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct LtcFrame {
    /// Échantillon où la trame se termine.
    pub sample: u64,
    pub hours: u8,
    pub minutes: u8,
    pub seconds: u8,
    pub frames: u8,
    pub drop_frame: bool,
    /// Bits utilisateur (8 groupes de 4 bits).
    pub user_bits: u32,
}

impl LtcFrame {
    pub fn timecode(&self) -> String {
        format!(
            "{:02}:{:02}:{:02}{}{:02}",
            self.hours,
            self.minutes,
            self.seconds,
            if self.drop_frame { ';' } else { ':' },
            self.frames
        )
    }
}

/// Résultat de la détection sur une piste.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LtcDetection {
    /// Première trame lue.
    pub first: LtcFrame,
    /// Timecode de la première trame lue (HH:MM:SS:FF, « ; » en drop frame).
    pub timecode: String,
    /// Cadence estimée d'après l'espacement des trames (i/s).
    pub fps: f64,
    /// Nombre de trames valides lues.
    pub frames: usize,
}

/// Transitions du signal (indices d'échantillons), avec hystérésis pour
/// ignorer le bruit autour du zéro.
fn transitions(samples: &[f32]) -> Vec<f64> {
    if samples.is_empty() {
        return Vec::new();
    }
    let mean = samples.iter().map(|&s| s as f64).sum::<f64>() / samples.len() as f64;
    let peak = samples
        .iter()
        .map(|&s| (s as f64 - mean).abs())
        .fold(0.0, f64::max);
    if peak < 0.01 {
        return Vec::new();
    }
    let hi = 0.3 * peak;
    let mut state: Option<bool> = None;
    let mut out = Vec::new();
    for (i, &s) in samples.iter().enumerate() {
        let v = s as f64 - mean;
        let new = if v > hi {
            Some(true)
        } else if v < -hi {
            Some(false)
        } else {
            None
        };
        if let Some(n) = new {
            if state.is_some_and(|st| st != n) {
                out.push(i as f64);
            }
            state = Some(n);
        }
    }
    out
}

fn bcd(reg: u128, first: usize, len: usize) -> u8 {
    // Bit `k` de la trame (ordre d'émission) : position 79 - k du registre.
    (0..len).fold(0u8, |acc, i| {
        acc | ((((reg >> (79 - (first + i))) & 1) as u8) << i)
    })
}

fn frame_from(reg: u128, sample: u64) -> Option<LtcFrame> {
    let frames = bcd(reg, 0, 4) + 10 * bcd(reg, 8, 2);
    let seconds = bcd(reg, 16, 4) + 10 * bcd(reg, 24, 3);
    let minutes = bcd(reg, 32, 4) + 10 * bcd(reg, 40, 3);
    let hours = bcd(reg, 48, 4) + 10 * bcd(reg, 56, 2);
    let units_ok =
        bcd(reg, 0, 4) < 10 && bcd(reg, 16, 4) < 10 && bcd(reg, 32, 4) < 10 && bcd(reg, 48, 4) < 10;
    if !units_ok || frames >= 30 || seconds >= 60 || minutes >= 60 || hours >= 24 {
        return None;
    }
    let user_bits = [4usize, 12, 20, 28, 36, 44, 52, 60]
        .iter()
        .enumerate()
        .fold(0u32, |acc, (i, &b)| {
            acc | ((bcd(reg, b, 4) as u32) << (4 * i))
        });
    Some(LtcFrame {
        sample,
        hours,
        minutes,
        seconds,
        frames,
        drop_frame: (reg >> (79 - 10)) & 1 == 1,
        user_bits,
    })
}

/// Décode toutes les trames LTC lisibles d'une piste (mono, flottant).
pub fn decode(samples: &[f32], sample_rate: u32) -> Vec<LtcFrame> {
    let edges = transitions(samples);
    if edges.len() < 160 {
        return Vec::new();
    }
    let gaps: Vec<f64> = edges.windows(2).map(|w| w[1] - w[0]).collect();
    // Durée d'un bit : la cadence dont les intervalles (bit entier ou
    // demi-bit) expliquent le mieux le signal.
    let score = |bit: f64| {
        gaps.iter()
            .filter(|&&g| (g - bit).abs() < 0.25 * bit || (g - bit / 2.0).abs() < 0.125 * bit)
            .count()
    };
    let bit = RATES
        .iter()
        .map(|fps| sample_rate as f64 / (fps * 80.0))
        .max_by_key(|&b| score(b))
        .unwrap_or(1.0);
    if score(bit) < gaps.len() * 8 / 10 {
        return Vec::new();
    }

    let mut out = Vec::new();
    let mut reg: u128 = 0;
    // Bits reçus depuis la dernière perte de synchronisation : une trame
    // n'est valide qu'entière (la première de l'extrait est souvent tronquée).
    let mut count = 0usize;
    let mut half_pending = false;
    for (i, &g) in gaps.iter().enumerate() {
        let bit_value = if g > 0.75 * bit && g < 1.5 * bit {
            if half_pending {
                // Demi-bit isolé : perte de synchronisation, on repart.
                half_pending = false;
                reg = 0;
                count = 0;
            }
            Some(0)
        } else if g > 0.25 * bit && g <= 0.75 * bit {
            if half_pending {
                half_pending = false;
                Some(1)
            } else {
                half_pending = true;
                None
            }
        } else {
            half_pending = false;
            reg = 0;
            count = 0;
            None
        };
        if let Some(b) = bit_value {
            reg = ((reg << 1) | b) & ((1u128 << 80) - 1);
            count += 1;
            if count >= 80 && reg & 0xFFFF == SYNC_WORD {
                if let Some(f) = frame_from(reg, edges[i + 1] as u64) {
                    out.push(f);
                }
            }
        }
    }
    out
}

/// Cherche du LTC dans un extrait : au moins 3 trames successives
/// régulièrement espacées. Renvoie `None` pour un son ordinaire.
pub fn detect(samples: &[f32], sample_rate: u32) -> Option<LtcDetection> {
    let frames = decode(samples, sample_rate);
    if frames.len() < 3 {
        return None;
    }
    let spacing: Vec<f64> = frames
        .windows(2)
        .map(|w| (w[1].sample - w[0].sample) as f64)
        .collect();
    let mean = spacing.iter().sum::<f64>() / spacing.len() as f64;
    if spacing.iter().any(|s| (s - mean).abs() > 0.1 * mean) {
        return None;
    }
    let fps = sample_rate as f64 / mean;
    let first = frames[0];
    Some(LtcDetection {
        timecode: first.timecode(),
        first,
        fps: (fps * 1000.0).round() / 1000.0,
        frames: frames.len(),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Encodeur de test (biphase), indépendant du décodeur : trame SMPTE 12M.
    pub(crate) fn encode(
        start: (u8, u8, u8, u8),
        fps: u8,
        frames: usize,
        sample_rate: u32,
    ) -> Vec<f32> {
        let (h, mut m, mut s, mut f) = start;
        let bit_len = sample_rate as f64 / (fps as f64 * 80.0);
        let mut out = Vec::new();
        let mut level = 0.5f32;
        let mut t = 0.0f64;
        for _ in 0..frames {
            let mut bits = [0u8; 80];
            let mut put = |first: usize, len: usize, v: u8| {
                for i in 0..len {
                    bits[first + i] = (v >> i) & 1;
                }
            };
            put(0, 4, f % 10);
            put(8, 2, f / 10);
            put(16, 4, s % 10);
            put(24, 3, s / 10);
            put(32, 4, m % 10);
            put(40, 3, m / 10);
            put(48, 4, h % 10);
            put(56, 2, h / 10);
            for (i, b) in [0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1]
                .iter()
                .enumerate()
            {
                bits[64 + i] = *b;
            }
            for &b in &bits {
                let start = t;
                t += bit_len;
                level = -level; // transition en début de bit
                let mid = start + bit_len / 2.0;
                while (out.len() as f64) < t {
                    let n = out.len() as f64;
                    let v = if b == 1 && n >= mid { -level } else { level };
                    out.push(v);
                }
                if b == 1 {
                    level = -level;
                }
            }
            f += 1;
            if f >= fps {
                f = 0;
                s += 1;
                if s == 60 {
                    s = 0;
                    m += 1;
                }
            }
        }
        out
    }

    #[test]
    fn decodes_generated_ltc() {
        let sig = encode((10, 20, 30, 12), 25, 50, 48_000);
        let frames = decode(&sig, 48_000);
        assert!(frames.len() >= 48, "{} trames", frames.len());
        // La première trame de l'extrait est incomplète : la suivante est la première lue.
        assert_eq!(frames[0].timecode(), "10:20:30:13");
        assert_eq!(frames[12].timecode(), "10:20:31:00");
        let d = detect(&sig, 48_000).unwrap();
        assert!((d.fps - 25.0).abs() < 0.05, "{}", d.fps);
    }

    #[test]
    fn ignores_ordinary_sound_and_silence() {
        let sine: Vec<f32> = (0..96_000)
            .map(|i| (i as f32 * 2.0 * std::f32::consts::PI * 1000.0 / 48_000.0).sin() * 0.3)
            .collect();
        assert!(detect(&sine, 48_000).is_none());
        // Bruit pseudo-aléatoire.
        let mut x = 12345u32;
        let noise: Vec<f32> = (0..96_000)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (x >> 16) as f32 / 32_768.0 - 1.0
            })
            .collect();
        assert!(detect(&noise, 48_000).is_none());
        assert!(detect(&vec![0.0; 96_000], 48_000).is_none());
    }

    #[test]
    fn tolerates_level_noise_and_polarity() {
        let mut sig: Vec<f32> = encode((1, 2, 3, 4), 30, 40, 48_000)
            .iter()
            .map(|s| -s * 0.1)
            .collect();
        let mut x = 7u32;
        for s in sig.iter_mut() {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
            *s += ((x >> 16) as f32 / 32_768.0 - 1.0) * 0.005;
        }
        let d = detect(&sig, 48_000).unwrap();
        assert_eq!(d.timecode, "01:02:03:05");
        assert!((d.fps - 30.0).abs() < 0.05);
    }
}
