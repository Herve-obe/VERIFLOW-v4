//! État de l'alimentation du poste : avertit quand un portable est sur
//! batterie avant ou pendant un offload. Le code propre à chaque OS se réduit
//! à l'appel système ; la logique est dans des fonctions pures testées partout.

use serde::Serialize;

/// État de l'alimentation d'un poste qui possède une batterie.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct PowerStatus {
    pub on_battery: bool,
    /// Charge restante, 0 à 100.
    pub percent: Option<u8>,
    /// Autonomie estimée par le système.
    pub minutes_left: Option<u32>,
}

#[derive(Serialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BatteryReason {
    /// Charge faible.
    Low,
    /// Copie longue sur batterie.
    Long,
    /// Autonomie inférieure à la durée de la copie.
    Short,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct BatteryWarning {
    pub reason: BatteryReason,
    pub percent: Option<u8>,
    pub minutes_left: Option<u32>,
    /// Durée estimée de la copie, en minutes (arrondie au-dessus).
    pub estimated_minutes: u32,
}

/// Débit prudent d'une copie vérifiée, en octets par seconde.
pub const PRUDENT_RATE: u64 = 100_000_000;
/// Débit prudent quand un disque mécanique est impliqué.
pub const PRUDENT_RATE_HDD: u64 = 60_000_000;
/// Charge en dessous de laquelle on avertit, en pourcent.
pub const LOW_PERCENT: u8 = 20;
/// Durée de copie au-delà de laquelle on avertit sur batterie, en secondes.
pub const LONG_SECS: u64 = 600;

/// Durée estimée : lecture de la source plus écriture de chaque destination.
pub fn estimate_secs(total_bytes: u64, destinations: usize, any_hdd: bool) -> u64 {
    let rate = if any_hdd {
        PRUDENT_RATE_HDD
    } else {
        PRUDENT_RATE
    };
    total_bytes.saturating_mul(1 + destinations as u64) / rate
}

/// Avertissement à afficher, ou `None` (secteur, état inconnu, copie courte).
pub fn assess(status: Option<&PowerStatus>, est_secs: u64) -> Option<BatteryWarning> {
    let status = status.filter(|s| s.on_battery)?;
    let reason = if status
        .minutes_left
        .is_some_and(|m| u64::from(m) * 60 * 2 < est_secs.saturating_mul(3))
    {
        BatteryReason::Short
    } else if status.percent.is_some_and(|p| p <= LOW_PERCENT) {
        BatteryReason::Low
    } else if est_secs > LONG_SECS {
        BatteryReason::Long
    } else {
        return None;
    };
    Some(BatteryWarning {
        reason,
        percent: status.percent,
        minutes_left: status.minutes_left,
        estimated_minutes: u32::try_from(est_secs.div_ceil(60)).unwrap_or(u32::MAX),
    })
}

/// État courant de l'alimentation. `None` sur toute erreur ou sans batterie.
pub fn power_status() -> Option<PowerStatus> {
    platform::power_status()
}

/// Interprète les valeurs de `GetSystemPowerStatus`.
#[cfg_attr(not(windows), allow(dead_code))]
fn from_windows(ac: u8, flag: u8, percent: u8, life_secs: u32) -> Option<PowerStatus> {
    // 255 : inconnu ; drapeau 128 : pas de batterie système.
    if ac == 255 || flag & 128 != 0 {
        return None;
    }
    // Sur batterie sans niveau connu : on avertit quand même (durée seule).
    Some(PowerStatus {
        on_battery: ac == 0,
        percent: (percent != 255).then_some(percent.min(100)),
        minutes_left: (life_secs != u32::MAX).then_some(life_secs / 60),
    })
}

/// Interprète la sortie de `pmset -g batt`.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
fn parse_pmset(text: &str) -> Option<PowerStatus> {
    let on_battery = if text.contains("'Battery Power'") {
        true
    } else if text.contains("'AC Power'") {
        false
    } else {
        return None;
    };
    // Sans ligne InternalBattery (Mac fixe), pas d'avertissement possible.
    let line = text.lines().find(|l| l.contains("InternalBattery"))?;
    let pct_end = line.find('%')?;
    let digits: String = line[..pct_end]
        .chars()
        .rev()
        .take_while(|c| c.is_ascii_digit())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    let percent = digits.parse::<u8>().ok().filter(|p| *p <= 100)?;
    let minutes_left = line
        .split(';')
        .find_map(|part| {
            let t = part.trim().split(" remaining").next()?.trim();
            let (h, m) = t.split_once(':')?;
            Some(h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?)
        })
        // « 0:00 remaining » : estimation pas encore calculée, donc inconnue.
        .filter(|m| *m > 0);
    Some(PowerStatus {
        on_battery,
        percent: Some(percent),
        minutes_left,
    })
}

/// Lit `/sys/class/power_supply` (ou un dossier de même forme).
#[cfg_attr(any(windows, target_os = "macos"), allow(dead_code))]
fn read_sysfs(dir: &std::path::Path) -> Option<PowerStatus> {
    let read = |p: &std::path::Path, name: &str| {
        std::fs::read_to_string(p.join(name))
            .ok()
            .map(|s| s.trim().to_string())
    };
    let num = |p: &std::path::Path, name: &str| read(p, name).and_then(|s| s.parse::<u64>().ok());
    let (mut mains, mut discharging) = (false, false);
    // Par batterie : niveau et capacité pleine (énergie ou charge).
    let mut caps: Vec<u64> = Vec::new();
    let mut fulls: Vec<Option<(bool, u64)>> = Vec::new();
    // Sommes pour l'autonomie : (énergie, puissance) puis (charge, courant),
    // avec le nombre de batteries qui ont fourni chaque paire.
    let (mut energy, mut power, mut charge, mut current) = (0u64, 0u64, 0u64, 0u64);
    let (mut n_energy, mut n_charge) = (0, 0);
    let mut batteries = 0;
    for entry in std::fs::read_dir(dir).ok()?.flatten() {
        let p = entry.path();
        match read(&p, "type").as_deref() {
            Some("Mains") | Some("USB") => {
                if read(&p, "online").as_deref() == Some("1") {
                    mains = true;
                }
            }
            Some("Battery") => {
                // Les batteries de périphériques (souris, casque) sont ignorées.
                if read(&p, "scope").is_some_and(|s| s != "System") {
                    continue;
                }
                batteries += 1;
                if read(&p, "status").as_deref() == Some("Discharging") {
                    discharging = true;
                }
                if let Some(c) = num(&p, "capacity") {
                    caps.push(c.min(100));
                    // (vrai : énergie, faux : charge), jamais mélangées.
                    fulls.push(
                        num(&p, "energy_full")
                            .map(|f| (true, f))
                            .or_else(|| num(&p, "charge_full").map(|f| (false, f))),
                    );
                }
                if let (Some(e), Some(w)) = (num(&p, "energy_now"), num(&p, "power_now")) {
                    energy += e;
                    power += w;
                    n_energy += 1;
                }
                if let (Some(c), Some(i)) = (num(&p, "charge_now"), num(&p, "current_now")) {
                    charge += c;
                    current += i;
                    n_charge += 1;
                }
            }
            _ => {}
        }
    }
    if batteries == 0 {
        return None;
    }
    // Niveau pondéré par la capacité pleine quand elle est connue pour toutes
    // les batteries dans la même unité ; sinon moyenne simple.
    let weights: Option<Vec<u64>> = match fulls.first() {
        Some(Some((kind, _)))
            if fulls.len() == caps.len()
                && fulls.iter().all(|f| f.is_some_and(|(k, _)| k == *kind)) =>
        {
            Some(fulls.iter().flatten().map(|(_, f)| *f).collect())
        }
        _ => None,
    }
    .filter(|w: &Vec<u64>| w.iter().sum::<u64>() > 0);
    let percent = if caps.is_empty() {
        None
    } else if let Some(w) = weights {
        let total: u64 = w.iter().sum();
        let weighted: u64 = caps.iter().zip(&w).map(|(c, f)| c * f).sum();
        Some((weighted as f64 / total as f64).round() as u8)
    } else {
        Some((caps.iter().sum::<u64>() as f64 / caps.len() as f64).round() as u8)
    };
    // Autonomie : seulement si toutes les batteries donnent la même paire.
    // Dans un parc mixte (une en énergie, une en charge), les unités ne
    // s'additionnent pas et une somme partielle surestimerait l'autonomie.
    let hours = if n_energy == batteries && power > 0 {
        Some(energy as f64 / power as f64)
    } else if n_charge == batteries && current > 0 {
        Some(charge as f64 / current as f64)
    } else {
        None
    };
    let on_battery = !mains && discharging;
    Some(PowerStatus {
        on_battery,
        percent,
        minutes_left: if on_battery {
            hours.map(|h| (h * 60.0).round() as u32)
        } else {
            None
        },
    })
}

#[cfg(windows)]
mod platform {
    use super::*;
    use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

    pub fn power_status() -> Option<PowerStatus> {
        let mut s = SYSTEM_POWER_STATUS {
            ACLineStatus: 0,
            BatteryFlag: 0,
            BatteryLifePercent: 0,
            SystemStatusFlag: 0,
            BatteryLifeTime: 0,
            BatteryFullLifeTime: 0,
        };
        // SAFETY : `s` est une structure valide, écrite entièrement par l'appel.
        let ok = unsafe { GetSystemPowerStatus(&mut s) };
        if ok == 0 {
            return None;
        }
        from_windows(
            s.ACLineStatus,
            s.BatteryFlag,
            s.BatteryLifePercent,
            s.BatteryLifeTime,
        )
    }
}

#[cfg(target_os = "macos")]
mod platform {
    use super::*;

    pub fn power_status() -> Option<PowerStatus> {
        let out = std::process::Command::new("/usr/bin/pmset")
            .args(["-g", "batt"])
            .output()
            .ok()?;
        if !out.status.success() {
            return None;
        }
        parse_pmset(&String::from_utf8_lossy(&out.stdout))
    }
}

#[cfg(not(any(windows, target_os = "macos")))]
mod platform {
    use super::*;

    pub fn power_status() -> Option<PowerStatus> {
        read_sysfs(std::path::Path::new("/sys/class/power_supply"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;

    const PMSET_BATT: &str = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=4653155)\t85%; discharging; 3:12 remaining present: true\n";

    fn put(dir: &Path, name: &str, files: &[(&str, &str)]) {
        let d = dir.join(name);
        fs::create_dir_all(&d).unwrap();
        for (k, v) in files {
            fs::write(d.join(k), format!("{v}\n")).unwrap();
        }
    }

    fn batt(on_battery: bool, percent: Option<u8>, minutes_left: Option<u32>) -> PowerStatus {
        PowerStatus {
            on_battery,
            percent,
            minutes_left,
        }
    }

    #[test]
    fn pmset_on_battery() {
        assert_eq!(
            parse_pmset(PMSET_BATT),
            Some(batt(true, Some(85), Some(192)))
        );
    }

    #[test]
    fn pmset_on_ac() {
        let t = "Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t100%; charged; 0:00 remaining present: true\n";
        let s = parse_pmset(t).unwrap();
        assert!(!s.on_battery);
        assert_eq!(s.percent, Some(100));
    }

    #[test]
    fn pmset_no_estimate() {
        let t = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t60%; discharging; (no estimate) present: true\n";
        assert_eq!(parse_pmset(t), Some(batt(true, Some(60), None)));
    }

    #[test]
    fn pmset_zero_remaining_unknown() {
        let t = "Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t70%; discharging; 0:00 remaining present: true\n";
        assert_eq!(parse_pmset(t), Some(batt(true, Some(70), None)));
    }

    #[test]
    fn pmset_desktop_mac_none() {
        assert_eq!(parse_pmset("Now drawing from 'AC Power'\n"), None);
    }

    #[test]
    fn pmset_garbage_none() {
        assert_eq!(parse_pmset("n'importe quoi"), None);
        assert_eq!(parse_pmset(""), None);
    }

    #[test]
    fn sysfs_laptop_discharging() {
        let t = tempfile::tempdir().unwrap();
        put(
            t.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "50"),
                ("energy_now", "30000000"),
                ("power_now", "15000000"),
            ],
        );
        put(t.path(), "AC", &[("type", "Mains"), ("online", "0")]);
        assert_eq!(read_sysfs(t.path()), Some(batt(true, Some(50), Some(120))));
    }

    #[test]
    fn sysfs_charge_current_fallback() {
        let t = tempfile::tempdir().unwrap();
        put(
            t.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "40"),
                ("charge_now", "2000000"),
                ("current_now", "1000000"),
            ],
        );
        assert_eq!(read_sysfs(t.path()), Some(batt(true, Some(40), Some(120))));
    }

    #[test]
    fn sysfs_on_mains() {
        let t = tempfile::tempdir().unwrap();
        put(
            t.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Charging"),
                ("capacity", "70"),
            ],
        );
        put(t.path(), "AC", &[("type", "Mains"), ("online", "1")]);
        let s = read_sysfs(t.path()).unwrap();
        assert!(!s.on_battery);
        assert_eq!(s.percent, Some(70));
    }

    #[test]
    fn sysfs_ignores_device_scope() {
        let t = tempfile::tempdir().unwrap();
        put(
            t.path(),
            "hidpp_battery_0",
            &[
                ("type", "Battery"),
                ("scope", "Device"),
                ("status", "Discharging"),
                ("capacity", "10"),
            ],
        );
        assert_eq!(read_sysfs(t.path()), None);
        put(
            t.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("scope", "System"),
                ("status", "Full"),
                ("capacity", "90"),
            ],
        );
        let s = read_sysfs(t.path()).unwrap();
        assert!(!s.on_battery);
        assert_eq!(s.percent, Some(90));
    }

    #[test]
    fn sysfs_empty_none() {
        let t = tempfile::tempdir().unwrap();
        assert_eq!(read_sysfs(t.path()), None);
        assert_eq!(read_sysfs(&t.path().join("absent")), None);
    }

    #[test]
    fn sysfs_two_batteries_average() {
        let t = tempfile::tempdir().unwrap();
        for (n, cap) in [("BAT0", "80"), ("BAT1", "40")] {
            put(
                t.path(),
                n,
                &[
                    ("type", "Battery"),
                    ("status", "Discharging"),
                    ("capacity", cap),
                ],
            );
        }
        let s = read_sysfs(t.path()).unwrap();
        assert!(s.on_battery);
        assert_eq!(s.percent, Some(60));
        assert_eq!(s.minutes_left, None);
    }

    #[test]
    fn sysfs_two_batteries_weighted() {
        let t = tempfile::tempdir().unwrap();
        for (n, cap, full) in [("BAT0", "100", "10000"), ("BAT1", "50", "30000")] {
            put(
                t.path(),
                n,
                &[
                    ("type", "Battery"),
                    ("status", "Discharging"),
                    ("capacity", cap),
                    ("energy_full", full),
                ],
            );
        }
        // (100*10000 + 50*30000) / 40000 = 62,5 arrondi à 63.
        assert_eq!(read_sysfs(t.path()).unwrap().percent, Some(63));
    }

    #[test]
    fn sysfs_mixed_units_simple_average_no_autonomy() {
        let t = tempfile::tempdir().unwrap();
        put(
            t.path(),
            "BAT0",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "100"),
                ("energy_full", "10000"),
                ("energy_now", "10000"),
                ("power_now", "5000"),
            ],
        );
        put(
            t.path(),
            "BAT1",
            &[
                ("type", "Battery"),
                ("status", "Discharging"),
                ("capacity", "50"),
                ("charge_full", "30000"),
            ],
        );
        let s = read_sysfs(t.path()).unwrap();
        assert_eq!(s.percent, Some(75));
        assert_eq!(s.minutes_left, None);
    }

    #[test]
    fn windows_unknown_values() {
        assert_eq!(from_windows(255, 0, 50, 100), None);
        assert_eq!(from_windows(0, 128, 50, 100), None);
        assert_eq!(
            from_windows(0, 0, 255, 100),
            Some(batt(true, None, Some(1)))
        );
        assert!(!from_windows(1, 0, 255, u32::MAX).unwrap().on_battery);
        assert_eq!(
            from_windows(0, 0, 50, u32::MAX),
            Some(batt(true, Some(50), None))
        );
        assert_eq!(
            from_windows(0, 0, 50, 7200),
            Some(batt(true, Some(50), Some(120)))
        );
        assert!(!from_windows(1, 8, 50, u32::MAX).unwrap().on_battery);
    }

    #[test]
    fn estimate_100gb_two_destinations() {
        assert_eq!(estimate_secs(100_000_000_000, 2, false), 3000);
    }

    #[test]
    fn estimate_hdd_slower() {
        assert!(estimate_secs(1_000_000_000, 1, true) > estimate_secs(1_000_000_000, 1, false));
        assert_eq!(estimate_secs(u64::MAX, 5, false), u64::MAX / PRUDENT_RATE);
    }

    #[test]
    fn assess_ac_none() {
        assert_eq!(assess(Some(&batt(false, Some(5), Some(1))), 10_000), None);
    }

    #[test]
    fn assess_unknown_none() {
        assert_eq!(assess(None, 10_000), None);
    }

    #[test]
    fn assess_short_copy_full_battery_none() {
        assert_eq!(assess(Some(&batt(true, Some(100), Some(300))), 300), None);
    }

    #[test]
    fn assess_low_battery_short_copy() {
        let w = assess(Some(&batt(true, Some(15), None)), 90).unwrap();
        assert_eq!(w.reason, BatteryReason::Low);
        assert_eq!(w.estimated_minutes, 2);
    }

    #[test]
    fn assess_long_copy() {
        let w = assess(Some(&batt(true, Some(90), Some(600))), 3000).unwrap();
        assert_eq!(w.reason, BatteryReason::Long);
        assert_eq!(w.estimated_minutes, 50);
    }

    #[test]
    fn assess_autonomy_too_short() {
        // 10 min d'autonomie pour 10 min de copie : sous la marge de 1,5.
        let w = assess(Some(&batt(true, Some(90), Some(10))), 600).unwrap();
        assert_eq!(w.reason, BatteryReason::Short);
        // Marge suffisante : pas d'avertissement.
        assert_eq!(assess(Some(&batt(true, Some(90), Some(15))), 600), None);
    }
}
