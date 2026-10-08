<!-- Boutons de transport : début, retour, lecture/pause, avance, stop. -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";

  let {
    playing = false,
    speed = 0,
    onToggle,
    onStop,
    onBack,
    onForward,
  }: {
    playing?: boolean;
    speed?: number;
    onToggle: () => void;
    onStop: () => void;
    onBack?: () => void;
    onForward?: () => void;
  } = $props();
</script>

<div class="transport">
  <button onclick={onStop} title={t("player.stop.key")} aria-label={t("player.stop")}>
    <svg viewBox="0 0 16 16"><rect x="3" y="3" width="10" height="10" fill="currentColor" /></svg>
  </button>
  {#if onBack}
    <button onclick={onBack} title={t("player.back.key")} aria-label={t("player.back")}>
      <svg viewBox="0 0 16 16"><path d="M8 3 L1 8 L8 13 Z M15 3 L8 8 L15 13 Z" fill="currentColor" /></svg>
    </button>
  {/if}
  <button class="main" onclick={onToggle} title={t("player.play.key")} aria-label={t("player.play")}>
    {#if playing}
      <svg viewBox="0 0 16 16"><rect x="3" y="2" width="3.5" height="12" fill="currentColor" /><rect x="9.5" y="2" width="3.5" height="12" fill="currentColor" /></svg>
    {:else}
      <svg viewBox="0 0 16 16"><path d="M4 2 L14 8 L4 14 Z" fill="currentColor" /></svg>
    {/if}
  </button>
  {#if onForward}
    <button onclick={onForward} title={t("player.forward.key")} aria-label={t("player.forward")}>
      <svg viewBox="0 0 16 16"><path d="M1 3 L8 8 L1 13 Z M8 3 L15 8 L8 13 Z" fill="currentColor" /></svg>
    </button>
  {/if}
  {#if speed !== 0 && speed !== 1}
    <span class="speed mono">{speed > 0 ? "" : "-"}{Math.abs(speed)}x</span>
  {/if}
</div>

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
  }
  /* Centrage compatible avec les WebKit anciens (Catalina), où un bouton en
     grille place l'icône à gauche : pas de marge intérieure, icône en bloc. */
  button {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    flex: none;
    width: 34px;
    height: 30px;
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    color: var(--vf-text);
    cursor: pointer;
  }
  button:hover {
    background: var(--vf-surface-hover);
  }
  button.main {
    width: 44px;
    color: var(--vf-on-accent);
    background: var(--vf-accent);
    border-color: var(--vf-accent);
  }
  svg {
    display: block;
    width: 14px;
    height: 14px;
    margin: 0 auto;
  }
  .speed {
    color: var(--vf-accent);
    font-size: var(--vf-text-sm);
    min-width: 32px;
  }
</style>
