<!-- Création, ouverture et fermeture du fichier projet .veriflow (charte §5.3). -->
<script lang="ts">
  import { app } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";
  import { createProject, openProject, closeProject } from "../lib/project";

  const create = () => createProject();
  const open = () => openProject();
  const close = () => closeProject();
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
