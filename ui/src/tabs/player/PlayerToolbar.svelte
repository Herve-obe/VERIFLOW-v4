<!-- Barre de l'onglet PLAYER : boutons à bascule des panneaux et réinitialisation de la disposition.
     Props : mode ("video" | "audio"). Lit et modifie playerLayout[mode].visible. -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { PANELS, playerLayout, resetLayout } from "../../stores/playerLayout.svelte";
  import { t } from "../../i18n/index.svelte";

  let { mode }: { mode: Mode } = $props();
  const visible = $derived(playerLayout[mode].visible);
</script>

<div class="bar" role="toolbar" aria-label={t("player.panel.title")}>
  {#each PANELS as p (p)}
    <button
      type="button"
      class:on={visible[p]}
      aria-pressed={visible[p]}
      onclick={(e) => {
        playerLayout[mode].visible[p] = !visible[p];
        e.currentTarget.blur();
      }}
    >
      {t(`player.panel.${p}`)}
    </button>
  {/each}
  <span class="spacer"></span>
  <button type="button" class="reset" onclick={(e) => { resetLayout(mode); e.currentTarget.blur(); }}>
    {t("player.panel.reset")}
  </button>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    padding: var(--vf-space-1) var(--vf-space-2);
    background: var(--vf-surface);
    border-bottom: 1px solid var(--vf-border);
    font-size: var(--vf-text-sm);
  }
  .spacer {
    flex: 1;
  }
  .bar button {
    padding: 2px var(--vf-space-2);
    background: transparent;
    color: var(--vf-text-muted);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-sm);
  }
  .bar button:hover {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  /* Plus précis que « .bar button » : l'état actif l'emporte (accent plein, texte sur accent). */
  .bar button.on {
    background: var(--vf-accent);
    border-color: var(--vf-accent);
    color: var(--vf-on-accent);
    font-weight: 600;
  }
  .bar button.reset {
    border-style: dashed;
  }
</style>
