//! Sortie sur la carte son (WASAPI, CoreAudio, ALSA) via cpal.
//!
//! Architecture temps réel :
//! - un fil de lecture lit les fichiers, mixe et rééchantillonne
//!   (`Producer`), puis dépose le signal stéréo dans un tampon circulaire
//!   sans verrou (environ 50 ms) ;
//! - la fonction de rappel de la carte son ne fait que vider ce tampon :
//!   aucune allocation, aucun verrou, aucune lecture disque.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SizedSample, StreamConfig};
use rtrb::{Consumer, RingBuffer};
use serde::Serialize;

use super::mixer::MixerControls;
use super::producer::{LoudnessMeters, Producer, SessionInfo};
use crate::{Error, Result};

/// Sortie audio choisie.
#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    pub device: String,
    pub sample_rate: u32,
    pub channels: u16,
    /// Vrai si la carte ne gère pas la fréquence des fichiers (conversion à la volée).
    pub resampling: bool,
}

/// État partagé entre l'interface, le fil de lecture et la carte son.
#[derive(Debug, Default)]
struct Shared {
    playing: AtomicBool,
    flush: AtomicBool,
    ended: AtomicBool,
    base: AtomicU64,
    played: AtomicU64,
}

enum Command {
    Seek(u64),
    ResetLoudness,
    Quit,
}

/// Moteur audio d'une session ouverte.
pub struct AudioEngine {
    info: SessionInfo,
    output: OutputInfo,
    controls: Arc<MixerControls>,
    meters: Arc<LoudnessMeters>,
    shared: Arc<Shared>,
    commands: Sender<Command>,
    stream_quit: Sender<()>,
    ratio: f64,
}

fn err(e: impl std::fmt::Display) -> Error {
    Error::Audio(e.to_string())
}

/// Choisit la configuration de sortie : la fréquence des fichiers si la carte
/// la gère (lecture native jusqu'à 192 kHz), sinon la configuration par défaut.
fn choose_config(device: &cpal::Device, wanted_rate: u32) -> Result<cpal::SupportedStreamConfig> {
    let default = device.default_output_config().map_err(err)?;
    if let Ok(configs) = device.supported_output_configs() {
        let mut best: Option<cpal::SupportedStreamConfig> = None;
        for range in configs {
            if range.channels() < 2
                || wanted_rate < range.min_sample_rate()
                || wanted_rate > range.max_sample_rate()
            {
                continue;
            }
            let candidate = range.with_sample_rate(wanted_rate);
            // Préférence : flottant 32 bits, puis le format par défaut.
            let score = |c: &cpal::SupportedStreamConfig| {
                (c.sample_format() == cpal::SampleFormat::F32) as u8 * 2
                    + (c.sample_format() == default.sample_format()) as u8
            };
            if best.as_ref().is_none_or(|b| score(&candidate) > score(b)) {
                best = Some(candidate);
            }
        }
        if let Some(b) = best {
            return Ok(b);
        }
    }
    Ok(default)
}

impl AudioEngine {
    /// Ouvre une session sur la sortie audio par défaut du système.
    pub fn open(paths: &[PathBuf]) -> Result<Self> {
        // Lecture préalable pour connaître la fréquence des fichiers.
        let file_rate = Producer::open(paths, 48_000)?.info().sample_rate;

        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| Error::Audio("aucune sortie audio disponible".into()))?;
        let device_name = device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| "Sortie par défaut".into());
        let supported = choose_config(&device, file_rate)?;
        let config: StreamConfig = supported.config();
        let output = OutputInfo {
            device: device_name,
            sample_rate: config.sample_rate,
            channels: config.channels,
            resampling: config.sample_rate != file_rate,
        };

        let mut producer = Producer::open(paths, config.sample_rate)?;
        let info = producer.info().clone();
        let controls = producer.controls();
        let meters = producer.meters();
        let measuring = producer.measuring.clone();
        let shared = Arc::new(Shared::default());

        // Tampon circulaire stéréo d'environ 50 ms.
        let capacity = ((config.sample_rate as usize / 20) * 2).max(4096);
        let (mut tx, rx) = RingBuffer::<f32>::new(capacity);

        // La carte son est pilotée depuis un fil dédié : le flux cpal n'est pas
        // transférable entre fils sur toutes les plateformes.
        let (ready_tx, ready_rx) = channel::<Result<()>>();
        let (quit_tx, quit_rx) = channel::<()>();
        let stream_shared = shared.clone();
        let format = supported.sample_format();
        thread::Builder::new()
            .name("veriflow-audio-out".into())
            .spawn(move || {
                let stream = match format {
                    cpal::SampleFormat::F32 => build::<f32>(&device, &config, rx, stream_shared),
                    cpal::SampleFormat::I16 => build::<i16>(&device, &config, rx, stream_shared),
                    cpal::SampleFormat::I32 => build::<i32>(&device, &config, rx, stream_shared),
                    cpal::SampleFormat::U16 => build::<u16>(&device, &config, rx, stream_shared),
                    other => Err(Error::Audio(format!("format de sortie {other} non géré"))),
                };
                match stream.and_then(|s| s.play().map_err(err).map(|_| s)) {
                    Ok(stream) => {
                        let _ = ready_tx.send(Ok(()));
                        let _ = quit_rx.recv();
                        drop(stream);
                    }
                    Err(e) => {
                        let _ = ready_tx.send(Err(e));
                    }
                }
            })?;
        ready_rx
            .recv()
            .map_err(|_| Error::Audio("la sortie audio n'a pas démarré".into()))??;

        // Fil de lecture : fichiers -> mixage -> tampon circulaire.
        let (cmd_tx, cmd_rx) = channel();
        let reader_shared = shared.clone();
        thread::Builder::new()
            .name("veriflow-audio-read".into())
            .spawn(move || {
                reader_loop(&mut producer, &mut tx, &cmd_rx, &reader_shared, &measuring)
            })?;

        Ok(Self {
            ratio: info.sample_rate as f64 / output.sample_rate as f64,
            info,
            output,
            controls,
            meters,
            shared,
            commands: cmd_tx,
            stream_quit: quit_tx,
        })
    }

    pub fn info(&self) -> &SessionInfo {
        &self.info
    }
    pub fn output(&self) -> &OutputInfo {
        &self.output
    }
    pub fn controls(&self) -> &MixerControls {
        &self.controls
    }
    pub fn loudness(&self) -> &LoudnessMeters {
        &self.meters
    }

    pub fn play(&self) {
        if self.shared.ended.load(Ordering::Relaxed) {
            self.seek(0);
        }
        self.shared.playing.store(true, Ordering::Relaxed);
    }

    pub fn pause(&self) {
        self.shared.playing.store(false, Ordering::Relaxed);
    }

    /// Arrêt : pause et retour au début.
    pub fn stop(&self) {
        self.pause();
        self.seek(0);
        let _ = self.commands.send(Command::ResetLoudness);
    }

    pub fn is_playing(&self) -> bool {
        self.shared.playing.load(Ordering::Relaxed)
    }

    /// Saut à une position, en images source.
    pub fn seek(&self, frame: u64) {
        let _ = self
            .commands
            .send(Command::Seek(frame.min(self.info.frames)));
    }

    /// Position courante en images source.
    pub fn position(&self) -> u64 {
        let base = self.shared.base.load(Ordering::Relaxed);
        let played = self.shared.played.load(Ordering::Relaxed) as f64 * self.ratio;
        (base + played as u64).min(self.info.frames)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        let _ = self.commands.send(Command::Quit);
        let _ = self.stream_quit.send(());
    }
}

fn reader_loop(
    producer: &mut Producer,
    tx: &mut rtrb::Producer<f32>,
    commands: &Receiver<Command>,
    shared: &Shared,
    measuring: &AtomicBool,
) {
    let mut block: Vec<f32> = Vec::new();
    let mut sent = 0usize;
    loop {
        while let Ok(cmd) = commands.try_recv() {
            match cmd {
                Command::Quit => return,
                Command::ResetLoudness => producer.reset_loudness(),
                Command::Seek(frame) => {
                    producer.seek(frame);
                    block.clear();
                    sent = 0;
                    // Demande à la carte son de vider le tampon, puis attend.
                    shared.flush.store(true, Ordering::Release);
                    while shared.flush.load(Ordering::Acquire) {
                        thread::sleep(Duration::from_millis(1));
                    }
                    shared.base.store(frame, Ordering::Relaxed);
                    shared.played.store(0, Ordering::Relaxed);
                    shared.ended.store(false, Ordering::Relaxed);
                }
            }
        }
        let playing = shared.playing.load(Ordering::Relaxed);
        measuring.store(playing, Ordering::Relaxed);
        if !playing {
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        if sent == block.len() {
            match producer.next(&mut block) {
                Ok(0) | Err(_) => {
                    shared.ended.store(true, Ordering::Relaxed);
                    thread::sleep(Duration::from_millis(5));
                    continue;
                }
                Ok(_) => sent = 0,
            }
        }
        let n = tx.slots().min(block.len() - sent);
        if n == 0 {
            thread::sleep(Duration::from_millis(2));
            continue;
        }
        if let Ok(chunk) = tx.write_chunk_uninit(n) {
            let written = chunk.fill_from_iter(block[sent..sent + n].iter().copied());
            sent += written;
        }
    }
}

fn build<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    mut rx: Consumer<f32>,
    shared: Arc<Shared>,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    device
        .build_output_stream::<T, _, _>(
            *config,
            move |data: &mut [T], _| {
                if shared.flush.load(Ordering::Acquire) {
                    let n = rx.slots();
                    if let Ok(chunk) = rx.read_chunk(n) {
                        chunk.commit_all();
                    }
                    shared.flush.store(false, Ordering::Release);
                }
                let playing = shared.playing.load(Ordering::Relaxed);
                let mut played = 0u64;
                for frame in data.chunks_mut(channels) {
                    let (l, r) = if playing && rx.slots() >= 2 {
                        let l = rx.pop().unwrap_or(0.0);
                        let r = rx.pop().unwrap_or(0.0);
                        played += 1;
                        (l, r)
                    } else {
                        (0.0, 0.0)
                    };
                    for (i, s) in frame.iter_mut().enumerate() {
                        *s = T::from_sample(match i {
                            0 => l,
                            1 => r,
                            _ => 0.0,
                        });
                    }
                }
                shared.played.fetch_add(played, Ordering::Relaxed);
            },
            |e| eprintln!("VERIFLOW audio : {e}"),
            None,
        )
        .map_err(err)
}
