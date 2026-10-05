//! Table de mixage du PLAYER AUDIO (charte §7.3) : jusqu'à 32 pistes (et plus),
//! gain, panoramique à puissance constante, SOLO, MUTE, crêtes par piste et master.
//!
//! Les réglages et les mesures sont des atomiques : l'interface les modifie ou
//! les lit sans jamais bloquer le fil audio.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

/// Flottant partageable sans verrou.
#[derive(Debug, Default)]
pub struct AtomicF32(AtomicU32);

impl AtomicF32 {
    pub fn new(v: f32) -> Self {
        Self(AtomicU32::new(v.to_bits()))
    }
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }
    pub fn set(&self, v: f32) {
        self.0.store(v.to_bits(), Ordering::Relaxed)
    }
    /// Conserve le maximum (crête) entre deux lectures.
    pub fn max(&self, v: f32) {
        let mut cur = self.0.load(Ordering::Relaxed);
        while v > f32::from_bits(cur) {
            match self.0.compare_exchange_weak(
                cur,
                v.to_bits(),
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => cur = actual,
            }
        }
    }
    /// Lit la valeur et la remet à zéro (lecture des crêtes par l'interface).
    pub fn take(&self) -> f32 {
        f32::from_bits(self.0.swap(0, Ordering::Relaxed))
    }
}

/// Réglages d'une piste.
#[derive(Debug)]
pub struct TrackControl {
    /// Gain linéaire (1.0 = 0 dB).
    pub gain: AtomicF32,
    /// Panoramique de -1.0 (gauche) à +1.0 (droite).
    pub pan: AtomicF32,
    pub mute: AtomicBool,
    pub solo: AtomicBool,
    /// Crête post-fader depuis la dernière lecture.
    pub peak: AtomicF32,
}

impl Default for TrackControl {
    fn default() -> Self {
        Self {
            gain: AtomicF32::new(1.0),
            pan: AtomicF32::new(0.0),
            mute: AtomicBool::new(false),
            solo: AtomicBool::new(false),
            peak: AtomicF32::new(0.0),
        }
    }
}

/// Ensemble des réglages et mesures partagés entre l'interface et le fil audio.
#[derive(Debug)]
pub struct MixerControls {
    pub tracks: Vec<TrackControl>,
    pub master_gain: AtomicF32,
    pub master_peak: [AtomicF32; 2],
}

impl MixerControls {
    pub fn new(tracks: usize) -> Self {
        Self {
            tracks: (0..tracks).map(|_| TrackControl::default()).collect(),
            master_gain: AtomicF32::new(1.0),
            master_peak: [AtomicF32::new(0.0), AtomicF32::new(0.0)],
        }
    }
}

/// Convertit des décibels en gain linéaire (-inf dB pour -144 dB et moins).
pub fn db_to_gain(db: f32) -> f32 {
    if db <= -144.0 {
        0.0
    } else {
        10f32.powf(db / 20.0)
    }
}

/// Convertit un niveau linéaire en dBFS.
pub fn gain_to_db(g: f32) -> f32 {
    if g <= 0.0 {
        f32::NEG_INFINITY
    } else {
        20.0 * g.log10()
    }
}

/// Gains gauche/droite d'un panoramique à puissance constante (-3 dB au centre).
pub fn pan_gains(pan: f32) -> (f32, f32) {
    let angle = (pan.clamp(-1.0, 1.0) + 1.0) * std::f32::consts::FRAC_PI_4;
    (angle.cos(), angle.sin())
}

/// Mixe `frames` images entrelacées sur `channels` pistes vers une sortie
/// stéréo entrelacée (`out` doit contenir `frames * 2` échantillons).
pub fn mix(input: &[f32], channels: usize, controls: &MixerControls, out: &mut [f32]) {
    let frames = out.len() / 2;
    debug_assert!(input.len() >= frames * channels);
    out.fill(0.0);
    let any_solo = controls
        .tracks
        .iter()
        .any(|t| t.solo.load(Ordering::Relaxed));

    for (ch, track) in controls.tracks.iter().enumerate().take(channels) {
        let audible = if any_solo {
            track.solo.load(Ordering::Relaxed)
        } else {
            !track.mute.load(Ordering::Relaxed)
        };
        let gain = track.gain.get();
        let mut peak = 0f32;
        if audible && gain > 0.0 {
            let (gl, gr) = pan_gains(track.pan.get());
            for f in 0..frames {
                let s = input[f * channels + ch] * gain;
                peak = peak.max(s.abs());
                out[f * 2] += s * gl;
                out[f * 2 + 1] += s * gr;
            }
        }
        track.peak.max(peak);
    }

    let master = controls.master_gain.get();
    let (mut pl, mut pr) = (0f32, 0f32);
    for f in 0..frames {
        out[f * 2] *= master;
        out[f * 2 + 1] *= master;
        pl = pl.max(out[f * 2].abs());
        pr = pr.max(out[f * 2 + 1].abs());
    }
    controls.master_peak[0].max(pl);
    controls.master_peak[1].max(pr);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-5
    }

    #[test]
    fn center_pan_is_minus_3db() {
        let (l, r) = pan_gains(0.0);
        assert!(approx(l, r));
        assert!((gain_to_db(l) + 3.0103).abs() < 1e-3);
        assert!(pan_gains(-1.0).1.abs() < 1e-6);
        assert!(pan_gains(1.0).0.abs() < 1e-6);
    }

    #[test]
    fn db_conversions() {
        assert!(approx(db_to_gain(0.0), 1.0));
        assert!(approx(db_to_gain(-6.0206), 0.5));
        assert_eq!(db_to_gain(-200.0), 0.0);
        assert!(approx(gain_to_db(0.5), -6.0206));
    }

    #[test]
    fn mute_solo_gain_and_meters() {
        let c = MixerControls::new(3);
        // 2 images, 3 pistes : piste 0 = 0.5, piste 1 = 0.25, piste 2 = 1.0
        let input = [0.5, 0.25, 1.0, 0.5, 0.25, 1.0];
        let mut out = [0.0; 4];
        c.tracks[0].pan.set(-1.0);
        c.tracks[1].pan.set(1.0);
        c.tracks[2].mute.store(true, Ordering::Relaxed);
        mix(&input, 3, &c, &mut out);
        assert!(approx(out[0], 0.5) && approx(out[1], 0.25));
        assert!(approx(c.tracks[0].peak.take(), 0.5));
        assert_eq!(c.tracks[2].peak.take(), 0.0, "piste muette : pas de crête");
        assert!(approx(c.master_peak[0].take(), 0.5));

        // SOLO de la piste 1 : seule audible, même si la piste 0 n'est pas muette.
        c.tracks[1].solo.store(true, Ordering::Relaxed);
        c.tracks[1].gain.set(db_to_gain(-6.0206));
        mix(&input, 3, &c, &mut out);
        assert!(approx(out[0], 0.0) && approx(out[1], 0.125));

        // Gain master.
        c.master_gain.set(2.0);
        mix(&input, 3, &c, &mut out);
        assert!(approx(out[1], 0.25));
    }

    #[test]
    fn peaks_accumulate_until_taken() {
        let p = AtomicF32::new(0.0);
        p.max(0.3);
        p.max(0.2);
        assert!(approx(p.take(), 0.3));
        assert_eq!(p.take(), 0.0);
    }
}
