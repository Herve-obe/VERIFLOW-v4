<!-- Création, ouverture et fermeture du fichier projet .veriflow (charte §5.3). -->
<script lang="ts">
  import { app } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";
  import { projectCreate, projectOpen, projectClose, pickProjectToCreate, pickProjectToOpen } from "../lib/api";

  async function run(action: () => Promise<void>) {
    try {
      await action();
    } catch (err) {
      app.status = `${t("project.error")} : ${err}`;
    }
  }

  const create = () =>
    run(async () => {
      const path = await pickProjectToCreate();
      if (path) app.project = await projectCreate(path);
    });

  const open = () =>
    run(async () => {
      const path = await pickProjectToOpen();
      if (path) app.project = await projectOpen(path);
    });

  const close = () =>
    run(async () => {
      await projectClose();
      app.project = null;
    });
</script>

<div class="project">
  <span class="name" title={app.project?.path ?? ""}>{app.project?.name ?? t("project.none")}</span>
  <button onclick={create}>{t("project.new")}</button>
  <button onclick={open}>{t("project.open")}</button>
  {#if app.project}
    <button onclick={close}>{t("project.close")}</button>
  {/if}
</div>

<style>
  .project {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
  }
  .name {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--vf-text-muted);
    margin-right: var(--vf-space-2);
  }
  button {
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  button:hover {
    background: var(--vf-surface-hover);
  }
</style>
