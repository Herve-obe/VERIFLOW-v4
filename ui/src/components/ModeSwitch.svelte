<!-- Interrupteur glissant VIDEO / AUDIO (charte §3). -->
<script lang="ts">
  import { app, setMode } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";
</script>

<div class="switch" role="radiogroup" aria-label={t("mode.toggle.hint")} title={t("mode.toggle.hint")}>
  <span class="thumb" class:audio={app.mode === "audio"}></span>
  <button role="radio" aria-checked={app.mode === "video"} class:active={app.mode === "video"} onclick={() => setMode("video")}>
    {t("mode.video")}
  </button>
  <button role="radio" aria-checked={app.mode === "audio"} class:active={app.mode === "audio"} onclick={() => setMode("audio")}>
    {t("mode.audio")}
  </button>
</div>

<style>
  .switch {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 1fr;
    background: var(--vf-bg);
    border: 1px solid var(--vf-border);
    border-radius: 999px;
    padding: 2px;
    width: 164px;
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 2px;
    width: calc(50% - 2px);
    border-radius: 999px;
    background: var(--vf-accent);
    transition: transform var(--vf-transition), background var(--vf-transition);
  }
  .thumb.audio {
    transform: translateX(100%);
  }
  button {
    position: relative;
    z-index: 1;
    border: 0;
    background: transparent;
    padding: var(--vf-space-1) 0;
    font-size: var(--vf-text-sm);
    font-weight: 600;
    letter-spacing: 0.08em;
    color: var(--vf-text-muted);
    cursor: pointer;
    border-radius: 999px;
  }
  button.active {
    color: var(--vf-on-accent);
  }
</style>
