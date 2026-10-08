<!-- Explorateur de dossiers (OFFLOAD, MEDIA) : volumes, favoris et dossiers du projet,
     mis à jour en direct. Panneau redimensionnable et repliable (Ctrl+B). -->
<script lang="ts">
  import { onMount } from "svelte";
  import TreeNode from "./TreeNode.svelte";
  import { explorer, startExplorer, toggleFavorite, toggleExpanded, originOf } from "../../stores/explorer.svelte";
  import { app } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes } from "../../lib/format";
  import { norm, projectRoots, watch } from "../../lib/explorer";

  export interface ExplorerAction {
    label: string;
    run: (path: string) => void;
  }

  let {
    owner,
    selected = null,
    onSelect,
    actions = [],
  }: {
    owner: string;
    selected?: string | null;
    onSelect: (path: string) => void;
    actions?: ExplorerAction[];
  } = $props();

  const WIDTH_KEY = "veriflow.explorer.width";
  const COLLAPSED_KEY = "veriflow.explorer.collapsed";
  const read = (k: string, d: string) => {
    try {
      return localStorage.getItem(k) ?? d;
    } catch {
      return d;
    }
  };
  let width = $state(Number(read(WIDTH_KEY, "250")));
  let collapsed = $state(read(COLLAPSED_KEY, "false") === "true");
  // État déplié partagé entre les onglets (voir stores/explorer.svelte.ts).
  const expanded = $derived(explorer.expanded);
  let roots = $state<string[]>([]);
  let menu = $state<{ x: number; y: number; path: string } | null>(null);

  $effect(() => {
    try {
      localStorage.setItem(WIDTH_KEY, String(width));
      localStorage.setItem(COLLAPSED_KEY, String(collapsed));
    } catch {
      /* stockage indisponible */
    }
  });

  const toggle = (path: string) => toggleExpanded(path);

  // Seuls les dossiers dépliés sont surveillés (léger, même sur un gros RAID).
  $effect(() => {
    const paths = [...expanded].map(originOf);
    watch(owner, paths).catch(() => {});
  });

  // Dossiers du projet : relus à l'ouverture d'un projet et après chaque changement.
  $effect(() => {
    void app.project;
    void explorer.revision;
    projectRoots()
      .then((r) => (roots = r))
      .catch(() => (roots = []));
  });

  function resize(e: PointerEvent) {
    const start = e.clientX;
    const w0 = width;
    const move = (ev: PointerEvent) => (width = Math.min(520, Math.max(160, w0 + ev.clientX - start)));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  function onKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "b" && app.tab === owner.split("-")[0]) {
      e.preventDefault();
      collapsed = !collapsed;
    }
  }

  const entry = (path: string) => ({ path, name: path.split(/[\\/]/).filter(Boolean).pop() ?? path, has_children: true });
  const openMenu = (e: MouseEvent, path: string) => (menu = { x: e.clientX, y: e.clientY, path });

  onMount(startExplorer);
</script>

<svelte:window onkeydown={onKeydown} onclick={() => (menu = null)} />

{#if collapsed}
  <button class="reopen" onclick={() => (collapsed = false)} title={t("explorer.show")} aria-label={t("explorer.show")}>
    <svg viewBox="0 0 10 10"><path d="M3 1.5 L7 5 L3 8.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
  </button>
{:else}
  <aside class="explorer" style:width={`${width}px`} aria-label={t("explorer.title")}>
    <header>
      <span>{t("explorer.title")}</span>
      <button onclick={() => (collapsed = true)} title={t("explorer.hide")} aria-label={t("explorer.hide")}>
        <svg viewBox="0 0 10 10"><path d="M7 1.5 L3 5 L7 8.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
      </button>
    </header>
    <div class="scroll" role="tree">
      <h4>{t("explorer.volumes")}</h4>
      {#each explorer.volumes as v (v.mount_point)}
        <TreeNode
          entry={entry(v.mount_point)}
          label={v.name}
          detail={`${bytes(v.available)}${v.kind === "hdd" ? " · HDD" : ""}${v.removable ? " ⏏" : ""}`}
          {expanded}
          {selected}
          {toggle}
          {onSelect}
          onActivate={toggle}
          onContext={openMenu}
        />
      {:else}
        <p class="muted">{t("explorer.no.volume")}</p>
      {/each}

      <h4>{t("explorer.favorites")}</h4>
      {#each explorer.favorites as f (f)}
        <TreeNode entry={entry(f)} {expanded} {selected} {toggle} {onSelect} onActivate={toggle} onContext={openMenu} />
      {:else}
        <p class="muted">{t("explorer.no.favorite")}</p>
      {/each}

      {#if roots.length > 0}
        <h4>{t("explorer.project")}</h4>
        {#each roots as r (r)}
          <TreeNode entry={entry(r)} {expanded} {selected} {toggle} {onSelect} onActivate={toggle} onContext={openMenu} />
        {/each}
      {/if}
    </div>
    <div class="handle" role="separator" aria-orientation="vertical" onpointerdown={resize}></div>
  </aside>
{/if}

{#if menu}
  <div class="menu" style:left={`${menu.x}px`} style:top={`${menu.y}px`} role="menu">
    {#each actions as a (a.label)}
      <button role="menuitem" onclick={() => menu && a.run(menu.path)}>{a.label}</button>
    {/each}
    <button role="menuitem" onclick={() => menu && toggleFavorite(menu.path)}>
      {explorer.favorites.includes(menu.path) ? t("explorer.unpin") : t("explorer.pin")}
    </button>
  </div>
{/if}

<style>
  .explorer {
    position: relative;
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    background: var(--vf-surface);
    border-right: 1px solid var(--vf-border);
    flex: none;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--vf-space-2) var(--vf-space-3);
    font-size: var(--vf-text-xs);
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--vf-text-muted);
  }
  header button,
  .reopen {
    background: none;
    border: 0;
    color: var(--vf-text-muted);
    cursor: pointer;
    padding: 2px;
  }
  header svg,
  .reopen svg {
    width: 12px;
    height: 12px;
  }
  .reopen {
    width: 18px;
    height: 100%;
    background: var(--vf-surface);
    border-right: 1px solid var(--vf-border);
  }
  .scroll {
    overflow: auto;
    flex: 1;
    padding: 0 var(--vf-space-1) var(--vf-space-3);
  }
  h4 {
    margin: var(--vf-space-3) var(--vf-space-2) var(--vf-space-1);
    font-size: var(--vf-text-xs);
    color: var(--vf-accent);
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .muted {
    margin: 0 var(--vf-space-3);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-disabled);
  }
  .handle {
    position: absolute;
    top: 0;
    right: -3px;
    width: 6px;
    height: 100%;
    cursor: col-resize;
  }
  .menu {
    position: fixed;
    z-index: 20;
    display: flex;
    flex-direction: column;
    min-width: 200px;
    padding: var(--vf-space-1);
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    box-shadow: 0 6px 24px rgb(0 0 0 / 0.4);
  }
  .menu button {
    background: none;
    border: 0;
    color: var(--vf-text);
    text-align: left;
    padding: var(--vf-space-1) var(--vf-space-2);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .menu button:hover {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
  }
</style>
