// Accès aux commandes SYNC (voir src-tauri/src/sync.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface FrameRate {
  num: number;
  den: number;
}

export type TimeSource = "metadata" | "ltc" | "none";
export type Method = "timecode" | "ltc" | "waveform" | "manual";

export interface SyncFile {
  path: string;
  name: string;
  video: boolean;
  start: number | null;
  source: TimeSource;
  timecode: string | null;
  duration: number;
  rate: FrameRate | null;
  width: number;
  height: number;
  sample_rate: number;
  channels: number;
  ltc_channel: number | null;
  tracks: string[];
  error: string | null;
}

export interface Pair {
  video: number;
  audio: number | null;
  offset: number;
  method: Method;
  refined: boolean;
  confidence: number | null;
  drift: number | null;
  drift_frames: number | null;
  validated: boolean;
  note: string | null;
}

export interface Analysis {
  videos: SyncFile[];
  audios: SyncFile[];
  pairs: Pair[];
}

export interface Options {
  refine: boolean;
  waveform_search: boolean;
  drift: boolean;
  window: number;
}

export interface RewrapOptions {
  dest: string | null;
  format: "mov" | "mxf";
  keep_camera_audio: boolean;
  suffix: string;
  existing: "rename" | "overwrite" | "skip";
}

export interface ExportItem {
  video: SyncFile;
  audio: SyncFile | null;
  offset: number;
}

export interface ExportResult {
  video: string;
  output: string | null;
  error: string | null;
}

export type SyncEvent =
  | { type: "reading"; index: number; total: number; name: string }
  | { type: "matching"; index: number; total: number; name: string };

export interface ExportProgress {
  kind: "progress";
  index: number;
  total: number;
  fraction: number;
  name: string;
}

export interface Waves {
  video: number[];
  audio: number[];
  start: number;
  length: number;
}

export const fps = (r: FrameRate | null) => (r ? r.num / r.den : 25);

/** Timecode d'une heure (secondes depuis minuit) à la cadence donnée. */
export function timecodeAt(seconds: number, rate: FrameRate | null): string {
  const f = fps(rate);
  const nominal = Math.round(f);
  const total = Math.round((((seconds % 86400) + 86400) % 86400) * f);
  const pad = (n: number) => String(n).padStart(2, "0");
  const fr = total % nominal;
  const s = Math.floor(total / nominal);
  return `${pad(Math.floor(s / 3600) % 24)}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}:${pad(fr)}`;
}

export const expand = (paths: string[]) => invoke<{ videos: string[]; audios: string[] }>("sync_expand", { paths });
export const analyze = (videos: string[], audios: string[], options: Options) => invoke<Analysis>("sync_analyze", { videos, audios, options });
export const cancel = () => invoke<void>("sync_cancel");
export const refine = (video: SyncFile, audio: SyncFile, offset: number, window: number) =>
  invoke<{ offset: number; confidence: number }>("sync_refine", { video, audio, offset, window });
export const waveforms = (video: SyncFile, audio: SyncFile, offset: number, at: number, length: number, points: number) =>
  invoke<Waves>("sync_waveforms", { video, audio, offset, at, length, points });
export const rewrap = (items: ExportItem[], options: RewrapOptions) => invoke<ExportResult[]>("sync_rewrap", { items, options });
export const timeline = (kind: "fcpxml" | "otio", title: string, items: ExportItem[], path: string) =>
  invoke<void>("sync_timeline", { kind, title, items, path });
export const onProgress = (cb: (e: SyncEvent) => void): Promise<UnlistenFn> => listen<SyncEvent>("sync", (e) => cb(e.payload));
export const onExport = (cb: (e: ExportProgress) => void): Promise<UnlistenFn> => listen<ExportProgress>("sync-export", (e) => cb(e.payload));
