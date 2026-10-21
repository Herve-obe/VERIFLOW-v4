// Données de VERIFLOW sur le poste (stockage du moteur web, clés « veriflow. ») :
// remise à zéro à l'ouverture, ou effacement complet (Préférences > Général).
// Les projets .veriflow, rangés là où l'utilisateur les a créés, ne sont
// jamais touchés.

const PREFIX = "veriflow.";
const STARTUP_KEY = "veriflow.startup";

export type StartupMode = "resume" | "fresh";

/** Gardés par la remise à zéro : réglages et créations de l'utilisateur. Le
 *  reste est l'état de travail (destinations et options d'OFFLOAD, réglages
 *  de SYNC, dossiers dépliés, affichages du PLAYER...). */
const KEEP = new Set([
  "veriflow.language",
  STARTUP_KEY,
  "veriflow.recent",
  "veriflow.explorer.favorites",
  "veriflow.transcode.user",
  "veriflow.audio.output",
  "veriflow.prefs",
]);

function keys(): string[] {
  try {
    return Object.keys(localStorage).filter((k) => k.startsWith(PREFIX));
  } catch {
    return [];
  }
}

function remove(list: string[]) {
  try {
    list.forEach((k) => localStorage.removeItem(k));
  } catch {
    /* stockage indisponible */
  }
}

export function startupMode(): StartupMode {
  try {
    return localStorage.getItem(STARTUP_KEY) === "fresh" ? "fresh" : "resume";
  } catch {
    return "resume";
  }
}

export function setStartupMode(mode: StartupMode) {
  try {
    localStorage.setItem(STARTUP_KEY, mode);
  } catch {
    /* stockage indisponible */
  }
}

/** Efface l'état de travail, garde les réglages. */
export function resetSession() {
  remove(keys().filter((k) => !KEEP.has(k)));
}

/** À appeler avant le chargement de l'interface (les vues lisent leurs
 *  réglages à leur création). */
export function applyStartupMode() {
  if (startupMode() === "fresh") resetSession();
}

/** Efface toutes les données de VERIFLOW sur ce poste, réglages compris. */
export function clearAllData() {
  remove(keys());
}
