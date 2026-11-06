// Source de clips de l'onglet PLAYER (partagée par la liste, la bande, le bandeau et l'inspecteur).
//
// API (contrat pour les lots 3 à 6) :
//   player.clips    : MediaEntry[]       clips de la source active, déjà filtrés par mode (le lot 3 les remplit).
//   player.current  : number             index du clip chargé dans player.clips, -1 si aucun.
//   player.source   : "folder" | "playlist"
//   player.details  : SvelteMap<chemin, Described>   cache des describe (plafonné à DETAILS_MAX, le plus ancien sort).
//   currentClip()   : MediaEntry | null
//   currentDetails(): Described | null
//   setClips(clips, source, current?) : remplace la source (remet current à -1 ou à l'index donné).
//   cacheDetails(list)  : range des Described dans le cache.
//   select(i)       : fixe current et charge le clip (app.playerVideo = path, ou app.playerAudio = [path] pour un audio).
//   next() / prev() : select(current ± 1), sans effet aux bornes.
//   playerSources[mode] : état de source par mode (dir, entries du dossier, playlist, missing, loading).
//   loadFolder(mode, dir) : liste le dossier (récursif, filtré par mode) puis décrit par lots de 24 ;
//     un changement de dossier annule les lots en cours (compteur de génération).
//   playlist : addToPlaylist / removeFromPlaylist / moveInPlaylist / clearPlaylist (mémorisée par mode,
//     veriflow.player.playlist.video / .audio), checkPlaylist(mode) repère les fichiers disparus.
// Les lots ne touchent jamais app.playerVideo / playerAudio autrement que par select()
// ou par le clic de liste décrit au lot 3.
import { SvelteMap } from "svelte/reactivity";
import { app } from "./app.svelte";
import { describe, list, type Described, type MediaEntry } from "../lib/media";
import type { Mode } from "./app.svelte";

export type ClipSource = "folder" | "playlist";
export const DETAILS_MAX = 5000;

export const player = $state({
  clips: [] as MediaEntry[],
  current: -1,
  source: "folder" as ClipSource,
  details: new SvelteMap<string, Described>(),
});

export const currentClip = (): MediaEntry | null => player.clips[player.current] ?? null;

export const currentDetails = (): Described | null => {
  const c = currentClip();
  return c ? (player.details.get(c.path) ?? null) : null;
};

export function setClips(clips: MediaEntry[], source: ClipSource, current = -1) {
  player.clips = clips;
  player.source = source;
  player.current = current >= 0 && current < clips.length ? current : -1;
}

export function cacheDetails(list: Described[]) {
  for (const d of list) {
    player.details.delete(d.path); // ré-insérer garde l'ordre d'ancienneté
    player.details.set(d.path, d);
  }
  while (player.details.size > DETAILS_MAX) {
    const oldest = player.details.keys().next().value;
    if (oldest === undefined) break;
    player.details.delete(oldest);
  }
}

const inflight = new Set<string>();
/** Chemins dont l'analyse a échoué : pas de nouvel essai avant un rechargement explicite. */
const failed = new Set<string>();
const pending = new Set<string>();
let pumping = false;
const NOW_MAX = 48;

const usable = (d: Described) => !d.details.error || !!d.details.probe || !!d.details.wav;

/** Range les descriptions valides ; les erreurs ne vont pas dans le cache. */
function keep(batch: Described[]) {
  cacheDetails(batch.filter(usable));
  for (const d of batch) if (!usable(d)) failed.add(d.path);
}

async function pump() {
  if (pumping) return;
  pumping = true;
  try {
    while (pending.size > 0) {
      const todo = [...pending].slice(0, NOW_MAX);
      for (const p of todo) {
        pending.delete(p);
        inflight.add(p);
      }
      try {
        keep(await describe(todo));
      } catch {
        for (const p of todo) failed.add(p);
      } finally {
        for (const p of todo) inflight.delete(p);
      }
    }
  } finally {
    pumping = false;
  }
}

/** Décrit tout de suite (hors lots) les chemins absents du cache : un seul appel à la fois, demandes fusionnées. */
export function describeNow(paths: string[]) {
  for (const p of paths) {
    if (!player.details.has(p) && !inflight.has(p) && !failed.has(p)) pending.add(p);
  }
  void pump();
}

/** Oublie les analyses de la playlist du mode (ouverture de la playlist, volume éjecté). */
export function resetChecked(mode: Mode) {
  checked[mode].clear();
}

export function select(i: number) {
  const c = player.clips[i];
  if (!c) return;
  player.current = i;
  describeNow([c.path]);
  if (c.kind === "audio") app.playerAudio = [c.path];
  else app.playerVideo = c.path;
}

export const next = () => select(player.current + 1);
export const prev = () => select(player.current - 1);

// ---------------------------------------------------------------------------
// Sources par mode : dossier choisi dans l'explorateur et playlist.

export interface ModeSource {
  dir: string;
  entries: MediaEntry[];
  loading: boolean;
  /** Source affichée dans la liste. */
  source: ClipSource;
  playlist: MediaEntry[];
  /** Chemins de la playlist introuvables (grisés, jamais retirés en silence). */
  missing: string[];
  /** Chemins présents mais illisibles. */
  unreadable: string[];
  /** Dernier message d'erreur de chargement, vide sinon. */
  error: string;
}

const plKey = (mode: Mode) => `veriflow.player.playlist.${mode}`;

function loadPlaylist(mode: Mode): MediaEntry[] {
  try {
    const j = JSON.parse(localStorage.getItem(plKey(mode)) ?? "[]");
    if (!Array.isArray(j)) return [];
    return j.filter((e): e is MediaEntry => !!e && typeof e.path === "string" && typeof e.name === "string");
  } catch {
    return [];
  }
}

const blank = (mode: Mode): ModeSource => ({
  dir: "",
  entries: [],
  loading: false,
  source: "folder",
  playlist: loadPlaylist(mode),
  missing: [],
  unreadable: [],
  error: "",
});

export const playerSources = $state({ video: blank("video"), audio: blank("audio") });

const generation: Record<Mode, number> = { video: 0, audio: 0 };
const BATCH = 24;

export async function loadFolder(mode: Mode, dir: string) {
  const m = playerSources[mode];
  const gen = ++generation[mode];
  const reload = m.dir === dir;
  const old = m.entries;
  m.dir = dir;
  m.source = "folder";
  m.entries = [];
  m.error = "";
  m.loading = true;
  try {
    const all = await list(dir, true);
    if (gen !== generation[mode]) return;
    const entries = all.filter((e) => e.kind === mode);
    m.entries = entries;
    if (reload) {
      // Rechargement explicite du dossier courant : tout est réanalysé.
      for (const e of old) {
        player.details.delete(e.path);
        failed.delete(e.path);
      }
    }
    // Le clip chargé passe en premier, puis le reste par lots.
    const loaded = mode === "video" ? app.playerVideo : (app.playerAudio?.[0] ?? null);
    const paths = entries.map((e) => e.path);
    if (loaded) paths.sort((a, b) => Number(b === loaded) - Number(a === loaded));
    for (let i = 0; i < paths.length; i += BATCH) {
      const todo = paths.slice(i, i + BATCH).filter((p) => !inflight.has(p) && !failed.has(p) && (!player.details.has(p) || p === loaded));
      if (todo.length === 0) continue;
      for (const p of todo) inflight.add(p);
      let batch: Described[];
      try {
        batch = await describe(todo);
      } finally {
        for (const p of todo) inflight.delete(p);
      }
      if (gen !== generation[mode]) return;
      keep(batch);
    }
  } catch (e) {
    if (gen === generation[mode]) m.error = String(e);
  } finally {
    if (gen === generation[mode]) m.loading = false;
  }
}

function savePlaylist(mode: Mode) {
  try {
    localStorage.setItem(plKey(mode), JSON.stringify($state.snapshot(playerSources[mode].playlist)));
  } catch {
    /* stockage indisponible : la playlist vaut pour la session */
  }
}

export function addToPlaylist(mode: Mode, items: MediaEntry[]) {
  const m = playerSources[mode];
  const have = new Set(m.playlist.map((e) => e.path));
  const add = items.filter((e) => e.kind === mode && !have.has(e.path));
  if (add.length === 0) return;
  m.playlist = [...m.playlist, ...add];
  savePlaylist(mode);
}

export function removeFromPlaylist(mode: Mode, paths: string[]) {
  const m = playerSources[mode];
  const gone = new Set(paths);
  m.playlist = m.playlist.filter((e) => !gone.has(e.path));
  for (const p of paths) checked[mode].delete(p);
  m.missing = m.missing.filter((p) => !gone.has(p));
  m.unreadable = m.unreadable.filter((p) => !gone.has(p));
  savePlaylist(mode);
}

export function moveInPlaylist(mode: Mode, path: string, delta: -1 | 1) {
  const m = playerSources[mode];
  const i = m.playlist.findIndex((e) => e.path === path);
  const j = i + delta;
  if (i < 0 || j < 0 || j >= m.playlist.length) return;
  const next = [...m.playlist];
  [next[i], next[j]] = [next[j], next[i]];
  m.playlist = next;
  savePlaylist(mode);
}

export function clearPlaylist(mode: Mode) {
  const m = playerSources[mode];
  m.playlist = [];
  checked[mode].clear();
  m.missing = [];
  m.unreadable = [];
  savePlaylist(mode);
}

const checkGen: Record<Mode, number> = { video: 0, audio: 0 };
const checked: Record<Mode, Set<string>> = { video: new Set(), audio: new Set() };
const GONE = /no such file|not found|introuvable|os error (2|3)\b|cannot find|n'existe pas|does not exist/i;

/** Décrit les entrées nouvelles de la playlist (par lots) et repère les fichiers introuvables ou illisibles. */
export async function checkPlaylist(mode: Mode) {
  const m = playerSources[mode];
  const gen = ++checkGen[mode];
  const inList = new Set(m.playlist.map((e) => e.path));
  for (const p of [...checked[mode]]) if (!inList.has(p)) checked[mode].delete(p);
  const paths = [...inList].filter((p) => !checked[mode].has(p));
  try {
    for (let i = 0; i < paths.length; i += BATCH) {
      const batch = await describe(paths.slice(i, i + BATCH));
      if (gen !== checkGen[mode]) return;
      cacheDetails(batch.filter((d) => !d.details.error));
      for (const d of batch) {
        checked[mode].add(d.path);
        const bad = !!d.details.error && !d.details.probe && !d.details.wav;
        const gone = bad && GONE.test(d.details.error ?? "");
        m.missing = [...m.missing.filter((p) => p !== d.path), ...(gone ? [d.path] : [])];
        m.unreadable = [...m.unreadable.filter((p) => p !== d.path), ...(bad && !gone ? [d.path] : [])];
      }
    }
  } catch {
    /* cœur indisponible : l'état précédent est conservé */
  }
}
