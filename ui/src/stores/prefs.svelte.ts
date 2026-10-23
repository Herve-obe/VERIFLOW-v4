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
  /** Affichages du PLAYER au premier lancement d'un lecteur ; `reportAuto` :
   *  saisie du rapport ouverte à chaque nouvelle prise (projet ouvert). */
  player: { tcOverlay: boolean; scopes: boolean; markerColor: MarkerColor; reportAuto: boolean };
  /** Destination proposée par TRANSCODE à chaque ouverture. */
  transcode: { dest: string; nextToSource: boolean };
}

export const DEFAULT_PREFS: Prefs = {
  shortcuts: {},
  offload: { template: "{date}/{carte}", algorithms: ["xxh128"], eject: false, operator: "" },
  player: { tcOverlay: false, scopes: false, markerColor: "red", reportAuto: false },
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

/** Rubriques réglées par le bouton « Valider » des Préférences. */
export type AppliedPrefs = Pick<Prefs, "offload" | "player" | "transcode">;

const listeners = new Set<() => void>();

/** Appelé quand les Préférences sont validées : l'onglet reprend aussitôt
 *  ces valeurs. Rend la fonction de désabonnement (à rendre par onMount). */
export function onPrefsApplied(fn: () => void): () => void {
  listeners.add(fn);
  return () => listeners.delete(fn);
}

/** Enregistre les rubriques validées et les applique aux onglets ouverts. */
export function applyPrefs(next: AppliedPrefs) {
  const v = structuredClone(next);
  prefs.offload = v.offload;
  prefs.player = v.player;
  prefs.transcode = v.transcode;
  // Choix mémorisés par les lecteurs (VideoView, MarkersPanel) : effacés
  // pour qu'un lecteur ouvert plus tard parte aussi des Préférences.
  try {
    ["veriflow.player.tc.overlay", "veriflow.player.scopes", "veriflow.marker.color"].forEach((k) => localStorage.removeItem(k));
  } catch {
    /* stockage indisponible */
  }
  listeners.forEach((fn) => fn());
}

/** Raccourci en vigueur d'une action : celui de l'utilisateur, sinon celui
 *  par défaut (undefined : aucun). */
export function effectiveShortcut(id: string, fallback?: Shortcut): Shortcut | undefined {
  if (id in prefs.shortcuts) return prefs.shortcuts[id] ?? undefined;
  return fallback;
}
