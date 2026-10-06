// Marqueurs et exports de logs du PLAYER (voir src-tauri/src/logs.rs).
import { invoke } from "@tauri-apps/api/core";

/** Couleurs des marqueurs (celles d'Avid, reconnues par Premiere, Resolve, OTIO). */
export const MARKER_COLORS = ["red", "green", "blue", "cyan", "magenta", "yellow", "black", "white"] as const;
export type MarkerColor = (typeof MARKER_COLORS)[number];

export interface Marker {
  id: number;
  path: string;
  /** Position en images depuis le début du clip. */
  frame: number;
  in_frame: number | null;
  out_frame: number | null;
  color: MarkerColor;
  comment: string;
  scene: string;
  take: string;
}

export type LogFormat = "edl" | "ale" | "csv" | "fcpxml" | "otio";
export const LOG_FORMATS: { id: LogFormat; label: string }[] = [
  { id: "edl", label: "EDL CMX3600" },
  { id: "ale", label: "ALE" },
  { id: "csv", label: "CSV" },
  { id: "fcpxml", label: "FCPXML" },
  { id: "otio", label: "OTIO" },
];

/** Grille des marqueurs sur un fichier son (sans image) : 25 i/s, comme le cœur. */
export const AUDIO_LOG_FPS = 25;

export const markersList = (path: string | null) => invoke<Marker[]>("markers_list", { path });
export const markerSave = (marker: Marker) => invoke<Marker>("marker_save", { marker });
export const markerDelete = (id: number) => invoke<void>("marker_delete", { id });
export const logsExport = (format: LogFormat, paths: string[], dest: string, title: string) =>
  invoke<string>("logs_export", { format, paths, dest, title });

/** Couleur CSS d'un marqueur (jetons du thème). */
export const markerCss = (c: MarkerColor) => `var(--vf-marker-${c})`;
