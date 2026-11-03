//! Ordonnanceur des offloads simultanés : décide, sans rien lancer, si un
//! travail peut démarrer à côté de ceux qui tournent. Deux cartes de lecteurs
//! différents se copient en même temps ; ce qui se gênerait attend.

use std::path::{Path, PathBuf};

use serde::Serialize;

/// Nombre maximal de copies simultanées.
pub const MAX_PARALLEL_JOBS: usize = 3;

/// Ressources qu'un travail occupe pendant sa copie.
#[derive(Debug, Clone)]
pub struct Claim {
    /// Volume de la source (point de montage).
    pub source_volume: PathBuf,
    /// Dossiers finals écrits (un par destination).
    pub roots: Vec<PathBuf>,
    /// Par destination : volume, disque dur mécanique, octets à y écrire.
    pub dest_volumes: Vec<(PathBuf, bool, u64)>,
}

/// Raison pour laquelle un travail attend.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Wait {
    /// Nombre maximal de copies simultanées atteint.
    Limit,
    /// La source est sur un volume déjà lu par un travail en cours.
    SourceBusy,
    /// Un dossier final est identique à, ou contient ou est contenu dans, celui
    /// d'un travail en cours.
    RootBusy,
    /// Une destination est un disque dur déjà utilisé par un travail en cours.
    HddBusy,
    /// L'espace libre ne suffit pas pour les deux copies sur un même volume.
    Space,
}

/// Vrai si l'un des deux dossiers est le même que l'autre ou le contient.
fn overlaps(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// Octets réservés par un claim sur un volume.
fn reserved_on(c: &Claim, volume: &Path) -> u64 {
    c.dest_volumes
        .iter()
        .filter(|(v, _, _)| v == volume)
        .map(|(_, _, bytes)| *bytes)
        .sum()
}

/// Un travail peut-il démarrer à côté de ceux qui tournent ? `free` donne
/// l'espace libre d'un volume (`None` : inconnu, jamais bloquant).
pub fn can_start(
    c: &Claim,
    running: &[Claim],
    free: &dyn Fn(&Path) -> Option<u64>,
) -> Result<(), Wait> {
    if running.len() >= MAX_PARALLEL_JOBS {
        return Err(Wait::Limit);
    }
    if running.iter().any(|r| r.source_volume == c.source_volume) {
        return Err(Wait::SourceBusy);
    }
    let root_clash = c.roots.iter().any(|a| {
        running
            .iter()
            .flat_map(|r| &r.roots)
            .any(|b| overlaps(a, b))
    });
    if root_clash {
        return Err(Wait::RootBusy);
    }
    for (volume, hdd, _) in &c.dest_volumes {
        let used = running
            .iter()
            .any(|r| r.dest_volumes.iter().any(|(v, _, _)| v == volume));
        if *hdd && used {
            return Err(Wait::HddBusy);
        }
    }
    for (volume, _, _) in &c.dest_volumes {
        let others: Vec<&Claim> = running
            .iter()
            .filter(|r| r.dest_volumes.iter().any(|(v, _, _)| v == volume))
            .collect();
        if others.is_empty() {
            continue;
        }
        let total: u64 =
            reserved_on(c, volume) + others.iter().map(|r| reserved_on(r, volume)).sum::<u64>();
        if free(volume).is_some_and(|f| total > f) {
            return Err(Wait::Space);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(src: &str, roots: &[&str], dests: &[(&str, bool, u64)]) -> Claim {
        Claim {
            source_volume: src.into(),
            roots: roots.iter().map(PathBuf::from).collect(),
            dest_volumes: dests
                .iter()
                .map(|(v, h, b)| ((*v).into(), *h, *b))
                .collect(),
        }
    }

    fn no_limit(_: &Path) -> Option<u64> {
        None
    }

    #[test]
    fn schedule_waits_on_same_root() {
        let run = [claim("/A", &["/ssd/J1/A001"], &[("/ssd", false, 10)])];
        let c = claim("/B", &["/ssd/J1/A001"], &[("/ssd", false, 10)]);
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::RootBusy));
    }

    #[test]
    fn schedule_waits_on_nested_root() {
        let run = [claim("/A", &["/ssd/J1"], &[("/ssd", false, 10)])];
        let inner = claim("/B", &["/ssd/J1/A001"], &[("/ssd", false, 10)]);
        assert_eq!(can_start(&inner, &run, &no_limit), Err(Wait::RootBusy));
        // Et dans l'autre sens.
        let run = [claim("/A", &["/ssd/J1/A001"], &[("/ssd", false, 10)])];
        let outer = claim("/B", &["/ssd/J1"], &[("/ssd", false, 10)]);
        assert_eq!(can_start(&outer, &run, &no_limit), Err(Wait::RootBusy));
        // Un nom qui commence pareil n'est pas un dossier inclus.
        let near = claim("/B", &["/ssd/J1bis"], &[("/ssd", false, 10)]);
        assert_eq!(can_start(&near, &run, &no_limit), Ok(()));
    }

    #[test]
    fn schedule_allows_two_cards_same_ssd() {
        let run = [claim("/A", &["/ssd/J1/A001"], &[("/ssd", false, 10)])];
        let c = claim("/B", &["/ssd/J1/B001"], &[("/ssd", false, 10)]);
        assert_eq!(can_start(&c, &run, &no_limit), Ok(()));
    }

    #[test]
    fn schedule_waits_on_shared_hdd() {
        let run = [claim("/A", &["/hdd/A001"], &[("/hdd", true, 10)])];
        let c = claim("/B", &["/hdd/B001"], &[("/hdd", true, 10)]);
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::HddBusy));
        // Un disque dur libre n'attend pas.
        assert_eq!(can_start(&c, &[], &no_limit), Ok(()));
    }

    #[test]
    fn schedule_waits_on_same_source_volume() {
        let run = [claim("/A", &["/ssd/A001"], &[("/ssd", false, 10)])];
        let c = claim("/A", &["/ssd2/A002"], &[("/ssd2", false, 10)]);
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::SourceBusy));
    }

    #[test]
    fn schedule_respects_limit() {
        let run: Vec<Claim> = (0..MAX_PARALLEL_JOBS)
            .map(|i| {
                claim(
                    &format!("/s{i}"),
                    &[&format!("/d{i}/x")],
                    &[(&format!("/d{i}"), false, 1)],
                )
            })
            .collect();
        let c = claim("/other", &["/free/x"], &[("/free", false, 1)]);
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::Limit));
        assert_eq!(can_start(&c, &run[1..], &no_limit), Ok(()));
    }

    #[test]
    fn schedule_waits_when_reserved_space_exceeds_free() {
        let run = [claim("/A", &["/ssd/A001"], &[("/ssd", false, 60)])];
        let c = claim("/B", &["/ssd/B001"], &[("/ssd", false, 60)]);
        let free = |v: &Path| (v == Path::new("/ssd")).then_some(100);
        assert_eq!(can_start(&c, &run, &free), Err(Wait::Space));
        let roomy = |_: &Path| Some(120);
        assert_eq!(can_start(&c, &run, &roomy), Ok(()));
        // Seul sur le volume : l'espace n'est pas l'affaire de l'ordonnanceur.
        assert_eq!(can_start(&c, &[], &free), Ok(()));
    }
}
