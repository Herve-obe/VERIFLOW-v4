// Accès aux commandes du PLAYER (voir src-tauri/src/player.rs).
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { FrameRate } from "./timecode";

export interface VideoClip {
  info: {
    path: string;
    format: string;
    duration: number;
    size: number;
    video: { codec: string; width: number; height: number; rate: FrameRate; pix_fmt: string; frame_count: number } | null;
    audio: { codec: string; sample_rate: number; channels: number; bits: number | null }[];
    start_timecode: string | null;
  };
  display_width: number;
  display_height: number;
  frame_count: number;
  start_frame: number;
}

export interface TrackInfo {
  name: string;
  file: number;
  channel: number;
}

export interface AudioOpened {
  session: {
    files: { path: string; channels: number; sample_rate: number; bits: number; format: "Int" | "Float"; frames: number; time_reference: number | null; ixml: { scene: string | null; take: string | null; tape: string | null; project: string | null; note: string | null; circled: boolean | null } }[];
    tracks: TrackInfo[];
    sample_rate: number;
    frames: number;
    duration: number;
  };
  output: OutputInfo;
}

export interface OutputInfo {
  device: string;
  id: string;
  host: string;
  first_channel: number;
  sample_rate: number;
  channels: number;
  resampling: boolean;
}

/** Sortie audio du poste (voir core/src/player/audio/engine.rs). */
export interface OutputDevice {
  id: string;
  host: string;
  name: string;
  default: boolean;
  channels: number;
}

/** Sortie demandée : `device` absent = sortie par défaut du système. */
export interface OutputChoice {
  device: string | null;
  first_channel: number;
}

export interface AudioStatus {
  playing: boolean;
  position: number;
  track_peaks: (number | null)[];
  master_peaks: [number | null, number | null];
  lufs_momentary: number | null;
  lufs_short_term: number | null;
  lufs_integrated: number | null;
}

const VIDEO_EXT = ["mov", "mp4", "mxf", "mkv", "avi", "mts", "m2ts", "mpg", "mpeg", "m4v", "webm", "braw", "r3d"];
const AUDIO_EXT = ["wav", "bwf", "rf64", "w64", "aif", "aiff", "flac", "mp3", "m4a", "aac", "ogg", "opus"];
const LUT_EXT = ["cube", "3dl", "dat", "m3d", "csp"];

export async function pickLut(label: string): Promise<string | null> {
  const r = await open({ multiple: false, filters: [{ name: label, extensions: LUT_EXT }] });
  return typeof r === "string" ? r : null;
}

export async function pickVideo(label: string): Promise<string | null> {
  const r = await open({ multiple: false, filters: [{ name: label, extensions: VIDEO_EXT }] });
  return typeof r === "string" ? r : null;
}

export async function pickAudio(label: string): Promise<string[]> {
  const r = await open({ multiple: true, filters: [{ name: label, extensions: AUDIO_EXT }] });
  if (!r) return [];
  return Array.isArray(r) ? r : [r];
}

/** Emplacement du lecteur : « player » (onglet PLAYER) ou « preview » (lecteur rapide de MEDIA).
 *  Le son d'une vidéo utilise l'emplacement audio « <emplacement>-av ». */
export type Slot = "player" | "preview";
export type AudioSlot = Slot | "player-av" | "preview-av";

export const videoOpen = (path: string, slot: Slot = "player") => invoke<VideoClip>("video_open", { path, slot });
/** Image JPEG encodée. Selon le canal IPC utilisé, Tauri renvoie un ArrayBuffer
 *  (canal rapide) ou un tableau de nombres (canal de secours) : on normalise. */
export async function videoFrame(index: number, slot: Slot = "player"): Promise<Uint8Array<ArrayBuffer>> {
  const raw = await invoke<ArrayBuffer | number[]>("video_frame", { index, slot });
  return raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);
}
export const videoClose = (slot: Slot = "player") => invoke<void>("video_close", { slot });
/** LUT d'affichage ; `path` null pour la retirer. */
export const videoLut = (path: string | null, slot: Slot = "player") => invoke<void>("video_lut", { path, slot });

export const audioOutputs = () => invoke<OutputDevice[]>("audio_outputs");
export const audioOpen = (paths: string[], slot: AudioSlot = "player", output: OutputChoice | null = null) =>
  invoke<AudioOpened>("audio_open", { paths, slot, output });
export const audioClose = (slot: AudioSlot = "player") => invoke<void>("audio_close", { slot });
export const audioTransport = (action: "play" | "pause" | "stop", slot: AudioSlot = "player") =>
  invoke<void>("audio_transport", { action, slot });
export const audioSeek = (seconds: number, slot: AudioSlot = "player") => invoke<void>("audio_seek", { seconds, slot });
export const audioTrack = (index: number, gainDb: number, pan: number, mute: boolean, solo: boolean, slot: AudioSlot = "player") =>
  invoke<void>("audio_track", { index, gainDb, pan, mute, solo, slot });
export const audioStatus = (slot: AudioSlot = "player") => invoke<AudioStatus>("audio_status", { slot });
