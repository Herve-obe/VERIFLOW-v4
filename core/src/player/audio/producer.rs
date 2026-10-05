//! Production du signal stéréo : lecture des fichiers, mixage, mesure de
//! loudness (EBU R128) et rééchantillonnage vers la fréquence de la carte son.
//! Indépendant de la carte son : testable hors temps réel.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

use ebur128::{EbuR128, Mode};
use rubato::audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};
use serde::Serialize;

use super::mixer::{mix, AtomicF32, MixerControls};
use crate::media::wav::{WavInfo, WavReader};
use crate::{Error, Result};

/// Taille des blocs lus dans les fichiers, en images.
pub const CHUNK: usize = 1024;

/// Description d'une piste de la session.
#[derive(Debug, Clone, Serialize)]
pub struct TrackInfo {
    pub name: String,
    pub file: usize,
    pub channel: u16,
}

/// Session audio : un ou plusieurs fichiers lus ensemble (WAV polyphoniques
/// et/ou monos d'une même prise), à la même fréquence d'échantillonnage.
#[derive(Debug, Clone, Serialize)]
pub struct SessionInfo {
    pub files: Vec<WavInfo>,
    pub tracks: Vec<TrackInfo>,
    pub sample_rate: u32,
    pub frames: u64,
    pub duration: f64,
}

/// Mesures partagées avec l'interface.
#[derive(Debug, Default)]
pub struct LoudnessMeters {
    pub momentary: AtomicF32,
    pub short_term: AtomicF32,
    pub integrated: AtomicF32,
}

pub struct Producer {
    readers: Vec<WavReader>,
    info: SessionInfo,
    controls: Arc<MixerControls>,
    meters: Arc<LoudnessMeters>,
    ebu: EbuR128,
    resampler: Option<Fft<f32>>,
    pos: u64,
    multi: Vec<f32>,
    file_buf: Vec<f32>,
    stereo: Vec<f32>,
    resampled: Vec<f32>,
    /// Vrai pendant la lecture : la loudness intégrée n'est cumulée qu'en lecture.
    pub measuring: Arc<AtomicBool>,
    /// Images source depuis la dernière mise à jour des valeurs de loudness.
    since_update: usize,
}

impl Producer {
    /// Ouvre les fichiers ; `output_rate` est la fréquence de la carte son.
    pub fn open(paths: &[PathBuf], output_rate: u32) -> Result<Self> {
        if paths.is_empty() {
            return Err(Error::Unsupported("aucun fichier audio".into()));
        }
        let readers = paths
            .iter()
            .map(|p| WavReader::open(p))
            .collect::<Result<Vec<_>>>()?;
        let sample_rate = readers[0].info().sample_rate;
        if let Some(r) = readers.iter().find(|r| r.info().sample_rate != sample_rate) {
            return Err(Error::Unsupported(format!(
                "{} : fréquence {} Hz différente des autres fichiers ({} Hz)",
                r.info().path.display(),
                r.info().sample_rate,
                sample_rate
            )));
        }
        let mut tracks = Vec::new();
        for (file, r) in readers.iter().enumerate() {
            for channel in 0..r.info().channels {
                tracks.push(TrackInfo {
                    name: r.info().track_name(channel as usize),
                    file,
                    channel,
                });
            }
        }
        let frames = readers.iter().map(|r| r.info().frames).max().unwrap_or(0);
        let info = SessionInfo {
            files: readers.iter().map(|r| r.info().clone()).collect(),
            sample_rate,
            frames,
            duration: frames as f64 / sample_rate as f64,
            tracks,
        };
        let resampler = if output_rate != sample_rate {
            Some(
                Fft::<f32>::new(
                    sample_rate as usize,
                    output_rate as usize,
                    CHUNK,
                    2,
                    FixedSync::Input,
                )
                .map_err(|e| Error::Unsupported(format!("rééchantillonnage : {e}")))?,
            )
        } else {
            None
        };
        let ebu = EbuR128::new(
            2,
            sample_rate,
            Mode::M | Mode::S | Mode::I | Mode::HISTOGRAM,
        )
        .map_err(|e| Error::Unsupported(format!("mesure de loudness : {e:?}")))?;
        let controls = Arc::new(MixerControls::new(info.tracks.len()));
        Ok(Self {
            readers,
            controls,
            meters: Arc::new(LoudnessMeters::default()),
            ebu,
            resampler,
            pos: 0,
            multi: Vec::new(),
            file_buf: Vec::new(),
            stereo: vec![0.0; CHUNK * 2],
            resampled: Vec::new(),
            measuring: Arc::new(AtomicBool::new(true)),
            since_update: usize::MAX,
            info,
        })
    }

    pub fn info(&self) -> &SessionInfo {
        &self.info
    }
    pub fn controls(&self) -> Arc<MixerControls> {
        self.controls.clone()
    }
    pub fn meters(&self) -> Arc<LoudnessMeters> {
        self.meters.clone()
    }
    pub fn position(&self) -> u64 {
        self.pos
    }

    pub fn seek(&mut self, frame: u64) {
        self.pos = frame.min(self.info.frames);
        if let Some(r) = self.resampler.as_mut() {
            r.reset();
        }
    }

    /// Remet la loudness intégrée à zéro.
    pub fn reset_loudness(&mut self) {
        self.ebu.reset();
    }

    /// Produit le bloc stéréo suivant (à la fréquence de sortie) dans `out`.
    /// Renvoie le nombre d'images source consommées (0 en fin de session).
    pub fn next(&mut self, out: &mut Vec<f32>) -> Result<usize> {
        out.clear();
        let remaining = self.info.frames.saturating_sub(self.pos);
        if remaining == 0 {
            return Ok(0);
        }
        let frames = CHUNK.min(remaining as usize);
        let channels = self.info.tracks.len();

        // Assemble toutes les pistes de tous les fichiers en un bloc entrelacé.
        self.multi.clear();
        self.multi.resize(CHUNK * channels, 0.0);
        let mut offset = 0;
        for r in &mut self.readers {
            let ch = r.info().channels as usize;
            let got = r.read(self.pos, frames, &mut self.file_buf)?;
            for f in 0..got {
                let src = &self.file_buf[f * ch..(f + 1) * ch];
                self.multi[f * channels + offset..f * channels + offset + ch].copy_from_slice(src);
            }
            offset += ch;
        }

        self.stereo.resize(CHUNK * 2, 0.0);
        mix(
            &self.multi,
            channels,
            &self.controls,
            &mut self.stereo[..frames * 2],
        );
        // Bloc partiel en fin de fichier : complété par du silence.
        self.stereo[frames * 2..].fill(0.0);

        if self.measuring.load(Ordering::Relaxed) {
            let _ = self.ebu.add_frames_f32(&self.stereo[..frames * 2]);
            self.since_update = self.since_update.saturating_add(frames);
        }
        // Valeurs de loudness recalculées 10 fois par seconde (suffisant pour
        // l'affichage) ou en fin de session.
        let end = self.pos + frames as u64 >= self.info.frames;
        if self.since_update >= self.info.sample_rate as usize / 10
            || (end && self.since_update > 0)
        {
            self.since_update = 0;
            let lufs = |r: std::result::Result<f64, ebur128::Error>| {
                r.map(|v| v as f32).unwrap_or(f32::NEG_INFINITY)
            };
            self.meters
                .momentary
                .set(lufs(self.ebu.loudness_momentary()));
            self.meters
                .short_term
                .set(lufs(self.ebu.loudness_shortterm()));
            self.meters.integrated.set(lufs(self.ebu.loudness_global()));
        }
        self.pos += frames as u64;

        match self.resampler.as_mut() {
            None => out.extend_from_slice(&self.stereo[..frames * 2]),
            Some(rs) => {
                self.resampled.resize(rs.output_frames_max() * 2, 0.0);
                let input = InterleavedSlice::new(&self.stereo, 2, CHUNK)
                    .map_err(|e| Error::Unsupported(e.to_string()))?;
                let cap = self.resampled.len() / 2;
                let mut output = InterleavedSlice::new_mut(&mut self.resampled, 2, cap)
                    .map_err(|e| Error::Unsupported(e.to_string()))?;
                let (_, written) = rs
                    .process_into_buffer(&input, &mut output, None)
                    .map_err(|e| Error::Unsupported(e.to_string()))?;
                out.extend_from_slice(&self.resampled[..written * 2]);
            }
        }
        Ok(frames)
    }
}

/// Position de lecture partagée entre le fil de lecture et la carte son.
#[derive(Debug, Default)]
pub struct Position {
    /// Image source de référence (dernier saut).
    pub base: AtomicU64,
    /// Images de sortie jouées depuis la référence.
    pub played: AtomicU64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::media::wav::write_test_wav;
    use crate::player::audio::mixer::gain_to_db;

    fn session(dir: &std::path::Path, rate: u32, frames: u64) -> Vec<PathBuf> {
        let poly = dir.join("poly.wav");
        let ixml = "<BWFXML><TRACK_LIST><TRACK><CHANNEL_INDEX>1</CHANNEL_INDEX><NAME>Perche</NAME></TRACK></TRACK_LIST></BWFXML>";
        // Piste 1 : sinus 1 kHz à -20 dBFS crête ; piste 2 : silence.
        write_test_wav(&poly, 2, rate, 24, false, frames, Some(ixml), |n, c| {
            if c == 0 {
                0.1 * (2.0 * std::f64::consts::PI * 1000.0 * n as f64 / rate as f64).sin()
            } else {
                0.0
            }
        })
        .unwrap();
        let mono = dir.join("mono.wav");
        write_test_wav(&mono, 1, rate, 32, true, frames / 2, None, |_, _| 0.05).unwrap();
        vec![poly, mono]
    }

    #[test]
    fn mixes_poly_and_mono_files() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Producer::open(&session(dir.path(), 48_000, 48_000), 48_000).unwrap();
        let names: Vec<_> = p.info().tracks.iter().map(|t| t.name.as_str()).collect();
        assert_eq!(names, ["Perche", "Piste 2", "Piste 1"]);
        assert_eq!(p.info().frames, 48_000);

        let mut out = Vec::new();
        let mut total = 0;
        while p.next(&mut out).unwrap() > 0 {
            total += out.len() / 2;
        }
        assert_eq!(total, 48_000);
        // Crête de la piste 1 : 0.1 (-20 dBFS).
        let peak = gain_to_db(p.controls().tracks[0].peak.take());
        assert!((peak + 20.0).abs() < 0.1, "crête {peak}");
        // Sinus 1 kHz -20 dBFS crête, panoramique centré (-3 dB par canal) :
        // -26 dBFS RMS par canal, +3 dB pour la somme des deux canaux, soit -23 LUFS.
        let i = p.meters().integrated.get();
        assert!((-23.5..-22.5).contains(&i), "loudness {i}");
    }

    #[test]
    fn resamples_192k_to_48k() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Producer::open(&session(dir.path(), 192_000, 192_000), 48_000).unwrap();
        let mut out = Vec::new();
        let mut total = 0;
        while p.next(&mut out).unwrap() > 0 {
            total += out.len() / 2;
        }
        // 1 s à 192 kHz donne 48 000 images, plus le complément du dernier bloc.
        assert!((47_000..=48_000 + CHUNK).contains(&total), "{total} images");
    }

    #[test]
    fn rejects_mixed_sample_rates() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        write_test_wav(&a, 1, 48_000, 16, false, 10, None, |_, _| 0.0).unwrap();
        write_test_wav(&b, 1, 96_000, 16, false, 10, None, |_, _| 0.0).unwrap();
        assert!(Producer::open(&[a, b], 48_000).is_err());
    }

    #[test]
    fn seek_moves_position() {
        let dir = tempfile::tempdir().unwrap();
        let mut p = Producer::open(&session(dir.path(), 48_000, 48_000), 48_000).unwrap();
        p.seek(47_500);
        let mut out = Vec::new();
        assert_eq!(p.next(&mut out).unwrap(), 500);
        assert_eq!(p.next(&mut out).unwrap(), 0);
    }
}
