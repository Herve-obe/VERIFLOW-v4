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
});

export function setMode(mode: Mode) {
  app.mode = mode;
  document.documentElement.dataset.mode = mode;
}

export function toggleMode() {
  setMode(app.mode === "video" ? "audio" : "video");
}
