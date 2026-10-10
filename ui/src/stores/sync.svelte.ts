// État de l'onglet SYNC, gardé quand on change d'onglet : fichiers, analyse,
// paire sélectionnée.
import type { Analysis } from "../lib/sync";

export const sync = $state({
  videos: [] as string[],
  audios: [] as string[],
  analysis: null as Analysis | null,
  selected: -1,
});
