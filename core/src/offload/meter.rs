//! Mesure du débit réel de l'offload, par disque et par phase.
//!
//! Seul le temps passé dans les appels d'entrée-sortie est compté (lecture de
//! la source, écriture fsync compris, relecture de chaque destination), plus
//! le temps où le calcul d'empreintes fait attendre la lecture (goulot
//! processeur). Aucun verrou, aucun appel d'horloge par octet : un
//! `Instant::now()` par appel d'E/S, soit par bloc de 8 Mio.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;

/// Compteur partagé entre fils : octets, temps occupé et fenêtre d'activité.
#[derive(Debug)]
pub struct Meter {
    bytes: AtomicU64,
    busy_ns: AtomicU64,
    /// Début de la première opération, en ns depuis `origin` (`u64::MAX` : aucune).
    first_ns: AtomicU64,
    /// Fin de la dernière opération, en ns depuis `origin`.
    last_ns: AtomicU64,
    origin: Instant,
}

fn ns(d: Duration) -> u64 {
    u64::try_from(d.as_nanos()).unwrap_or(u64::MAX)
}

impl Meter {
    pub fn new(origin: Instant) -> Self {
        Self {
            bytes: AtomicU64::new(0),
            busy_ns: AtomicU64::new(0),
            first_ns: AtomicU64::new(u64::MAX),
            last_ns: AtomicU64::new(0),
            origin,
        }
    }

    /// Enregistre une opération commencée à `start` et qui se termine maintenant.
    pub fn record(&self, bytes: u64, start: Instant) {
        self.record_between(bytes, start, Instant::now());
    }

    /// Enregistre une opération entre deux instants connus.
    pub fn record_between(&self, bytes: u64, start: Instant, end: Instant) {
        self.bytes.fetch_add(bytes, Ordering::Relaxed);
        self.busy_ns
            .fetch_add(ns(end.saturating_duration_since(start)), Ordering::Relaxed);
        self.first_ns.fetch_min(
            ns(start.saturating_duration_since(self.origin)),
            Ordering::Relaxed,
        );
        self.last_ns.fetch_max(
            ns(end.saturating_duration_since(self.origin)),
            Ordering::Relaxed,
        );
    }

    pub fn bytes(&self) -> u64 {
        self.bytes.load(Ordering::Relaxed)
    }

    pub fn busy_ns(&self) -> u64 {
        self.busy_ns.load(Ordering::Relaxed)
    }

    pub fn stat(&self) -> PhaseStat {
        let first = self.first_ns.load(Ordering::Relaxed);
        let last = self.last_ns.load(Ordering::Relaxed);
        PhaseStat {
            bytes: self.bytes(),
            busy_s: self.busy_ns() as f64 / 1e9,
            active_s: if first == u64::MAX {
                0.0
            } else {
                last.saturating_sub(first) as f64 / 1e9
            },
        }
    }
}

/// Mesure d'une phase : octets, temps passé dans les appels d'E/S, durée
/// entre la première et la dernière opération.
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PhaseStat {
    pub bytes: u64,
    pub busy_s: f64,
    pub active_s: f64,
}

/// Tous les compteurs d'un offload.
#[derive(Debug)]
pub struct Meters {
    pub source: Meter,
    pub writes: Vec<Meter>,
    pub verifies: Vec<Meter>,
    /// Temps où le calcul d'empreintes a fait attendre la lecture.
    pub cpu: Meter,
}

/// Compteurs d'une relecture : disque lu et attente du calcul.
#[derive(Clone, Copy)]
pub struct Probe<'a> {
    pub io: &'a Meter,
    pub cpu: &'a Meter,
}

/// Valeurs des compteurs à un instant (entrée de la fenêtre glissante).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    pub source: u64,
    pub write: Vec<u64>,
    pub verify: Vec<u64>,
    pub cpu_ns: u64,
}

impl Meters {
    pub fn new(destinations: usize) -> Self {
        let origin = Instant::now();
        let many = || (0..destinations).map(|_| Meter::new(origin)).collect();
        Self {
            source: Meter::new(origin),
            writes: many(),
            verifies: many(),
            cpu: Meter::new(origin),
        }
    }

    pub fn probe(&self, dest: usize) -> Probe<'_> {
        Probe {
            io: &self.verifies[dest],
            cpu: &self.cpu,
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            source: self.source.bytes(),
            write: self.writes.iter().map(Meter::bytes).collect(),
            verify: self.verifies.iter().map(Meter::bytes).collect(),
            cpu_ns: self.cpu.busy_ns(),
        }
    }

    /// Bilan de fin de copie. `total_bytes` : octets de la source ;
    /// `duration_s` : durée totale, copie et vérification comprises.
    pub fn throughput(&self, total_bytes: u64, duration_s: f64) -> Throughput {
        let dest_busy: Vec<f64> = self
            .writes
            .iter()
            .zip(&self.verifies)
            .map(|(w, v)| (w.busy_ns() + v.busy_ns()) as f64 / 1e9)
            .collect();
        Throughput {
            source_read: self.source.stat(),
            write: self.writes.iter().map(Meter::stat).collect(),
            verify: self.verifies.iter().map(Meter::stat).collect(),
            effective_rate: if duration_s > 0.0 {
                total_bytes as f64 / duration_s
            } else {
                0.0
            },
            bottleneck: bottleneck(
                duration_s,
                self.source.busy_ns() as f64 / 1e9,
                &dest_busy,
                self.cpu.busy_ns() as f64 / 1e9,
            ),
        }
    }
}

/// Bilan de débit d'un offload.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Throughput {
    pub source_read: PhaseStat,
    /// Une entrée par destination.
    pub write: Vec<PhaseStat>,
    pub verify: Vec<PhaseStat>,
    /// Octets de la source divisés par la durée totale (copie et vérification),
    /// en octets par seconde : le chiffre comparable aux autres logiciels.
    pub effective_rate: f64,
    /// `source`, `dest:<i>` (i depuis 0) ou `cpu` ; vide si rien n'a été mesuré.
    pub bottleneck: String,
}

/// Facteur limitant : le plus fort taux d'occupation (temps d'E/S sur durée
/// totale). Un disque de destination cumule écriture et relecture. À égalité,
/// la source passe avant les destinations, puis le calcul.
pub fn bottleneck(
    duration_s: f64,
    source_busy_s: f64,
    dests_busy_s: &[f64],
    cpu_busy_s: f64,
) -> String {
    let ratio = |busy: f64| {
        if duration_s > 0.0 {
            busy / duration_s
        } else {
            busy
        }
    };
    let mut best = (0.0, String::new());
    let mut consider = |busy: f64, name: String| {
        if ratio(busy) > best.0 {
            best = (ratio(busy), name);
        }
    };
    consider(source_busy_s, "source".into());
    for (i, &b) in dests_busy_s.iter().enumerate() {
        consider(b, format!("dest:{i}"));
    }
    consider(cpu_busy_s, "cpu".into());
    best.1
}

/// Débits sur la fenêtre glissante, en octets par seconde.
#[derive(Debug, Clone, PartialEq)]
pub struct RatesSample {
    pub source: f64,
    pub write: Vec<f64>,
    pub verify: Vec<f64>,
    /// Part du temps où la lecture attend le calcul (0 à 1).
    pub hash_wait_ratio: f64,
}

/// Fenêtre glissante d'une seconde. Pure : les instants sont fournis par
/// l'appelant.
pub struct RateWindow {
    samples: VecDeque<(Instant, Snapshot)>,
    last_emit: Instant,
}

const WINDOW: Duration = Duration::from_secs(1);

impl RateWindow {
    pub fn new(now: Instant, snap: Snapshot) -> Self {
        Self {
            samples: VecDeque::from([(now, snap)]),
            last_emit: now,
        }
    }

    /// Ajoute un relevé ; renvoie les débits au plus une fois par seconde.
    pub fn tick(&mut self, now: Instant, snap: Snapshot) -> Option<RatesSample> {
        self.samples.push_back((now, snap));
        // On garde comme référence le dernier relevé vieux d'au moins une seconde.
        if let Some(limit) = now.checked_sub(WINDOW) {
            while self.samples.len() >= 2 && self.samples[1].0 <= limit {
                self.samples.pop_front();
            }
        }
        if now.saturating_duration_since(self.last_emit) < WINDOW {
            return None;
        }
        self.last_emit = now;
        let (t0, old) = self.samples.front()?;
        let cur = &self.samples.back()?.1;
        let dt = now.saturating_duration_since(*t0);
        if dt.is_zero() {
            return None;
        }
        let secs = dt.as_secs_f64();
        let rate = |new: u64, old: u64| new.saturating_sub(old) as f64 / secs;
        let zip = |a: &[u64], b: &[u64]| -> Vec<f64> {
            a.iter()
                .enumerate()
                .map(|(i, &n)| rate(n, b.get(i).copied().unwrap_or(0)))
                .collect()
        };
        Some(RatesSample {
            source: rate(cur.source, old.source),
            write: zip(&cur.write, &old.write),
            verify: zip(&cur.verify, &old.verify),
            hash_wait_ratio: (cur.cpu_ns.saturating_sub(old.cpu_ns) as f64 / dt.as_nanos() as f64)
                .clamp(0.0, 1.0),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(source: u64, write: u64, verify: u64, cpu_ns: u64) -> Snapshot {
        Snapshot {
            source,
            write: vec![write],
            verify: vec![verify],
            cpu_ns,
        }
    }

    #[test]
    fn rate_window_pure() {
        let t0 = Instant::now();
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        let mut w = RateWindow::new(t0, snap(0, 0, 0, 0));
        // Moins d'une seconde : rien.
        assert_eq!(w.tick(at(500), snap(50, 0, 0, 0)), None);
        // Une seconde : débit moyen de la fenêtre, 25 % d'attente du calcul.
        let r = w
            .tick(at(1000), snap(100, 40, 10, 250_000_000))
            .expect("une seconde écoulée");
        assert_eq!(r.source, 100.0);
        assert_eq!(r.write, vec![40.0]);
        assert_eq!(r.verify, vec![10.0]);
        assert!((r.hash_wait_ratio - 0.25).abs() < 1e-9);
        // Au plus un événement par seconde.
        assert_eq!(w.tick(at(1500), snap(200, 40, 10, 250_000_000)), None);
        // La fenêtre glisse : référence = relevé de 1000 ms.
        let r = w
            .tick(at(2000), snap(300, 40, 110, 250_000_000))
            .expect("nouvelle seconde");
        assert_eq!(r.source, 200.0);
        assert_eq!(r.write, vec![0.0]);
        assert_eq!(r.verify, vec![100.0]);
        assert_eq!(r.hash_wait_ratio, 0.0);
    }

    #[test]
    fn bottleneck_picks_highest_busy_ratio() {
        assert_eq!(bottleneck(10.0, 2.0, &[3.0, 8.0], 1.0), "dest:1");
        assert_eq!(bottleneck(10.0, 9.0, &[3.0, 8.0], 1.0), "source");
        assert_eq!(bottleneck(10.0, 2.0, &[3.0], 9.5), "cpu");
        assert_eq!(bottleneck(10.0, 0.0, &[0.0], 0.0), "");
    }

    #[test]
    fn meter_tracks_busy_and_active_windows() {
        let t0 = Instant::now();
        let m = Meter::new(t0);
        let at = |ms: u64| t0 + Duration::from_millis(ms);
        m.record_between(10, at(100), at(200));
        m.record_between(5, at(500), at(600));
        let s = m.stat();
        assert_eq!(s.bytes, 15);
        assert!((s.busy_s - 0.2).abs() < 1e-9);
        assert!((s.active_s - 0.5).abs() < 1e-9);
        assert_eq!(Meter::new(t0).stat(), PhaseStat::default());
    }
}
