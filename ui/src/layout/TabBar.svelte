<!-- Barre des six onglets. -->
<script lang="ts">
  import { app, TABS } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";
</script>

<div class="tabs" role="tablist">
  {#each TABS as tab, i (tab)}
    <button
      role="tab"
      aria-selected={app.tab === tab}
      class:active={app.tab === tab}
      title={`${t("tab." + tab)} (Alt+${i + 1})`}
      onclick={() => (app.tab = tab)}
    >
      {t("tab." + tab)}
    </button>
  {/each}
</div>

<style>
  .tabs {
    display: flex;
    height: 100%;
  }
  button {
    border: 0;
    background: transparent;
    padding: 0 var(--vf-space-4);
    font-size: var(--vf-text-sm);
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--vf-text-muted);
    border-bottom: 2px solid transparent;
    cursor: pointer;
    transition: color var(--vf-transition), border-color var(--vf-transition);
  }
  button:hover {
    color: var(--vf-text);
  }
  button.active {
    color: var(--vf-text);
    border-bottom-color: var(--vf-accent);
  }
</style>
