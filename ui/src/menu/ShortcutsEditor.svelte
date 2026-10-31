<!-- Préférences > Raccourcis : chaque action des menus et du lecteur, son
     raccourci, « Modifier » (appuyer sur la nouvelle combinaison), « Retirer »
     et « Par défaut ». Un raccourci déjà pris par une autre action lui est
     retiré, avec un message. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { EXTRA_ACTIONS, MENUS, command, shortcutLabel, type Entry } from "./commands.svelte";
  import { prefs, effectiveShortcut } from "../stores/prefs.svelte";
  import type { Shortcut } from "../shortcuts/defaults";
  import { t } from "../i18n/index.svelte";

  interface Row {
    id: string;
    label: () => string;
    current: () => Shortcut | undefined;
    fallback?: Shortcut;
  }

  const flat = (items: Entry[]): string[] =>
    items.flatMap((e) => (e === "-" ? [] : typeof e === "string" ? [e] : flat(e.items())));

  const groups = MENUS.map((m) => {
    const rows: Row[] = flat(m.items())
      .map((id) => command(id))
      .filter((c) => !!c?.prefId)
      .map((c) => ({ id: c!.prefId!, label: c!.label, current: () => command(c!.id)?.shortcut, fallback: c!.defaultShortcut }));
    if (m.id === "view") {
      for (const x of EXTRA_ACTIONS) {
        rows.push({ id: x.prefId, label: x.label, current: () => effectiveShortcut(x.prefId, x.fallback), fallback: x.fallback });
      }
    }
    return { label: m.label, rows };
  }).filter((g) => g.rows.length > 0);
  const allRows = groups.flatMap((g) => g.rows);

  const same = (a?: Shortcut, b?: Shortcut) =>
    !!a && !!b && a.key === b.key && !!a.ctrl === !!b.ctrl && !!a.shift === !!b.shift && !!a.alt === !!b.alt;

  let capturing = $state<string | null>(null);
  let message = $state("");

  function onCapture(e: KeyboardEvent) {
    if (!capturing) return;
    e.preventDefault();
    e.stopImmediatePropagation();
    if (["Control", "Shift", "Alt", "Meta", "AltGraph"].includes(e.key)) return;
    if (e.key === "Escape") {
      stop();
      return;
    }
    const key = e.key.length === 1 ? e.key.toLowerCase() : e.key;
    const s: Shortcut = { key };
    if (e.ctrlKey || e.metaKey) s.ctrl = true;
    if (e.shiftKey) s.shift = true;
    if (e.altKey) s.alt = true;
    const id = capturing;
    const taken = allRows.find((r) => r.id !== id && same(r.current(), s));
    if (taken) {
      prefs.shortcuts[taken.id] = null;
      message = `${shortcutLabel(s)} : ${t("prefs.sc.moved")} « ${taken.label()} »`;
    } else {
      message = "";
    }
    prefs.shortcuts[id] = s;
    stop();
  }

  function start(id: string) {
    capturing = id;
    message = t("prefs.sc.press");
    window.addEventListener("keydown", onCapture, true);
  }
  function stop() {
    capturing = null;
    window.removeEventListener("keydown", onCapture, true);
    if (message === t("prefs.sc.press")) message = "";
  }
  onDestroy(stop);

  const isDefault = (r: Row) => !(r.id in prefs.shortcuts);
  function reset(id: string) {
    delete prefs.shortcuts[id];
  }
  function resetAll() {
    for (const k of Object.keys(prefs.shortcuts)) delete prefs.shortcuts[k];
    message = "";
  }
</script>

<div class="editor">
  <div class="top">
    <p class="note">{t("prefs.sc.hint")}</p>
    <button class="ghost" onclick={resetAll} disabled={Object.keys(prefs.shortcuts).length === 0}>{t("prefs.sc.reset.all")}</button>
  </div>
  {#if message}<p class="message">{message}</p>{/if}
  {#each groups as g (g.label())}
    <h4>{g.label()}</h4>
    {#each g.rows as r (r.id)}
      <div class="row" class:active={capturing === r.id}>
        <span class="label">{r.label()}</span>
        <span class="keys" class:custom={!isDefault(r)}>
          {capturing === r.id ? "…" : r.current() ? shortcutLabel(r.current()) : t("prefs.sc.none")}
        </span>
        <button class="ghost" onclick={() => (capturing === r.id ? stop() : start(r.id))}>
          {capturing === r.id ? t("prefs.sc.cancel") : t("prefs.sc.edit")}
        </button>
        <button class="ghost" onclick={() => (prefs.shortcuts[r.id] = null)} disabled={!r.current()} title={t("prefs.sc.remove")}>×</button>
        <button class="ghost" onclick={() => reset(r.id)} disabled={isDefault(r)} title={r.fallback ? shortcutLabel(r.fallback) : ""}>
          {t("prefs.sc.default")}
        </button>
      </div>
    {/each}
  {/each}
</div>

<style>
  .top {
    display: flex;
    align-items: flex-start;
    gap: var(--vf-space-3);
  }
  .top .note {
    flex: 1;
    margin: 0;
  }
  .note {
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
  }
  .message {
    margin: var(--vf-space-2) 0 0;
    color: var(--vf-warning);
    font-size: var(--vf-text-xs);
  }
  h4 {
    margin: var(--vf-space-4) 0 var(--vf-space-1);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr 120px auto auto auto;
    align-items: center;
    gap: var(--vf-space-2);
    padding: 3px var(--vf-space-1);
    border-bottom: 1px solid var(--vf-border);
    font-size: var(--vf-text-sm);
  }
  .row.active {
    background: var(--vf-selection);
  }
  .label {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .keys {
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
    text-align: right;
  }
  .keys.custom {
    color: var(--vf-accent);
  }
  .ghost {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px var(--vf-space-2);
    font-size: var(--vf-text-xs);
    cursor: pointer;
  }
  .ghost:hover:not(:disabled) {
    background: var(--vf-surface-hover);
  }
  .ghost:disabled {
    color: var(--vf-text-disabled);
    cursor: default;
  }
</style>
