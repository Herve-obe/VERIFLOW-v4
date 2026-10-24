// Lecteur rapide de MEDIA dans une fenêtre du système (src-tauri/src/quick.rs).
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface QuickMedia {
  path: string;
  kind: "video" | "audio";
  name: string;
}

interface Geometry {
  x: number;
  y: number;
  width: number;
  height: number;
  /** Fenêtre agrandie sur l'écran qui contient (x, y). */
  maximized?: boolean;
}

/** Dernière place de la fenêtre (pixels physiques), partagée par les fenêtres. */
const GEOMETRY_KEY = "veriflow.quick.geometry.px";

export function saveGeometry(g: Geometry) {
  try {
    localStorage.setItem(GEOMETRY_KEY, JSON.stringify(g));
  } catch {
    /* stockage indisponible */
  }
}

export function loadGeometry(): Geometry | null {
  try {
    const g = JSON.parse(localStorage.getItem(GEOMETRY_KEY) ?? "null");
    return g && [g.x, g.y, g.width, g.height].every(Number.isFinite) ? g : null;
  } catch {
    return null;
  }
}

/** Ouvre (ou recharge) le lecteur rapide sur un média. */
export const openQuick = (media: QuickMedia) =>
  invoke<void>("quick_open", { media, geometry: loadGeometry() });
/** Média demandé, lu par la fenêtre à son chargement. */
export const pendingQuick = () => invoke<QuickMedia | null>("quick_pending");
export const onQuickLoad = (cb: (m: QuickMedia) => void): Promise<UnlistenFn> =>
  listen<QuickMedia>("quick-load", (e) => cb(e.payload));
/** « Ouvrir dans le PLAYER » depuis la fenêtre : la fenêtre principale s'en charge. */
export const sendToPlayer = (m: QuickMedia) => emitTo("main", "quick-to-player", m);
export const onToPlayer = (cb: (m: QuickMedia) => void): Promise<UnlistenFn> =>
  listen<QuickMedia>("quick-to-player", (e) => cb(e.payload));
