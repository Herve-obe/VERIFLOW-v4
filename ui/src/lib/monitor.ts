// Sortie moniteur (voir src-tauri/src/monitor.rs).
import { invoke } from "@tauri-apps/api/core";
import { emitTo, listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Slot } from "./player";

export interface Screen {
  index: number;
  name: string;
  width: number;
  height: number;
  main: boolean;
}

/** Image à afficher : le moniteur la relit dans le cache du lecteur. */
export interface MonitorFrame {
  slot: Slot;
  index: number | null;
}

export const screens = () => invoke<Screen[]>("monitor_screens");
export const openMonitor = (index: number) => invoke<void>("monitor_open", { index });
export const closeMonitor = () => invoke<void>("monitor_close");
export const sendFrame = (f: MonitorFrame) => emitTo("monitor", "monitor-frame", f).catch(() => {});
export const onFrame = (cb: (f: MonitorFrame) => void): Promise<UnlistenFn> =>
  listen<MonitorFrame>("monitor-frame", (e) => cb(e.payload));
export const onClosed = (cb: () => void): Promise<UnlistenFn> => listen("monitor-closed", () => cb());
/** Fenêtre moniteur chargée et à l'écoute : elle demande l'image en cours. */
export const sendReady = () => emitTo("main", "monitor-ready").catch(() => {});
export const onReady = (cb: () => void): Promise<UnlistenFn> => listen("monitor-ready", () => cb());
