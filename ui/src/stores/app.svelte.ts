// État global de l'interface : mode VIDEO/AUDIO, onglet actif, projet courant.
export type Mode = "video" | "audio";
export const TABS = ["offload", "media", "player", "sync", "transcode", "report"] as const;
export type Tab = (typeof TABS)[number];

export interface ProjectInfo {
  path: string;
  name: string;
  created_at: string;
}

export const app = $state({
  mode: "video" as Mode,
  tab: "offload" as Tab,
  project: null as ProjectInfo | null,
  status: "",
  /** Média demandé dans l'onglet PLAYER (bouton « Ouvrir dans le PLAYER » de MEDIA). */
  playerVideo: null as string | null,
  playerAudio: null as string[] | null,
});

/** Ouvre un média dans l'onglet PLAYER, dans le mode qui lui correspond. */
export function openInPlayer(path: string, kind: "video" | "audio" | "image" | string) {
  if (kind === "audio") {
    app.playerAudio = [path];
    setMode("audio");
  } else {
    app.playerVideo = path;
    setMode("video");
  }
  app.tab = "player";
}

export function setMode(mode: Mode) {
  app.mode = mode;
  document.documentElement.dataset.mode = mode;
}

export function toggleMode() {
  setMode(app.mode === "video" ? "audio" : "video");
}
