//! Commandes de l'éditeur de sous-titres (TRANSCODE).

use std::path::PathBuf;

use veriflow_core::subtitles::{self, Cue, Issue, Rules};

type CmdResult<T> = Result<T, String>;

fn text<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

#[tauri::command]
pub fn subtitles_load(path: PathBuf) -> CmdResult<Vec<Cue>> {
    subtitles::load(&path).map_err(text)
}

#[tauri::command]
pub fn subtitles_save(path: PathBuf, cues: Vec<Cue>, template: Option<PathBuf>) -> CmdResult<()> {
    subtitles::save(&path, &cues, template.as_deref()).map_err(text)
}

#[tauri::command]
pub fn subtitles_check(cues: Vec<Cue>, rules: Rules) -> Vec<Vec<Issue>> {
    subtitles::check(&cues, &rules)
}
