<!-- Interrupteur glissant DÉBUTANT / EXPERT. -->
<script lang="ts">
  import { level, setLevel } from "../stores/level.svelte";
  import { t } from "../i18n/index.svelte";
</script>

<div class="switch" role="radiogroup" aria-label={t("level.toggle.hint")} title={t("level.toggle.hint")}>
  <span class="thumb" class:expert={level.value === "expert"}></span>
  <button role="radio" aria-checked={level.value === "beginner"} class:active={level.value === "beginner"} onclick={() => setLevel("beginner")}>
    {t("level.beginner")}
  </button>
  <button role="radio" aria-checked={level.value === "expert"} class:active={level.value === "expert"} onclick={() => setLevel("expert")}>
    {t("level.expert")}
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
    width: 190px;
  }
  @media (max-width: 1150px) {
    .switch {
      width: 150px;
    }
    .switch button {
      letter-spacing: 0.02em;
    }
  }
  .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 2px;
    width: calc(50% - 2px);
    border-radius: 999px;
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    box-sizing: border-box;
    transition: transform var(--vf-transition);
  }
  .thumb.expert {
    transform: translateX(100%);
  }
  .switch button {
    position: relative;
    z-index: 1;
    border: 0;
    background: transparent;
    padding: var(--vf-space-1) 0;
    font-size: var(--vf-text-sm);
    font-weight: 600;
    letter-spacing: 0.06em;
    color: var(--vf-text-muted);
    cursor: pointer;
    border-radius: 999px;
  }
  .switch button:hover {
    color: var(--vf-text);
  }
  .switch button.active {
    color: var(--vf-text);
  }
</style>
