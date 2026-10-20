// Éditeur de sous-titres (voir src-tauri/src/subtitles.rs).
import { invoke } from "@tauri-apps/api/core";

export interface Cue {
  start: number;
  end: number;
  text: string;
  /** ASS : autres champs de la ligne Dialogue (style, calque, marges...). */
  ass?: Record<string, string>;
}

export type Issue =
  | { kind: "inverted" | "overlap" | "empty" }
  | { kind: "gap"; frames: number }
  | { kind: "too_short" | "too_long"; seconds: number }
  | { kind: "too_fast"; cps: number }
  | { kind: "line_too_long"; chars: number }
  | { kind: "too_many_lines"; lines: number };

export interface Rules {
  max_chars?: number;
  max_lines?: number;
  max_cps?: number;
  min_duration?: number;
  max_duration?: number;
  min_gap_frames?: number;
  fps?: number;
}

export const loadSubtitles = (path: string) => invoke<Cue[]>("subtitles_load", { path });
/** `template` : fichier ASS ouvert, dont l'en-tête et les styles sont recopiés. */
export const saveSubtitles = (path: string, cues: Cue[], template: string | null) =>
  invoke<void>("subtitles_save", { path, cues, template });
export const checkSubtitles = (cues: Cue[], rules: Rules) => invoke<Issue[][]>("subtitles_check", { cues, rules });

/** « HH:MM:SS,mmm ». */
export function formatTime(seconds: number): string {
  const ms = Math.round(Math.max(0, seconds) * 1000);
  const pad = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${pad(Math.floor(ms / 3600000))}:${pad(Math.floor(ms / 60000) % 60)}:${pad(Math.floor(ms / 1000) % 60)},${pad(ms % 1000, 3)}`;
}

/** « HH:MM:SS,mmm », « HH:MM:SS.mmm », « MM:SS.mmm » ou secondes. */
export function parseTime(text: string): number | null {
  const s = text.trim().replace(",", ".");
  if (/^\d+(\.\d+)?$/.test(s)) return Number(s);
  const p = s.split(":");
  if (p.length < 2 || p.length > 3) return null;
  const [h, m, sec] = p.length === 3 ? p : ["0", ...p];
  const H = Number(h);
  const M = Number(m);
  const S = Number(sec);
  if (![H, M, S].every(Number.isFinite) || M >= 60 || S >= 60 || H < 0) return null;
  return H * 3600 + M * 60 + S;
}
