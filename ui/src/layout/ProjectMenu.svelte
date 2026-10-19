<!-- Projet ouvert (nom, chemin en info-bulle). Création, ouverture et
     enregistrement : menu Fichier (charte §5.3). -->
<script lang="ts">
  import { app } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";
</script>

<span class="name" class:none={!app.project} title={app.project?.path ?? ""}>
  {app.project?.name ?? t("project.none")}
</span>

<style>
  .name {
    min-width: 0;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--vf-text);
    font-size: var(--vf-text-sm);
  }
  .name.none {
    color: var(--vf-text-muted);
  }
</style>
