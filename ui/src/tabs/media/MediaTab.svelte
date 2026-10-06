<!-- Onglet MEDIA : parcourir, prévisualiser, consulter et éditer les métadonnées (charte §7.2). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import Thumb from "./Thumb.svelte";
  import Wave from "./Wave.svelte";
  import Inspector from "./Inspector.svelte";
  import QuickPlayer from "./QuickPlayer.svelte";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import { explorer } from "../../stores/explorer.svelte";
  import { watch, norm } from "../../lib/explorer";
  import { app } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes } from "../../lib/format";
  import { isTyping } from "../../shortcuts";
  import { reveal } from "../../lib/offload";
  import { list, describe, fields, exportMeta, clock, type MediaEntry, type Described, type FieldDef, type ExportKind } from "../../lib/media";

  let dir = $state("");
  let recursive = $state(true);
  let entries = $state<MediaEntry[]>([]);
  let details = $state(new Map<string, Described>());
  let fieldDefs = $state<FieldDef[]>([]);
  let selected = $state<Set<string>>(new Set());
  let anchor = $state<number | null>(null);
  let view = $state<"list" | "grid">("list");
  let search = $state("");
  let circledOnly = $state(false);
  let loading = $state(false);
  let quick = $state<MediaEntry | null>(null);
  let status = $state("");

  const modeKinds = $derived(app.mode === "audio" ? ["audio"] : ["video", "image"]);
  const visible = $derived(
    entries.filter((e) => {
      if (!modeKinds.includes(e.kind)) return false;
      const v = details.get(e.path)?.values ?? {};
      if (circledOnly && v.circled !== "true") return false;
      if (!search) return true;
      const q = search.toLowerCase();
      return [e.rel, v.scene, v.take, v.comment, v.reel, v.keywords].some((s) => s?.toLowerCase().includes(q));
    }),
  );
  const selection = $derived(visible.filter((e) => selected.has(e.path)));

  async function choose() {
    const r = await open({ directory: true, multiple: false, title: t("media.open") });
    if (typeof r === "string") await load(r);
  }

  async function load(path: string) {
    dir = path;
    loading = true;
    selected = new Set();
    details = new Map();
    try {
      entries = await list(path, recursive);
      status = `${entries.length} ${t("media.count")}${entries.length >= 5000 ? ` (${t("media.truncated")})` : ""}`;
      await describeAll(entries.map((e) => e.path));
    } catch (e) {
      status = String(e);
    } finally {
      loading = false;
    }
  }

  // Mise à jour en direct : les médias qui arrivent (offload en cours) apparaissent,
  // ceux qui disparaissent sont retirés ; les descriptions déjà faites sont conservées.
  $effect(() => {
    watch("media-list", dir ? [dir] : [], recursive).catch(() => {});
  });

  let refreshTimer = 0;
  $effect(() => {
    void explorer.revision;
    const d = dir ? norm(dir) : "";
    if (!d || !explorer.lastChanged.some((c) => c === d || c.startsWith(`${d}/`))) return;
    window.clearTimeout(refreshTimer);
    refreshTimer = window.setTimeout(async () => {
      try {
        const fresh = await list(dir, recursive);
        const known = new Set(entries.map((e) => e.path));
        const added = fresh.filter((e) => !known.has(e.path) || details.get(e.path)?.details === undefined);
        entries = fresh;
        status = `${entries.length} ${t("media.count")}`;
        if (added.length > 0) await describeAll(added.map((e) => e.path));
      } catch {
        /* dossier retiré : la liste reste en l'état */
      }
    }, 800);
  });

  // Descriptions par lots : la liste s'affiche tout de suite, les colonnes se remplissent ensuite.
  async function describeAll(paths: string[]) {
    for (let i = 0; i < paths.length; i += 24) {
      const batch = await describe(paths.slice(i, i + 24));
      const next = new Map(details);
      for (const d of batch) next.set(d.path, d);
      details = next;
    }
  }

  function click(e: MouseEvent, index: number, path: string) {
    const next = new Set(e.ctrlKey || e.metaKey ? selected : []);
    if (e.shiftKey && anchor !== null) {
      const [a, b] = [Math.min(anchor, index), Math.max(anchor, index)];
      for (const m of visible.slice(a, b + 1)) next.add(m.path);
    } else if ((e.ctrlKey || e.metaKey) && next.has(path)) {
      next.delete(path);
    } else {
      next.add(path);
      anchor = index;
    }
    selected = next;
  }

  function onKeydown(e: KeyboardEvent) {
    if (app.tab !== "media" || quick || isTyping(e.target)) return;
    if (e.key === " " && selection.length > 0) {
      e.preventDefault();
      quick = selection[0];
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "a") {
      e.preventDefault();
      selected = new Set(visible.map((m) => m.path));
    } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const cur = anchor ?? -1;
      const n = Math.min(visible.length - 1, Math.max(0, cur + (e.key === "ArrowDown" ? 1 : -1)));
      if (visible[n]) {
        selected = new Set([visible[n].path]);
        anchor = n;
      }
    }
  }

  async function runExport(kind: ExportKind) {
    const targets = (selection.length > 0 ? selection : visible).map((m) => m.path);
    if (targets.length === 0) return;
    let out: string | null = null;
    if (kind === "csv" || kind === "ale") {
      out = await save({ defaultPath: `metadonnees.${kind}`, filters: [{ name: kind.toUpperCase(), extensions: [kind] }] });
    } else {
      const r = await open({ directory: true, multiple: false, title: t(`media.export.${kind}`) });
      out = typeof r === "string" ? r : null;
    }
    if (!out) return;
    try {
      const res = await exportMeta(kind, targets, dir, out);
      status = `${res.written.length} ${t("media.export.done")}${res.errors.length ? `, ${res.errors.length} ${t("media.export.errors")} : ${res.errors[0]}` : ""}`;
      if (res.written[0]) reveal(kind === "csv" || kind === "ale" ? res.written[0] : out).catch(() => {});
    } catch (e) {
      status = String(e);
    }
  }

  async function refresh(paths: string[]) {
    const batch = await describe(paths);
    const next = new Map(details);
    for (const d of batch) next.set(d.path, d);
    details = next;
  }

  const fmt = (e: MediaEntry) => {
    const d = details.get(e.path)?.details;
    const v = d?.probe?.video;
    if (v) return `${v.width}x${v.height} · ${(v.rate.num / v.rate.den).toFixed(2)}`;
    const w = d?.wav;
    if (w) return `${w.sample_rate / 1000} kHz · ${w.bits}${w.format === "Float" ? "f" : ""} · ${w.channels} ch`;
    const a = d?.probe?.audio[0];
    return a ? `${a.sample_rate / 1000} kHz · ${a.channels} ch` : "";
  };
  const codec = (e: MediaEntry) => {
    const p = details.get(e.path)?.details.probe;
    return p?.video?.codec ?? p?.audio[0]?.codec ?? (details.get(e.path)?.details.wav ? "pcm" : "");
  };
  const duration = (e: MediaEntry) => {
    const d = details.get(e.path)?.details;
    return clock(d?.probe?.duration ?? (d?.wav ? d.wav.frames / d.wav.sample_rate : 0));
  };
  const tc = (e: MediaEntry) => details.get(e.path)?.details.probe?.start_timecode ?? "";
  const val = (e: MediaEntry, k: string) => details.get(e.path)?.values[k] ?? "";

  onMount(async () => {
    try {
      fieldDefs = await fields();
    } catch {
      fieldDefs = [];
    }
  });
</script>

<svelte:window onkeydown={onKeydown} />

<div class="layout">
<Explorer
  owner="media"
  selected={dir || null}
  onSelect={(p) => load(p)}
  actions={[
    { label: t("explorer.open.media"), run: (p) => load(p) },
    { label: t("explorer.reveal"), run: (p) => reveal(p).catch(() => {}) },
  ]}
/>
<div class="media">
  <div class="toolbar">
    <button onclick={choose}>{t("media.open")}</button>
    <span class="path mono" title={dir}>{dir || t("media.none")}</span>
    <label class="check"><input type="checkbox" bind:checked={recursive} onchange={() => dir && load(dir)} /> {t("media.recursive")}</label>
    <input class="search" placeholder={t("media.search")} bind:value={search} />
    <label class="check"><input type="checkbox" bind:checked={circledOnly} /> {t("media.circled.only")}</label>
    <div class="seg">
      <button class:on={view === "list"} onclick={() => (view = "list")}>{t("media.view.list")}</button>
      <button class:on={view === "grid"} onclick={() => (view = "grid")}>{t("media.view.grid")}</button>
    </div>
    <div class="exports">
      <span class="small">{t("media.export")}</span>
      <button onclick={() => runExport("csv")}>CSV</button>
      <button onclick={() => runExport("ale")}>ALE</button>
      <button onclick={() => runExport("xmp")}>XMP</button>
      <button onclick={() => runExport("working_copy")} title={t("media.export.working_copy.hint")}>{t("media.export.copy")}</button>
    </div>
  </div>

  <div class="body">
    <div class="items">
      {#if loading && entries.length === 0}
        <p class="muted">{t("player.loading")}</p>
      {:else if !dir}
        <div class="empty">
          <p>{t("media.howto")}</p>
          <p class="small">{t("media.shortcuts")}</p>
        </div>
      {:else if view === "list"}
        <table>
          <thead>
            <tr>
              <th class="pv"></th><th>{t("offload.file")}</th><th>{t("media.col.duration")}</th><th>{t("player.codec")}</th>
              <th>{t("media.col.format")}</th><th>TC</th><th>{t("field.scene")}</th><th>{t("field.take")}</th>
              <th title={t("field.circled")}>○</th><th>{t("offload.size")}</th>
            </tr>
          </thead>
          <tbody>
            {#each visible as e, i (e.path)}
              <tr class:sel={selected.has(e.path)} onclick={(ev) => click(ev, i, e.path)} ondblclick={() => (quick = e)}>
                <td class="pv">
                  {#if e.kind === "audio"}<Wave path={e.path} width={80} height={28} />{:else}<Thumb path={e.path} width={64} />{/if}
                </td>
                <td class="name" title={e.rel}>{e.rel}{#if details.get(e.path)?.edited.length}<span class="dot" title={t("media.edited")}></span>{/if}</td>
                <td class="mono">{duration(e)}</td>
                <td class="mono">{codec(e)}</td>
                <td class="mono">{fmt(e)}</td>
                <td class="mono">{tc(e)}</td>
                <td>{val(e, "scene")}</td>
                <td>{val(e, "take")}</td>
                <td class="circ">{val(e, "circled") === "true" ? "●" : ""}</td>
                <td class="mono num">{bytes(e.size)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      {:else}
        <div class="grid">
          {#each visible as e, i (e.path)}
            <button class="card" class:sel={selected.has(e.path)} onclick={(ev) => click(ev, i, e.path)} ondblclick={() => (quick = e)}>
              {#if e.kind === "audio"}<Wave path={e.path} width={200} height={60} />{:else}<Thumb path={e.path} width={200} />{/if}
              <span class="cname" title={e.rel}>{e.name}</span>
              <span class="small mono">{duration(e)} {val(e, "scene") ? `· Sc ${val(e, "scene")}` : ""} {val(e, "take") ? `· T ${val(e, "take")}` : ""}</span>
            </button>
          {/each}
        </div>
      {/if}
    </div>
    <Inspector {selection} {details} {fieldDefs} onSaved={refresh} />
  </div>
  <div class="statusline small">{status} {selection.length > 0 ? `· ${selection.length} ${t("media.selected")}` : ""}</div>
</div>
</div>

{#if quick}
  <QuickPlayer media={quick} onClose={() => (quick = null)} />
{/if}

<style>
  .layout {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .media {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100%;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: var(--vf-space-2) var(--vf-space-3);
    padding: var(--vf-space-2) var(--vf-space-3);
    border-bottom: 1px solid var(--vf-border);
    background: var(--vf-surface);
  }
  .path {
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
  }
  .search {
    width: 180px;
  }
  input {
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px var(--vf-space-2);
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
  .seg {
    display: flex;
  }
  .seg button.on {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: var(--vf-accent);
  }
  .exports {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    margin-left: auto;
  }
  .check {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    font-size: var(--vf-text-sm);
  }
  .check input {
    accent-color: var(--vf-accent);
  }
  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 340px;
    min-height: 0;
  }
  .items {
    overflow: auto;
    min-height: 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--vf-text-sm);
  }
  th {
    position: sticky;
    top: 0;
    background: var(--vf-surface);
    text-align: left;
    font-weight: 600;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
    padding: var(--vf-space-1) var(--vf-space-2);
    border-bottom: 1px solid var(--vf-border);
    z-index: 1;
  }
  td {
    padding: 3px var(--vf-space-2);
    border-bottom: 1px solid var(--vf-border);
    white-space: nowrap;
  }
  tr {
    cursor: default;
  }
  tr:hover {
    background: var(--vf-surface);
  }
  tr.sel,
  .card.sel {
    background: color-mix(in srgb, var(--vf-accent) 22%, transparent);
  }
  .pv {
    width: 84px;
  }
  .name {
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dot {
    display: inline-block;
    width: 6px;
    height: 6px;
    margin-left: var(--vf-space-1);
    border-radius: 50%;
    background: var(--vf-accent);
    vertical-align: middle;
  }
  .circ {
    color: var(--vf-success);
  }
  .num {
    text-align: right;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(214px, 1fr));
    gap: var(--vf-space-3);
    padding: var(--vf-space-3);
  }
  .card {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--vf-space-1);
    padding: var(--vf-space-2);
    text-align: left;
  }
  .cname {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--vf-text-sm);
  }
  .empty {
    padding: var(--vf-space-8);
    text-align: center;
    color: var(--vf-text-muted);
  }
  .muted {
    color: var(--vf-text-muted);
    padding: var(--vf-space-4);
  }
  .small {
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .statusline {
    padding: 2px var(--vf-space-3);
    border-top: 1px solid var(--vf-border);
  }
</style>
