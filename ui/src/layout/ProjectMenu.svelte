<!-- Projet ouvert : nom et dossier (chemin complet en info-bulle). Création,
     ouverture et enregistrement : menu Fichier (charte §5.3). -->
<script lang="ts">
  import { app } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";

  const folder = $derived.by(() => {
    const p = app.project?.path ?? "";
    const parts = p.split(/[\\/]/).filter(Boolean);
    return parts.length > 1 ? parts[parts.length - 2] : "";
  });
</script>

<span class="name" class:none={!app.project} title={app.project?.path ?? ""}>
  {#if app.project}
    <b>{app.project.name}</b>{#if folder}<span class="folder">·&nbsp;{folder}</span>{/if}
  {:else}
    {t("project.none")}
  {/if}
</span>

<style>
  .name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  b {
    color: var(--vf-text);
    font-weight: 500;
  }
  .folder {
    margin-left: var(--vf-space-1);
  }
</style>
