// Pont vers le cœur Rust (commandes Tauri). Toute communication UI -> cœur passe par ici.
import { invoke } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { ProjectInfo } from "../stores/app.svelte";
import { t } from "../i18n/index.svelte";

export interface AppInfo {
  name: string;
  version: string;
  os: string;
  arch: string;
}

/** Vrai quand l'interface tourne dans Tauri (et non dans un simple navigateur). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function appInfo(): Promise<AppInfo> {
  if (!inTauri) return { name: "VERIFLOW", version: "dev", os: "web", arch: "" };
  return invoke<AppInfo>("app_info");
}

const PROJECT_FILTER = () => [{ name: t("dialog.filter.project"), extensions: ["veriflow"] }];

export async function pickProjectToCreate(): Promise<string | null> {
  return save({ filters: PROJECT_FILTER(), defaultPath: "Projet.veriflow" });
}

export async function pickProjectToOpen(): Promise<string | null> {
  const result = await open({ filters: PROJECT_FILTER(), multiple: false, directory: false });
  return typeof result === "string" ? result : null;
}

export const projectCreate = (path: string) => invoke<ProjectInfo>("project_create", { path });
export const projectOpen = (path: string) => invoke<ProjectInfo>("project_open", { path });
export const projectClose = () => invoke<void>("project_close");
