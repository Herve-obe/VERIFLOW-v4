// Création et ouverture du projet, partagées par le menu du haut et les
// fenêtres qui demandent un projet (marqueurs du PLAYER...).
import { app } from "../stores/app.svelte";
import { t } from "../i18n/index.svelte";
import { projectCreate, projectOpen, projectClose, pickProjectToCreate, pickProjectToOpen } from "./api";

async function run(action: () => Promise<boolean>): Promise<boolean> {
  try {
    return await action();
  } catch (err) {
    app.status = `${t("project.error")} : ${err}`;
    return false;
  }
}

/** Crée un projet (boîte d'enregistrement) ; vrai si un projet est ouvert ensuite. */
export const createProject = () =>
  run(async () => {
    const path = await pickProjectToCreate();
    if (!path) return false;
    app.project = await projectCreate(path);
    return true;
  });

/** Ouvre un projet existant ; vrai si un projet est ouvert ensuite. */
export const openProject = () =>
  run(async () => {
    const path = await pickProjectToOpen();
    if (!path) return false;
    app.project = await projectOpen(path);
    return true;
  });

export const closeProject = () =>
  run(async () => {
    await projectClose();
    app.project = null;
    return true;
  });
