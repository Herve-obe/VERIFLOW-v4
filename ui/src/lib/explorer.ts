// Accès à l'explorateur de dossiers (voir src-tauri/src/explorer.rs).
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Volume } from "./offload";

export interface DirEntry {
  path: string;
  name: string;
  has_children: boolean;
}

export type ExplorerNotice = { kind: "changed"; paths: string[] } | { kind: "volumes"; volumes: Volume[] };

export const listDirs = (path: string) => invoke<DirEntry[]>("explorer_list", { path });
export const volumes = () => invoke<Volume[]>("explorer_volumes");
export const projectRoots = () => invoke<string[]>("explorer_project_roots");
export const watch = (owner: string, paths: string[], recursive = false) =>
  invoke<void>("explorer_watch", { owner, paths, recursive });
export const onExplorer = (cb: (n: ExplorerNotice) => void): Promise<UnlistenFn> =>
  listen<ExplorerNotice>("explorer", (e) => cb(e.payload));

/** Type de donnée utilisé pour le glisser-déposer d'un dossier. */
export const DRAG_TYPE = "application/x-veriflow-path";

/** Normalise un chemin pour les comparaisons (séparateurs, barre finale). */
export const norm = (p: string) => p.replace(/\\/g, "/").replace(/\/+$/, "") || "/";
