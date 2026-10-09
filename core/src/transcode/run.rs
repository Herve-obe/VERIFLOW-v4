//! Lancement de FFmpeg : dossier de travail, avancement, annulation.

use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use super::build::Plan;
use super::filters::Asset;
use crate::{tools, Error, Result};

/// Dossier temporaire où sont copiés les fichiers utilisés par les filtres ;
/// supprimé à la fin.
pub struct WorkDir {
    pub path: PathBuf,
}

impl WorkDir {
    pub fn new(assets: &[(String, Asset)]) -> Result<Self> {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "veriflow-transcode-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path)?;
        for (name, asset) in assets {
            let target = path.join(name);
            match asset {
                Asset::File(src) => {
                    std::fs::copy(src, &target)?;
                }
                Asset::Bytes(b) => std::fs::write(&target, b)?,
                Asset::Text(t) => std::fs::write(&target, t)?,
            }
        }
        Ok(Self { path })
    }
}

impl Drop for WorkDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

/// Lance FFmpeg pour un plan et suit son avancement. `output` : fichier
/// produit, ou `None` pour une analyse (sortie « null »). Renvoie les
/// messages de FFmpeg (lus par les analyses).
pub fn run(
    plan: &Plan,
    input: &Path,
    output: Option<&Path>,
    verbose: bool,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(f64, f64),
) -> Result<String> {
    let work = WorkDir::new(&plan.assets)?;
    let mut cmd = tools::command("ffmpeg")?;
    cmd.current_dir(&work.path)
        .args([
            "-hide_banner",
            "-nostdin",
            "-y",
            "-v",
            if verbose { "info" } else { "error" },
        ])
        .args(["-progress", "pipe:1", "-nostats", "-stats_period", "0.5"])
        .args(&plan.input_args)
        .arg("-i")
        .arg(input);
    for extra in &plan.extra_inputs {
        cmd.args(&extra.args).arg("-i").arg(&extra.path);
    }
    cmd.args(&plan.args);
    match output {
        Some(out) => {
            cmd.args(["-f", &plan.format]).arg(out);
        }
        None => {
            cmd.args(["-f", "null", "-"]);
        }
    }
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let duration = plan.duration;
    let mut child: Child = cmd.spawn()?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let child = Mutex::new(child);
    let finished = AtomicBool::new(false);
    let cancelled = AtomicBool::new(false);
    let (status, text) = thread::scope(|scope| -> Result<_> {
        let errors = scope.spawn(move || {
            let mut text = String::new();
            if let Some(mut e) = stderr {
                let _ = e.read_to_string(&mut text);
            }
            text
        });
        // Surveillance de l'annulation, indépendante du rythme des messages de FFmpeg.
        scope.spawn(|| {
            while !finished.load(Ordering::Relaxed) {
                if cancel.load(Ordering::Relaxed) {
                    cancelled.store(true, Ordering::Relaxed);
                    if let Ok(mut c) = child.lock() {
                        let _ = c.kill();
                    }
                    break;
                }
                thread::sleep(Duration::from_millis(100));
            }
        });
        if let Some(out) = stdout {
            let (mut time, mut speed) = (0.0f64, 0.0f64);
            for line in BufReader::new(out).lines().map_while(|l| l.ok()) {
                let Some((k, v)) = line.split_once('=') else {
                    continue;
                };
                match k {
                    // Les deux clés sont en microsecondes (historique de FFmpeg).
                    "out_time_us" | "out_time_ms" => {
                        if let Ok(us) = v.trim().parse::<f64>() {
                            time = us / 1_000_000.0;
                        }
                    }
                    "speed" => speed = v.trim().trim_end_matches('x').parse().unwrap_or(speed),
                    "progress" => {
                        let f = if duration > 0.0 {
                            (time / duration).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        progress(if v.trim() == "end" { 1.0 } else { f }, speed);
                    }
                    _ => {}
                }
            }
        }
        let status = loop {
            let done = child.lock().unwrap_or_else(|e| e.into_inner()).try_wait();
            match done {
                Ok(Some(s)) => break Ok(s),
                Ok(None) => thread::sleep(Duration::from_millis(50)),
                Err(e) => break Err(e),
            }
        };
        finished.store(true, Ordering::Relaxed);
        let text = errors.join().unwrap_or_default();
        Ok((status?, text))
    })?;
    if cancelled.load(Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    if !status.success() {
        // Première ligne utile : les dernières lignes de FFmpeg sont génériques.
        const GENERIC: [&str; 8] = [
            "Fontconfig",
            "Task finished",
            "Terminating thread",
            "Nothing was written",
            "Conversion failed",
            "Error sending frames",
            "Could not open encoder before EOF",
            "Error while filtering",
        ];
        let message = text
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty() && !GENERIC.iter().any(|g| l.contains(g)))
            .or_else(|| text.lines().map(str::trim).rfind(|l| !l.is_empty()))
            .unwrap_or("échec sans message")
            .to_owned();
        return Err(Error::Tool {
            tool: "ffmpeg".into(),
            message,
        });
    }
    Ok(text)
}
