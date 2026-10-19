// Création, ouverture et enregistrement du projet, partagés par la barre de
// menus et les fenêtres qui demandent un projet (marqueurs du PLAYER...).
import { app } from "../stores/app.svelte";
import { t } from "../i18n/index.svelte";
import {
  pickProjectToCreate,
  pickProjectToOpen,
  pickProjectToSaveAs,
  projectClose,
  projectCreate,
  projectOpen,
  projectSave,
  projectSaveAs,
} from "./api";
import type { ProjectInfo } from "../stores/app.svelte";

// Projets récents (10 au plus), mémorisés sur le poste.
const RECENT_KEY = "veriflow.recent";
const RECENT_MAX = 10;

function readRecent(): string[] {
  try {
    const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(v) ? v.filter((p) => typeof p === "string") : [];
  } catch {
    return [];
  }
}

export const recent = $state({ paths: readRecent() });

function storeRecent() {
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify(recent.paths));
  } catch {
    /* stockage indisponible */
  }
}

function remember(info: ProjectInfo) {
  recent.paths = [info.path, ...recent.paths.filter((p) => p !== info.path)].slice(0, RECENT_MAX);
  storeRecent();
}

export function clearRecent() {
  recent.paths = [];
  storeRecent();
}

async function run(action: () => Promise<boolean>): Promise<boolean> {
  try {
    return await action();
  } catch (err) {
    app.status = `${t("project.error")} : ${err}`;
    return false;
  }
}

function opened(info: ProjectInfo) {
  app.project = info;
  remember(info);
}

/** Crée un projet (boîte d'enregistrement) ; vrai si un projet est ouvert ensuite. */
export const createProject = () =>
  run(async () => {
    const path = await pickProjectToCreate();
    if (!path) return false;
    opened(await projectCreate(path));
    return true;
  });

/** Ouvre un projet existant ; vrai si un projet est ouvert ensuite. */
export const openProject = () =>
  run(async () => {
    const path = await pickProjectToOpen();
    if (!path) return false;
    opened(await projectOpen(path));
    return true;
  });

/** Ouvre un projet de la liste des récents (retiré de la liste s'il a disparu). */
export const openRecent = (path: string) =>
  run(async () => {
    try {
      opened(await projectOpen(path));
      return true;
    } catch (err) {
      recent.paths = recent.paths.filter((p) => p !== path);
      storeRecent();
      throw err;
    }
  });

/** Enregistre le projet : tout est déjà écrit au fur et à mesure, le cœur le confirme. */
export const saveProject = () =>
  run(async () => {
    if (!app.project) return false;
    await projectSave();
    app.status = t("project.saved");
    return true;
  });

/** Enregistre une copie du projet sous un autre nom ; la copie devient le projet ouvert. */
export const saveProjectAs = () =>
  run(async () => {
    if (!app.project) return false;
    const path = await pickProjectToSaveAs(app.project.path);
    if (!path) return false;
    opened(await projectSaveAs(path));
    app.status = `${t("project.saved.as")} ${app.project?.path ?? path}`;
    return true;
  });

export const closeProject = () =>
  run(async () => {
    await projectClose();
    app.project = null;
    return true;
  });
