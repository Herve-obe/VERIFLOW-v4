//! Moteur d'OFFLOAD (charte §7.1).
//!
//! Pour chaque fichier :
//! 1. lecture unique de la source par blocs, calcul des empreintes à la volée ;
//! 2. écriture simultanée vers toutes les destinations (un fil par disque),
//!    dans un fichier temporaire `.nom.vfpart`, puis fsync, date d'origine
//!    et renommage : un fichier final est toujours un fichier complet ;
//! 3. relecture intégrale de chaque destination depuis le support physique
//!    (cache système contourné) et comparaison bit à bit des empreintes.
//!
//! Une destination en échec (disque plein, câble arraché) n'arrête pas les
//! autres. Les fichiers déjà présents et complets ne sont pas recopiés :
//! ils sont seulement revérifiés (reprise après coupure). Si leurs empreintes
//! ont été établies avant la copie (`existing::verify`), les rushes identiques
//! sont conservés sans relecture et les autres remplacés.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use serde::Serialize;

use super::existing::Known;
use super::hash::{HashAlgo, ParallelHasher};
use super::io::{drop_cache, open_uncached, read_full, AlignedBuf};
use super::meter::{Meters, Probe, RateWindow, Throughput};
use super::scan::{SourceFile, SourceInventory};
use crate::{Error, Result};

/// Empreintes calculées : (algorithme, valeur).
pub type Hashes = Vec<(HashAlgo, String)>;

/// Taille des blocs de lecture et d'écriture.
pub const CHUNK: usize = 8 << 20;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", content = "detail", rename_all = "snake_case")]
pub enum DestStatus {
    /// Copié puis vérifié.
    Verified,
    /// Déjà présent (reprise) puis vérifié.
    ResumedVerified,
    Failed(String),
}

#[derive(Debug, Clone, Serialize)]
pub struct FileResult {
    pub rel: String,
    pub size: u64,
    /// Date de modification d'origine (RFC 3339).
    pub modified: String,
    /// Empreintes de la source.
    pub hashes: Vec<(HashAlgo, String)>,
    /// Statut par destination (même ordre que `OffloadSpec::destinations`).
    pub destinations: Vec<DestStatus>,
}

impl FileResult {
    pub fn ok(&self) -> bool {
        self.destinations
            .iter()
            .all(|d| !matches!(d, DestStatus::Failed(_)))
    }
}

#[derive(Debug, Clone)]
pub struct OffloadSpec {
    /// Dossiers racine de copie, un par destination (la carte y est recopiée telle quelle).
    pub destinations: Vec<PathBuf>,
    /// Algorithmes (au moins un ; le premier est l'algorithme principal).
    pub algorithms: Vec<HashAlgo>,
    /// Rushes déjà présents vérifiés avant la copie (mode « compléter ») :
    /// les identiques sont conservés, les autres remplacés.
    pub known: Option<Known>,
    /// Relit le fichier N pendant la copie du fichier N+1. Faux : fichier par
    /// fichier (disque dur mécanique, où deux accès simultanés se gênent).
    pub overlap: bool,
}

/// Événements envoyés pendant la copie.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    FileStarted {
        index: usize,
        rel: String,
        size: u64,
    },
    Progress {
        /// Octets traités (lecture source + vérification de chaque destination).
        done: u64,
        total: u64,
        /// Débit de lecture de la source, en octets par seconde.
        rate: f64,
    },
    FileDone {
        index: usize,
        result: FileResult,
    },
    /// Débits réels sur la dernière seconde, en octets par seconde (au plus
    /// un événement par seconde). `write` et `verify` : une valeur par destination.
    Rates {
        source: f64,
        write: Vec<f64>,
        verify: Vec<f64>,
        /// Part du temps (0 à 1) où la lecture attend le calcul des empreintes.
        hash_wait_ratio: f64,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct OffloadSummary {
    pub files: Vec<FileResult>,
    pub total_bytes: u64,
    pub started_at: String,
    pub finished_at: String,
    pub duration_s: f64,
    pub cancelled: bool,
    pub failed_files: usize,
    /// Débit réel par disque et par phase.
    pub throughput: Throughput,
}

fn rfc3339(t: SystemTime) -> String {
    chrono::DateTime::<chrono::Utc>::from(t).to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

/// Espace libre manquant par destination (`None` si tout tient).
pub fn check_space(inv: &SourceInventory, destinations: &[PathBuf]) -> Vec<Option<u64>> {
    destinations
        .iter()
        .map(|d| {
            // Les fichiers déjà complets ne seront pas recopiés.
            let needed: u64 = inv
                .files
                .iter()
                .filter(|f| {
                    fs::metadata(d.join(&f.rel))
                        .map(|m| m.len() != f.size)
                        .unwrap_or(true)
                })
                .map(|f| f.size)
                .sum();
            let mut probe = d.as_path();
            while !probe.exists() {
                probe = probe.parent()?;
            }
            let free = fs4::available_space(probe).ok()?;
            (needed > free).then(|| needed - free)
        })
        .collect()
}

/// Chemin absolu normalisé, même si sa fin n'existe pas encore.
fn absolute(path: &Path) -> PathBuf {
    let mut existing = path.to_path_buf();
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.file_name(), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name.to_owned());
                existing = parent.to_path_buf();
            }
            _ => break,
        }
    }
    let mut out = fs::canonicalize(&existing).unwrap_or(existing);
    out.extend(rest.into_iter().rev());
    out
}

pub(super) fn temp_path(final_path: &Path) -> PathBuf {
    let name = final_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    final_path.with_file_name(format!(".{name}.vfpart"))
}

/// Écrit les blocs reçus dans un fichier temporaire, puis le finalise.
fn writer(
    rx: std::sync::mpsc::Receiver<Arc<Vec<u8>>>,
    final_path: PathBuf,
    modified: SystemTime,
    expected: u64,
    meters: &Meters,
    dest: usize,
) -> std::result::Result<(), String> {
    let meter = &meters.writes[dest];
    let tmp = temp_path(&final_path);
    let result = (|| -> std::io::Result<()> {
        if let Some(parent) = final_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&tmp)?;
        let mut written = 0u64;
        for chunk in rx {
            let t = Instant::now();
            let r = f.write_all(&chunk);
            // Le temps d'un appel en échec compte aussi, avant de propager l'erreur.
            meter.record(if r.is_ok() { chunk.len() as u64 } else { 0 }, t);
            r?;
            written += chunk.len() as u64;
        }
        // Le canal se ferme aussi quand la lecture est interrompue (annulation,
        // source illisible) : seul le compte complet d'octets autorise le renommage.
        if written != expected {
            return Err(std::io::Error::other(format!(
                "copie incomplète ({written} octets sur {expected})"
            )));
        }
        // Le fsync fait partie du temps d'écriture du disque.
        let t = Instant::now();
        f.sync_all()?;
        meter.record(0, t);
        drop_cache(&f);
        drop(f);
        filetime::set_file_mtime(&tmp, filetime::FileTime::from_system_time(modified))?;
        fs::rename(&tmp, &final_path)?;
        Ok(())
    })();
    result.map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e.to_string()
    })
}

/// Relit un fichier depuis le disque et calcule ses empreintes.
pub(super) fn hash_from_disk(
    path: &Path,
    algos: &[HashAlgo],
    cancel: &AtomicBool,
) -> std::io::Result<Vec<(HashAlgo, String)>> {
    hash_from_disk_progress(path, algos, cancel, None, |_| {})
}

/// Comme `hash_from_disk`, avec un rappel appelé après chaque bloc lu
/// (octets lus depuis le début du fichier).
pub(super) fn hash_from_disk_progress(
    path: &Path,
    algos: &[HashAlgo],
    cancel: &AtomicBool,
    probe: Option<Probe>,
    mut progress: impl FnMut(u64),
) -> std::io::Result<Vec<(HashAlgo, String)>> {
    let mut buf = AlignedBuf::new(CHUNK);
    let mut f = retry_busy(
        || open_uncached(path),
        is_sharing_violation,
        Duration::from_millis(200),
    )?;
    let size = f.metadata()?.len();
    hash_uncached_with(
        &mut f,
        size,
        buf.as_mut_slice(),
        algos,
        cancel,
        &mut progress,
        probe,
    )
}

/// Relecture complète avec un tampon fourni (un par fil vérificateur).
/// L'ouverture est réessayée si le fichier est occupé ; il n'y a jamais
/// de résultat sans relecture complète.
fn hash_file(
    path: &Path,
    algos: &[HashAlgo],
    buf: &mut [u8],
    cancel: &AtomicBool,
    probe: Probe,
) -> std::io::Result<Vec<(HashAlgo, String)>> {
    let mut f = retry_busy(
        || open_uncached(path),
        is_sharing_violation,
        Duration::from_millis(200),
    )?;
    let size = f.metadata()?.len();
    hash_uncached_with(&mut f, size, buf, algos, cancel, &mut |_| {}, Some(probe))
}

/// Lecture sans cache : sous Windows (FILE_FLAG_NO_BUFFERING), chaque lecture
/// doit partir d'une position alignée sur le secteur. Après la dernière
/// lecture, partielle, la position ne l'est plus : on s'arrête donc à la
/// taille connue du fichier au lieu de relire pour constater la fin
/// (relecture qui échouait avec « Paramètre incorrect », os error 87).
#[cfg(test)]
fn hash_uncached(
    f: &mut impl Read,
    size: u64,
    buf: &mut [u8],
    algos: &[HashAlgo],
    cancel: &AtomicBool,
) -> std::io::Result<Vec<(HashAlgo, String)>> {
    hash_uncached_with(f, size, buf, algos, cancel, &mut |_| {}, None)
}

fn hash_uncached_with(
    f: &mut impl Read,
    size: u64,
    buf: &mut [u8],
    algos: &[HashAlgo],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
    probe: Option<Probe>,
) -> std::io::Result<Vec<(HashAlgo, String)>> {
    let hasher = ParallelHasher::new(algos);
    let mut done = 0u64;
    while done < size {
        if cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("annulé"));
        }
        // Lecture du tampon entier : la taille demandée reste un multiple du secteur.
        let t = Instant::now();
        let n = loop {
            match f.read(buf) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                r => break r?,
            }
        };
        if let Some(p) = probe {
            p.io.record(n as u64, t);
        }
        if n == 0 {
            break; // fichier raccourci pendant la lecture : l'empreinte différera
        }
        let t = Instant::now();
        hasher.update(&Arc::new(buf[..n].to_vec()));
        if let Some(p) = probe {
            p.cpu.record(0, t);
        }
        done += n as u64;
        progress(done);
    }
    Ok(hasher.finish())
}

struct Counter {
    done: u64,
    total: u64,
    started: Instant,
    source_bytes: u64,
    last_emit: Instant,
    meters: Arc<Meters>,
    window: RateWindow,
}

impl Counter {
    fn add(&mut self, bytes: u64, source: bool, emit: &mut impl FnMut(Event)) {
        self.done += bytes;
        if source {
            self.source_bytes += bytes;
        }
        if self.last_emit.elapsed().as_millis() >= 150 {
            self.flush(emit);
        }
    }
    fn flush(&mut self, emit: &mut impl FnMut(Event)) {
        self.last_emit = Instant::now();
        let secs = self.started.elapsed().as_secs_f64().max(1e-3);
        emit(Event::Progress {
            done: self.done,
            total: self.total,
            rate: self.source_bytes as f64 / secs,
        });
        self.tick_rates(emit);
    }

    /// Débits réels de la dernière seconde (au plus un événement par seconde).
    fn tick_rates(&mut self, emit: &mut impl FnMut(Event)) {
        let snap = self.meters.snapshot();
        if let Some(r) = self.window.tick(Instant::now(), snap) {
            emit(Event::Rates {
                source: r.source,
                write: r.write,
                verify: r.verify,
                hash_wait_ratio: r.hash_wait_ratio,
            });
        }
    }
}

/// Intervalle de réveil pendant les attentes longues (débits, annulation).
const RATES_TICK: Duration = Duration::from_millis(250);

/// Attend la fin d'un fil en continuant d'émettre les débits.
fn join_ticking<T>(
    h: thread::JoinHandle<T>,
    col: &mut Collector,
    emit: &mut impl FnMut(Event),
) -> thread::Result<T> {
    let mut last = Instant::now();
    while !h.is_finished() {
        thread::sleep(Duration::from_millis(10));
        if last.elapsed() >= RATES_TICK {
            col.counter.tick_rates(emit);
            last = Instant::now();
        }
    }
    h.join()
}

/// Nombre maximal de vérifications en attente par destination (mémoire bornée).
const VERIFY_AHEAD: usize = 8;

/// Relecture demandée à un fil vérificateur.
struct VerifyTask {
    index: usize,
    dest: usize,
    path: PathBuf,
}

/// Réponse d'un vérificateur : (index du fichier, destination, empreintes relues).
type VerifyReply = (usize, usize, std::io::Result<Hashes>);

/// Fichier copié dont certaines destinations attendent encore leur relecture.
struct Pending {
    /// Résultat sans les statuts, complété quand tout le monde a répondu.
    result: FileResult,
    status: Vec<Option<DestStatus>>,
    resumed: Vec<bool>,
    waiting: usize,
}

/// Issue de l'étage de copie.
enum CopyOutcome {
    /// Rien à relire : le résultat est définitif.
    Done(FileResult),
    /// Relectures à lancer, une par (destination, chemin).
    Pending(Pending, Vec<(usize, PathBuf)>),
}

/// Reçoit les réponses des vérificateurs. Seul le fil principal y touche :
/// compteur, événements et résultats restent donc sans verrou.
struct Collector<'a> {
    counter: Counter,
    rx: Receiver<VerifyReply>,
    pending: HashMap<usize, Pending>,
    /// Résultats rangés dans l'ordre de l'inventaire.
    done: Vec<Option<FileResult>>,
    /// Une vérification a été interrompue par l'annulation.
    aborted: bool,
    cancel: &'a AtomicBool,
}

impl Collector<'_> {
    fn finish(&mut self, index: usize, p: Pending, emit: &mut impl FnMut(Event)) {
        let mut result = p.result;
        result.destinations = p
            .status
            .into_iter()
            .map(|s| s.expect("statut défini"))
            .collect();
        emit(Event::FileDone {
            index,
            result: result.clone(),
        });
        self.done[index] = Some(result);
    }

    fn handle(&mut self, reply: VerifyReply, emit: &mut impl FnMut(Event)) {
        let (index, dest, r) = reply;
        let Some(p) = self.pending.get_mut(&index) else {
            return;
        };
        if r.is_err() && self.cancel.load(Ordering::Relaxed) {
            // Relecture interrompue : le fichier sera revérifié à la reprise.
            self.pending.remove(&index);
            self.aborted = true;
            return;
        }
        let size = p.result.size;
        let source_hashes = &p.result.hashes;
        p.status[dest] = Some(match r {
            Ok(h) if h == *source_hashes => {
                if p.resumed[dest] {
                    DestStatus::ResumedVerified
                } else {
                    DestStatus::Verified
                }
            }
            Ok(h) => DestStatus::Failed(format!(
                "empreinte différente de la source ({} attendu {}, lu {})",
                source_hashes[0].0, source_hashes[0].1, h[0].1
            )),
            Err(e) => DestStatus::Failed(format!("relecture impossible : {e}")),
        });
        p.waiting -= 1;
        let complete = p.waiting == 0;
        self.counter.add(size, false, emit);
        if complete {
            if let Some(p) = self.pending.remove(&index) {
                self.finish(index, p, emit);
            }
        }
    }

    /// Traite sans attendre les réponses déjà arrivées.
    fn drain(&mut self, emit: &mut impl FnMut(Event)) {
        while let Ok(reply) = self.rx.try_recv() {
            self.handle(reply, emit);
        }
    }

    /// Attend que `done` soit vrai pour l'état courant (les vérificateurs
    /// disparus ne répondront plus : leurs fichiers sont alors en échec).
    fn wait_while(
        &mut self,
        mut busy: impl FnMut(&HashMap<usize, Pending>) -> bool,
        emit: &mut impl FnMut(Event),
    ) {
        self.drain(emit);
        while busy(&self.pending) {
            // Réveil régulier : les débits continuent pendant une longue relecture.
            match self.rx.recv_timeout(RATES_TICK) {
                Ok(reply) => self.handle(reply, emit),
                Err(RecvTimeoutError::Timeout) => self.counter.tick_rates(emit),
                Err(RecvTimeoutError::Disconnected) => {
                    let lost: Vec<_> = self.pending.drain().collect();
                    for (index, mut p) in lost {
                        // Relectures jamais faites : la progression doit quand même finir.
                        let missing = p.status.iter().filter(|s| s.is_none()).count() as u64;
                        self.counter.add(p.result.size * missing, false, emit);
                        for s in p.status.iter_mut().filter(|s| s.is_none()) {
                            *s = Some(DestStatus::Failed("vérification interrompue".into()));
                        }
                        self.finish(index, p, emit);
                    }
                    break;
                }
            }
        }
    }

    fn wait_file(&mut self, index: usize, emit: &mut impl FnMut(Event)) {
        self.wait_while(|p| p.contains_key(&index), emit);
    }

    fn wait_all(&mut self, emit: &mut impl FnMut(Event)) {
        self.wait_while(|p| !p.is_empty(), emit);
    }
}

/// Réessaie une opération tant que le fichier est occupé (antivirus, indexeur).
fn retry_busy<T>(
    mut op: impl FnMut() -> std::io::Result<T>,
    is_busy: impl Fn(&std::io::Error) -> bool,
    delay: Duration,
) -> std::io::Result<T> {
    let mut retries = 0;
    loop {
        match op() {
            Err(e) if retries < 3 && is_busy(&e) => {
                retries += 1;
                thread::sleep(delay);
            }
            r => return r,
        }
    }
}

/// Windows : « le fichier est utilisé par un autre processus » (os error 32).
fn is_sharing_violation(e: &std::io::Error) -> bool {
    cfg!(windows) && e.raw_os_error() == Some(32)
}

/// Fil vérificateur d'une destination : relit les fichiers un à un, avec son
/// propre tampon aligné, et renvoie les empreintes au fil principal.
fn verifier(
    rx: Receiver<VerifyTask>,
    reply: Sender<VerifyReply>,
    algos: &[HashAlgo],
    cancel: &AtomicBool,
    before_verify: &(dyn Fn(usize, &Path) + Sync),
    probe: Probe,
) {
    let mut buf = AlignedBuf::new(CHUNK);
    for task in rx {
        let r = if cancel.load(Ordering::Relaxed) {
            Err(std::io::Error::other("annulé"))
        } else {
            // Une panique ne doit pas faire disparaître le fil : ses tâches
            // suivantes et le fil principal en dépendent.
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                before_verify(task.index, &task.path);
                hash_file(&task.path, algos, buf.as_mut_slice(), cancel, probe)
            }))
            .unwrap_or_else(|_| Err(std::io::Error::other("vérification interrompue")))
        };
        if reply.send((task.index, task.dest, r)).is_err() {
            break;
        }
    }
}

/// Taille et date de modification d'une source (époque si la date manque).
fn stamp(m: &fs::Metadata) -> (u64, SystemTime) {
    (m.len(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH))
}

/// Crochets de test, sans état global.
struct Hooks<'a> {
    /// Fil principal, par destination écrite (index, chemin final), avant sa relecture.
    after_write: &'a dyn Fn(usize, &Path),
    /// Fil principal, après chaque bloc lu de la source (index, nombre de blocs lus).
    after_block: &'a dyn Fn(usize, usize),
    /// Fil vérificateur, avant la relecture (index, chemin).
    before_verify: &'a (dyn Fn(usize, &Path) + Sync),
}

/// Étage de copie d'un fichier : lecture unique de la source, écriture vers
/// toutes les destinations (renommage compris) et empreintes de la source.
/// La relecture des destinations est confiée aux vérificateurs.
fn copy_stage(
    index: usize,
    file: &SourceFile,
    spec: &OffloadSpec,
    cancel: &AtomicBool,
    col: &mut Collector,
    emit: &mut impl FnMut(Event),
    hooks: &Hooks,
) -> Result<CopyOutcome> {
    let n = spec.destinations.len();
    let finals: Vec<PathBuf> = spec
        .destinations
        .iter()
        .map(|d| d.join(&file.rel))
        .collect();
    let mut status: Vec<Option<DestStatus>> = vec![None; n];
    let mut resumed = vec![false; n];

    // Mode « compléter » : rushes vérifiés identiques avant la copie.
    if let Some(known) = &spec.known {
        for (i, path) in finals.iter().enumerate() {
            if known.is_identical(i, &file.rel, path) {
                status[i] = Some(DestStatus::ResumedVerified);
            }
        }
        if let Some(hashes) = known.source.get(&file.rel) {
            if status.iter().all(Option::is_some) {
                // Rien à écrire ni à relire : empreintes de la vérification.
                col.counter.add(file.size * (1 + n as u64), false, emit);
                return Ok(CopyOutcome::Done(FileResult {
                    rel: file.rel.clone(),
                    size: file.size,
                    modified: rfc3339(file.modified),
                    hashes: hashes.clone(),
                    destinations: status.into_iter().flatten().collect(),
                }));
            }
        }
    }

    // Destinations à écrire (les fichiers complets déjà présents sont seulement
    // revérifiés ; en mode « compléter », les fichiers non identiques sont remplacés).
    let meters = col.counter.meters.clone();
    let mut senders: Vec<(usize, SyncSender<Arc<Vec<u8>>>)> = Vec::new();
    let mut handles = Vec::new();
    for (i, path) in finals.iter().enumerate() {
        if status[i].is_some() {
            continue;
        }
        let existing = fs::metadata(path).ok();
        match existing {
            Some(m) if spec.known.is_none() && m.len() == file.size => resumed[i] = true,
            Some(_) if spec.known.is_none() => {
                status[i] = Some(DestStatus::Failed(
                    "un fichier différent porte déjà ce nom : il n'a pas été écrasé".into(),
                ))
            }
            _ => {
                let (tx, rx) = sync_channel::<Arc<Vec<u8>>>(4);
                let path = path.clone();
                let modified = file.modified;
                senders.push((i, tx));
                let size = file.size;
                let meters = meters.clone();
                handles.push((
                    i,
                    thread::spawn(move || writer(rx, path, modified, size, &meters, i)),
                ));
            }
        }
    }

    // Lecture unique de la source.
    let hasher = ParallelHasher::new(&spec.algorithms);
    let mut read_bytes = 0u64;
    let mut blocks = 0usize;
    let read_result = (|| -> std::io::Result<(u64, SystemTime)> {
        let mut src = File::open(&file.path)?;
        // État de la source à l'ouverture, comparé à celui de la fin de lecture.
        let before = stamp(&src.metadata()?);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(std::io::Error::other("annulé"));
            }
            let mut buf = vec![0u8; CHUNK];
            let t = Instant::now();
            let got = read_full(&mut src, &mut buf)?;
            meters.source.record(got as u64, t);
            if got == 0 {
                break;
            }
            buf.truncate(got);
            let chunk = Arc::new(buf);
            // Si le calcul est en retard, `update` bloque : c'est le goulot processeur.
            let t = Instant::now();
            hasher.update(&chunk);
            meters.source_cpu().record(0, t);
            // Un disque en échec ferme son canal : on continue avec les autres.
            senders.retain(|(_, tx)| tx.send(chunk.clone()).is_ok());
            read_bytes += got as u64;
            blocks += 1;
            (hooks.after_block)(index, blocks);
            col.counter.add(got as u64, true, emit);
            // Les relectures du fichier précédent avancent pendant cette copie.
            col.drain(emit);
        }
        Ok(before)
    })();
    drop(senders);
    let before = match read_result {
        Ok(before) => before,
        Err(e) => {
            // Source illisible ou annulation : les fichiers temporaires sont supprimés
            // (le fil d'écriture refuse de renommer une copie incomplète).
            for (_, h) in handles {
                let _ = join_ticking(h, col, emit);
            }
            for path in &finals {
                let _ = fs::remove_file(temp_path(path));
            }
            if cancel.load(Ordering::Relaxed) {
                return Err(Error::Cancelled);
            }
            // Octets jamais lus ni relus : la progression doit quand même arriver au total.
            col.counter.add(
                file.size.saturating_sub(read_bytes) + file.size * n as u64,
                false,
                emit,
            );
            let msg = format!("lecture de la source impossible : {e}");
            return Ok(CopyOutcome::Done(FileResult {
                rel: file.rel.clone(),
                size: file.size,
                modified: rfc3339(file.modified),
                hashes: Vec::new(),
                destinations: vec![DestStatus::Failed(msg); n],
            }));
        }
    };
    // Fin du calcul : si elle est longue, les débits continuent d'être émis.
    let t = Instant::now();
    let source_hashes = {
        let (tx, rx) = channel();
        thread::scope(|sc| {
            sc.spawn(move || {
                let _ = tx.send(hasher.finish());
            });
            loop {
                match rx.recv_timeout(RATES_TICK) {
                    Ok(h) => break h,
                    Err(RecvTimeoutError::Timeout) => col.counter.tick_rates(emit),
                    Err(RecvTimeoutError::Disconnected) => {
                        panic!("fil de calcul d'empreinte")
                    }
                }
            }
        })
    };
    meters.source_cpu().record(0, t);
    let mut written = Vec::new();
    for (i, h) in handles {
        match join_ticking(h, col, emit) {
            Ok(Ok(())) => written.push(i),
            Ok(Err(e)) => status[i] = Some(DestStatus::Failed(format!("écriture : {e}"))),
            Err(_) => status[i] = Some(DestStatus::Failed("écriture interrompue".into())),
        }
    }
    for &i in &written {
        (hooks.after_write)(index, &finals[i]);
    }

    // Source modifiée pendant la copie : les empreintes ne décrivent plus un
    // état cohérent. Les copies écrites par ce passage sont retirées.
    let stale = before != (file.size, file.modified);
    let unchanged = !stale
        && fs::metadata(&file.path)
            .map(|m| stamp(&m) == before)
            .unwrap_or(false);
    if !unchanged {
        let why = if stale {
            "source modifiée depuis l'inventaire"
        } else {
            "source modifiée pendant la copie"
        };
        for &i in &written {
            let _ = fs::remove_file(&finals[i]);
        }
        // Aucune relecture n'aura lieu : leurs octets comptent comme faits.
        col.counter.add(file.size * n as u64, false, emit);
        // Les causes réelles déjà établies (écriture en échec...) sont conservées,
        // sauf le refus d'une copie de taille inattendue, qui vient de la modification.
        let destinations = status
            .into_iter()
            .map(|s| match s {
                Some(DestStatus::Failed(m)) if !m.starts_with("écriture : copie incomplète") => {
                    DestStatus::Failed(m)
                }
                _ => DestStatus::Failed(why.into()),
            })
            .collect();
        return Ok(CopyOutcome::Done(FileResult {
            rel: file.rel.clone(),
            size: file.size,
            modified: rfc3339(file.modified),
            hashes: Vec::new(),
            destinations,
        }));
    }

    let checks: Vec<(usize, PathBuf)> = (0..n)
        .filter(|&i| status[i].is_none())
        .map(|i| (i, finals[i].clone()))
        .collect();
    let result = FileResult {
        rel: file.rel.clone(),
        size: file.size,
        modified: rfc3339(file.modified),
        hashes: source_hashes,
        destinations: Vec::new(),
    };
    // Destinations sans relecture (échec d'écriture, déjà vérifiées) : octets comptés.
    col.counter
        .add(file.size * (n - checks.len()) as u64, false, emit);
    if checks.is_empty() {
        return Ok(CopyOutcome::Done(FileResult {
            destinations: status.into_iter().flatten().collect(),
            ..result
        }));
    }
    Ok(CopyOutcome::Pending(
        Pending {
            result,
            status,
            resumed,
            waiting: checks.len(),
        },
        checks,
    ))
}

/// Recrée les dossiers vides et rétablit les dates des dossiers d'origine
/// (l'écriture des fichiers modifie la date des dossiers parents).
fn restore_directories(inv: &SourceInventory, destinations: &[PathBuf]) {
    for dest in destinations {
        for dir in &inv.dirs {
            let _ = fs::create_dir_all(dest.join(&dir.rel));
        }
        // Du plus profond au moins profond, pour ne pas modifier un parent après coup.
        for dir in inv.dirs.iter().rev() {
            let _ = filetime::set_file_mtime(
                dest.join(&dir.rel),
                filetime::FileTime::from_system_time(dir.modified),
            );
        }
    }
}

/// Lance l'offload complet d'une source inventoriée.
pub fn run(
    inv: &SourceInventory,
    spec: &OffloadSpec,
    cancel: &AtomicBool,
    emit: impl FnMut(Event),
) -> Result<OffloadSummary> {
    run_with(inv, spec, cancel, emit, &|_, _| {})
}

/// Variante de `run` avec crochet d'écriture (voir `run_hooked`).
pub(crate) fn run_with(
    inv: &SourceInventory,
    spec: &OffloadSpec,
    cancel: &AtomicBool,
    emit: impl FnMut(Event),
    after_write: &dyn Fn(usize, &Path),
) -> Result<OffloadSummary> {
    let hooks = Hooks {
        after_write,
        after_block: &|_, _| {},
        before_verify: &|_, _| {},
    };
    run_hooked(inv, spec, cancel, emit, &hooks)
}

/// Comme `run`, avec les crochets de test de `Hooks`.
fn run_hooked(
    inv: &SourceInventory,
    spec: &OffloadSpec,
    cancel: &AtomicBool,
    mut emit: impl FnMut(Event),
    hooks: &Hooks,
) -> Result<OffloadSummary> {
    if spec.destinations.is_empty() {
        return Err(Error::Unsupported("aucune destination".into()));
    }
    if spec.algorithms.is_empty() {
        return Err(Error::Unsupported("aucun algorithme d'empreinte".into()));
    }
    let root = absolute(&inv.root);
    for d in &spec.destinations {
        let dest = absolute(d);
        if dest.starts_with(&root) || root.starts_with(&dest) {
            return Err(Error::Unsupported(format!(
                "la destination {} et la source se chevauchent",
                d.display()
            )));
        }
    }
    let started_at = SystemTime::now();
    let algos = &spec.algorithms;
    let meters = Arc::new(Meters::new(spec.destinations.len()));
    let (files, cancelled) = thread::scope(|s| -> Result<(Vec<FileResult>, bool)> {
        // Un vérificateur persistant par destination.
        let (reply_tx, reply_rx) = channel::<VerifyReply>();
        let mut task_tx = Vec::new();
        for dest in 0..spec.destinations.len() {
            let (tx, rx) = sync_channel::<VerifyTask>(VERIFY_AHEAD);
            let reply = reply_tx.clone();
            let before_verify = hooks.before_verify;
            let probe = meters.probe(dest);
            s.spawn(move || verifier(rx, reply, algos, cancel, before_verify, probe));
            task_tx.push(tx);
        }
        drop(reply_tx);
        let mut col = Collector {
            counter: Counter {
                done: 0,
                total: inv.total_bytes * (1 + spec.destinations.len() as u64),
                started: Instant::now(),
                source_bytes: 0,
                last_emit: Instant::now(),
                meters: meters.clone(),
                window: RateWindow::new(Instant::now(), meters.snapshot()),
            },
            rx: reply_rx,
            pending: HashMap::new(),
            done: vec![None; inv.files.len()],
            aborted: false,
            cancel,
        };
        let mut cancelled = false;
        for (index, file) in inv.files.iter().enumerate() {
            col.drain(&mut emit);
            emit(Event::FileStarted {
                index,
                rel: file.rel.clone(),
                size: file.size,
            });
            match copy_stage(index, file, spec, cancel, &mut col, &mut emit, hooks) {
                Ok(CopyOutcome::Done(result)) => {
                    emit(Event::FileDone {
                        index,
                        result: result.clone(),
                    });
                    col.done[index] = Some(result);
                }
                Ok(CopyOutcome::Pending(p, checks)) => {
                    col.pending.insert(index, p);
                    for (dest, path) in checks {
                        let task = VerifyTask { index, dest, path };
                        if task_tx[dest].send(task).is_err() {
                            let lost = std::io::Error::other("vérification interrompue");
                            col.handle((index, dest, Err(lost)), &mut emit);
                        }
                    }
                    if !spec.overlap {
                        col.wait_file(index, &mut emit);
                    }
                }
                Err(Error::Cancelled) => {
                    cancelled = true;
                    break;
                }
                Err(e) => return Err(e),
            }
            if col.aborted {
                cancelled = true;
                break;
            }
        }
        // Toutes les relectures doivent être terminées (ou interrompues)
        // avant les dossiers, le MHL et le rapport.
        // Plus aucune tâche à envoyer : fermer les files permet de détecter la
        // disparition d'un vérificateur au lieu d'attendre indéfiniment.
        drop(task_tx);
        col.wait_all(&mut emit);
        cancelled |= col.aborted;
        if !cancelled {
            restore_directories(inv, &spec.destinations);
        }
        col.counter.flush(&mut emit);
        Ok((col.done.into_iter().flatten().collect(), cancelled))
    })?;
    let finished_at = SystemTime::now();
    let duration_s = finished_at
        .duration_since(started_at)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    Ok(OffloadSummary {
        failed_files: files.iter().filter(|f| !f.ok()).count(),
        total_bytes: inv.total_bytes,
        started_at: rfc3339(started_at),
        finished_at: rfc3339(finished_at),
        duration_s,
        throughput: meters.throughput(duration_s, cancelled),
        cancelled,
        files,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::offload::scan::scan;

    /// Lecteur qui reproduit FILE_FLAG_NO_BUFFERING (Windows) : erreur 87 si
    /// une lecture part d'une position ou demande une taille non alignée.
    struct Unbuffered {
        data: Vec<u8>,
        pos: usize,
    }

    impl Read for Unbuffered {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            const SECTOR: usize = 4096;
            if !self.pos.is_multiple_of(SECTOR) || !buf.len().is_multiple_of(SECTOR) {
                return Err(std::io::Error::from_raw_os_error(87));
            }
            // Comme Windows, renvoie parfois moins que demandé (sans dépasser la fin).
            let want = buf.len().min(SECTOR * 3);
            let n = want.min(self.data.len() - self.pos);
            buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
            self.pos += n;
            Ok(n)
        }
    }

    #[test]
    fn uncached_read_stops_at_file_size_without_unaligned_read() {
        let cancel = AtomicBool::new(false);
        let mut buf = AlignedBuf::new(CHUNK);
        for size in [0usize, 128, 4096, 39_117, CHUNK, CHUNK * 2 + 777] {
            let data: Vec<u8> = (0..size).map(|i| (i % 253) as u8).collect();
            let mut r = Unbuffered {
                data: data.clone(),
                pos: 0,
            };
            let got = hash_uncached(
                &mut r,
                size as u64,
                buf.as_mut_slice(),
                &[HashAlgo::Xxh128],
                &cancel,
            )
            .unwrap_or_else(|e| panic!("taille {size} : {e}"));
            assert_eq!(
                got[0].1,
                crate::offload::hash::hash_data(HashAlgo::Xxh128, &data),
                "taille {size}"
            );
        }
    }

    fn card(dir: &Path) -> PathBuf {
        let root = dir.join("A001");
        fs::create_dir_all(root.join("CLIP")).unwrap();
        // Un gros fichier (plusieurs blocs) et des petits.
        let big: Vec<u8> = (0..(CHUNK * 2 + 12345)).map(|i| (i % 251) as u8).collect();
        fs::write(root.join("CLIP/C0001.MP4"), &big).unwrap();
        fs::write(root.join("CLIP/C0002.MP4"), b"petit").unwrap();
        fs::write(root.join("vide.txt"), b"").unwrap();
        let old = filetime::FileTime::from_unix_time(1_700_000_000, 0);
        filetime::set_file_mtime(root.join("CLIP/C0002.MP4"), old).unwrap();
        root
    }

    fn spec(dests: &[PathBuf]) -> OffloadSpec {
        OffloadSpec {
            destinations: dests.to_vec(),
            algorithms: vec![HashAlgo::Xxh128, HashAlgo::Md5],
            known: None,
            overlap: false,
        }
    }

    fn statuses(s: &OffloadSummary) -> Vec<DestStatus> {
        s.files
            .iter()
            .flat_map(|f| f.destinations.clone())
            .collect()
    }

    #[test]
    fn copies_to_two_destinations_and_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dests = [dir.path().join("SSD1/A001"), dir.path().join("SSD2/A001")];
        let inv = scan(&src).unwrap();
        let mut events = 0;
        let s = run(&inv, &spec(&dests), &AtomicBool::new(false), |_| {
            events += 1
        })
        .unwrap();
        assert_eq!(s.failed_files, 0);
        assert!(statuses(&s).iter().all(|d| *d == DestStatus::Verified));
        assert!(events > 3);
        for d in &dests {
            for f in &inv.files {
                assert_eq!(
                    fs::read(d.join(&f.rel)).unwrap(),
                    fs::read(&f.path).unwrap()
                );
            }
            // Date de modification d'origine conservée, pas de fichier temporaire.
            let m = fs::metadata(d.join("CLIP/C0002.MP4"))
                .unwrap()
                .modified()
                .unwrap();
            assert_eq!(
                m,
                inv.files
                    .iter()
                    .find(|f| f.rel == "CLIP/C0002.MP4")
                    .unwrap()
                    .modified
            );
            assert!(!d.join("CLIP/.C0002.MP4.vfpart").exists());
        }
    }

    #[test]
    fn resume_reverifies_without_copying_and_detects_corruption() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        run(
            &inv,
            &spec(std::slice::from_ref(&dest)),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();

        // Reprise : rien n'est recopié, tout est revérifié.
        let s = run(
            &inv,
            &spec(std::slice::from_ref(&dest)),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert!(statuses(&s)
            .iter()
            .all(|d| *d == DestStatus::ResumedVerified));

        // Corruption silencieuse (même taille) : détectée à la vérification.
        let mut data = fs::read(dest.join("CLIP/C0002.MP4")).unwrap();
        data[0] ^= 0xFF;
        fs::write(dest.join("CLIP/C0002.MP4"), &data).unwrap();
        let s = run(
            &inv,
            &spec(std::slice::from_ref(&dest)),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(s.failed_files, 1);
        let bad = s.files.iter().find(|f| f.rel == "CLIP/C0002.MP4").unwrap();
        assert!(matches!(&bad.destinations[0], DestStatus::Failed(m) if m.contains("empreinte")));
    }

    #[test]
    fn never_overwrites_a_different_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD1/A001");
        fs::create_dir_all(dest.join("CLIP")).unwrap();
        fs::write(dest.join("CLIP/C0002.MP4"), b"autre contenu plus long").unwrap();
        let inv = scan(&src).unwrap();
        let s = run(
            &inv,
            &spec(std::slice::from_ref(&dest)),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(s.failed_files, 1);
        assert_eq!(
            fs::read(dest.join("CLIP/C0002.MP4")).unwrap(),
            b"autre contenu plus long"
        );
    }

    #[test]
    fn a_failing_destination_does_not_stop_the_others() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        // Destination impossible : un fichier occupe la place du dossier CLIP.
        let broken = dir.path().join("BROKEN/A001");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("CLIP"), b"je suis un fichier").unwrap();
        let good = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        let s = run(
            &inv,
            &spec(&[broken, good.clone()]),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        for f in &s.files {
            assert!(
                matches!(f.destinations[1], DestStatus::Verified),
                "{}",
                f.rel
            );
            if f.rel.starts_with("CLIP/") {
                assert!(matches!(f.destinations[0], DestStatus::Failed(_)));
            }
        }
        assert!(good.join("CLIP/C0001.MP4").exists());
    }

    #[test]
    fn rejects_destination_inside_source() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let inv = scan(&src).unwrap();
        let err = run(
            &inv,
            &spec(&[src.join("copie")]),
            &AtomicBool::new(false),
            |_| {},
        );
        assert!(err.is_err());
    }

    #[test]
    fn cancel_leaves_no_partial_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        let cancel = AtomicBool::new(false);
        let s = run(&inv, &spec(std::slice::from_ref(&dest)), &cancel, |e| {
            if matches!(e, Event::Progress { .. }) || matches!(e, Event::FileStarted { .. }) {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert!(s.cancelled);
        let leftovers: Vec<_> = walkdir::WalkDir::new(&dest)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".vfpart"))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn space_check_reports_missing_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let inv = scan(&src).unwrap();
        let checks = check_space(&inv, &[dir.path().join("SSD1/A001")]);
        assert_eq!(checks, vec![None]);
    }

    /// Carte de ~300 petits fichiers (1 à 4 Kio) répartis en dossiers, plus deux gros.
    fn many_files_card(dir: &Path) -> PathBuf {
        let root = dir.join("A001");
        for k in 0..300usize {
            let sub = root.join(format!("CLIP/D{:02}", k % 10));
            fs::create_dir_all(&sub).unwrap();
            let len = 1024 + (k * 37) % 3072;
            let data: Vec<u8> = (0..len).map(|i| ((i * 7 + k) % 251) as u8).collect();
            fs::write(sub.join(format!("F{k:03}.bin")), data).unwrap();
        }
        for (name, seed) in [("GROS1.MP4", 3usize), ("GROS2.MP4", 5)] {
            let big: Vec<u8> = (0..(CHUNK + 4321))
                .map(|i| ((i * seed) % 253) as u8)
                .collect();
            fs::write(root.join(name), big).unwrap();
        }
        root
    }

    fn spec_overlap(dests: &[PathBuf], overlap: bool) -> OffloadSpec {
        OffloadSpec {
            overlap,
            ..spec(dests)
        }
    }

    fn partials(dest: &Path) -> usize {
        walkdir::WalkDir::new(dest)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".vfpart"))
            .count()
    }

    /// Empreintes de la racine d'un MHL, sans les dates.
    fn root_hash(
        inv: &SourceInventory,
        s: &OffloadSummary,
        dest: &Path,
        sp: &OffloadSpec,
    ) -> String {
        use crate::offload::mhl::{write_generation, MhlAuthor};
        let p = write_generation(inv, s, 0, dest, &sp.algorithms, &MhlAuthor::default()).unwrap();
        let xml = fs::read_to_string(p).unwrap();
        let block = xml
            .split("<roothash>")
            .nth(1)
            .and_then(|r| r.split("</roothash>").next())
            .unwrap();
        block
            .lines()
            .filter_map(|l| {
                let (_, rest) = l.split_once('>')?;
                let (value, _) = rest.split_once("</")?;
                Some(value.to_string())
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn overlap_same_results_and_mhl_as_sequential() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let inv = scan(&src).unwrap();
        let seq_dest = dir.path().join("SEQ/A001");
        let ovl_dest = dir.path().join("OVL1/A001");
        let ovl_dest2 = dir.path().join("OVL2/A001");

        let t = Instant::now();
        let sp = spec_overlap(std::slice::from_ref(&seq_dest), false);
        let seq = run(&inv, &sp, &AtomicBool::new(false), |_| {}).unwrap();
        let t_seq = t.elapsed();
        let t = Instant::now();
        let spo = spec_overlap(&[ovl_dest.clone(), ovl_dest2], true);
        let mut done_events = 0;
        let ovl = run(&inv, &spo, &AtomicBool::new(false), |e| {
            if matches!(e, Event::FileDone { .. }) {
                done_events += 1;
            }
        })
        .unwrap();
        let t_ovl = t.elapsed();
        eprintln!("300 fichiers : sans recouvrement {t_seq:?}, avec {t_ovl:?}");

        assert_eq!(done_events, inv.files.len());
        assert_eq!(seq.files.len(), inv.files.len());
        assert_eq!(ovl.files.len(), inv.files.len());
        for ((a, b), f) in seq.files.iter().zip(&ovl.files).zip(&inv.files) {
            assert_eq!(a.rel, f.rel);
            assert_eq!(b.rel, f.rel);
            assert_eq!(a.hashes, b.hashes);
            assert_eq!(a.destinations[0], b.destinations[0]);
            assert_eq!(b.destinations[0], b.destinations[1]);
            assert_eq!(b.destinations[0], DestStatus::Verified);
        }
        assert_eq!(
            root_hash(&inv, &seq, &seq_dest, &sp),
            root_hash(&inv, &ovl, &ovl_dest, &spo)
        );
        assert_eq!(partials(&ovl_dest), 0);
    }

    #[test]
    fn dest_modified_between_write_and_verify_is_detected() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let inv = scan(&src).unwrap();
        let dest = dir.path().join("SSD1/A001");
        let target = 120;
        let s = run_with(
            &inv,
            &spec_overlap(std::slice::from_ref(&dest), true),
            &AtomicBool::new(false),
            |_| {},
            &|index, path| {
                if index == target {
                    // Même taille, un octet change : seule l'empreinte le voit.
                    let mut data = fs::read(path).unwrap();
                    data[0] ^= 0xFF;
                    fs::write(path, data).unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(s.failed_files, 1);
        for (i, f) in s.files.iter().enumerate() {
            if i == target {
                assert!(
                    matches!(&f.destinations[0], DestStatus::Failed(m) if m.contains("empreinte")),
                    "{:?}",
                    f.destinations
                );
            } else {
                assert_eq!(f.destinations[0], DestStatus::Verified, "{}", f.rel);
            }
        }
    }

    #[test]
    fn source_modified_during_copy_is_failed() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let inv = scan(&src).unwrap();
        let dests = [dir.path().join("SSD1/A001"), dir.path().join("SSD2/A001")];
        let target = 50;
        let victim = inv.files[target].path.clone();
        let mut last = (0, 0);
        let s = run_with(
            &inv,
            &spec_overlap(&dests, true),
            &AtomicBool::new(false),
            |e| {
                if let Event::Progress { done, total, .. } = e {
                    last = (done, total);
                }
            },
            &|index, _| {
                if index == target {
                    let mut f = OpenOptions::new().append(true).open(&victim).unwrap();
                    f.write_all(b"ajout").unwrap();
                }
            },
        )
        .unwrap();
        assert_eq!(s.failed_files, 1);
        // Les relectures non faites comptent : la progression atteint le total.
        assert_eq!(last.0, last.1);
        let bad = &s.files[target];
        for d in &bad.destinations {
            assert_eq!(
                *d,
                DestStatus::Failed("source modifiée pendant la copie".into())
            );
        }
        for d in &dests {
            assert!(!d.join(&bad.rel).exists());
        }
        assert!(s
            .files
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != target)
            .all(|(_, f)| f.ok()));
    }

    #[test]
    fn cancel_during_verify_leaves_no_partial_and_resume_is_exact() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let inv = scan(&src).unwrap();
        let dest = dir.path().join("SSD1/A001");
        let sp = spec_overlap(std::slice::from_ref(&dest), true);

        let cancel = AtomicBool::new(false);
        let s = run_with(&inv, &sp, &cancel, |_| {}, &|index, _| {
            if index == 150 {
                cancel.store(true, Ordering::Relaxed);
            }
        })
        .unwrap();
        assert!(s.cancelled);
        // Le fichier 150 est renommé mais pas relu : il n'est pas dans le résultat.
        assert!(s.files.len() <= 150);
        assert!(s.files.iter().all(FileResult::ok));
        assert!(s.files.iter().all(|f| f.rel != inv.files[150].rel));
        assert_eq!(partials(&dest), 0);

        // Reprise : tout est complet et exact, les fichiers déjà copiés sont revérifiés.
        let s = run(&inv, &sp, &AtomicBool::new(false), |_| {}).unwrap();
        assert!(!s.cancelled);
        assert_eq!(s.failed_files, 0);
        assert_eq!(s.files.len(), inv.files.len());
        assert!(statuses(&s).contains(&DestStatus::ResumedVerified));
        assert!(statuses(&s).contains(&DestStatus::Verified));
        for f in &inv.files {
            assert_eq!(
                fs::read(dest.join(&f.rel)).unwrap(),
                fs::read(&f.path).unwrap()
            );
        }
        assert_eq!(partials(&dest), 0);
    }

    #[test]
    fn failing_destination_with_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let broken = dir.path().join("BROKEN/A001");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("CLIP"), b"je suis un fichier").unwrap();
        let good = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        let s = run(
            &inv,
            &spec_overlap(&[broken, good.clone()], true),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(s.files.len(), inv.files.len());
        for (f, i) in s.files.iter().zip(&inv.files) {
            assert_eq!(f.rel, i.rel);
            assert_eq!(f.destinations[1], DestStatus::Verified, "{}", f.rel);
            if f.rel.starts_with("CLIP/") {
                assert!(matches!(f.destinations[0], DestStatus::Failed(_)));
            } else {
                assert_eq!(f.destinations[0], DestStatus::Verified);
            }
        }
        assert_eq!(s.failed_files, 300);
    }

    #[test]
    fn busy_file_is_retried_three_times_then_reported() {
        let busy = |e: &std::io::Error| e.raw_os_error() == Some(32);
        let mut calls = 0;
        let r = retry_busy(
            || {
                calls += 1;
                if calls < 3 {
                    Err(std::io::Error::from_raw_os_error(32))
                } else {
                    Ok(calls)
                }
            },
            busy,
            Duration::from_millis(1),
        );
        assert_eq!(r.unwrap(), 3);

        let mut calls = 0;
        let r: std::io::Result<()> = retry_busy(
            || {
                calls += 1;
                Err(std::io::Error::from_raw_os_error(32))
            },
            busy,
            Duration::from_millis(1),
        );
        assert!(r.is_err());
        assert_eq!(calls, 4, "un essai puis trois nouvelles tentatives");

        // Une autre erreur n'est pas réessayée.
        let mut calls = 0;
        let _: std::io::Result<()> = retry_busy(
            || {
                calls += 1;
                Err(std::io::Error::from_raw_os_error(2))
            },
            busy,
            Duration::from_millis(1),
        );
        assert_eq!(calls, 1);
    }

    #[test]
    fn cancel_in_the_middle_of_a_big_file_leaves_nothing_and_resume_is_exact() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("A001");
        fs::create_dir_all(&root).unwrap();
        let big: Vec<u8> = (0..CHUNK * 4).map(|i| (i % 249) as u8).collect();
        fs::write(root.join("GROS.MP4"), &big).unwrap();
        let inv = scan(&root).unwrap();
        let dest = dir.path().join("SSD1/A001");
        let sp = spec_overlap(std::slice::from_ref(&dest), true);
        let cancel = AtomicBool::new(false);
        // Annulation déterministe : après le deuxième bloc sur quatre.
        let hooks = Hooks {
            after_write: &|_, _| {},
            after_block: &|_, blocks| {
                if blocks == 2 {
                    cancel.store(true, Ordering::Relaxed);
                }
            },
            before_verify: &|_, _| {},
        };
        let s = run_hooked(&inv, &sp, &cancel, |_| {}, &hooks).unwrap();
        assert!(s.cancelled);
        assert!(s.files.is_empty());
        assert_eq!(partials(&dest), 0);
        assert!(!dest.join("GROS.MP4").exists());

        let s = run(&inv, &sp, &AtomicBool::new(false), |_| {}).unwrap();
        assert_eq!(s.failed_files, 0);
        assert_eq!(statuses(&s), vec![DestStatus::Verified]);
        assert_eq!(fs::read(dest.join("GROS.MP4")).unwrap(), big);
    }

    #[test]
    fn verifier_panic_is_reported_without_blocking() {
        for n in [1usize, 2] {
            let dir = tempfile::tempdir().unwrap();
            let src = many_files_card(dir.path());
            let inv = scan(&src).unwrap();
            let dests: Vec<PathBuf> = (0..n)
                .map(|k| dir.path().join(format!("SSD{k}/A001")))
                .collect();
            let s = run_hooked(
                &inv,
                &spec_overlap(&dests, true),
                &AtomicBool::new(false),
                |_| {},
                &Hooks {
                    after_write: &|_, _| {},
                    after_block: &|_, _| {},
                    before_verify: &|index, _| {
                        if index == 5 {
                            panic!("panique volontaire du vérificateur");
                        }
                    },
                },
            )
            .unwrap();
            assert_eq!(s.files.len(), inv.files.len(), "{n} destination(s)");
            assert_eq!(s.failed_files, 1);
            for (i, f) in s.files.iter().enumerate() {
                for d in &f.destinations {
                    if i == 5 {
                        assert!(
                            matches!(d, DestStatus::Failed(m) if m.contains("vérification interrompue")),
                            "{d:?}"
                        );
                    } else {
                        assert_eq!(*d, DestStatus::Verified, "{}", f.rel);
                    }
                }
            }
        }
    }

    #[test]
    fn overlap_verifies_file_n_while_copying_the_next() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let inv = scan(&src).unwrap();
        let dest = dir.path().join("SSD1/A001");
        let next_started = AtomicBool::new(false);
        let waited_in_vain = AtomicBool::new(false);
        let mut events: Vec<(&str, usize)> = Vec::new();
        let s = run_hooked(
            &inv,
            &spec_overlap(std::slice::from_ref(&dest), true),
            &AtomicBool::new(false),
            |e| match e {
                Event::FileStarted { index, .. } => {
                    if index == 11 {
                        next_started.store(true, Ordering::SeqCst);
                    }
                    events.push(("started", index));
                }
                Event::FileDone { index, .. } => events.push(("done", index)),
                _ => {}
            },
            &Hooks {
                after_write: &|_, _| {},
                after_block: &|_, _| {},
                // La relecture du fichier 10 ne se termine qu'une fois le 11 commencé.
                before_verify: &|index, _| {
                    if index == 10 {
                        let t = Instant::now();
                        while !next_started.load(Ordering::SeqCst) {
                            if t.elapsed().as_secs() >= 10 {
                                waited_in_vain.store(true, Ordering::SeqCst);
                                break;
                            }
                            thread::sleep(Duration::from_millis(1));
                        }
                    }
                },
            },
        )
        .unwrap();
        assert!(
            !waited_in_vain.load(Ordering::SeqCst),
            "pas de recouvrement"
        );
        assert_eq!(s.failed_files, 0);
        let pos = |kind: &str, i: usize| events.iter().position(|e| *e == (kind, i)).unwrap();
        assert!(pos("started", 11) < pos("done", 10));
        // La réponse du fichier 10 est traitée avant la fin du suivant.
        assert!(pos("done", 10) < pos("done", 11));
    }

    #[test]
    fn rates_and_throughput_json_contract() {
        let e = Event::Rates {
            source: 100.0,
            write: vec![50.0],
            verify: vec![10.0],
            hash_wait_ratio: 0.25,
        };
        assert_eq!(
            serde_json::to_value(&e).unwrap(),
            serde_json::json!({"type":"rates","source":100.0,"write":[50.0],"verify":[10.0],"hash_wait_ratio":0.25})
        );
        let t = serde_json::to_value(Throughput::default()).unwrap();
        for k in [
            "source_read",
            "write",
            "verify",
            "effective_rate",
            "bottleneck",
        ] {
            assert!(t.get(k).is_some(), "{k}");
        }
        assert!(t["source_read"].get("active_s").is_some());
    }

    #[test]
    fn meters_count_exact_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dests = [dir.path().join("D1/A001"), dir.path().join("D2/A001")];
        let inv = scan(&src).unwrap();
        let mut rates = Vec::new();
        let s = run(&inv, &spec(&dests), &AtomicBool::new(false), |e| {
            if let Event::Rates { .. } = e {
                rates.push(e);
            }
        })
        .unwrap();
        let t = &s.throughput;
        assert_eq!(t.source_read.bytes, inv.total_bytes);
        assert_eq!(t.write.len(), 2);
        assert_eq!(t.verify.len(), 2);
        for i in 0..2 {
            assert_eq!(t.write[i].bytes, inv.total_bytes);
            assert_eq!(t.verify[i].bytes, inv.total_bytes);
            assert!(t.write[i].busy_s > 0.0 && t.verify[i].busy_s > 0.0);
        }
        assert!(t.effective_rate.unwrap() > 0.0);
        assert!(
            t.bottleneck.is_empty()
                || t.bottleneck == "source"
                || t.bottleneck.starts_with("dest:")
                || t.bottleneck == "cpu"
        );
        // Les événements de débit (s'il y en a eu) portent une valeur par destination.
        for e in rates {
            if let Event::Rates { write, verify, .. } = e {
                assert_eq!((write.len(), verify.len()), (2, 2));
            }
        }
    }

    #[test]
    fn resumed_files_not_counted_as_written() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dests = [dir.path().join("D1/A001")];
        let inv = scan(&src).unwrap();
        run(&inv, &spec(&dests), &AtomicBool::new(false), |_| {}).unwrap();
        // Reprise : tout est déjà là, seule la relecture a lieu.
        let s = run(&inv, &spec(&dests), &AtomicBool::new(false), |_| {}).unwrap();
        let t = &s.throughput;
        assert_eq!(t.write[0].bytes, 0);
        assert_eq!(t.verify[0].bytes, inv.total_bytes);
        assert_eq!(t.source_read.bytes, inv.total_bytes);
    }

    #[test]
    fn known_identical_not_counted() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dests = [dir.path().join("D1/A001")];
        let inv = scan(&src).unwrap();
        let first = run(&inv, &spec(&dests), &AtomicBool::new(false), |_| {}).unwrap();
        let known = Known {
            source: first
                .files
                .iter()
                .map(|f| (f.rel.clone(), f.hashes.clone()))
                .collect(),
            identical: vec![first
                .files
                .iter()
                .map(|f| {
                    let m = fs::metadata(dests[0].join(&f.rel)).unwrap();
                    (f.rel.clone(), (m.len(), m.modified().ok()))
                })
                .collect()],
        };
        let mut sp = spec(&dests);
        sp.known = Some(known);
        let s = run(&inv, &sp, &AtomicBool::new(false), |_| {}).unwrap();
        let t = &s.throughput;
        assert_eq!(t.source_read.bytes, 0);
        assert_eq!(t.write[0].bytes, 0);
        assert_eq!(t.verify[0].bytes, 0);
        assert_eq!(t.bottleneck, "");
        // Rien n'a été lu à la source : pas de débit global.
        assert_eq!(t.effective_rate, None);
        assert!(s.failed_files == 0 && !s.files.is_empty());
    }

    #[test]
    fn meters_count_exact_bytes_with_overlap() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let dests = [dir.path().join("D1/A001"), dir.path().join("D2/A001")];
        let inv = scan(&src).unwrap();
        let s = run(
            &inv,
            &spec_overlap(&dests, true),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        let t = &s.throughput;
        assert_eq!(t.source_read.bytes, inv.total_bytes);
        for i in 0..2 {
            assert_eq!(t.write[i].bytes, inv.total_bytes);
            assert_eq!(t.verify[i].bytes, inv.total_bytes);
        }
    }

    #[test]
    fn failed_write_counts_fewer_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let broken = dir.path().join("BROKEN/A001");
        fs::create_dir_all(&broken).unwrap();
        fs::write(broken.join("CLIP"), b"je suis un fichier").unwrap();
        let good = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        let s = run(
            &inv,
            &spec(&[broken, good]),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        let t = &s.throughput;
        assert!(t.write[0].bytes < inv.total_bytes);
        assert_eq!(t.write[1].bytes, inv.total_bytes);
        assert_eq!(t.source_read.bytes, inv.total_bytes);
    }

    #[test]
    fn cancelled_run_has_no_effective_rate() {
        let dir = tempfile::tempdir().unwrap();
        let src = card(dir.path());
        let dest = dir.path().join("SSD1/A001");
        let inv = scan(&src).unwrap();
        let s = run(
            &inv,
            &spec(std::slice::from_ref(&dest)),
            &AtomicBool::new(true),
            |_| {},
        )
        .unwrap();
        assert!(s.cancelled);
        assert_eq!(s.throughput.effective_rate, None);
    }

    #[test]
    fn writer_refuses_to_rename_an_incomplete_copy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sous/F.bin");
        let (tx, rx) = sync_channel::<Arc<Vec<u8>>>(4);
        tx.send(Arc::new(vec![1u8; 100])).unwrap();
        drop(tx);
        let meters = Meters::new(1);
        let r = writer(rx, path.clone(), SystemTime::now(), 200, &meters, 0);
        assert!(r.is_err());
        assert!(!path.exists());
        assert!(!temp_path(&path).exists());

        let (tx, rx) = sync_channel::<Arc<Vec<u8>>>(4);
        tx.send(Arc::new(vec![1u8; 200])).unwrap();
        drop(tx);
        writer(rx, path.clone(), SystemTime::now(), 200, &meters, 0).unwrap();
        assert_eq!(fs::metadata(&path).unwrap().len(), 200);
    }

    #[test]
    fn source_changed_since_inventory_is_failed() {
        let dir = tempfile::tempdir().unwrap();
        let src = many_files_card(dir.path());
        let inv = scan(&src).unwrap();
        let victim = &inv.files[7];
        let mut f = OpenOptions::new().append(true).open(&victim.path).unwrap();
        f.write_all(b"ajout apres l'inventaire").unwrap();
        drop(f);
        let dest = dir.path().join("SSD1/A001");
        let s = run(
            &inv,
            &spec_overlap(std::slice::from_ref(&dest), true),
            &AtomicBool::new(false),
            |_| {},
        )
        .unwrap();
        assert_eq!(s.failed_files, 1);
        assert_eq!(
            s.files[7].destinations[0],
            DestStatus::Failed("source modifiée depuis l'inventaire".into())
        );
        assert!(!dest.join(&victim.rel).exists());
    }
}
