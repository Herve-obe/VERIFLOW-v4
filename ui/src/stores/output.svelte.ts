// Sortie audio choisie (carte son, pilote, paire de canaux), commune au
// PLAYER AUDIO, au son des vidéos et au lecteur rapide. Mémorisée sur le poste.
import { audioOutputs, type OutputChoice, type OutputDevice } from "../lib/player";

const KEY = "veriflow.audio.output";

function load(): OutputChoice {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "null");
    if (v && typeof v.first_channel === "number") return { device: v.device ?? null, first_channel: v.first_channel };
  } catch {
    /* réglage absent ou illisible */
  }
  return { device: null, first_channel: 0 };
}

export const output = $state({
  choice: load(),
  devices: [] as OutputDevice[],
  loading: false,
  /** Incrémenté à chaque changement : les lecteurs ouverts rouvrent la sortie. */
  revision: 0,
});

export async function refreshOutputs() {
  output.loading = true;
  try {
    output.devices = await audioOutputs();
  } catch {
    output.devices = [];
  } finally {
    output.loading = false;
  }
}

export function setOutput(choice: OutputChoice) {
  output.choice = choice;
  output.revision++;
  try {
    localStorage.setItem(KEY, JSON.stringify(choice));
  } catch {
    /* stockage indisponible */
  }
}

/** Copie simple du choix courant, à transmettre au cœur. */
export const currentOutput = (): OutputChoice => ({ ...output.choice });
