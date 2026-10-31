//! Pas de mise en veille pendant un travail long : copie, conversion,
//! synchronisation. Une copie coupée par la veille d'un portable doit être
//! reprise ; sous macOS, la même déclaration évite aussi App Nap, qui ralentit
//! une application passée en arrière-plan. L'écran peut toujours s'éteindre.
//!
//! Un seul fil tient la demande auprès du système, tant qu'au moins une garde
//! [`KeepAwake`] existe ; elle est rendue dès la fin du dernier travail.

use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::thread;

struct Keeper {
    stop: Sender<()>,
    users: usize,
}

static KEEPER: Mutex<Option<Keeper>> = Mutex::new(None);

/// Tant que cette garde existe, le système ne se met pas en veille.
#[must_use = "la veille est de nouveau permise dès que la garde est rendue"]
pub struct KeepAwake(());

/// Empêche la mise en veille jusqu'à la fin de la garde rendue.
pub fn keep_awake() -> KeepAwake {
    if let Ok(mut keeper) = KEEPER.lock() {
        match keeper.as_mut() {
            Some(k) => k.users += 1,
            None => {
                let (stop, rx) = channel();
                let spawned = thread::Builder::new()
                    .name("veriflow-veille".into())
                    .spawn(move || hold(rx));
                // Sans fil (cas extrême), le travail continue sans garde.
                if spawned.is_ok() {
                    *keeper = Some(Keeper { stop, users: 1 });
                }
            }
        }
    }
    KeepAwake(())
}

impl Drop for KeepAwake {
    fn drop(&mut self) {
        if let Ok(mut keeper) = KEEPER.lock() {
            if let Some(k) = keeper.as_mut() {
                k.users = k.users.saturating_sub(1);
                if k.users == 0 {
                    if let Some(k) = keeper.take() {
                        let _ = k.stop.send(());
                    }
                }
            }
        }
    }
}

/// Windows : état d'exécution du fil (rendu à la fin, ou à la sortie du fil).
#[cfg(windows)]
fn hold(rx: Receiver<()>) {
    use windows_sys::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_SYSTEM_REQUIRED,
    };
    // SAFETY : appel documenté, sans pointeur ; vaut pour ce fil seulement.
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED);
    }
    let _ = rx.recv();
    // SAFETY : idem, retour à l'état normal.
    unsafe {
        SetThreadExecutionState(ES_CONTINUOUS);
    }
}

/// macOS : activité déclarée « lancée par l'utilisateur » (ni veille
/// automatique ni App Nap).
#[cfg(target_os = "macos")]
fn hold(rx: Receiver<()>) {
    use objc2_foundation::{NSActivityOptions, NSProcessInfo, NSString};
    objc2::rc::autoreleasepool(|_| {
        let info = NSProcessInfo::processInfo();
        let token = info.beginActivityWithOptions_reason(
            NSActivityOptions::UserInitiated,
            &NSString::from_str("VERIFLOW : copie ou conversion en cours"),
        );
        let _ = rx.recv();
        // SAFETY : jeton rendu par beginActivityWithOptions:reason: ci-dessus.
        unsafe { info.endActivity(&token) };
    });
}

/// Linux : verrou de systemd-logind tenu par `systemd-inhibit`, qui attend la
/// fin de son entrée ; absent (autre système d'init), rien n'est fait.
#[cfg(all(unix, not(target_os = "macos")))]
fn hold(rx: Receiver<()>) {
    use std::process::{Command, Stdio};
    let child = Command::new("systemd-inhibit")
        .args([
            "--what=sleep:idle",
            "--who=VERIFLOW",
            "--why=Copie ou conversion en cours",
            "--mode=block",
            "cat",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();
    let _ = rx.recv();
    if let Ok(mut c) = child {
        // Entrée fermée : `cat` se termine, et le verrou avec lui.
        drop(c.stdin.take());
        let _ = c.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Les autres tests (copies, conversions) prennent aussi des gardes en
    // parallèle : seul ce qui reste vrai dans tous les cas est vérifié.
    #[test]
    fn keeper_held_while_a_guard_exists() {
        let a = keep_awake();
        let b = keep_awake();
        assert!(KEEPER
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|k| k.users >= 2));
        drop(a);
        assert!(KEEPER
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|k| k.users >= 1));
        drop(b);
    }
}
