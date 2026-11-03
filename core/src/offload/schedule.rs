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
    /// Dossier source (chemin absolu).
    pub source: PathBuf,
    /// La source est sur un disque dur mécanique.
    pub source_hdd: bool,
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
    /// La source est dans le dossier final d'un travail en cours, ou
    /// inversement (sauvegarde d'un dossier où une carte est en train de
    /// s'écrire).
    SourceInUse,
    /// Une destination est un disque dur déjà utilisé par un travail en cours
    /// (écrit ou lu : source d'un disque dur utilisé comme destination, et
    /// inversement).
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
fn same_path_ci(a: &Path, b: &Path, case_insensitive: bool) -> bool {
    normalize(a, case_insensitive) == normalize(b, case_insensitive)
}

/// Vrai si l'un des deux dossiers est le même que l'autre ou le contient
/// (comparaison par composants : `/a/bc` n'est pas dans `/a/b`).
fn overlaps_ci(a: &Path, b: &Path, case_insensitive: bool) -> bool {
    let (a, b) = (
        normalize(a, case_insensitive),
        normalize(b, case_insensitive),
    );
    a.starts_with(&b) || b.starts_with(&a)
}

/// Deux chemins désignent-ils le même emplacement (casse selon le système) ?
pub fn same_path(a: &Path, b: &Path) -> bool {
    same_path_ci(a, b, CASE_INSENSITIVE)
}

/// L'un des deux dossiers est-il le même que l'autre ou le contient ?
pub fn overlaps(a: &Path, b: &Path) -> bool {
    overlaps_ci(a, b, CASE_INSENSITIVE)
}

/// Un travail utilise-t-il ce volume (source, dossier final ou destination) ?
/// Sert à décider d'une éjection.
pub fn claim_uses_volume(c: &Claim, volume: &Path) -> bool {
    same_path(&c.source_volume, volume)
        || overlaps(&c.source, volume)
        || c.dest_volumes.iter().any(|(v, _, _)| same_path(v, volume))
        || c.roots.iter().any(|r| overlaps(r, volume))
}

/// Octets réservés par un claim sur un volume.
fn reserved_on(c: &Claim, volume: &Path, ci: bool) -> u64 {
    c.dest_volumes
        .iter()
        .filter(|(v, _, _)| same_path_ci(v, volume, ci))
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
        .any(|r| same_path_ci(&r.source_volume, &c.source_volume, ci))
    {
        return Err(Wait::SourceBusy);
    }
    let root_clash = c.roots.iter().any(|a| {
        running
            .iter()
            .flat_map(|r| &r.roots)
            .any(|b| overlaps_ci(a, b, ci))
    });
    if root_clash {
        return Err(Wait::RootBusy);
    }
    let source_clash = running.iter().any(|r| {
        r.roots.iter().any(|b| overlaps_ci(&c.source, b, ci))
            || c.roots.iter().any(|a| overlaps_ci(&r.source, a, ci))
    });
    if source_clash {
        return Err(Wait::SourceInUse);
    }
    for (volume, hdd, _) in &c.dest_volumes {
        // Disque dur écrit par un autre travail, ou lu comme source par lui.
        let used = running.iter().any(|r| {
            r.dest_volumes
                .iter()
                .any(|(v, _, _)| same_path_ci(v, volume, ci))
                || (r.source_hdd && same_path_ci(&r.source_volume, volume, ci))
        });
        if *hdd && used {
            return Err(Wait::HddBusy);
        }
    }
    // Source sur un disque dur écrit par un autre travail.
    if c.source_hdd
        && running.iter().any(|r| {
            r.dest_volumes
                .iter()
                .any(|(v, _, _)| same_path_ci(v, &c.source_volume, ci))
        })
    {
        return Err(Wait::HddBusy);
    }
    for (volume, _, _) in &c.dest_volumes {
        let others: Vec<&Claim> = running
            .iter()
            .filter(|r| {
                r.dest_volumes
                    .iter()
                    .any(|(v, _, _)| same_path_ci(v, volume, ci))
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

/// Décision d'un passage d'ordonnancement : indices des travaux en attente à
/// lancer, et raison d'attente de chacun des autres. Ordre d'arrivée ; un
/// travail bloqué ne bloque pas les suivants, et les travaux lancés dans ce
/// passage comptent comme en cours pour les suivants. Pour ne pas affamer un
/// travail qui attend de la place ou un disque dur, les suivants qui écrivent
/// sur le même volume attendent aussi, pour la même raison.
pub fn select(
    pending: &[Claim],
    running: &[Claim],
    free: &dyn Fn(&Path) -> Option<u64>,
) -> (Vec<usize>, Vec<(usize, Wait)>) {
    let mut current: Vec<Claim> = running.to_vec();
    let mut launch = Vec::new();
    let mut waits = Vec::new();
    let mut held: Vec<(PathBuf, Wait)> = Vec::new();
    for (i, c) in pending.iter().enumerate() {
        let verdict = can_start(c, &current, free).and_then(|()| {
            match c
                .dest_volumes
                .iter()
                .find_map(|(v, _, _)| held.iter().find(|(h, _)| same_path(h, v)))
            {
                Some((_, reason)) => Err(*reason),
                None => Ok(()),
            }
        });
        match verdict {
            Ok(()) => {
                launch.push(i);
                current.push(c.clone());
            }
            Err(reason) => {
                // Un disque dur occupé ne retient que ses volumes de disque dur
                // (les SSD de ce travail restent libres pour les suivants).
                match reason {
                    Wait::Space => {
                        held.extend(c.dest_volumes.iter().map(|(v, _, _)| (v.clone(), reason)))
                    }
                    Wait::HddBusy => held.extend(
                        c.dest_volumes
                            .iter()
                            .filter(|(_, hdd, _)| *hdd)
                            .map(|(v, _, _)| (v.clone(), reason)),
                    ),
                    _ => {}
                }
                waits.push((i, reason));
            }
        }
    }
    (launch, waits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(src: &str, roots: &[&str], dests: &[(&str, bool, u64)]) -> Claim {
        Claim {
            source: PathBuf::from(src).join("card"),
            source_hdd: false,
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
            assert!(!overlaps_ci(Path::new("/a/bc"), Path::new("/a/b"), ci));
            assert!(overlaps_ci(Path::new("/a/b/c"), Path::new("/a/b"), ci));
        }
    }

    fn with_source(mut c: Claim, source: &str) -> Claim {
        c.source = source.into();
        c
    }

    #[test]
    fn schedule_waits_when_source_inside_running_root() {
        // Sauvegarde de SSD1/J01 pendant qu'une carte s'écrit dans SSD1/J01/A002.
        let run = [claim("/A", &["/ssd1/J01/A002"], &[("/ssd1", false, 10)])];
        let c = with_source(
            claim("/ssd1", &["/ssd2/J01"], &[("/ssd2", false, 10)]),
            "/ssd1/J01",
        );
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::SourceInUse));
        // Source dans un dossier final en cours.
        let c = with_source(
            claim("/ssd1", &["/ssd2/J01"], &[("/ssd2", false, 10)]),
            "/ssd1/J01/A002/CLIP",
        );
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::SourceInUse));
        // Source voisine : permis.
        let c = with_source(
            claim("/ssd9", &["/ssd2/J01"], &[("/ssd2", false, 10)]),
            "/ssd1/J01/B001",
        );
        assert_eq!(can_start(&c, &run, &no_limit), Ok(()));
    }

    #[test]
    fn schedule_waits_when_running_source_inside_new_root() {
        // Le sens inverse : la sauvegarde tourne, une carte veut écrire dedans.
        let run = [with_source(
            claim("/ssd1", &["/ssd2/J01"], &[("/ssd2", false, 10)]),
            "/ssd1/J01",
        )];
        let c = claim("/A", &["/ssd1/J01/A002"], &[("/ssd1", false, 10)]);
        assert_eq!(can_start(&c, &run, &no_limit), Err(Wait::SourceInUse));
    }

    #[test]
    fn schedule_hdd_source_and_destination_conflict() {
        let mut hdd_src = claim("/hdd", &["/ssd/a"], &[("/ssd", false, 10)]);
        hdd_src.source_hdd = true;
        // Un travail écrit sur le disque dur que l'autre lit.
        let c = claim("/B", &["/hdd/b"], &[("/hdd", true, 10)]);
        assert_eq!(
            can_start(&c, std::slice::from_ref(&hdd_src), &no_limit),
            Err(Wait::HddBusy)
        );
        // Et inversement : on veut lire un disque dur où un autre écrit.
        let writer = claim("/B", &["/hdd/b"], &[("/hdd", true, 10)]);
        assert_eq!(
            can_start(&hdd_src, &[writer], &no_limit),
            Err(Wait::HddBusy)
        );
    }

    #[test]
    fn schedule_claim_uses_volume() {
        let c = claim("/A", &["/ssd/x"], &[("/ssd", false, 1)]);
        assert!(claim_uses_volume(&c, Path::new("/A")));
        assert!(claim_uses_volume(&c, Path::new("/ssd")));
        assert!(!claim_uses_volume(&c, Path::new("/other")));
    }

    #[test]
    fn select_keeps_arrival_order_and_does_not_block_followers() {
        let run = [claim("/A", &["/hdd/a"], &[("/hdd", true, 10)])];
        let pending = [
            claim("/B", &["/hdd/b"], &[("/hdd", true, 10)]), // bloqué
            claim("/C", &["/ssd/c"], &[("/ssd", false, 10)]), // part
        ];
        let (go, waits) = select(&pending, &run, &no_limit);
        assert_eq!(go, [1]);
        assert_eq!(waits, [(0, Wait::HddBusy)]);
    }

    #[test]
    fn select_counts_launched_as_running() {
        let pending = [
            claim("/A", &["/d1/a"], &[("/d1", false, 1)]),
            claim("/A", &["/d2/b"], &[("/d2", false, 1)]), // même source
            claim("/C", &["/d1/a"], &[("/d1", false, 1)]), // même dossier final
            claim("/D", &["/d3/d"], &[("/d3", false, 1)]),
            claim("/E", &["/d4/e"], &[("/d4", false, 1)]), // part aussi
        ];
        let (go, waits) = select(&pending, &[], &no_limit);
        assert_eq!(go, [0, 3, 4]);
        assert_eq!(waits, [(1, Wait::SourceBusy), (2, Wait::RootBusy)]);
    }

    #[test]
    fn select_limit_applies_to_launched_ones() {
        let pending: Vec<Claim> = (0..MAX_PARALLEL_JOBS + 1)
            .map(|i| {
                claim(
                    &format!("/s{i}"),
                    &[&format!("/d{i}/x")],
                    &[(&format!("/d{i}"), false, 1)],
                )
            })
            .collect();
        let (go, waits) = select(&pending, &[], &no_limit);
        assert_eq!(go.len(), MAX_PARALLEL_JOBS);
        assert_eq!(waits, [(MAX_PARALLEL_JOBS, Wait::Limit)]);
    }

    #[test]
    fn select_followers_on_held_volume_wait_too() {
        let run = [claim("/A", &["/hdd/a"], &[("/hdd", true, 10)])];
        let pending = [
            claim("/B", &["/hdd/b"], &[("/hdd", true, 10)]),
            // Un SSD ne serait pas bloqué, mais celui-ci écrit sur le même
            // volume qu'un travail qui attend : il ne le double pas.
            claim("/C", &["/hdd/c"], &[("/hdd", false, 1)]),
        ];
        let (go, waits) = select(&pending, &run, &no_limit);
        assert!(go.is_empty());
        assert_eq!(waits, [(0, Wait::HddBusy), (1, Wait::HddBusy)]);
    }

    #[test]
    fn select_hdd_wait_holds_only_hdd_volumes() {
        let run = [claim("/A", &["/hdd/a"], &[("/hdd", true, 10)])];
        let pending = [
            // Attend le disque dur, mais écrit aussi sur un SSD.
            claim(
                "/B",
                &["/hdd/b", "/ssd/b"],
                &[("/hdd", true, 10), ("/ssd", false, 10)],
            ),
            claim("/C", &["/ssd/c"], &[("/ssd", false, 10)]),
        ];
        let (go, waits) = select(&pending, &run, &no_limit);
        assert_eq!(go, [1]);
        assert_eq!(waits, [(0, Wait::HddBusy)]);
    }
}
