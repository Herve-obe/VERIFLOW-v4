//! Commandes du PLAYER : lecture vidéo image par image et moteur audio multipiste.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tauri::ipc::Response;
use tauri::State;
use veriflow_core::player::audio::engine::{AudioEngine, OutputInfo};
use veriflow_core::player::audio::mixer::{db_to_gain, gain_to_db};
use veriflow_core::player::audio::producer::SessionInfo;
use veriflow_core::player::video::{FrameFormat, VideoClip, VideoPlayer};

/// Taille maximale des images envoyées à l'interface (aperçu).
const PREVIEW_MAX: (u32, u32) = (1280, 720);

/// Lecteurs ouverts, par emplacement : « player » (onglet PLAYER) et
/// « preview » (lecteur rapide de l'onglet MEDIA). Les deux sont indépendants.
#[derive(Default)]
pub struct PlayerState {
    video: Arc<Mutex<HashMap<String, VideoPlayer>>>,
    audio: Mutex<HashMap<String, AudioEngine>>,
}

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

// ---------- Vidéo ----------

#[tauri::command]
pub async fn video_open(
    path: PathBuf,
    slot: String,
    state: State<'_, PlayerState>,
) -> CmdResult<VideoClip> {
    let players = state.video.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let player = VideoPlayer::open(&path, PREVIEW_MAX.0, PREVIEW_MAX.1, FrameFormat::Jpeg)
            .map_err(text)?;
        let clip = player.clip().clone();
        players.lock().map_err(text)?.insert(slot, player);
        Ok(clip)
    })
    .await
    .map_err(text)?
}

/// Renvoie l'image demandée, encodée en JPEG (taille d'affichage du clip).
#[tauri::command]
pub async fn video_frame(
    index: i64,
    slot: String,
    state: State<'_, PlayerState>,
) -> CmdResult<Response> {
    let players = state.video.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let mut guard = players.lock().map_err(text)?;
        let player = guard.get_mut(&slot).ok_or("aucun clip vidéo ouvert")?;
        let frame = player
            .frame(index)
            .map_err(text)?
            .ok_or("image indisponible")?;
        Ok(Response::new(frame.as_ref().clone()))
    })
    .await
    .map_err(text)?
}

#[tauri::command]
pub fn video_close(slot: String, state: State<'_, PlayerState>) -> CmdResult<()> {
    state.video.lock().map_err(text)?.remove(&slot);
    Ok(())
}

// ---------- Audio ----------

#[derive(Serialize)]
pub struct AudioOpened {
    session: SessionInfo,
    output: OutputInfo,
}

#[tauri::command]
pub async fn audio_open(
    paths: Vec<PathBuf>,
    slot: String,
    state: State<'_, PlayerState>,
) -> CmdResult<AudioOpened> {
    // Ferme la session précédente de cet emplacement avant d'ouvrir la carte son.
    state.audio.lock().map_err(text)?.remove(&slot);
    let engine = tauri::async_runtime::spawn_blocking(move || AudioEngine::open(&paths))
        .await
        .map_err(text)?
        .map_err(text)?;
    let opened = AudioOpened {
        session: engine.info().clone(),
        output: engine.output().clone(),
    };
    state.audio.lock().map_err(text)?.insert(slot, engine);
    Ok(opened)
}

#[tauri::command]
pub fn audio_close(slot: String, state: State<'_, PlayerState>) -> CmdResult<()> {
    state.audio.lock().map_err(text)?.remove(&slot);
    Ok(())
}

fn with_audio<T>(
    state: &PlayerState,
    slot: &str,
    f: impl FnOnce(&AudioEngine) -> T,
) -> CmdResult<T> {
    let guard = state.audio.lock().map_err(text)?;
    let engine = guard.get(slot).ok_or("aucune session audio ouverte")?;
    Ok(f(engine))
}

/// Transport : "play", "pause" ou "stop".
#[tauri::command]
pub fn audio_transport(
    action: String,
    slot: String,
    state: State<'_, PlayerState>,
) -> CmdResult<()> {
    with_audio(&state, &slot, |e| match action.as_str() {
        "play" => e.play(),
        "pause" => e.pause(),
        "stop" => e.stop(),
        _ => {}
    })
}

#[tauri::command]
pub fn audio_seek(seconds: f64, slot: String, state: State<'_, PlayerState>) -> CmdResult<()> {
    with_audio(&state, &slot, |e| {
        e.seek((seconds.max(0.0) * e.info().sample_rate as f64) as u64)
    })
}

/// Réglages d'une piste ; `index` = -1 pour le master (seul le gain s'applique).
#[tauri::command]
pub fn audio_track(
    index: i32,
    gain_db: f32,
    pan: f32,
    mute: bool,
    solo: bool,
    slot: String,
    state: State<'_, PlayerState>,
) -> CmdResult<()> {
    with_audio(&state, &slot, |e| {
        let c = e.controls();
        if index < 0 {
            c.master_gain.set(db_to_gain(gain_db));
        } else if let Some(t) = c.tracks.get(index as usize) {
            t.gain.set(db_to_gain(gain_db));
            t.pan.set(pan);
            t.mute.store(mute, Ordering::Relaxed);
            t.solo.store(solo, Ordering::Relaxed);
        }
    })
}

#[derive(Serialize)]
pub struct AudioStatus {
    playing: bool,
    position: f64,
    /// Crêtes depuis le dernier appel, en dBFS (null = silence).
    track_peaks: Vec<Option<f32>>,
    master_peaks: [Option<f32>; 2],
    lufs_momentary: Option<f32>,
    lufs_short_term: Option<f32>,
    lufs_integrated: Option<f32>,
}

fn finite(v: f32) -> Option<f32> {
    v.is_finite().then_some(v)
}

#[tauri::command]
pub fn audio_status(slot: String, state: State<'_, PlayerState>) -> CmdResult<AudioStatus> {
    with_audio(&state, &slot, |e| {
        let c = e.controls();
        let l = e.loudness();
        AudioStatus {
            playing: e.is_playing(),
            position: e.position() as f64 / e.info().sample_rate as f64,
            track_peaks: c
                .tracks
                .iter()
                .map(|t| finite(gain_to_db(t.peak.take())))
                .collect(),
            master_peaks: [
                finite(gain_to_db(c.master_peak[0].take())),
                finite(gain_to_db(c.master_peak[1].take())),
            ],
            lufs_momentary: finite(l.momentary.get()),
            lufs_short_term: finite(l.short_term.get()),
            lufs_integrated: finite(l.integrated.get()),
        }
    })
}
