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

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::{Instant, SystemTime};

use serde::Serialize;

use super::existing::Known;
use super::hash::{HashAlgo, MultiHasher};
use super::io::{drop_cache, open_uncached, read_full, AlignedBuf};
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
) -> std::result::Result<(), String> {
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
        for chunk in rx {
            f.write_all(&chunk)?;
        }
        f.sync_all()?;
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
    let mut f = open_uncached(path)?;
    let size = f.metadata()?.len();
    let mut buf = AlignedBuf::new(CHUNK);
    hash_uncached(&mut f, size, buf.as_mut_slice(), algos, cancel)
}

/// Lecture sans cache : sous Windows (FILE_FLAG_NO_BUFFERING), chaque lecture
/// doit partir d'une position alignée sur le secteur. Après la dernière
/// lecture, partielle, la position ne l'est plus : on s'arrête donc à la
/// taille connue du fichier au lieu de relire pour constater la fin
/// (relecture qui échouait avec « Paramètre incorrect », os error 87).
fn hash_uncached(
    f: &mut impl Read,
    size: u64,
    buf: &mut [u8],
    algos: &[HashAlgo],
    cancel: &AtomicBool,
) -> std::io::Result<Vec<(HashAlgo, String)>> {
    let mut hasher = MultiHasher::new(algos);
    let mut done = 0u64;
    while done < size {
        if cancel.load(Ordering::Relaxed) {
            return Err(std::io::Error::other("annulé"));
        }
        // Lecture du tampon entier : la taille demandée reste un multiple du secteur.
        let n = loop {
            match f.read(buf) {
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                r => break r?,
            }
        };
        if n == 0 {
            break; // fichier raccourci pendant la lecture : l'empreinte différera
        }
        hasher.update(&buf[..n]);
        done += n as u64;
    }
    Ok(hasher.finish())
}

struct Counter {
    done: u64,
    total: u64,
    started: Instant,
    source_bytes: u64,
    last_emit: Instant,
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
    }
}

/// Copie et vérifie un fichier vers toutes les destinations.
fn offload_file(
    file: &SourceFile,
    spec: &OffloadSpec,
    cancel: &AtomicBool,
    counter: &mut Counter,
    emit: &mut impl FnMut(Event),
) -> Result<FileResult> {
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
                counter.add(file.size * (1 + n as u64), false, emit);
                return Ok(FileResult {
                    rel: file.rel.clone(),
                    size: file.size,
                    modified: rfc3339(file.modified),
                    hashes: hashes.clone(),
                    destinations: status.into_iter().flatten().collect(),
                });
            }
        }
    }

    // Destinations à écrire (les fichiers complets déjà présents sont seulement
    // revérifiés ; en mode « compléter », les fichiers non identiques sont remplacés).
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
                handles.push((i, thread::spawn(move || writer(rx, path, modified))));
            }
        }
    }

    // Lecture unique de la source.
    let mut hasher = MultiHasher::new(&spec.algorithms);
    let read_result = (|| -> std::io::Result<()> {
        let mut src = File::open(&file.path)?;
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(std::io::Error::other("annulé"));
            }
            let mut buf = vec![0u8; CHUNK];
            let got = read_full(&mut src, &mut buf)?;
            if got == 0 {
                break;
            }
            buf.truncate(got);
            hasher.update(&buf);
            let chunk = Arc::new(buf);
            // Un disque en échec ferme son canal : on continue avec les autres.
            senders.retain(|(_, tx)| tx.send(chunk.clone()).is_ok());
            counter.add(got as u64, true, emit);
        }
        Ok(())
    })();
    drop(senders);
    if let Err(e) = &read_result {
        // Source illisible ou annulation : les fichiers temporaires sont supprimés.
        for (_, h) in handles {
            let _ = h.join();
        }
        for path in &finals {
            let _ = fs::remove_file(temp_path(path));
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(Error::Cancelled);
        }
        let msg = format!("lecture de la source impossible : {e}");
        return Ok(FileResult {
            rel: file.rel.clone(),
            size: file.size,
            modified: rfc3339(file.modified),
            hashes: Vec::new(),
            destinations: vec![DestStatus::Failed(msg); n],
        });
    }
    let source_hashes = hasher.finish();
    for (i, h) in handles {
        match h.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => status[i] = Some(DestStatus::Failed(format!("écriture : {e}"))),
            Err(_) => status[i] = Some(DestStatus::Failed("écriture interrompue".into())),
        }
    }

    // Vérification de chaque destination en parallèle, depuis le disque.
    let checks: Vec<(usize, PathBuf)> = (0..n)
        .filter(|&i| status[i].is_none())
        .map(|i| (i, finals[i].clone()))
        .collect();
    let algos = &spec.algorithms;
    let results: Vec<(usize, std::io::Result<Hashes>)> = thread::scope(|s| {
        let hs: Vec<_> = checks
            .into_iter()
            .map(|(i, path)| (i, s.spawn(move || hash_from_disk(&path, algos, cancel))))
            .collect();
        hs.into_iter()
            .map(|(i, h)| {
                (
                    i,
                    h.join()
                        .unwrap_or_else(|_| Err(std::io::Error::other("vérification interrompue"))),
                )
            })
            .collect()
    });
    if cancel.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    for (i, r) in results {
        counter.add(file.size, false, emit);
        status[i] = Some(match r {
            Ok(h) if h == source_hashes => {
                if resumed[i] {
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
    }

    Ok(FileResult {
        rel: file.rel.clone(),
        size: file.size,
        modified: rfc3339(file.modified),
        hashes: source_hashes,
        destinations: status
            .into_iter()
            .map(|s| s.expect("statut défini"))
            .collect(),
    })
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
    mut emit: impl FnMut(Event),
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
    let mut counter = Counter {
        done: 0,
        total: inv.total_bytes * (1 + spec.destinations.len() as u64),
        started: Instant::now(),
        source_bytes: 0,
        last_emit: Instant::now(),
    };
    let mut files = Vec::with_capacity(inv.files.len());
    let mut cancelled = false;
    for (index, file) in inv.files.iter().enumerate() {
        emit(Event::FileStarted {
            index,
            rel: file.rel.clone(),
            size: file.size,
        });
        match offload_file(file, spec, cancel, &mut counter, &mut emit) {
            Ok(result) => {
                emit(Event::FileDone {
                    index,
                    result: result.clone(),
                });
                files.push(result);
            }
            Err(Error::Cancelled) => {
                cancelled = true;
                break;
            }
            Err(e) => return Err(e),
        }
    }
    if !cancelled {
        restore_directories(inv, &spec.destinations);
    }
    counter.flush(&mut emit);
    let finished_at = SystemTime::now();
    Ok(OffloadSummary {
        failed_files: files.iter().filter(|f| !f.ok()).count(),
        total_bytes: inv.total_bytes,
        started_at: rfc3339(started_at),
        finished_at: rfc3339(finished_at),
        duration_s: finished_at
            .duration_since(started_at)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0),
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
}
