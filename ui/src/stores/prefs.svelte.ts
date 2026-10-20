// Préférences de l'utilisateur (Préférences : Général, Raccourcis, OFFLOAD,
// PLAYER, TRANSCODE), gardées sur le poste et conservées par la remise à zéro.
import type { Shortcut } from "../shortcuts/defaults";
import type { HashAlgo } from "../lib/offload";
import type { MarkerColor } from "../lib/logs";

export const PREFS_KEY = "veriflow.prefs";

export interface Prefs {
  /** Raccourcis modifiés, par action ; null : raccourci retiré. */
  shortcuts: Record<string, Shortcut | null>;
  /** Valeurs de départ d'OFFLOAD (première ouverture, ou après remise à zéro). */
  offload: { template: string; algorithms: HashAlgo[]; eject: boolean; operator: string };
  /** Affichages du PLAYER au premier lancement d'un lecteur. */
  player: { tcOverlay: boolean; scopes: boolean; markerColor: MarkerColor };
  /** Destination proposée par TRANSCODE à chaque ouverture. */
  transcode: { dest: string; nextToSource: boolean };
}

export const DEFAULT_PREFS: Prefs = {
  shortcuts: {},
  offload: { template: "{date}/{carte}", algorithms: ["xxh128"], eject: false, operator: "" },
  player: { tcOverlay: false, scopes: false, markerColor: "red" },
  transcode: { dest: "", nextToSource: false },
};

function load(): Prefs {
  const d = structuredClone(DEFAULT_PREFS);
  try {
    const s = JSON.parse(localStorage.getItem(PREFS_KEY) ?? "{}");
    return {
      shortcuts: { ...d.shortcuts, ...(s.shortcuts ?? {}) },
      offload: { ...d.offload, ...(s.offload ?? {}) },
      player: { ...d.player, ...(s.player ?? {}) },
      transcode: { ...d.transcode, ...(s.transcode ?? {}) },
    };
  } catch {
    return d;
  }
}

export const prefs = $state<Prefs>(load());

// Enregistrées à chaque changement.
$effect.root(() => {
  $effect(() => {
    const json = JSON.stringify(prefs);
    try {
      localStorage.setItem(PREFS_KEY, json);
    } catch {
      /* stockage indisponible */
    }
  });
});

/** Raccourci en vigueur d'une action : celui de l'utilisateur, sinon celui
 *  par défaut (undefined : aucun). */
export function effectiveShortcut(id: string, fallback?: Shortcut): Shortcut | undefined {
  if (id in prefs.shortcuts) return prefs.shortcuts[id] ?? undefined;
  return fallback;
}
