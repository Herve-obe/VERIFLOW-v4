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

/// Windows et macOS comparent les chemins sans tenir compte de la casse
/// (systèmes de fichiers par défaut) ; Linux reste exact.
const CASE_INSENSITIVE: bool = cfg!(any(windows, target_os = "macos"));

/// Forme comparable d'un chemin, par composants : préfixe `\\?\` retiré, `/` et
/// `\` traités comme séparateurs, composants vides et `.` ignorés, casse
/// repliée si `case_insensitive`. Un chemin absolu commence par le composant
/// `/`, pour ne pas le confondre avec un chemin relatif.
fn normalize(p: &Path, case_insensitive: bool) -> Vec<String> {
    let text = p.to_string_lossy();
    let text = text.strip_prefix(r"\\?\").unwrap_or(&text);
    let mut out = Vec::new();
    if text.starts_with(['/', '\\']) {
        out.push("/".to_string());
    }
    for part in text.split(['/', '\\']) {
        if part.is_empty() || part == "." {
            continue;
        }
        out.push(if case_insensitive {
            part.to_lowercase()
        } else {
            part.to_string()
        });
    }
    out
}

/// Deux chemins désignent-ils le même emplacement ?
fn same_path(a: &Path, b: &Path, case_insensitive: bool) -> bool {
    normalize(a, case_insensitive) == normalize(b, case_insensitive)
}

/// Vrai si l'un des deux dossiers est le même que l'autre ou le contient
/// (comparaison par composants : `/a/bc` n'est pas dans `/a/b`).
fn overlaps(a: &Path, b: &Path, case_insensitive: bool) -> bool {
    let (a, b) = (
        normalize(a, case_insensitive),
        normalize(b, case_insensitive),
    );
    a.starts_with(&b) || b.starts_with(&a)
}

/// Octets réservés par un claim sur un volume.
fn reserved_on(c: &Claim, volume: &Path, ci: bool) -> u64 {
    c.dest_volumes
        .iter()
        .filter(|(v, _, _)| same_path(v, volume, ci))
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
    can_start_with(c, running, free, CASE_INSENSITIVE)
}

/// `can_start` avec la sensibilité à la casse choisie (testable partout).
fn can_start_with(
    c: &Claim,
    running: &[Claim],
    free: &dyn Fn(&Path) -> Option<u64>,
    ci: bool,
) -> Result<(), Wait> {
    if running.len() >= MAX_PARALLEL_JOBS {
        return Err(Wait::Limit);
    }
    if running
        .iter()
        .any(|r| same_path(&r.source_volume, &c.source_volume, ci))
    {
        return Err(Wait::SourceBusy);
    }
    let root_clash = c.roots.iter().any(|a| {
        running
            .iter()
            .flat_map(|r| &r.roots)
            .any(|b| overlaps(a, b, ci))
    });
    if root_clash {
        return Err(Wait::RootBusy);
    }
    for (volume, hdd, _) in &c.dest_volumes {
        let used = running.iter().any(|r| {
            r.dest_volumes
                .iter()
                .any(|(v, _, _)| same_path(v, volume, ci))
        });
        if *hdd && used {
            return Err(Wait::HddBusy);
        }
    }
    for (volume, _, _) in &c.dest_volumes {
        let others: Vec<&Claim> = running
            .iter()
            .filter(|r| {
                r.dest_volumes
                    .iter()
                    .any(|(v, _, _)| same_path(v, volume, ci))
            })
            .collect();
        if others.is_empty() {
            continue;
        }
        let total: u64 = reserved_on(c, volume, ci)
            + others
                .iter()
                .map(|r| reserved_on(r, volume, ci))
                .sum::<u64>();
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

    #[test]
    fn schedule_normalize_components() {
        let n = |p: &str, ci| normalize(Path::new(p), ci);
        assert_eq!(n(r"\\?\E:\Rushes\J01", true), ["e:", "rushes", "j01"]);
        assert_eq!(n("E:/Rushes//J01/./", false), ["E:", "Rushes", "J01"]);
        assert_eq!(n("/Volumes/SSD", false), ["/", "Volumes", "SSD"]);
        assert_ne!(n("/a", false), n("a", false));
    }

    #[test]
    fn schedule_root_case_windows_style() {
        let run = [claim("S", &[r"E:\Rushes\J01"], &[("E:", false, 10)])];
        let c = claim("T", &[r"e:\rushes\j01\A001"], &[("e:", false, 10)]);
        assert_eq!(
            can_start_with(&c, &run, &no_limit, true),
            Err(Wait::RootBusy)
        );
        // Sensible à la casse : dossiers distincts.
        assert_eq!(can_start_with(&c, &run, &no_limit, false), Ok(()));
        // Préfixe \\?\ et séparateurs mixtes.
        let c = claim("T", &[r"\\?\e:\rushes/j01"], &[("e:", false, 10)]);
        assert_eq!(
            can_start_with(&c, &run, &no_limit, true),
            Err(Wait::RootBusy)
        );
    }

    #[test]
    fn schedule_volume_case_macos_style() {
        let run = [claim(
            "/Volumes/CARD",
            &["/Volumes/SSD/a"],
            &[("/Volumes/SSD", true, 10)],
        )];
        let c = claim(
            "/Volumes/card",
            &["/Volumes/ssd/B"],
            &[("/Volumes/ssd", true, 10)],
        );
        assert_eq!(
            can_start_with(&c, &run, &no_limit, true),
            Err(Wait::SourceBusy)
        );
        assert_eq!(can_start_with(&c, &run, &no_limit, false), Ok(()));
        // Même disque dur, autre source.
        let c = claim(
            "/Volumes/other",
            &["/Volumes/ssd/B"],
            &[("/Volumes/ssd", true, 10)],
        );
        assert_eq!(
            can_start_with(&c, &run, &no_limit, true),
            Err(Wait::HddBusy)
        );
        assert_eq!(can_start_with(&c, &run, &no_limit, false), Ok(()));
        // Même dossier final à la casse près.
        let c = claim(
            "/Volumes/other",
            &["/Volumes/ssd/A"],
            &[("/Volumes/x", false, 1)],
        );
        assert_eq!(
            can_start_with(&c, &run, &no_limit, true),
            Err(Wait::RootBusy)
        );
        assert_eq!(can_start_with(&c, &run, &no_limit, false), Ok(()));
    }

    #[test]
    fn schedule_space_case_insensitive_volume() {
        let run = [claim("/A", &["/Ssd/A001"], &[("/Ssd", false, 60)])];
        let c = claim("/B", &["/Ssd/B001"], &[("/SSD", false, 60)]);
        let free = |_: &Path| Some(100);
        assert_eq!(can_start_with(&c, &run, &free, true), Err(Wait::Space));
        assert_eq!(can_start_with(&c, &run, &free, false), Ok(()));
    }

    #[test]
    fn schedule_component_prefix_is_not_inclusion() {
        for ci in [false, true] {
            assert!(!overlaps(Path::new("/a/bc"), Path::new("/a/b"), ci));
            assert!(overlaps(Path::new("/a/b/c"), Path::new("/a/b"), ci));
        }
    }
}
