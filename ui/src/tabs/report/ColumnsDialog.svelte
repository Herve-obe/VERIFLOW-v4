<!-- Choix et ordre des colonnes d'un rapport : cases à cocher (les colonnes
     des rapports papier restent toujours affichées), glisser-déposer ou
     flèches pour l'ordre. -->
<script lang="ts">
  import { untrack } from "svelte";
  import Modal from "../../components/Modal.svelte";
  import { t } from "../../i18n/index.svelte";
  import type { ColumnDef } from "../../lib/report";

  let {
    catalog,
    keys,
    lang,
    onApply,
    onClose,
  }: {
    catalog: ColumnDef[];
    keys: string[];
    lang: "fr" | "en";
    onApply: (keys: string[]) => void;
    onClose: () => void;
  } = $props();

  interface Item {
    def: ColumnDef;
    on: boolean;
  }

  function build(order: string[]): Item[] {
    const shown = order.map((k) => catalog.find((c) => c.key === k)).filter((c): c is ColumnDef => !!c);
    const hidden = catalog.filter((c) => !order.includes(c.key));
    return [...shown.map((def) => ({ def, on: true })), ...hidden.map((def) => ({ def, on: def.base }))];
  }

  // Copie de travail : la liste n'est appliquée qu'au bouton « Appliquer ».
  let items = $state<Item[]>(untrack(() => build(keys)));
  let dragging = $state<number | null>(null);

  const label = (c: ColumnDef) => (lang === "fr" ? c.label_fr : c.label_en);

  function move(from: number, to: number) {
    if (to < 0 || to >= items.length || from === to) return;
    const [it] = items.splice(from, 1);
    items.splice(to, 0, it);
  }

  // Glisser-déposer à la souris (événements pointeur : fiables sur tous les
  // moteurs web, y compris celui de macOS Catalina).
  function startDrag(e: PointerEvent, i: number) {
    e.preventDefault();
    dragging = i;
    const over = (m: PointerEvent) => {
      const row = (document.elementFromPoint(m.clientX, m.clientY) as HTMLElement | null)?.closest<HTMLElement>("[data-i]");
      if (!row || dragging === null) return;
      const j = Number(row.dataset.i);
      if (j !== dragging) {
        move(dragging, j);
        dragging = j;
      }
    };
    const end = () => {
      dragging = null;
      window.removeEventListener("pointermove", over);
      window.removeEventListener("pointerup", end);
      document.body.classList.remove("vf-moving");
    };
    document.body.classList.add("vf-moving");
    window.addEventListener("pointermove", over);
    window.addEventListener("pointerup", end);
  }

  function reset() {
    items = build(catalog.filter((c) => c.base).map((c) => c.key));
  }

  function apply() {
    onApply(items.filter((i) => i.on).map((i) => i.def.key));
  }
</script>

<Modal title={t("report.columns.title")} {onClose} width="min(440px, 94vw)">
  <div class="body">
    <p class="hint">{t("report.columns.hint")}</p>
    <ul>
      {#each items as it, i (it.def.key)}
        <li data-i={i} class:drag={dragging === i} class:off={!it.on}>
          <button class="handle" onpointerdown={(e) => startDrag(e, i)} aria-label={t("report.columns.drag")} title={t("report.columns.drag")}>
            <svg viewBox="0 0 12 12"><path d="M2 3.5 H10 M2 6 H10 M2 8.5 H10" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" /></svg>
          </button>
          <label>
            <input type="checkbox" bind:checked={it.on} disabled={it.def.base} />
            <span>{label(it.def)}</span>
            {#if it.def.base}<span class="base">{t("report.columns.base")}</span>{/if}
          </label>
          <button class="icon" onclick={() => move(i, i - 1)} disabled={i === 0} aria-label={t("report.row.up")}>
            <svg viewBox="0 0 12 12"><path d="M3 7.5 L6 4.5 L9 7.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
          </button>
          <button class="icon" onclick={() => move(i, i + 1)} disabled={i === items.length - 1} aria-label={t("report.row.down")}>
            <svg viewBox="0 0 12 12"><path d="M3 4.5 L6 7.5 L9 4.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
          </button>
        </li>
      {/each}
    </ul>
    <div class="buttons">
      <button class="ghost" onclick={reset}>{t("report.columns.reset")}</button>
      <span class="spacer"></span>
      <button class="ghost" onclick={onClose}>{t("logs.cancel")}</button>
      <button class="primary" onclick={apply}>{t("report.columns.apply")}</button>
    </div>
  </div>
</Modal>

<style>
  .body {
    padding: var(--vf-space-3);
    font-size: var(--vf-text-sm);
  }
  .hint {
    margin: 0 0 var(--vf-space-2);
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
    line-height: 1.5;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 56vh;
    overflow-y: auto;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
  }
  li {
    display: flex;
    align-items: center;
    padding: 2px var(--vf-space-2);
    border-bottom: 1px solid var(--vf-border);
    background: var(--vf-bg);
  }
  li:last-child {
    border-bottom: 0;
  }
  li.drag {
    background: var(--vf-selection);
  }
  li.off span {
    color: var(--vf-text-muted);
  }
  label {
    flex: 1;
    display: flex;
    align-items: center;
    min-width: 0;
    padding: var(--vf-space-1) 0;
  }
  label input {
    margin: 0 var(--vf-space-2) 0 0;
    accent-color: var(--vf-accent);
  }
  .base {
    margin-left: var(--vf-space-2);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  button {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.35;
    cursor: default;
  }
  .handle,
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    background: none;
    border-color: transparent;
    color: var(--vf-text-muted);
  }
  .handle {
    cursor: grab;
    margin-right: var(--vf-space-1);
    touch-action: none;
  }
  .handle svg,
  .icon svg {
    width: 12px;
    height: 12px;
  }
  .primary {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: transparent;
    font-weight: 600;
  }
  .ghost {
    background: none;
  }
  .buttons {
    display: flex;
    align-items: center;
    margin-top: var(--vf-space-3);
  }
  .buttons > * + * {
    margin-left: var(--vf-space-2);
  }
  .spacer {
    flex: 1;
  }
</style>
