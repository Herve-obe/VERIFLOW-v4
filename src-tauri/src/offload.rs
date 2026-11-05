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
use veriflow_core::offload::cascade::{self, PendingCascade};
use veriflow_core::offload::engine::Event;
use veriflow_core::offload::existing::{self, CheckProgress, DestCheck, Known};
use veriflow_core::offload::job::{
    execute, local_date, planned_roots, preflight, render_template, split_cascade, JobResult,
    OffloadRequest, Preflight, TemplateVars,
};
use veriflow_core::offload::mhl_read::{self, MhlInfo};
use veriflow_core::offload::mhl_verify::{verify_root, VerifySummary};
use veriflow_core::offload::report::{write_verify_reports, ReportFiles, ReportInfo, VerifyData};
use veriflow_core::offload::scan::scan;
use veriflow_core::offload::schedule::{claim_uses_volume, overlaps, select, Claim, Wait};
use veriflow_core::offload::storage::{self, Volume};
use veriflow_core::project::PreviousOffload;

use crate::AppState;

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// Genre de travail : copie (étape 1), étape 2 d'une copie en cascade, ou
/// revérification d'un disque par son historique ASC MHL.
enum JobKind {
    Offload {
        request: Box<OffloadRequest>,
        eject: bool,
        /// Empreintes établies par la vérification des rushes présents.
        known: Option<Known>,
    },
    Cascade {
        pending: Box<PendingCascade>,
    },
    Verify {
        root: PathBuf,
        operator: Option<String>,
    },
}

struct Job {
    id: u64,
    kind: JobKind,
    cancel: Arc<AtomicBool>,
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
    /// Étapes 2 en file ou en cours : numéro du travail et clé de l'étape 2
    /// (une même étape 2 n'est jamais en double).
    cascades: Arc<Mutex<HashMap<u64, String>>>,
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
        /// 1 (copie de la carte) ou 2 (étape 2 en cascade).
        stage: u8,
        /// Étape 2 : numéro du travail de l'étape 1 qui l'a mise en file
        /// (absent après un redémarrage, quand elle est reprise à la main).
        cascade_of: Option<u64>,
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
        /// `offload` ou `verify` (le champ `kind` est pris par l'étiquette de
        /// l'événement).
        job_kind: JobType,
        /// Vérification : 0, le total réel arrive avec les événements Progress.
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
        job_kind: JobType,
        /// Absent pour une vérification.
        #[serde(skip_serializing_if = "Option::is_none")]
        result: Option<Box<JobResult>>,
        /// Vérification : compteurs, chaîne et écarts (`VerifySummary::for_ui`,
        /// sans les fichiers conformes) et fichiers de rapport (absents si annulée).
        #[serde(skip_serializing_if = "Option::is_none")]
        verify: Option<Box<VerifySummary>>,
        #[serde(skip_serializing_if = "Option::is_none")]
        reports: Option<ReportFiles>,
        /// Vérification : raison de l'échec d'écriture des rapports.
        #[serde(skip_serializing_if = "Option::is_none")]
        report_error: Option<String>,
        ejected: Option<Result<(), String>>,
        /// Étape 1 : la carte peut être éjectée (toutes les destinations de
        /// l'étape 1 vérifiées). Toujours faux pour une étape 2 ou une vérification.
        card_free: bool,
        /// Étape 1 : numéro du travail de l'étape 2 mis en file.
        #[serde(skip_serializing_if = "Option::is_none")]
        cascade_job: Option<u64>,
        /// Étape 1 : l'étape 2 n'a pas pu être inscrite sur disque (elle est
        /// quand même en file, mais ne survivrait pas à un redémarrage). Texte
        /// « cascade_store : détail ».
        #[serde(skip_serializing_if = "Option::is_none")]
        cascade_error: Option<String>,
        /// Code stable de l'échec d'éjection (traduction côté interface) :
        /// `source_in_use_eject` ou `eject_not_removable`. Absent si l'échec
        /// vient du système (texte de `ejected`).
        eject_code: Option<&'static str>,
    },
    Failed {
        job: u64,
        error: String,
        /// Code stable pour la traduction : `no_space`, `job_panic`,
        /// `job_spawn`, `no_history`, `mhl_v1`, `nested_history`,
        /// `cascade_history`, `cascade_diverged`, `cascade_no_source`,
        /// `cascade_no_immediate`. Absent : seul le texte `error` est affichable.
        error_code: Option<&'static str>,
    },
}

/// Genre d'un travail dans les notifications.
#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum JobType {
    Offload,
    Verify,
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

/// Oublie l'annulation et l'étape 2 réservée d'un travail (ordre des verrous :
/// file, annulations, étapes 2).
fn release(shared: &Shared, id: u64) {
    lock(&shared.cancels).remove(&id);
    lock(&shared.cascades).remove(&id);
}

/// Libère les ressources d'un travail terminé (même en erreur) et relance
/// l'ordonnancement.
fn finish(app: &AppHandle, shared: &Shared, id: u64) {
    release(shared, id);
    lock(&shared.sched).running.remove(&id);
    dispatch(app, shared);
}

/// Volume d'un dossier (le dossier lui-même si le volume est introuvable).
fn volume_or(p: &Path) -> PathBuf {
    storage::volume_of(p)
        .map(|v| v.mount_point)
        .unwrap_or_else(|| p.to_path_buf())
}

/// Ressources d'une demande : volumes, disques durs, octets à écrire. Lent
/// (lecture des volumes) : hors du fil de l'interface. `total_bytes` vient du
/// préflight de l'interface ; absent, la source est inventoriée. `stages` :
/// étape de chaque destination (`destinations` puis `cascade`) ; seules celles
/// de l'étape 1 sont copiées par ce travail.
fn claim_for(request: &OffloadRequest, total_bytes: Option<u64>, stages: &[u8]) -> Claim {
    let source = veriflow_core::absolute_path(&request.source);
    // Inventaire en échec : le travail échouera de lui-même au lancement.
    let bytes = total_bytes.unwrap_or_else(|| scan(&source).map(|i| i.total_bytes).unwrap_or(0));
    let immediate = |i: &usize| stages.get(*i).copied().unwrap_or(1) == 1;
    let roots: Vec<PathBuf> = planned_roots(request)
        .into_iter()
        .enumerate()
        .filter(|(i, _)| immediate(i))
        .map(|(_, r)| r)
        .collect();
    let dest_volumes = request
        .destinations
        .iter()
        .chain(&request.cascade)
        .enumerate()
        .filter(|(i, _)| immediate(i))
        .map(|(_, d)| {
            let d = veriflow_core::absolute_path(d);
            (volume_or(&d), storage::is_hdd(&d), bytes)
        })
        .collect();
    Claim {
        source_volume: volume_or(&source),
        source_hdd: storage::is_hdd(&source),
        roots,
        dest_volumes,
        source,
        deferred: false,
    }
}

/// Ressources de l'étape 2 : lecture de la copie de l'étape 1 (dossier SSD),
/// écriture des destinations lentes. Différée : passe après les copies de cartes.
fn claim_for_cascade(pending: &PendingCascade) -> Claim {
    let source = cascade::pick_source(&pending.stage1_roots)
        .or_else(|| pending.stage1_roots.first().map(PathBuf::as_path))
        .map(veriflow_core::absolute_path)
        .unwrap_or_else(|| pending.source.clone());
    let bytes = pending.stage1.total_bytes;
    Claim {
        source_volume: volume_or(&source),
        source_hdd: storage::is_hdd(&source),
        roots: pending.stage2_roots.clone(),
        dest_volumes: pending
            .stage2_roots
            .iter()
            .map(|r| {
                let r = veriflow_core::absolute_path(r);
                (volume_or(&r), storage::is_hdd(&r), bytes)
            })
            .collect(),
        source,
        deferred: true,
    }
}

fn project_name(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let guard = state.project.lock().ok()?;
    guard.as_ref().and_then(|p| p.info().ok()).map(|i| i.name)
}

fn run_job(app: &AppHandle, shared: &Shared, job: Job) {
    let Job {
        id,
        kind,
        cancel,
        claim,
        ..
    } = job;
    match kind {
        JobKind::Offload {
            request,
            eject,
            known,
        } => run_offload(app, shared, id, *request, eject, known, &cancel, &claim),
        JobKind::Cascade { pending } => run_cascade(app, id, &pending, &cancel),
        JobKind::Verify { root, operator } => run_verify(app, id, &root, operator, &cancel),
    }
}

/// Revérifie `root` d'après son historique ASC MHL, puis écrit les rapports
/// (rien n'est écrit si la vérification est annulée).
fn run_verify(
    app: &AppHandle,
    id: u64,
    root: &Path,
    operator: Option<String>,
    cancel: &AtomicBool,
) {
    let send = |n: Notice| {
        let _ = app.emit("offload", n);
    };
    // Pas de mise en veille pendant la relecture.
    let _awake = veriflow_core::power::keep_awake();
    send(Notice::Running {
        job: id,
        job_kind: JobType::Verify,
        files: 0,
        total_bytes: 0,
        roots: vec![root.to_path_buf()],
    });
    let summary = match verify_root(root, cancel, |event| {
        send(Notice::Engine { job: id, event })
    }) {
        Ok(s) => s,
        Err(e) => return send(failed(id, e.to_string(), mhl_read::error_code(&e))),
    };
    let (mut reports, mut report_error) = (None, None);
    if !summary.cancelled {
        let info = ReportInfo {
            project: project_name(app),
            operator,
            notes: None,
        };
        match write_verify_reports(&VerifyData {
            summary: &summary,
            info: &info,
        }) {
            Ok(f) => reports = Some(f),
            Err(e) => report_error = Some(e.to_string()),
        }
    }
    send(Notice::Done {
        job: id,
        job_kind: JobType::Verify,
        result: None,
        // Compteurs, chaîne et écarts seulement : le détail est dans le CSV.
        verify: Some(Box::new(summary.for_ui())),
        reports,
        report_error,
        ejected: None,
        card_free: false,
        cascade_job: None,
        cascade_error: None,
        eject_code: None,
    });
}

/// Fichier des étapes 2 en attente, dans le dossier de données de l'application.
fn cascade_store(app: &AppHandle) -> CmdResult<PathBuf> {
    app.path()
        .app_data_dir()
        .map(|d| d.join("offload_cascades.json"))
        .map_err(text)
}

/// Étape 2 conforme : rien d'annulé, aucun fichier en échec, historique
/// prolongé sur chaque destination. Seule une étape 2 conforme est retirée de
/// la liste des copies à reprendre.
fn stage2_conform(result: &JobResult) -> bool {
    !result.summary.cancelled
        && result.summary.failed_files == 0
        && result.mhl.iter().all(Option::is_some)
        && result.mhl_errors.iter().all(Option::is_none)
}

/// Étape 2 d'une copie en cascade : recopie de la copie vérifiée de l'étape 1,
/// sans relire la carte.
fn run_cascade(app: &AppHandle, id: u64, pending: &PendingCascade, cancel: &AtomicBool) {
    let send = |n: Notice| {
        let _ = app.emit("offload", n);
    };
    send(Notice::Running {
        job: id,
        job_kind: JobType::Offload,
        files: pending.stage1.files,
        total_bytes: pending.stage1.total_bytes,
        roots: pending.stage2_roots.clone(),
    });
    match cascade::execute(pending, cancel, |event| {
        send(Notice::Engine { job: id, event })
    }) {
        Ok(result) => {
            if let Ok(guard) = app.state::<AppState>().project.lock() {
                if let Some(p) = guard.as_ref() {
                    let _ = p.record_offload(&result, &pending.source);
                }
            }
            if stage2_conform(&result) {
                if let Ok(path) = cascade_store(app) {
                    let _ = cascade::remove(&path, &pending.key);
                }
            }
            send(Notice::Done {
                job: id,
                job_kind: JobType::Offload,
                result: Some(Box::new(result)),
                verify: None,
                reports: None,
                report_error: None,
                ejected: None,
                card_free: false,
                cascade_job: None,
                cascade_error: None,
                eject_code: None,
            });
        }
        Err(e) => send(failed(id, cascade::message(&e), cascade::error_code(&e))),
    }
}

#[allow(clippy::too_many_arguments)]
fn run_offload(
    app: &AppHandle,
    shared: &Shared,
    id: u64,
    request: OffloadRequest,
    eject: bool,
    known: Option<Known>,
    cancel: &AtomicBool,
    claim: &Claim,
) {
    let send = |n: Notice| {
        let _ = app.emit("offload", n);
    };
    let pre = match preflight(&request) {
        Ok(p) => p,
        Err(e) => {
            return send(failed(id, cascade::message(&e), cascade::error_code(&e)));
        }
    };
    // L'espace a pu manquer entre le préflight de l'interface et le lancement
    // (autre copie terminée entre-temps, après une attente).
    if pre.missing_space.iter().any(Option::is_some) {
        return send(failed(
            id,
            "espace insuffisant sur une destination",
            Some("no_space"),
        ));
    }
    send(Notice::Running {
        job: id,
        job_kind: JobType::Offload,
        files: pre.files,
        total_bytes: pre.total_bytes,
        roots: pre.roots.clone(),
    });
    let project = project_name(app);
    let result = execute(
        &request,
        pre,
        cancel,
        |event| send(Notice::Engine { job: id, event }),
        project.as_deref(),
        known,
    );
    match result {
        Ok(result) => {
            // Historique dans le projet ouvert (détection des cartes déjà copiées).
            if let Ok(guard) = app.state::<AppState>().project.lock() {
                if let Some(p) = guard.as_ref() {
                    let _ = p.record_offload(&result, &request.source);
                }
            }
            // Étape 2 : inscrite sur disque (reprise possible après un
            // redémarrage), puis mise en file depuis ce fil, sans attendre.
            let mut cascade_job = None;
            let mut cascade_error = None;
            if let Some(pending) = result.cascade.clone() {
                let saved = cascade_store(app)
                    .map_err(|e| e.to_string())
                    .and_then(|path| cascade::save(&path, &pending).map_err(|e| e.to_string()));
                if let Err(detail) = saved {
                    cascade_error = Some(format!("cascade_store : {detail}"));
                }
                cascade_job = queue_cascade(app, shared, pending, Some(id));
            }
            let card_free = result.card_free;
            // Éjection seulement si la carte est libérable (étape 1 entièrement
            // vérifiée), que l'étape 2 existe ou non, et quand plus aucun autre travail (en cours ou en
            // attente) n'utilise ce volume (source, destination ou dossier final).
            let mut eject_code = None;
            let ejected = (eject && result.card_free).then(|| {
                let volume = claim.source_volume.clone();
                let in_use = {
                    let sched = lock(&shared.sched);
                    sched.preparing.values().any(|p| overlaps(p, &volume))
                        || sched
                            .running
                            .iter()
                            .filter(|(other, _)| **other != id)
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
                storage::volume_of(&request.source)
                    .filter(|v| v.removable)
                    .ok_or_else(|| {
                        eject_code = Some("eject_not_removable");
                        "source non amovible : pas d'éjection".to_owned()
                    })
                    .and_then(|v| storage::eject(&v.mount_point).map_err(text))
            });
            send(Notice::Done {
                job: id,
                job_kind: JobType::Offload,
                result: Some(Box::new(result)),
                verify: None,
                reports: None,
                report_error: None,
                ejected,
                card_free,
                cascade_job,
                cascade_error,
                eject_code,
            });
        }
        Err(e) => send(failed(id, cascade::message(&e), cascade::error_code(&e))),
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
            .map_err(|e| cascade::message(&e))?;
    let (pre, request) = pre;
    let previous = {
        let state = app.state::<AppState>();
        let guard = state.project.lock().map_err(text)?;
        guard
            .as_ref()
            .and_then(|p| p.previous_offloads(&pre.fingerprint).ok())
            .unwrap_or_default()
    };
    // Dans l'ordre de `roots` : destinations puis cascade.
    let hdd: Vec<bool> = request
        .destinations
        .iter()
        .chain(&request.cascade)
        .map(|d| storage::is_hdd(d))
        .collect();
    let source_hdd = storage::is_hdd(&request.source);
    // Durée et batterie ne portent que sur l'étape 1 : l'étape 2 se fait plus
    // tard, sans la carte.
    let immediate: Vec<bool> = pre.stages.iter().map(|s| *s == 1).collect();
    let estimated_secs = veriflow_core::battery::estimate_secs(
        pre.total_bytes,
        immediate.iter().filter(|i| **i).count(),
        source_hdd
            || hdd
                .iter()
                .zip(&immediate)
                .any(|(h, immediate)| *h && *immediate),
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
        let pre = preflight(&request).map_err(|e| cascade::message(&e))?;
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
    // Date figée maintenant : minuit passé pendant l'attente ne change pas
    // le dossier final.
    request.date.get_or_insert_with(local_date);
    let source = request.source.display().to_string();
    let abs_source = veriflow_core::absolute_path(&request.source);
    // Étape de chaque destination (lecture des volumes) ; refus sans
    // destination immédiate (`cascade_no_immediate`).
    let (request, stages) = tauri::async_runtime::spawn_blocking(move || {
        split_cascade(&request, storage::is_slow).map(|s| (request, s))
    })
    .await
    .map_err(text)?
    .map_err(|e| cascade::message(&e))?;
    enqueue(
        &app,
        &state.shared,
        id,
        source,
        abs_source,
        None,
        move || {
            let claim = claim_for(&request, total_bytes, &stages);
            let kind = JobKind::Offload {
                request: Box::new(request),
                eject,
                known,
            };
            (kind, claim)
        },
    )
    .await
}

/// Étape 2 d'une copie en cascade, pour le suivi des doublons.
struct CascadeRef {
    key: String,
    /// Travail de l'étape 1 à l'origine de la mise en file.
    of: Option<u64>,
}

/// Annonce un travail : annulation enregistrée, étape 2 réservée, notification
/// Queued. Renvoie le drapeau d'annulation, ou rien si l'étape 2 est déjà en
/// file ou en cours.
fn announce(
    app: &AppHandle,
    shared: &Shared,
    id: u64,
    source: String,
    cascade: Option<&CascadeRef>,
) -> Option<Arc<AtomicBool>> {
    if let Some(c) = cascade {
        let mut active = lock(&shared.cascades);
        if active.values().any(|k| *k == c.key) {
            return None;
        }
        active.insert(id, c.key.clone());
    }
    let cancel = Arc::new(AtomicBool::new(false));
    lock(&shared.cancels).insert(id, cancel.clone());
    let _ = app.emit(
        "offload",
        Notice::Queued {
            job: id,
            source,
            stage: if cascade.is_some() { 2 } else { 1 },
            cascade_of: cascade.and_then(|c| c.of),
        },
    );
    Some(cancel)
}

/// Calcule le claim puis met le travail en file et relance l'ordonnanceur.
/// Synchrone (lecture des volumes) : jamais depuis le fil de l'interface.
fn queue_job(
    app: &AppHandle,
    shared: &Shared,
    id: u64,
    cancel: Arc<AtomicBool>,
    build: impl FnOnce() -> (JobKind, Claim),
) {
    let (kind, claim) = build();
    // Drapeau lu sous le verrou, comme `offload_cancel` : une annulation
    // ne peut pas passer entre la lecture et la mise en file.
    let mut sched = lock(&shared.sched);
    sched.preparing.remove(&id);
    if cancel.load(Ordering::Relaxed) {
        drop(sched);
        release(shared, id);
        let _ = app.emit("offload", Notice::Dropped { job: id });
        return;
    }
    sched.pending.push_back(Job {
        id,
        kind,
        cancel,
        claim,
        last_wait: None,
    });
    drop(sched);
    dispatch(app, shared);
}

/// Met l'étape 2 en file depuis le fil d'un travail (sans `block_on`).
/// Renvoie son numéro, ou rien si cette étape 2 est déjà en file.
fn queue_cascade(
    app: &AppHandle,
    shared: &Shared,
    pending: PendingCascade,
    of: Option<u64>,
) -> Option<u64> {
    let id = app
        .state::<OffloadState>()
        .next_id
        .fetch_add(1, Ordering::Relaxed);
    let reference = CascadeRef {
        key: pending.key.clone(),
        of,
    };
    let cancel = announce(
        app,
        shared,
        id,
        pending.source.display().to_string(),
        Some(&reference),
    )?;
    queue_job(app, shared, id, cancel, move || {
        let claim = claim_for_cascade(&pending);
        (
            JobKind::Cascade {
                pending: Box::new(pending),
            },
            claim,
        )
    });
    Some(id)
}

/// Met un travail en file : le claim se calcule hors du fil de l'interface
/// (lecture des volumes), puis l'ordonnanceur est relancé. `source` : texte de
/// la notification Queued ; `busy` : dossier compté par le test d'éjection
/// pendant le calcul.
async fn enqueue(
    app: &AppHandle,
    shared: &Shared,
    id: u64,
    source: String,
    busy: PathBuf,
    cascade: Option<CascadeRef>,
    build: impl FnOnce() -> (JobKind, Claim) + Send + 'static,
) -> CmdResult<u64> {
    let Some(cancel) = announce(app, shared, id, source, cascade.as_ref()) else {
        return Err("cascade_active : cette étape 2 est déjà en file ou en cours".to_owned());
    };
    let shared = shared.clone();
    lock(&shared.sched).preparing.insert(id, busy);
    let (worker, worker_app) = (shared.clone(), app.clone());
    let prepared = tauri::async_runtime::spawn_blocking(move || {
        queue_job(&worker_app, &worker, id, cancel, build);
    })
    .await;
    if let Err(e) = prepared {
        // Panique du calcul : le travail n'existe pas, rien ne doit rester.
        lock(&shared.sched).preparing.remove(&id);
        release(&shared, id);
        let _ = app.emit("offload", failed(id, text(&e), Some("job_spawn")));
        return Err(text(e));
    }
    Ok(id)
}

/// Erreur de lecture d'un historique : `code` stable pour la traduction
/// (`no_history`, `mhl_v1`, `nested_history`), absent si seul le texte compte.
#[derive(Serialize)]
pub struct MhlError {
    code: Option<&'static str>,
    message: String,
}

/// Résumé de l'historique ASC MHL de `root` (générations, fichiers, méthodes,
/// anomalies de chaîne). Sans `ascmhl/`, `histories` liste les sous-dossiers
/// qui en ont.
#[tauri::command]
pub async fn offload_mhl_info(root: PathBuf) -> Result<MhlInfo, MhlError> {
    let unexpected = |e: tauri::Error| MhlError {
        code: None,
        message: e.to_string(),
    };
    tauri::async_runtime::spawn_blocking(move || {
        mhl_read::info(&root).map_err(|e| MhlError {
            code: mhl_read::error_code(&e),
            message: e.to_string(),
        })
    })
    .await
    .map_err(unexpected)?
}

/// Ajoute la revérification d'un disque (historique ASC MHL) à la file ;
/// renvoie son numéro. Elle suit les mêmes règles d'attente qu'une copie : le
/// volume de `root` est lu et `root` est un dossier occupé.
#[tauri::command]
pub async fn offload_mhl_verify(
    root: PathBuf,
    operator: Option<String>,
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<u64> {
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let root = veriflow_core::absolute_path(&root);
    let source = root.display().to_string();
    enqueue(
        &app,
        &state.shared,
        id,
        source,
        root.clone(),
        None,
        move || {
            let volume = storage::volume_of(&root)
                .map(|v| v.mount_point)
                .unwrap_or_else(|| root.clone());
            let claim = Claim::verify(root.clone(), volume, storage::is_hdd(&root));
            (JobKind::Verify { root, operator }, claim)
        },
    )
    .await
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
        release(&state.shared, job);
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

/// Étape 2 en attente, telle que la voit l'interface.
#[derive(Serialize)]
pub struct PendingView {
    key: String,
    /// Nom et chemin de la carte d'origine.
    card: String,
    source: PathBuf,
    date: String,
    stage1_roots: Vec<PathBuf>,
    stage2_roots: Vec<PathBuf>,
    files: usize,
    total_bytes: u64,
    /// Fin de l'étape 1.
    finished_at: String,
    /// Une copie de l'étape 1 a encore un historique intact (sinon la reprise
    /// échouera avec `cascade_no_source`).
    stage1_available: bool,
    /// Numéro du travail si cette étape 2 est déjà en file ou en cours.
    job: Option<u64>,
}

/// Étapes 2 enregistrées et non terminées (reprise après un arrêt, une
/// annulation ou un échec).
#[tauri::command]
pub async fn offload_cascade_pending(
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<Vec<PendingView>> {
    let path = cascade_store(&app)?;
    let list = tauri::async_runtime::spawn_blocking(move || cascade::load(&path))
        .await
        .map_err(text)?
        .map_err(text)?;
    let active: HashMap<String, u64> = lock(&state.shared.cascades)
        .iter()
        .map(|(id, key)| (key.clone(), *id))
        .collect();
    tauri::async_runtime::spawn_blocking(move || {
        list.into_iter()
            .map(|p| PendingView {
                stage1_available: cascade::pick_source(&p.stage1_roots).is_some(),
                job: active.get(&p.key).copied(),
                key: p.key,
                card: p.card,
                source: p.source,
                date: p.date,
                stage1_roots: p.stage1_roots,
                stage2_roots: p.stage2_roots,
                files: p.stage1.files,
                total_bytes: p.stage1.total_bytes,
                finished_at: p.stage1.finished_at,
            })
            .collect()
    })
    .await
    .map_err(text)
}

/// Remet une étape 2 enregistrée en file ; renvoie son numéro. Les fichiers
/// déjà copiés sont revérifiés sans être recopiés.
#[tauri::command]
pub async fn offload_cascade_resume(
    key: String,
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<u64> {
    let path = cascade_store(&app)?;
    let list = tauri::async_runtime::spawn_blocking(move || cascade::load(&path))
        .await
        .map_err(text)?
        .map_err(text)?;
    let pending = list
        .into_iter()
        .find(|p| p.key == key)
        .ok_or("étape 2 introuvable : déjà terminée ou abandonnée")?;
    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let source = pending.source.display().to_string();
    let busy = pending.stage1_roots.first().cloned().unwrap_or_default();
    let reference = CascadeRef { key, of: None };
    enqueue(
        &app,
        &state.shared,
        id,
        source,
        busy,
        Some(reference),
        move || {
            let claim = claim_for_cascade(&pending);
            (
                JobKind::Cascade {
                    pending: Box::new(pending),
                },
                claim,
            )
        },
    )
    .await
}

/// Abandonne une étape 2 enregistrée (les copies déjà faites restent en
/// place). Refusée tant qu'elle est en file ou en cours.
#[tauri::command]
pub async fn offload_cascade_discard(
    key: String,
    app: AppHandle,
    state: State<'_, OffloadState>,
) -> CmdResult<()> {
    if lock(&state.shared.cascades).values().any(|k| *k == key) {
        return Err("cascade_active : étape 2 en file ou en cours, annule-la d'abord".to_owned());
    }
    let path = cascade_store(&app)?;
    tauri::async_runtime::spawn_blocking(move || cascade::remove(&path, &key))
        .await
        .map_err(text)?
        .map(|_| ())
        .map_err(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use veriflow_core::offload::cascade::Stage1Summary;

    fn pending() -> PendingCascade {
        PendingCascade {
            key: "k".into(),
            card: "A001".into(),
            source: PathBuf::from("/nonexistent/card"),
            fingerprint: "f".into(),
            stage1_roots: vec![PathBuf::from("/nonexistent/ssd/A001")],
            stage2_roots: vec![PathBuf::from("/nonexistent/hdd/A001")],
            algorithms: Vec::new(),
            operator: None,
            notes: None,
            project: None,
            date: "2026-01-01".into(),
            stage1: Stage1Summary {
                started_at: String::new(),
                finished_at: String::new(),
                duration_s: 1.0,
                total_bytes: 42,
                files: 3,
                effective_rate: None,
                bottleneck: String::new(),
                destinations: Vec::new(),
                reports: Vec::new(),
            },
        }
    }

    #[test]
    fn cascade_claim_is_deferred_and_reads_stage1_copy() {
        let claim = claim_for_cascade(&pending());
        assert!(claim.deferred);
        assert_eq!(claim.source, PathBuf::from("/nonexistent/ssd/A001"));
        assert_eq!(claim.roots, vec![PathBuf::from("/nonexistent/hdd/A001")]);
        assert_eq!(claim.dest_volumes.len(), 1);
        assert_eq!(claim.dest_volumes[0].2, 42);
    }

    #[test]
    fn offload_claim_covers_only_stage1_destinations() {
        let request = OffloadRequest {
            source: PathBuf::from("/nonexistent/card"),
            destinations: vec![
                PathBuf::from("/nonexistent/ssd"),
                PathBuf::from("/nonexistent/hdd"),
            ],
            template: "{card}".into(),
            vars: TemplateVars::default(),
            algorithms: Vec::new(),
            operator: None,
            notes: None,
            existing: Default::default(),
            date: Some("2026-01-01".into()),
            roots: None,
            history: Vec::new(),
            cascade: Vec::new(),
            cascade_auto: false,
        };
        let claim = claim_for(&request, Some(10), &[1, 2]);
        assert!(!claim.deferred);
        assert_eq!(claim.roots.len(), 1);
        assert_eq!(claim.dest_volumes.len(), 1);
        assert_eq!(claim_for(&request, Some(10), &[1, 1]).roots.len(), 2);
    }
}
