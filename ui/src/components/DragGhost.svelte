<!-- Étiquette qui suit le pointeur pendant le glisser d'un dossier. -->
<script lang="ts">
  import { drag } from "../stores/drag.svelte";

  const name = $derived(drag.path?.split(/[\\/]/).filter(Boolean).pop() ?? drag.path ?? "");
</script>

{#if drag.active}
  <div class="ghost" class:ok={drag.over !== null} style:left={`${drag.x + 12}px`} style:top={`${drag.y + 8}px`}>
    {name}
  </div>
{/if}

<style>
  .ghost {
    position: fixed;
    z-index: 1000;
    pointer-events: none;
    padding: 2px var(--vf-space-2);
    font-size: var(--vf-text-sm);
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    color: var(--vf-text-muted);
    white-space: nowrap;
  }
  .ghost.ok {
    border-color: var(--vf-accent);
    color: var(--vf-text);
  }
</style>
