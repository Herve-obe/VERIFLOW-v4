// Accès aux commandes MEDIA (voir src-tauri/src/media.rs).
import { invoke } from "@tauri-apps/api/core";
import type { FrameRate } from "./timecode";

export type MediaKind = "video" | "audio" | "image";

export interface MediaEntry {
  path: string;
  name: string;
  rel: string;
  kind: MediaKind;
  size: number;
  modified: string;
}

export interface FieldDef {
  id: string;
  group: string;
  kind: "text" | "bool" | "number";
  applies: "video" | "audio" | "both";
  ale: string | null;
  ixml: string | null;
  xmp: string | null;
}

export interface Described {
  path: string;
  details: {
    kind: MediaKind | null;
    probe: {
      format: string;
      duration: number;
      video: { codec: string; width: number; height: number; rate: FrameRate; pix_fmt: string; frame_count: number } | null;
      audio: { codec: string; sample_rate: number; channels: number; bits: number | null }[];
      start_timecode: string | null;
      tags: Record<string, string>;
    } | null;
    wav: { channels: number; sample_rate: number; bits: number; format: "Int" | "Float"; frames: number; time_reference: number | null } | null;
    embedded: Record<string, string>;
    error: string | null;
  };
  values: Record<string, string>;
  edited: string[];
}

export interface Waveform {
  peaks: number[];
  duration: number;
}

export type ExportKind = "csv" | "ale" | "xmp" | "working_copy";

const toBytes = (raw: ArrayBuffer | number[]) =>
  raw instanceof ArrayBuffer ? new Uint8Array(raw) : Uint8Array.from(raw);

export const fields = () => invoke<FieldDef[]>("media_fields");
export const list = (dir: string, recursive: boolean) => invoke<MediaEntry[]>("media_list", { dir, recursive });
export const describe = (paths: string[]) => invoke<Described[]>("media_describe", { paths });
export const waveform = (path: string) => invoke<Waveform>("media_waveform", { path });
export const setMeta = (paths: string[], values: Record<string, string>) => invoke<number>("media_set", { paths, values });
export const exportMeta = (kind: ExportKind, paths: string[], base: string, out: string) =>
  invoke<{ written: string[]; errors: string[] }>("media_export", { kind, paths, base, out });

// Images d'aperçu : converties en URL locales et gardées en mémoire.
const urls = new Map<string, Promise<string>>();

function image(key: string, load: () => Promise<ArrayBuffer | number[]>): Promise<string> {
  let p = urls.get(key);
  if (!p) {
    p = load().then((raw) => URL.createObjectURL(new Blob([toBytes(raw)], { type: "image/jpeg" })));
    p.catch(() => urls.delete(key));
    urls.set(key, p);
  }
  return p;
}

export const thumbnailUrl = (path: string) => image(`t:${path}`, () => invoke("media_thumbnail", { path }));
export const filmstripUrl = (path: string, index: number) =>
  image(`f:${index}:${path}`, () => invoke("media_filmstrip", { path, index }));

/** Durée en secondes vers H:MM:SS. */
export function clock(seconds: number | undefined): string {
  if (!seconds || !Number.isFinite(seconds)) return "";
  const s = Math.round(seconds);
  return `${Math.floor(s / 3600)}:${String(Math.floor(s / 60) % 60).padStart(2, "0")}:${String(s % 60).padStart(2, "0")}`;
}
