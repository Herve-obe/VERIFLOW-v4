//! Sortie sur la carte son (WASAPI et ASIO sous Windows, CoreAudio, ALSA/JACK) via cpal.
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
use serde::{Deserialize, Serialize};

use super::mixer::MixerControls;
use super::producer::{LoudnessMeters, Producer, SessionInfo};
use crate::{Error, Result};

/// Sortie audio disponible sur le poste.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct OutputDevice {
    /// Identifiant stable « pilote:périphérique » (à conserver dans les réglages).
    pub id: String,
    /// Pilote : wasapi, asio, coreaudio, alsa, jack...
    pub host: String,
    pub name: String,
    /// Sortie par défaut de ce pilote.
    pub default: bool,
    /// Nombre maximal de canaux de sortie.
    pub channels: u16,
}

/// Sortie demandée par l'utilisateur.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct OutputChoice {
    /// Identifiant renvoyé par [`output_devices`] ; aucun = sortie par défaut du système.
    #[serde(default)]
    pub device: Option<String>,
    /// Premier canal de la paire stéréo (0 = sorties 1-2, 2 = sorties 3-4...).
    #[serde(default)]
    pub first_channel: u16,
}

/// Liste les sorties audio de tous les pilotes disponibles.
pub fn output_devices() -> Vec<OutputDevice> {
    let mut out = Vec::new();
    for host_id in cpal::available_hosts() {
        let Ok(host) = cpal::host_from_id(host_id) else {
            continue;
        };
        let default = host.default_output_device().and_then(|d| d.id().ok());
        let Ok(devices) = host.output_devices() else {
            continue;
        };
        for d in devices {
            let Ok(id) = d.id() else {
                continue;
            };
            let channels = d
                .supported_output_configs()
                .ok()
                .and_then(|c| c.map(|r| r.channels()).max())
                .unwrap_or(2);
            out.push(OutputDevice {
                name: d
                    .description()
                    .map(|x| x.name().to_owned())
                    .unwrap_or_else(|_| id.id().to_owned()),
                host: host_id.to_string(),
                default: default.as_ref() == Some(&id),
                channels,
                id: id.to_string(),
            });
        }
    }
    out
}

/// Retrouve la sortie demandée ; à défaut, la sortie par défaut du système.
fn find_device(choice: &OutputChoice) -> Result<(cpal::Device, String)> {
    if let Some(wanted) = &choice.device {
        let found = wanted.parse::<cpal::DeviceId>().ok().and_then(|id| {
            cpal::host_from_id(id.host())
                .ok()?
                .device_by_id(&id)
                .map(|d| (d, id.host().to_string()))
        });
        if let Some(f) = found {
            return Ok(f);
        }
    }
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| Error::Audio("aucune sortie audio disponible".into()))?;
    Ok((device, host.id().to_string()))
}

/// Sortie audio choisie.
#[derive(Debug, Clone, Serialize)]
pub struct OutputInfo {
    pub device: String,
    /// Identifiant de la sortie réellement ouverte.
    pub id: String,
    pub host: String,
    /// Premier canal utilisé (0 = sorties 1-2).
    pub first_channel: u16,
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
fn choose_config(
    device: &cpal::Device,
    wanted_rate: u32,
    min_channels: u16,
) -> Result<cpal::SupportedStreamConfig> {
    let default = device.default_output_config().map_err(err)?;
    if let Ok(configs) = device.supported_output_configs() {
        let mut best: Option<cpal::SupportedStreamConfig> = None;
        for range in configs {
            if range.channels() < min_channels
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
    if default.channels() < min_channels {
        return Err(Error::Audio(format!(
            "la sortie n'a que {} canaux : paire {}-{} indisponible",
            default.channels(),
            min_channels - 1,
            min_channels
        )));
    }
    Ok(default)
}

impl AudioEngine {
    /// Ouvre une session sur la sortie audio par défaut du système.
    pub fn open(paths: &[PathBuf]) -> Result<Self> {
        Self::open_with(paths, &OutputChoice::default())
    }

    /// Ouvre une session sur la sortie choisie.
    pub fn open_with(paths: &[PathBuf], choice: &OutputChoice) -> Result<Self> {
        // Lecture préalable pour connaître la fréquence des fichiers.
        let file_rate = Producer::open(paths, 48_000)?.info().sample_rate;

        let (device, host) = find_device(choice)?;
        let device_name = device
            .description()
            .map(|d| d.name().to_owned())
            .unwrap_or_else(|_| "Sortie par défaut".into());
        let device_id = device.id().map(|i| i.to_string()).unwrap_or_default();
        let first_channel = choice.first_channel;
        let supported = choose_config(&device, file_rate, first_channel + 2)?;
        let config: StreamConfig = supported.config();
        let output = OutputInfo {
            device: device_name,
            id: device_id,
            host,
            first_channel,
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
                    cpal::SampleFormat::F32 => {
                        build::<f32>(&device, &config, rx, stream_shared, first_channel)
                    }
                    cpal::SampleFormat::I16 => {
                        build::<i16>(&device, &config, rx, stream_shared, first_channel)
                    }
                    cpal::SampleFormat::I32 => {
                        build::<i32>(&device, &config, rx, stream_shared, first_channel)
                    }
                    cpal::SampleFormat::U16 => {
                        build::<u16>(&device, &config, rx, stream_shared, first_channel)
                    }
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
    first_channel: u16,
) -> Result<cpal::Stream>
where
    T: SizedSample + FromSample<f32>,
{
    let channels = config.channels as usize;
    let first = first_channel as usize;
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
                        *s = T::from_sample(if i == first {
                            l
                        } else if i == first + 1 {
                            r
                        } else {
                            0.0
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
