//! Commandes SYNC : analyse d'un lot (avancement par événements, annulable),
//! ré-affinage d'une paire, formes d'onde, exports.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use veriflow_core::media::catalog::{self, MediaKind};
use veriflow_core::media::probe::probe;
use veriflow_core::sync::correlate::{decode_mono, envelope};
use veriflow_core::sync::export::{self, Item, RewrapOptions};
use veriflow_core::sync::{self, Analysis, Options, SyncFile};

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[derive(Default)]
pub struct SyncState {
    cancel: Mutex<Option<Arc<AtomicBool>>>,
}

impl SyncState {
    fn arm(&self) -> CmdResult<Arc<AtomicBool>> {
        let flag = Arc::new(AtomicBool::new(false));
        *self.cancel.lock().map_err(text)? = Some(flag.clone());
        Ok(flag)
    }
}

#[derive(Serialize)]
pub struct Split {
    videos: Vec<PathBuf>,
    audios: Vec<PathBuf>,
}

/// Répartit les chemins déposés (dossiers parcourus) en vidéos et sons.
#[tauri::command]
pub async fn sync_expand(paths: Vec<PathBuf>) -> CmdResult<Split> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut out = Split {
            videos: Vec::new(),
            audios: Vec::new(),
        };
        let mut push = |p: PathBuf, k: Option<MediaKind>| match k {
            Some(MediaKind::Video) => out.videos.push(p),
            Some(MediaKind::Audio) => out.audios.push(p),
            _ => {}
        };
        for p in paths {
            if p.is_dir() {
                for e in catalog::list(&p, true).map_err(text)? {
                    push(e.path, Some(e.kind));
                }
            } else {
                let k = catalog::kind_of(&p);
                push(p, k);
            }
        }
        Ok(out)
    })
    .await
    .map_err(text)?
}

#[tauri::command]
pub async fn sync_analyze(
    videos: Vec<PathBuf>,
    audios: Vec<PathBuf>,
    options: Options,
    app: AppHandle,
    state: State<'_, SyncState>,
) -> CmdResult<Analysis> {
    let cancel = state.arm()?;
    tauri::async_runtime::spawn_blocking(move || {
        sync::analyze(&videos, &audios, options, &cancel, |e| {
            let _ = app.emit("sync", e);
        })
        .map_err(text)
    })
    .await
    .map_err(text)?
}

#[tauri::command]
pub fn sync_cancel(state: State<'_, SyncState>) -> CmdResult<()> {
    if let Some(c) = state.cancel.lock().map_err(text)?.as_ref() {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[derive(Serialize)]
pub struct Refined {
    offset: f64,
    confidence: f64,
}

/// Recalcule le décalage par la forme d'onde autour d'un décalage donné.
#[tauri::command]
pub async fn sync_refine(
    video: SyncFile,
    audio: SyncFile,
    offset: f64,
    window: f64,
) -> CmdResult<Refined> {
    tauri::async_runtime::spawn_blocking(move || {
        let from = offset.max(0.0);
        let to = (offset + audio.duration).min(video.duration);
        let common = to - from;
        if common < 0.5 {
            return Err("pas de passage commun avec ce décalage".to_owned());
        }
        let length = common.min(10.0);
        let at = from + (common - length) / 2.0;
        let (offset, confidence) =
            sync::refine_at(&video, &audio, offset, window, at, length).map_err(text)?;
        Ok(Refined { offset, confidence })
    })
    .await
    .map_err(text)?
}

#[derive(Serialize)]
pub struct Waves {
    video: Vec<f32>,
    audio: Vec<f32>,
    start: f64,
    length: f64,
}

/// Formes d'onde superposables : le son témoin de la vidéo de `at` pendant
/// `length` secondes, et le son de l'enregistreur au même instant.
#[tauri::command]
pub async fn sync_waveforms(
    video: SyncFile,
    audio: SyncFile,
    offset: f64,
    at: f64,
    length: f64,
    points: usize,
) -> CmdResult<Waves> {
    tauri::async_runtime::spawn_blocking(move || {
        let rate = 8_000;
        let vi = probe(&video.path).map_err(text)?;
        let ai = probe(&audio.path).map_err(text)?;
        let length = length.clamp(0.1, 30.0);
        let at = at.clamp(0.0, (video.duration - length).max(0.0));
        let v = if vi.audio.is_empty() {
            Vec::new()
        } else {
            decode_mono(&video.path, &vi, at, length, rate, video.ltc_channel).map_err(text)?
        };
        // Même instant dans le son : t_son = t_vidéo - décalage (zéros avant son début).
        let a_at = at - offset;
        let pad = ((-a_at).max(0.0) * rate as f64) as usize;
        let mut a = vec![0f32; pad.min((length * rate as f64) as usize)];
        let a_len = (length - (-a_at).max(0.0)).max(0.0);
        if a_len > 0.0 {
            a.extend(
                decode_mono(
                    &audio.path,
                    &ai,
                    a_at.max(0.0),
                    a_len,
                    rate,
                    audio.ltc_channel,
                )
                .map_err(text)?,
            );
        }
        Ok(Waves {
            video: envelope(&v, points),
            audio: envelope(&a, points),
            start: at,
            length,
        })
    })
    .await
    .map_err(text)?
}

/// Paire à exporter.
#[derive(Deserialize)]
pub struct ExportItem {
    video: SyncFile,
    audio: Option<SyncFile>,
    offset: f64,
}

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum ExportNotice {
    Progress {
        index: usize,
        total: usize,
        fraction: f64,
        name: String,
    },
}

#[derive(Serialize)]
pub struct ExportResult {
    video: PathBuf,
    output: Option<PathBuf>,
    error: Option<String>,
}

/// Re-wrap des paires : nouveaux fichiers image + son calé, sans réencodage.
#[tauri::command]
pub async fn sync_rewrap(
    items: Vec<ExportItem>,
    options: RewrapOptions,
    app: AppHandle,
    state: State<'_, SyncState>,
) -> CmdResult<Vec<ExportResult>> {
    let cancel = state.arm()?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut taken = HashSet::new();
        let total = items.len();
        let mut out = Vec::new();
        for (index, it) in items.iter().enumerate() {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let Some(audio) = &it.audio else { continue };
            let name = it.video.name.clone();
            let mut progress = |fraction: f64, _| {
                let _ = app.emit(
                    "sync-export",
                    ExportNotice::Progress {
                        index,
                        total,
                        fraction,
                        name: name.clone(),
                    },
                );
            };
            let r = export::rewrap(
                &it.video,
                audio,
                it.offset,
                &options,
                &mut taken,
                &cancel,
                &mut progress,
            );
            out.push(ExportResult {
                video: it.video.path.clone(),
                output: r.as_ref().ok().cloned(),
                error: r.err().map(text),
            });
        }
        Ok(out)
    })
    .await
    .map_err(text)?
}

/// Timeline FCPXML ou OTIO des paires.
#[tauri::command]
pub fn sync_timeline(
    kind: String,
    title: String,
    items: Vec<ExportItem>,
    path: PathBuf,
) -> CmdResult<()> {
    let list: Vec<Item> = items
        .iter()
        .map(|it| Item {
            video: &it.video,
            audio: it.audio.as_ref().map(|a| (a, it.offset)),
        })
        .collect();
    let content = match kind.as_str() {
        "otio" => export::to_otio(&title, &list),
        _ => export::to_fcpxml(&title, &list),
    };
    std::fs::write(path, content).map_err(text)
}
