//! Commandes OFFLOAD : contrôles, file d'attente de copies, progression en
//! direct (événements), annulation, éjection.

use std::collections::{HashMap, VecDeque};
use std::panic::{self, AssertUnwindSafe};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use veriflow_core::offload::engine::Event;
use veriflow_core::offload::existing::{self, CheckProgress, DestCheck, Known};
use veriflow_core::offload::job::{
    execute, local_date, planned_roots, preflight, render_template, JobResult, OffloadRequest,
    Preflight, TemplateVars,
};
use veriflow_core::offload::scan::scan;
use veriflow_core::offload::schedule::{claim_uses_volume, overlaps, select, Claim, Wait};
use veriflow_core::offload::storage::{self, Volume};
use veriflow_core::project::PreviousOffload;

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

struct Job {
    id: u64,
    request: OffloadRequest,
    eject: bool,
    cancel: Arc<AtomicBool>,
    /// Empreintes établies par la vérification des rushes présents.
    known: Option<Known>,
    /// Ressources occupées pendant la copie (voir `schedule`).
    claim: Claim,
    /// Dernière raison d'attente notifiée (une notification par changement).
    last_wait: Option<Wait>,
}

/// File d'attente et travaux en cours, sous un seul verrou. Le verrou n'est
/// jamais tenu pendant une copie ni un appel système lent.
#[derive(Default)]
struct Sched {
    pending: VecDeque<Job>,
    /// Travaux acceptés dont le claim se calcule encore : id et dossier source
    /// (comptés par le test d'éjection).
    preparing: HashMap<u64, PathBuf>,
    running: HashMap<u64, Claim>,
}

type Cancels = Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>;

/// Verrou repris même empoisonné : l'état reste cohérent (opérations courtes).
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[derive(Clone, Default)]
struct Shared {
    sched: Arc<Mutex<Sched>>,
    cancels: Cancels,
}

pub struct OffloadState {
    shared: Shared,
    next_id: AtomicU64,
    /// Vérification des rushes présents en cours (annulable).
    check_cancel: Mutex<Option<Arc<AtomicBool>>>,
    /// Résultats des vérifications, repris au lancement de la copie.
    checks: Mutex<HashMap<u64, Known>>,
}

/// Événement envoyé à l'interface (canal « offload »).
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Notice {
    Queued {
        job: u64,
        source: String,
    },
    /// Le travail attend (raison : limit, source_busy, root_busy, hdd_busy,
    /// space) ; renvoyé seulement quand la raison change.
    Waiting {
        job: u64,
        reason: Wait,
    },
    /// Travail annulé avant son lancement : retiré de la file.
    Dropped {
        job: u64,
    },
    /// Le travail vient d'être lancé (contrôles en cours) : l'ancienne raison
    /// d'attente ne doit plus s'afficher. `running` suit une fois la source
    /// inventoriée.
    Starting {
        job: u64,
    },
    Running {
        job: u64,
        files: usize,
        total_bytes: u64,
        roots: Vec<PathBuf>,
    },
    Engine {
        job: u64,
        event: Event,
    },
    Done {
        job: u64,
        result: JobResult,
        ejected: Option<Result<(), String>>,
        /// Code stable de l'échec d'éjection (traduction côté interface) :
        /// `source_in_use_eject` ou `eject_not_removable`. Absent si l'échec
        /// vient du système (texte de `ejected`).
        eject_code: Option<&'static str>,
    },
    Failed {
        job: u64,
        error: String,
        /// Code stable pour la traduction : `no_space`, `job_panic`,
        /// `job_spawn`. Absent : seul le texte `error` est affichable.
        error_code: Option<&'static str>,
    },
}

fn failed(job: u64, error: impl Into<String>, error_code: Option<&'static str>) -> Notice {
    Notice::Failed {
        job,
        error: error.into(),
        error_code,
    }
}

impl OffloadState {
    pub fn new() -> Self {
        Self {
            shared: Shared::default(),
            next_id: AtomicU64::new(1),
            check_cancel: Mutex::new(None),
            checks: Mutex::new(HashMap::new()),
        }
    }
}

/// Lance tous les travaux en attente qui peuvent l'être, dans l'ordre
/// d'arrivée : un travail bloqué ne bloque pas les suivants. Appelée à
/// l'ajout d'un travail et à la fin de chacun. Ne pas l'appeler depuis le fil
/// de l'interface (lecture des volumes).
fn dispatch(app: &AppHandle, shared: &Shared) {
    // Espace libre lu avant de prendre le verrou.
    let free_map: HashMap<PathBuf, u64> = storage::volumes()
        .into_iter()
        .map(|v| (v.mount_point, v.available))
        .collect();
    let free = |v: &Path| free_map.get(v).copied();
    let mut launch = Vec::new();
    let mut waiting = Vec::new();
    {
        let mut sched = lock(&shared.sched);
        let pending: Vec<Claim> = sched.pending.iter().map(|j| j.claim.clone()).collect();
        let running: Vec<Claim> = sched.running.values().cloned().collect();
        let (go, waits) = select(&pending, &running, &free);
        for (i, reason) in waits {
            let job = &mut sched.pending[i];
            if job.last_wait != Some(reason) {
                job.last_wait = Some(reason);
                waiting.push((job.id, reason));
            }
        }
        // Retrait de la fin vers le début : les indices restent valables.
        for i in go.into_iter().rev() {
            if let Some(job) = sched.pending.remove(i) {
                sched.running.insert(job.id, job.claim.clone());
                launch.push(job);
            }
        }
        launch.reverse();
    }
    for (job, reason) in waiting {
        let _ = app.emit("offload", Notice::Waiting { job, reason });
    }
    for job in launch {
        let id = job.id;
        let _ = app.emit("offload", Notice::Starting { job: id });
        let (app2, shared2) = (app.clone(), shared.clone());
        let spawned = thread::Builder::new()
            .name(format!("veriflow-offload-{id}"))
            .spawn(move || {
                let outcome =
                    panic::catch_unwind(AssertUnwindSafe(|| run_job(&app2, &shared2, job)));
                if outcome.is_err() {
                    let _ = app2.emit(
                        "offload",
                        failed(id, "arrêt inattendu du travail", Some("job_panic")),
                    );
                }
                finish(&app2, &shared2, id);
            });
        if let Err(e) = spawned {
            let _ = app.emit("offload", failed(id, e.to_string(), Some("job_spawn")));
            finish(app, shared, id);
        }
    }
}

/// Libère les ressources d'un travail terminé (même en erreur) et relance
/// l'ordonnancement.
fn finish(app: &AppHandle, shared: &Shared, id: u64) {
    lock(&shared.cancels).remove(&id);
    lock(&shared.sched).running.remove(&id);
    dispatch(app, shared);
}

/// Ressources d'une demande : volumes, disques durs, octets à écrire. Lent
/// (lecture des volumes) : hors du fil de l'interface. `total_bytes` vient du
/// préflight de l'interface ; absent, la source est inventoriée.
fn claim_for(request: &OffloadRequest, total_bytes: Option<u64>) -> Claim {
    let source = veriflow_core::absolute_path(&request.source);
    let volume_or = |p: &Path| {
        storage::volume_of(p)
            .map(|v| v.mount_point)
            .unwrap_or_else(|| p.to_path_buf())
    };
    // Inventaire en échec : le travail échouera de lui-même au lancement.
    let bytes = total_bytes.unwrap_or_else(|| scan(&source).map(|i| i.total_bytes).unwrap_or(0));
    Claim {
        source_volume: volume_or(&source),
        source_hdd: storage::is_hdd(&source),
        roots: planned_roots(request),
        dest_volumes: request
            .destinations
            .iter()
            .map(|d| {
                let d = veriflow_core::absolute_path(d);
                (volume_or(&d), storage::is_hdd(&d), bytes)
            })
            .collect(),
        source,
    }
}

fn project_name(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().ok()?;
    guard.as_ref().and_then(|p| p.info().ok()).map(|i| i.name)
}

fn run_job(app: &AppHandle, shared: &Shared, job: Job) {
    let send = |n: Notice| {
        let _ = app.emit("offload", n);
    };
    let pre = match preflight(&job.request) {
        Ok(p) => p,
        Err(e) => {
            return send(failed(job.id, e.to_string(), None));
        }
    };
    // L'espace a pu manquer entre le préflight de l'interface et le lancement
    // (autre copie terminée entre-temps, après une attente).
    if pre.missing_space.iter().any(Option::is_some) {
        return send(failed(
            job.id,
            "espace insuffisant sur une destination",
            Some("no_space"),
        ));
    }
    send(Notice::Running {
        job: job.id,
        files: pre.files,
        total_bytes: pre.total_bytes,
        roots: pre.roots.clone(),
    });
    let project = project_name(app);
    let result = execute(
        &job.request,
        pre,
        &job.cancel,
        |event| send(Notice::Engine { job: job.id, event }),
        project.as_deref(),
        job.known,
    );
    match result {
        Ok(result) => {
            // Historique dans le projet ouvert (détection des cartes déjà copiées).
            if let Ok(guard) = app.state::<AppState>().project.lock() {
                if let Some(p) = guard.as_ref() {
                    let _ = p.record_offload(&result, &job.request.source);
                }
            }
            let success = !result.summary.cancelled && result.summary.failed_files == 0;
            // Éjection seulement quand plus aucun autre travail (en cours ou en
            // attente) n'utilise ce volume (source, destination ou dossier final).
            let mut eject_code = None;
            let ejected = (job.eject && success).then(|| {
                let volume = job.claim.source_volume.clone();
                let in_use = {
                    let sched = lock(&shared.sched);
                    sched.preparing.values().any(|p| overlaps(p, &volume))
                        || sched
                            .running
                            .iter()
                            .filter(|(id, _)| **id != job.id)
                            .map(|(_, c)| c)
                            .chain(sched.pending.iter().map(|j| &j.claim))
                            .any(|c| claim_uses_volume(c, &volume))
                };
                if in_use {
                    eject_code = Some("source_in_use_eject");
                    return Err(
                        "source encore utilisée par un autre travail : pas d'éjection".to_owned(),
                    );
                }
                storage::volume_of(&job.request.source)
                    .filter(|v| v.removable)
                    .ok_or_else(|| {
                        eject_code = Some("eject_not_removable");
                        "source non amovible : pas d'éjection".to_owned()
                    })
                    .and_then(|v| storage::eject(&v.mount_point).map_err(text))
            });
            send(Notice::Done {
                job: job.id,
                result,
                ejected,
                eject_code,
            });
        }
        Err(e) => send(failed(job.id, e.to_string(), None)),
    }
}

/// Chemin relatif produit par le modèle d'arborescence, pour l'aperçu sous le
/// champ (date du jour, ou date imposée d'une reprise).
#[tauri::command]
pub fn offload_template_preview(
    template: String,
    vars: TemplateVars,
    card: String,
    date: Option<String>,
) -> String {
    render_template(&template, &card, &vars, &date.unwrap_or_else(local_date))
        .display()
        .to_string()
}

#[tauri::command]
pub fn offload_volumes() -> Vec<Volume> {
    storage::volumes()
}

/// État de l'alimentation du poste (`None` : poste fixe ou inconnu).
#[tauri::command]
pub async fn power_status() -> Option<veriflow_core::battery::PowerStatus> {
    tauri::async_runtime::spawn_blocking(veriflow_core::battery::power_status)
        .await
        .ok()
        .flatten()
}

/// Réévalue l'avertissement batterie au moment de lancer la copie : le
/// préflight a pu être fait sur secteur.
#[tauri::command]
pub async fn battery_warning(
    estimated_secs: u64,
) -> Option<veriflow_core::battery::BatteryWarning> {
    let status = tauri::async_runtime::spawn_blocking(veriflow_core::battery::power_status)
        .await
        .ok()
        .flatten();
    veriflow_core::battery::assess(status.as_ref(), estimated_secs)
}

#[derive(Serialize)]
pub struct PreflightView {
    #[serde(flatten)]
    pre: Preflight,
    /// Destinations sur disque dur mécanique (copie plus lente).
    hdd: Vec<bool>,
    source_hdd: bool,
    /// Durée estimée de la copie, en secondes.
    estimated_secs: u64,
    /// Avertissement si le poste est sur batterie.
    battery: Option<veriflow_core::battery::BatteryWarning>,
    /// Copies déjà enregistrées dans le projet pour cette carte.
    previous: Vec<PreviousOffload>,
}

#[tauri::command]
pub async fn offload_preflight(
    mut request: OffloadRequest,
    app: AppHandle,
) -> CmdResult<PreflightView> {
    // Copies précédentes du projet : une carte réutilisée sans formatage y
    // est retrouvée (plans du lundi déjà copiés quand elle revient mardi).
    request.history = {
        let state = app.state::<AppState>();
        let guard = state.project.lock().map_err(text)?;
        guard
            .as_ref()
            .and_then(|p| p.offload_roots().ok())
            .unwrap_or_default()
            .into_iter()
            .map(std::path::PathBuf::from)
            .collect()
    };
    let pre =
        tauri::async_runtime::spawn_blocking(move || preflight(&request).map(|p| (p, request)))
            .await
            .map_err(text)?
            .map_err(text)?;
    let (pre, request) = pre;
    let previous = {
        let state = app.state::<AppState>();
        let guard = state.project.lock().map_err(text)?;
        guard
            .as_ref()
            .and_then(|p| p.previous_offloads(&pre.fingerprint).ok())
            .unwrap_or_default()
    };
    let hdd: Vec<bool> = request
        .destinations
        .iter()
        .map(|d| storage::is_hdd(d))
        .collect();
    let source_hdd = storage::is_hdd(&request.source);
    let estimated_secs = veriflow_core::battery::estimate_secs(
        pre.total_bytes,
        request.destinations.len(),
        source_hdd || hdd.iter().any(|h| *h),
    );
    // Une lecture de batterie en échec ne fait jamais échouer le préflight.
    let status = tauri::async_runtime::spawn_blocking(veriflow_core::battery::power_status)
        .await
        .ok()
        .flatten();
    let battery = veriflow_core::battery::assess(status.as_ref(), estimated_secs);
    Ok(PreflightView {
        hdd,
        source_hdd,
        estimated_secs,
        battery,
        previous,
        pre,
    })
}

#[derive(Serialize)]
pub struct CheckView {
    /// Numéro à rappeler au lancement de la copie (`offload_start`).
    id: u64,
    roots: Vec<PathBuf>,
    destinations: Vec<DestCheck>,
}

/// Vérifie les empreintes des rushes déjà présents dans les destinations.
/// Avancement envoyé sur le canal « offload-check ».
#[tauri::command]
pub async fn offload_check_existing(
    request: OffloadRequest,
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<CheckView> {
    let cancel = Arc::new(AtomicBool::new(false));
    *state.check_cancel.lock().map_err(text)? = Some(cancel.clone());
    let emitter = app.clone();
    let (view, known) = tauri::async_runtime::spawn_blocking(move || {
        let pre = preflight(&request).map_err(text)?;
        let inv = pre.inventory.as_ref().ok_or("inventaire absent")?;
        let (destinations, known) = existing::verify(
            inv,
            &pre.roots,
            &request.algorithms,
            &cancel,
            |p: CheckProgress| {
                let _ = emitter.emit("offload-check", p);
            },
        )
        .map_err(text)?;
        Ok::<_, String>((
            CheckView {
                id: 0,
                roots: pre.roots.clone(),
                destinations,
            },
            known,
        ))
    })
    .await
    .map_err(text)??;
    *state.check_cancel.lock().map_err(text)? = None;
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    state.checks.lock().map_err(text)?.insert(id, known);
    Ok(CheckView { id, ..view })
}

#[tauri::command]
pub fn offload_check_cancel(state: State<'_, OffloadState>) -> CmdResult<()> {
    if let Some(c) = state.check_cancel.lock().map_err(text)?.as_ref() {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}

/// Ajoute une copie à la file d'attente ; renvoie son numéro. `check` :
/// vérification préalable des rushes présents (mode « compléter »). La copie
/// démarre dès que ses ressources sont libres (voir `schedule`). `total_bytes` :
/// taille de la source donnée par le préflight (sinon la source est parcourue).
#[tauri::command]
pub async fn offload_start(
    mut request: OffloadRequest,
    eject: bool,
    check: Option<u64>,
    total_bytes: Option<u64>,
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<u64> {
    let known = match check {
        Some(c) => state.checks.lock().map_err(text)?.remove(&c),
        None => None,
    };
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let cancel = Arc::new(AtomicBool::new(false));
    lock(&state.shared.cancels).insert(id, cancel.clone());
    // Date figée maintenant : minuit passé pendant l'attente ne change pas
    // le dossier final.
    request.date.get_or_insert_with(local_date);
    let source = request.source.display().to_string();
    let _ = app.emit("offload", Notice::Queued { job: id, source });
    let shared = state.shared.clone();
    lock(&shared.sched)
        .preparing
        .insert(id, veriflow_core::absolute_path(&request.source));
    let (worker, worker_app) = (shared.clone(), app.clone());
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        let (shared, app) = (worker, worker_app);
        let claim = claim_for(&request, total_bytes);
        // Drapeau lu sous le verrou, comme `offload_cancel` : une annulation
        // ne peut pas passer entre la lecture et la mise en file.
        let mut sched = lock(&shared.sched);
        sched.preparing.remove(&id);
        if cancel.load(Ordering::Relaxed) {
            drop(sched);
            lock(&shared.cancels).remove(&id);
            let _ = app.emit("offload", Notice::Dropped { job: id });
            return;
        }
        sched.pending.push_back(Job {
            id,
            request,
            eject,
            cancel,
            known,
            claim,
            last_wait: None,
        });
        drop(sched);
        dispatch(&app, &shared);
    })
    .await;
    if let Err(e) = prepared {
        // Panique du calcul : le travail n'existe pas, rien ne doit rester.
        lock(&shared.sched).preparing.remove(&id);
        lock(&shared.cancels).remove(&id);
        let _ = app.emit("offload", failed(id, text(&e), Some("job_spawn")));
        return Err(text(e));
    }
    Ok(id)
}

/// Annule un travail : retiré de la file s'il n'a pas démarré, sinon arrêté.
#[tauri::command]
pub fn offload_cancel(job: u64, app: AppHandle, state: State<'_, OffloadState>) -> CmdResult<()> {
    // Verrou de la file tenu pendant le drapeau : voir `offload_start`
    // (ordre des verrous : file, puis annulations).
    let mut sched = lock(&state.shared.sched);
    let dropped = sched
        .pending
        .iter()
        .position(|j| j.id == job)
        .and_then(|i| sched.pending.remove(i));
    if dropped.is_some() {
        lock(&state.shared.cancels).remove(&job);
        drop(sched);
        let _ = app.emit("offload", Notice::Dropped { job });
    } else if let Some(c) = lock(&state.shared.cancels).get(&job) {
        c.store(true, Ordering::Relaxed);
    }
    Ok(())
}

#[tauri::command]
pub fn offload_eject(mount_point: PathBuf) -> CmdResult<()> {
    storage::eject(&mount_point).map_err(text)
}

/// Ouvre un fichier (rapport) ou un dossier avec l'application du système.
#[tauri::command]
pub fn reveal(path: PathBuf) -> CmdResult<()> {
    // Chemin absolu aux séparateurs natifs : l'Explorateur Windows ouvre
    // « Documents » sans message d'erreur s'il ne comprend pas le chemin.
    let path = veriflow_core::absolute_path(&path);
    if !path.exists() {
        return Err(format!("{} : introuvable", path.display()));
    }
    open_with_system(&path).map_err(text)
}

fn open_with_system(path: &Path) -> std::io::Result<()> {
    let mut cmd = if cfg!(target_os = "macos") {
        std::process::Command::new("open")
    } else if cfg!(windows) {
        let mut c = std::process::Command::new("explorer");
        c.arg(path);
        c.spawn()?;
        return Ok(());
    } else {
        std::process::Command::new("xdg-open")
    };
    cmd.arg(path).spawn().map(|_| ())
}
