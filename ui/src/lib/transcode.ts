// Accès aux commandes TRANSCODE (voir src-tauri/src/transcode.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type Category =
  | "intermediate"
  | "broadcast"
  | "delivery"
  | "web"
  | "proxy"
  | "legacy"
  | "image"
  | "no_reencode"
  | "audio"
  | "analysis";
export const CATEGORIES: Category[] = ["intermediate", "broadcast", "delivery", "web", "proxy", "legacy", "image", "no_reencode", "audio", "analysis"];
export type Domain = "video" | "audio" | "both";
export type BitDepth = "16" | "24" | "32f";
export type Existing = "rename" | "overwrite" | "skip";
export type AudioMode = "keep" | "first_two" | "mix" | "none";
export type Rotate = "none" | "cw90" | "ccw90" | "half" | "flip_h" | "flip_v";
export type Deinterlace = "yadif" | "bwdif" | "bwdif_field";
export type FpsMethod = "duplicate" | "blend" | "interpolate";
export type FitMode = "pad" | "crop";
export type Position = "top_left" | "top" | "top_right" | "center" | "bottom_left" | "bottom" | "bottom_right";
export const POSITIONS: Position[] = ["top_left", "top", "top_right", "center", "bottom_left", "bottom", "bottom_right"];

export interface PresetView {
  id: string;
  category: Category;
  domain: Domain;
  label: string;
  /** Famille : "video", "audio", "image", "conform", "cut_detect"... */
  kind: string;
  ext: string;
  scale: string;
  suffix: string;
  video: boolean;
  scale_free: boolean;
  audio: boolean;
  video_bitrate: boolean;
  encoder_choice: boolean;
  bit_depths: BitDepth[];
  audio_bitrates: number[];
  default_audio_bitrate: number;
  audio_mode: AudioMode;
  analysis: boolean;
  unavailable: string | null;
}

export interface LoudnessTarget {
  integrated: number;
  true_peak: number;
}

export interface TextStyle {
  position: Position;
  size: number;
  opacity: number;
  background: boolean;
}

export interface LogoStyle {
  path: string;
  position: Position;
  size: number;
  opacity: number;
}

export interface Trim {
  start: string | null;
  end: string | null;
}

/** Réglages, au format du cœur (voir core/src/transcode/settings.rs). */
export interface Settings {
  preset: string;
  scale: string | null;
  video_mbps: number | null;
  software: boolean;
  sample_rate: number | null;
  bit_depth: BitDepth | null;
  audio_kbps: number | null;
  loudness: LoudnessTarget | null;
  audio_mode: AudioMode | null;
  image: {
    crop: { top: number; bottom: number; left: number; right: number };
    aspect: string | null;
    frame: [number, number] | null;
    fit: FitMode;
    rotate: Rotate;
    deinterlace: Deinterlace | null;
    fps: string | null;
    fps_method: FpsMethod;
    decimate: boolean;
    lut: string | null;
    brightness: number;
    contrast: number;
    saturation: number;
    gamma: number;
    color_tags: string | null;
  };
  overlay: {
    timecode: TextStyle | null;
    tc_start: string | null;
    tc_offset: number;
    filename: TextStyle | null;
    text: TextStyle | null;
    text_value: string;
    logo: LogoStyle | null;
  };
  subtitles: { file: string | null; burn: boolean; size: number };
  trims: Record<string, Trim>;
  sequence: { single: boolean; position: string | null; every: number | null; tc_numbering: boolean };
  conform_rate: string | null;
  keep_pitch: boolean;
  audio_files: string[];
  audio_offset: number;
  sync_tc: boolean;
  insert_file: string | null;
  insert_at: string | null;
  reference: string | null;
  threshold: number | null;
  vmaf_after: boolean;
}

export const defaultTextStyle = (position: Position = "bottom"): TextStyle => ({ position, size: 4, opacity: 1, background: true });

export function defaultSettings(preset: string): Settings {
  return {
    preset,
    scale: null,
    video_mbps: null,
    software: false,
    sample_rate: null,
    bit_depth: null,
    audio_kbps: null,
    loudness: null,
    audio_mode: null,
    image: {
      crop: { top: 0, bottom: 0, left: 0, right: 0 },
      aspect: null,
      frame: null,
      fit: "pad",
      rotate: "none",
      deinterlace: null,
      fps: null,
      fps_method: "duplicate",
      decimate: false,
      lut: null,
      brightness: 0,
      contrast: 1,
      saturation: 1,
      gamma: 1,
      color_tags: null,
    },
    overlay: { timecode: null, tc_start: null, tc_offset: 0, filename: null, text: null, text_value: "", logo: null },
    subtitles: { file: null, burn: false, size: 0 },
    trims: {},
    sequence: { single: false, position: null, every: null, tc_numbering: false },
    conform_rate: null,
    keep_pitch: false,
    audio_files: [],
    audio_offset: 0,
    sync_tc: false,
    insert_file: null,
    insert_at: null,
    reference: null,
    threshold: null,
    vmaf_after: false,
  };
}

/** Réglages enregistrés : complète un objet ancien ou partiel avec les valeurs par défaut. */
export function completeSettings(partial: Partial<Settings> | undefined, preset: string): Settings {
  const d = defaultSettings(partial?.preset ?? preset);
  if (!partial) return d;
  return {
    ...d,
    ...partial,
    image: { ...d.image, ...(partial.image ?? {}), crop: { ...d.image.crop, ...(partial.image?.crop ?? {}) } },
    overlay: { ...d.overlay, ...(partial.overlay ?? {}) },
    subtitles: { ...d.subtitles, ...(partial.subtitles ?? {}) },
    sequence: { ...d.sequence, ...(partial.sequence ?? {}) },
    trims: {},
  };
}

export interface TranscodeRequest {
  sources: string[];
  settings: Settings;
  dest: string | null;
  prefix: string;
  suffix: string;
  replace_from: string;
  replace_to: string;
  numbering: boolean;
  number_start: number;
  number_digits: number;
  existing: Existing;
  checksum: boolean;
  report: boolean;
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

export interface Segment {
  start: number;
  end: number | null;
  start_tc: string | null;
  end_tc: string | null;
}

export interface Analysis {
  segments: Segment[];
  vmaf: number | null;
}

export type Status = "done" | "skipped" | "failed" | "cancelled";

export interface FileResult {
  source: string;
  output: string | null;
  outputs: string[];
  status: Status;
  message: string | null;
  encoder: EncoderChoice | null;
  loudness: Loudness | null;
  analysis: Analysis | null;
  checksum: string | null;
  size: number | null;
  vmaf: number | null;
  seconds: number;
}

export interface Summary {
  files: FileResult[];
  cancelled: boolean;
  seconds: number;
  report: string | null;
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
export const SCALES = ["source", "1/2", "1/4", "2160", "1080", "720", "540"];
export const FRAME_RATES = ["23.976", "24", "25", "29.97", "30", "48", "50", "59.94", "60"];
export const ASPECTS = ["16:9", "1.85:1", "2:1", "2.39:1", "4:3", "1:1", "4:5", "9:16"];

/** Seuil par défaut des détections, et unité affichée. */
export const THRESHOLDS: Record<string, { value: number; step: number; unit: string }> = {
  cut_detect: { value: 10, step: 1, unit: "%" },
  black_detect: { value: 0.1, step: 0.01, unit: "" },
  silence_detect: { value: -60, step: 1, unit: "dBFS" },
};

export const presets = (lang: string) => invoke<PresetView[]>("transcode_presets", { lang });
export const encoderFor = (preset: string, software: boolean) => invoke<EncoderChoice | null>("transcode_encoder", { preset, software });
export const expand = (paths: string[]) => invoke<string[]>("transcode_expand", { paths });
export const start = (request: TranscodeRequest) => invoke<number>("transcode_start", { request });
export const cancel = (job: number) => invoke<void>("transcode_cancel", { job });
export const writePreset = (path: string, content: string) => invoke<void>("transcode_preset_write", { path, content });
export const readPreset = (path: string) => invoke<string>("transcode_preset_read", { path });
export const onNotice = (cb: (n: Notice) => void): Promise<UnlistenFn> => listen<Notice>("transcode", (e) => cb(e.payload));
