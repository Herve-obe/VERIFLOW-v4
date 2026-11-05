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
// Les lots ne touchent jamais app.playerVideo / playerAudio autrement que par select()
// ou par le clic de liste décrit au lot 3.
import { SvelteMap } from "svelte/reactivity";
import { app } from "./app.svelte";
import type { Described, MediaEntry } from "../lib/media";

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

export function select(i: number) {
  const c = player.clips[i];
  if (!c) return;
  player.current = i;
  if (c.kind === "audio") app.playerAudio = [c.path];
  else app.playerVideo = c.path;
}

export const next = () => select(player.current + 1);
export const prev = () => select(player.current - 1);
