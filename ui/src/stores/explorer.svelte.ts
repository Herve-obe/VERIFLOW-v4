// État partagé des explorateurs : volumes, favoris et cache des sous-dossiers,
// tenu à jour par les événements du cœur.
import { listDirs, volumes as fetchVolumes, onExplorer, norm, type DirEntry } from "../lib/explorer";
import type { Volume } from "../lib/offload";

const FAV_KEY = "veriflow.explorer.favorites";

function loadFavorites(): string[] {
  try {
    return JSON.parse(localStorage.getItem(FAV_KEY) ?? "[]");
  } catch {
    return [];
  }
}

export const explorer = $state({
  volumes: [] as Volume[],
  favorites: loadFavorites(),
  /** Sous-dossiers déjà lus, par chemin normalisé. */
  children: {} as Record<string, DirEntry[]>,
  /** Incrémenté à chaque changement signalé (pour rafraîchir les vues). */
  revision: 0,
  lastChanged: [] as string[],
});

function saveFavorites() {
  try {
    localStorage.setItem(FAV_KEY, JSON.stringify(explorer.favorites));
  } catch {
    /* stockage indisponible */
  }
}

export function toggleFavorite(path: string) {
  explorer.favorites = explorer.favorites.includes(path)
    ? explorer.favorites.filter((p) => p !== path)
    : [...explorer.favorites, path];
  saveFavorites();
}

/** Chemin d'origine de chaque dossier lu : la forme normalisée perd la barre
 *  finale des racines Windows (« D:\ » devient « D: », relatif au lecteur). */
const origin = new Map<string, string>();

export async function loadChildren(path: string): Promise<DirEntry[]> {
  origin.set(norm(path), path);
  try {
    const list = await listDirs(path);
    explorer.children[norm(path)] = list;
    return list;
  } catch {
    explorer.children[norm(path)] = [];
    return [];
  }
}

let started = false;
/** À appeler une fois : charge les volumes et s'abonne aux changements. */
export function startExplorer() {
  if (started) return;
  started = true;
  fetchVolumes()
    .then((v) => (explorer.volumes = v))
    .catch(() => {});
  onExplorer((n) => {
    if (n.kind === "volumes") {
      explorer.volumes = n.volumes;
      return;
    }
    const changed = n.paths.map(norm);
    // Relit les dossiers affichés concernés (le dossier lui-même ou son parent).
    for (const key of Object.keys(explorer.children)) {
      if (changed.some((c) => c === key || c.startsWith(`${key}/`) && !c.slice(key.length + 1).includes("/"))) {
        loadChildren(origin.get(key) ?? key);
      }
    }
    explorer.lastChanged = changed;
    explorer.revision++;
  }).catch(() => {
    started = false;
  });
}
