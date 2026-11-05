// Disposition des panneaux de l'onglet PLAYER, mémorisée séparément pour VIDEO et AUDIO.
// Clés localStorage : veriflow.player.layout.video / veriflow.player.layout.audio.
//
// API (lots 3 à 7) :
//   playerLayout.video / playerLayout.audio : { sizes, visible } réactifs, modifiables
//     directement (bind:size={playerLayout[mode].sizes.left}, visible.inspector = false...).
//   layoutOf(mode)      : disposition du mode.
//   resetLayout(mode)   : tailles et visibilités par défaut du mode.
//   saveLayout(mode)    : écrit dans localStorage (appelé par PlayerTab dans un $effect).
//   LAYOUT_LIMITS       : bornes min/max des tailles (px), pour les poignées.
import type { Mode } from "./app.svelte";

export type PanelId = "explorer" | "clips" | "banner" | "strip" | "inspector";
export const PANELS: PanelId[] = ["explorer", "clips", "banner", "strip", "inspector"];

export interface LayoutSizes {
  /** Largeur de la colonne de gauche (explorateur et liste). */
  left: number;
  /** Largeur de l'inspecteur. */
  right: number;
  /** Hauteur de l'explorateur dans la colonne de gauche (la liste prend le reste). */
  leftSplit: number;
  /** Hauteur de la bande de clips. */
  strip: number;
}

export interface Layout {
  sizes: LayoutSizes;
  visible: Record<PanelId, boolean>;
}

export const DEFAULT_SIZES: LayoutSizes = { left: 260, right: 300, leftSplit: 220, strip: 120 };
export const LAYOUT_LIMITS: Record<keyof LayoutSizes, { min: number; max: number }> = {
  left: { min: 160, max: 520 },
  right: { min: 200, max: 560 },
  leftSplit: { min: 80, max: 600 },
  strip: { min: 72, max: 280 },
};
/** Largeur minimale de la zone centrale (l'inspecteur se réduit d'abord). */
export const CENTER_MIN = 480;

const key = (mode: Mode) => `veriflow.player.layout.${mode}`;

function defaults(): Layout {
  return {
    sizes: { ...DEFAULT_SIZES },
    visible: { explorer: true, clips: true, banner: true, strip: true, inspector: true },
  };
}

const num = (v: unknown, d: number, k: keyof LayoutSizes) =>
  typeof v === "number" && Number.isFinite(v)
    ? Math.min(LAYOUT_LIMITS[k].max, Math.max(LAYOUT_LIMITS[k].min, v))
    : d;

function load(mode: Mode): Layout {
  const out = defaults();
  try {
    const raw = localStorage.getItem(key(mode));
    if (!raw) return out;
    const j = JSON.parse(raw) as { sizes?: Partial<LayoutSizes>; visible?: Partial<Record<PanelId, boolean>> };
    for (const k of Object.keys(out.sizes) as (keyof LayoutSizes)[]) {
      out.sizes[k] = num(j.sizes?.[k], out.sizes[k], k);
    }
    for (const p of PANELS) {
      if (typeof j.visible?.[p] === "boolean") out.visible[p] = j.visible[p];
    }
  } catch {
    /* stockage absent ou JSON abîmé : disposition par défaut */
  }
  return out;
}

export const playerLayout = $state({ video: load("video"), audio: load("audio") });

export const layoutOf = (mode: Mode): Layout => playerLayout[mode];

export function resetLayout(mode: Mode) {
  const d = defaults();
  Object.assign(playerLayout[mode].sizes, d.sizes);
  Object.assign(playerLayout[mode].visible, d.visible);
}

export function saveLayout(mode: Mode) {
  const snap = $state.snapshot(playerLayout[mode]);
  try {
    localStorage.setItem(key(mode), JSON.stringify(snap));
  } catch {
    /* stockage indisponible : la disposition vaut pour la session */
  }
}
