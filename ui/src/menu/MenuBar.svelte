<!-- Barre de menus. Windows et Linux : dessinée ici, aux couleurs de VERIFLOW.
     macOS : menu natif en haut de l'écran (même contenu, voir commands.svelte.ts). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { MENUS, command, isMac, runCommand, setNativeMenu, shortcutLabel, startNativeMenu, type Entry } from "./commands.svelte";

  let open = $state<string | null>(null);
  let sub = $state<number | null>(null);
  let bar = $state<HTMLElement | null>(null);

  onMount(() => startNativeMenu());

  // macOS : le menu natif suit la langue, le projet ouvert, les récents...
  $effect(() => {
    if (!isMac) return;
    setNativeMenu().catch(() => {});
  });

  function choose(id: string) {
    open = null;
    sub = null;
    runCommand(id);
  }

  function onWindowPointer(e: PointerEvent) {
    if (open && bar && !bar.contains(e.target as Node)) open = null;
  }

  const enabled = (id: string) => command(id)?.enabled?.() ?? true;
</script>

<svelte:window onpointerdown={onWindowPointer} onkeydown={(e) => e.key === "Escape" && open && (open = null)} />

{#snippet entries(items: Entry[], depth: number)}
  {#each items as entry, i (typeof entry === "string" ? entry + i : "sub" + i)}
    {#if entry === "-"}
      <div class="sep" role="separator"></div>
    {:else if typeof entry !== "string"}
      <div class="item has-sub" role="menuitem" tabindex="-1" aria-haspopup="true" onpointerenter={() => (sub = i)}>
        <span class="label">{entry.submenu()}</span>
        <span class="arrow">›</span>
        {#if sub === i}
          <div class="dropdown nested" role="menu">
            {@render entries(entry.items(), depth + 1)}
          </div>
        {/if}
      </div>
    {:else}
      {@const c = command(entry)}
      {#if c}
        <button
          class="item"
          role="menuitem"
          disabled={!enabled(entry)}
          onpointerenter={() => depth === 0 && (sub = null)}
          onclick={() => choose(entry)}
        >
          <span class="label">{c.label()}</span>
          <span class="keys">{shortcutLabel(c.shortcut)}</span>
        </button>
      {/if}
    {/if}
  {/each}
{/snippet}

{#if !isMac}
  <!-- La barre ne prend pas le focus : couper, copier, coller agissent sur
       le champ de texte en cours. -->
  <nav class="menubar" bind:this={bar} onpointerdown={(e) => e.preventDefault()} aria-label="Menu">
    {#each MENUS as m (m.id)}
      <div class="menu">
        <button
          class="title"
          class:open={open === m.id}
          aria-haspopup="true"
          aria-expanded={open === m.id}
          onclick={() => {
            open = open === m.id ? null : m.id;
            sub = null;
          }}
          onpointerenter={() => {
            if (open && open !== m.id) {
              open = m.id;
              sub = null;
            }
          }}
        >
          {m.label()}
        </button>
        {#if open === m.id}
          <div class="dropdown" role="menu">
            {@render entries(m.items(), 0)}
          </div>
        {/if}
      </div>
    {/each}
  </nav>
{/if}

<style>
  .menubar {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .menu {
    position: relative;
  }
  .title {
    background: none;
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    color: var(--vf-text-muted);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  @media (max-width: 1150px) {
    .title {
      padding: var(--vf-space-1) 6px;
    }
  }
  .title:hover,
  .title.open {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  .dropdown {
    position: absolute;
    top: calc(100% + 4px);
    left: 0;
    z-index: 60;
    min-width: 260px;
    max-width: 460px;
    padding: var(--vf-space-1) 0;
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    box-shadow: 0 8px 24px var(--vf-shadow);
  }
  .dropdown.nested {
    top: calc(-1 * var(--vf-space-1) - 1px);
    left: 100%;
  }
  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--vf-space-6);
    width: 100%;
    padding: 5px var(--vf-space-3);
    background: none;
    border: 0;
    color: var(--vf-text);
    font-size: var(--vf-text-sm);
    text-align: left;
    cursor: pointer;
  }
  .item:hover:not(:disabled) {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
  }
  .item:hover:not(:disabled) .keys {
    color: var(--vf-on-accent);
  }
  .has-sub:hover {
    background: var(--vf-surface-hover);
  }
  .item:disabled .keys {
    color: var(--vf-text-disabled);
  }
  .item:disabled {
    color: var(--vf-text-disabled);
    cursor: default;
  }
  .label {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .keys {
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
    white-space: nowrap;
  }
  .arrow {
    color: var(--vf-text-muted);
  }
  .sep {
    height: 1px;
    margin: var(--vf-space-1) 0;
    background: var(--vf-border);
  }
</style>
