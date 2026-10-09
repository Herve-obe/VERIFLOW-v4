// Accès aux commandes TRANSCODE (voir src-tauri/src/transcode.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Category =
  | "intermediate"
  | "delivery"
  | "proxy"
  | "no_reencode"
  | "audio"
  | "analysis";
export type Domain = "video" | "audio";
export type BitDepth = "16" | "24" | "32f";
export type Existing = "rename" | "overwrite" | "skip";

export interface PresetView {
  id: string;
  category: Category;
  domain: Domain;
  label: string;
  ext: string;
  scale: string;
  suffix: string;
  video: boolean;
  audio: boolean;
  video_bitrate: boolean;
  encoder_choice: boolean;
  bit_depths: BitDepth[];
  audio_bitrates: number[];
  default_audio_bitrate: number;
  analysis: boolean;
}

export interface LoudnessTarget {
  integrated: number;
  true_peak: number;
}

export interface Settings {
  preset: string;
  scale: string | null;
  video_mbps: number | null;
  software: boolean;
  sample_rate: number | null;
  bit_depth: BitDepth | null;
  audio_kbps: number | null;
  loudness: LoudnessTarget | null;
}

export interface TranscodeRequest {
  sources: string[];
  settings: Settings;
  dest: string | null;
  suffix: string;
  existing: Existing;
}

export interface EncoderChoice {
  name: string;
  hardware: boolean;
  uncertified_prores: boolean;
}

export interface Loudness {
  integrated: number;
  range: number;
  true_peak: number;
  sample_peak: number;
  gain: number | null;
  peak_limited: boolean;
}

export type Status = "done" | "skipped" | "failed" | "cancelled";

export interface FileResult {
  source: string;
  output: string | null;
  status: Status;
  message: string | null;
  encoder: EncoderChoice | null;
  loudness: Loudness | null;
  seconds: number;
}

export interface Summary {
  files: FileResult[];
  cancelled: boolean;
  seconds: number;
}

export type EngineEvent =
  | { type: "file_started"; index: number; source: string }
  | { type: "progress"; index: number; fraction: number; speed: number }
  | { type: "file_done"; index: number; result: FileResult };

export type Notice =
  | { kind: "queued"; job: number; files: number }
  | { kind: "running"; job: number }
  | { kind: "engine"; job: number; event: EngineEvent }
  | { kind: "done"; job: number; summary: Summary }
  | { kind: "failed"; job: number; error: string };

/** Cibles de normalisation courantes. */
export const LOUDNESS_TARGETS: { id: string; target: LoudnessTarget }[] = [
  { id: "ebu", target: { integrated: -23, true_peak: -1 } },
  { id: "atsc", target: { integrated: -24, true_peak: -2 } },
  { id: "stream", target: { integrated: -14, true_peak: -1 } },
  { id: "podcast", target: { integrated: -16, true_peak: -1 } },
];

export const SAMPLE_RATES = [44100, 48000, 88200, 96000, 192000];

/** Tailles d'image proposées : "source", fractions ou hauteur en pixels. */
export const SCALES = ["source", "1/2", "1/4", "2160", "1080", "720", "540"];

export const presets = (lang: string) =>
  invoke<PresetView[]>("transcode_presets", { lang });
export const encoderFor = (preset: string, software: boolean) =>
  invoke<EncoderChoice | null>("transcode_encoder", { preset, software });
export const expand = (paths: string[]) =>
  invoke<string[]>("transcode_expand", { paths });
export const start = (request: TranscodeRequest) =>
  invoke<number>("transcode_start", { request });
export const cancel = (job: number) =>
  invoke<void>("transcode_cancel", { job });
export const onNotice = (cb: (n: Notice) => void): Promise<UnlistenFn> =>
  listen<Notice>("transcode", (e) => cb(e.payload));
