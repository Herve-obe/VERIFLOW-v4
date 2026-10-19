<!-- Boutons réduire, agrandir/restaurer et fermer de la barre de titre
     dessinée par VERIFLOW (Windows). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { t } from "../i18n/index.svelte";

  const win = getCurrentWindow();
  let maximized = $state(false);

  onMount(() => {
    const refresh = () => win.isMaximized().then((m) => (maximized = m));
    refresh();
    const stop = win.onResized(refresh);
    return () => void stop.then((u) => u());
  });
</script>

<div class="controls">
  <button onclick={() => win.minimize()} title={t("window.minimize")} aria-label={t("window.minimize")}>
    <svg viewBox="0 0 10 10"><path d="M0 5h10" /></svg>
  </button>
  <button
    onclick={() => win.toggleMaximize()}
    title={maximized ? t("window.restore") : t("window.maximize")}
    aria-label={maximized ? t("window.restore") : t("window.maximize")}
  >
    {#if maximized}
      <svg viewBox="0 0 10 10"><path d="M2.5 2.5V.5h7v7h-2M.5 2.5h7v7h-7z" /></svg>
    {:else}
      <svg viewBox="0 0 10 10"><path d="M.5 .5h9v9h-9z" /></svg>
    {/if}
  </button>
  <button class="close" onclick={() => win.close()} title={t("window.close")} aria-label={t("window.close")}>
    <svg viewBox="0 0 10 10"><path d="M0 0l10 10M10 0L0 10" /></svg>
  </button>
</div>

<style>
  .controls {
    display: flex;
    height: 100%;
  }
  button {
    width: 46px;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 0;
    color: var(--vf-text-muted);
    cursor: default;
  }
  button:hover {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  button.close:hover {
    background: var(--vf-window-close-hover);
    color: var(--vf-text);
  }
  svg {
    width: 10px;
    height: 10px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1;
  }
</style>
