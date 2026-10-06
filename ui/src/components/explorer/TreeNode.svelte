<!-- Un dossier de l'arbre, avec ses sous-dossiers chargés au dépliage. -->
<script lang="ts">
  import TreeNode from "./TreeNode.svelte";
  import { explorer, loadChildren } from "../../stores/explorer.svelte";
  import { norm, type DirEntry } from "../../lib/explorer";
  import { beginDrag } from "../../stores/drag.svelte";

  let {
    entry,
    depth = 0,
    expanded,
    selected,
    toggle,
    onSelect,
    onActivate,
    onContext,
    label = null,
    detail = null,
  }: {
    entry: DirEntry;
    depth?: number;
    expanded: Set<string>;
    selected: string | null;
    toggle: (path: string) => void;
    onSelect: (path: string) => void;
    onActivate: (path: string) => void;
    onContext: (e: MouseEvent, path: string) => void;
    label?: string | null;
    detail?: string | null;
  } = $props();

  const key = $derived(norm(entry.path));
  const open = $derived(expanded.has(key));
  const kids = $derived(explorer.children[key]);

  $effect(() => {
    if (open && !kids) loadChildren(entry.path);
  });
</script>

<div
  class="row"
  class:sel={selected !== null && norm(selected) === key}
  style:padding-left={`${6 + depth * 14}px`}
  role="treeitem"
  aria-selected={selected !== null && norm(selected) === key}
  aria-expanded={entry.has_children ? open : undefined}
  tabindex="-1"
  title={entry.path}
  onpointerdown={(e) => beginDrag(e, entry.path)}
  onclick={() => onSelect(entry.path)}
  ondblclick={() => onActivate(entry.path)}
  oncontextmenu={(e) => {
    e.preventDefault();
    onContext(e, entry.path);
  }}
  onkeydown={(e) => e.key === "Enter" && onActivate(entry.path)}
>
  <button
    class="twisty"
    class:hidden={!entry.has_children}
    tabindex="-1"
    aria-label={open ? "Replier" : "Déplier"}
    onclick={(e) => {
      e.stopPropagation();
      toggle(entry.path);
    }}
  >
    <svg viewBox="0 0 10 10" class:open><path d="M3 1.5 L7 5 L3 8.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
  </button>
  <svg class="folder" viewBox="0 0 16 16"><path d="M1.5 3.5h5l1.5 1.5h6.5v8h-13z" fill="currentColor" /></svg>
  <span class="name">{label ?? entry.name}</span>
  {#if detail}<span class="detail">{detail}</span>{/if}
</div>

{#if open && kids}
  {#each kids as child (child.path)}
    <TreeNode entry={child} depth={depth + 1} {expanded} {selected} {toggle} {onSelect} {onActivate} {onContext} />
  {/each}
{/if}

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 4px;
    height: 24px;
    padding-right: var(--vf-space-2);
    font-size: var(--vf-text-sm);
    cursor: default;
    white-space: nowrap;
    user-select: none;
    border-radius: var(--vf-radius-sm);
  }
  .row:hover {
    background: var(--vf-surface-hover);
  }
  .row.sel {
    background: color-mix(in srgb, var(--vf-accent) 25%, transparent);
  }
  .twisty {
    display: grid;
    place-items: center;
    width: 14px;
    height: 14px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--vf-text-muted);
    cursor: pointer;
    flex: none;
  }
  .twisty.hidden {
    visibility: hidden;
  }
  .twisty svg {
    width: 10px;
    height: 10px;
    transition: transform var(--vf-transition);
  }
  .twisty svg.open {
    transform: rotate(90deg);
  }
  .folder {
    width: 14px;
    height: 14px;
    flex: none;
    color: var(--vf-accent);
    opacity: 0.8;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .detail {
    margin-left: auto;
    padding-left: var(--vf-space-2);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
</style>
