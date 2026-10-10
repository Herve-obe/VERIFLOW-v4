<!-- Onglet SYNC (charte §7.5) : appariement des vidéos et des sons par
     timecode, LTC ou forme d'onde ; validation clip par clip ; exports.
     Vue VIDEO : une ligne par plan. Vue AUDIO : une ligne par son. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open, save } from "@tauri-apps/plugin-dialog";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import ProgressBar from "../../components/ProgressBar.svelte";
  import PairDetail from "./PairDetail.svelte";
  import "../transcode/form.css";
  import { drag, dropZone, startNativeDrop } from "../../stores/drag.svelte";
  import { app } from "../../stores/app.svelte";
  import { sync } from "../../stores/sync.svelte";
  import { t } from "../../i18n/index.svelte";
  import { reveal } from "../../lib/offload";
  import {
    analyze,
    cancel,
    expand,
    fps,
    onExport,
    onProgress,
    rewrap,
    timecodeAt,
    timeline,
    type ExportItem,
    type ExportResult,
    type Options,
    type Pair,
    type RewrapOptions,
  } from "../../lib/sync";

  const KEY = "veriflow.sync.settings";
  function load() {
    const d = {
      options: { refine: true, waveform_search: true, drift: true, window: 2 } as Options,
      rewrap: { dest: null, format: "mov", keep_camera_audio: true, suffix: "_sync", existing: "rename" } as RewrapOptions,
      validatedOnly: false,
      nextToVideo: true,
    };
    try {
      const s = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      return { ...d, ...s, options: { ...d.options, ...(s.options ?? {}) }, rewrap: { ...d.rewrap, ...(s.rewrap ?? {}) } };
    } catch {
      return d;
    }
  }
  let cfg = $state(load());
  $effect(() => {
    const snapshot = JSON.stringify(cfg);
    try {
      localStorage.setItem(KEY, snapshot);
    } catch {
      /* stockage indisponible */
    }
  });

  let running = $state(false);
  let step = $state("");
  let fraction = $state(0);
  let error = $state("");
  let exporting = $state(false);
  let exportStep = $state("");
  let exportFraction = $state(0);
  let results = $state<ExportResult[]>([]);
  let selectedFolder = $state<string | null>(null);

  async function add(paths: string[]) {
    error = "";
    try {
      const s = await expand(paths);
      sync.videos = [...new Set([...sync.videos, ...s.videos])];
      sync.audios = [...new Set([...sync.audios, ...s.audios])];
      if (!s.videos.length && !s.audios.length) error = t("transcode.no.media");
    } catch (e) {
      error = String(e);
    }
  }

  async function pick(directory: boolean) {
    const r = await open({ multiple: !directory, directory, title: t(directory ? "transcode.add.folder" : "transcode.add.files") });
    if (Array.isArray(r)) add(r);
    else if (typeof r === "string") add([r]);
  }

  startNativeDrop();
  const zone = { name: "sync", accept: (paths: string[]) => add(paths) };
  const explorerActions = [
    { label: t("transcode.explorer.add"), run: (p: string) => add([p]) },
    { label: t("explorer.reveal"), run: (p: string) => reveal(p).catch(() => {}) },
  ];

  async function run() {
    running = true;
    error = "";
    step = "";
    fraction = 0;
    try {
      sync.analysis = await analyze([...sync.videos], [...sync.audios], $state.snapshot(cfg.options));
      sync.selected = sync.analysis.pairs.findIndex((p) => p.audio !== null);
    } catch (e) {
      error = String(e);
    } finally {
      running = false;
    }
  }

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const a = $derived(sync.analysis);

  const methodLabel = (p: Pair) => `${t(`sync.method.${p.method}`)}${p.refined && p.method !== "waveform" ? ` + ${t("sync.method.waveform").toLowerCase()}` : ""}`;
  const offsetText = (p: Pair) => {
    if (!a || p.audio === null) return "";
    const f = p.offset * fps(a.videos[p.video].rate);
    return `${p.offset >= 0 ? "+" : ""}${p.offset.toFixed(3).replace(".", ",")} s (${f >= 0 ? "+" : ""}${f.toFixed(1).replace(".", ",")} i)`;
  };
  const conf = (p: Pair) => (p.confidence === null ? "" : `${Math.round(p.confidence * 100)} %`);

  // Vue AUDIO : chaque son, avec les plans qui l'utilisent.
  const bySound = $derived.by(() => {
    if (!a) return [];
    return a.audios.map((s, ai) => ({ sound: s, index: ai, pairs: a.pairs.map((p, i) => ({ p, i })).filter(({ p }) => p.audio === ai) }));
  });

  const items = (): ExportItem[] => {
    if (!a) return [];
    return a.pairs
      .filter((p) => p.audio !== null && (!cfg.validatedOnly || p.validated))
      .map((p) => ({ video: $state.snapshot(a.videos[p.video]), audio: $state.snapshot(a.audios[p.audio as number]), offset: p.offset }));
  };
  const exportCount = $derived(a ? a.pairs.filter((p) => p.audio !== null && (!cfg.validatedOnly || p.validated)).length : 0);

  async function pickDest() {
    const r = await open({ directory: true, multiple: false, title: t("transcode.dest.pick") });
    if (typeof r === "string") {
      cfg.rewrap.dest = r;
      cfg.nextToVideo = false;
    }
  }

  async function doRewrap() {
    exporting = true;
    results = [];
    error = "";
    try {
      const opts = { ...$state.snapshot(cfg.rewrap), dest: cfg.nextToVideo ? null : cfg.rewrap.dest };
      results = await rewrap(items(), opts);
    } catch (e) {
      error = String(e);
    } finally {
      exporting = false;
      exportStep = "";
    }
  }

  async function doTimeline(kind: "fcpxml" | "otio") {
    const path = await save({ defaultPath: `VERIFLOW_SYNC.${kind}`, filters: [{ name: kind.toUpperCase(), extensions: [kind] }] });
    if (!path) return;
    try {
      await timeline(kind, app.project?.name ?? "VERIFLOW SYNC", items(), path);
      results = [{ video: "", output: path, error: null }];
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    const un = [
      onProgress((e) => {
        step = `${t(e.type === "reading" ? "sync.step.reading" : "sync.step.matching")} ${e.name}`;
        fraction = e.type === "reading" ? (e.index + 1) / Math.max(1, e.total) / 2 : 0.5 + (e.index + 1) / Math.max(1, e.total) / 2;
      }),
      onExport((e) => {
        exportStep = e.name;
        exportFraction = (e.index + e.fraction) / Math.max(1, e.total);
      }),
    ];
    return () => un.forEach((p) => p.then((f) => f()).catch(() => {}));
  });
</script>

<div class="layout">
  <Explorer owner="sync" selected={selectedFolder} onSelect={(p) => (selectedFolder = p)} actions={explorerActions} />
  <div class="sync">
    <h2>{t("sync.title")} <span class="mode">{t("mode." + app.mode)}</span></h2>

    <div class="top">
      <div class="block drop" class:over={drag.over === "sync"} role="region" aria-label={t("sync.files")} use:dropZone={zone}>
        <div class="row">
          <h3>{t("sync.videos")} ({sync.videos.length}) · {t("sync.sounds")} ({sync.audios.length})</h3>
          {#if sync.videos.length || sync.audios.length}
            <button class="tc-btn" onclick={() => ((sync.videos = []), (sync.audios = []), (sync.analysis = null))}>{t("transcode.clear")}</button>
          {/if}
        </div>
        <div class="lists">
          <ul>
            {#each sync.videos as v (v)}
              <li title={v}><span>{name(v)}</span><button class="x" onclick={() => (sync.videos = sync.videos.filter((x) => x !== v))} aria-label={t("transcode.remove")}>×</button></li>
            {:else}<li class="muted">{t("sync.videos.none")}</li>{/each}
          </ul>
          <ul>
            {#each sync.audios as s (s)}
              <li title={s}><span>{name(s)}</span><button class="x" onclick={() => (sync.audios = sync.audios.filter((x) => x !== s))} aria-label={t("transcode.remove")}>×</button></li>
            {:else}<li class="muted">{t("sync.sounds.none")}</li>{/each}
          </ul>
        </div>
        <div class="row left">
          <button class="tc-btn" onclick={() => pick(false)}>+ {t("transcode.add.files")}</button>
          <button class="tc-btn" onclick={() => pick(true)}>+ {t("transcode.add.folder")}</button>
          <span class="muted small">{t("sync.drop")}</span>
        </div>
      </div>

      <div class="block options">
        <h3>{t("sync.analysis")}</h3>
        <label class="tc-check"><input type="checkbox" bind:checked={cfg.options.refine} /> {t("sync.opt.refine")}</label>
        <label class="tc-check"><input type="checkbox" bind:checked={cfg.options.waveform_search} /> {t("sync.opt.search")}</label>
        <label class="tc-check"><input type="checkbox" bind:checked={cfg.options.drift} /> {t("sync.opt.drift")}</label>
        <label class="tc-field">{t("sync.opt.window")}<input type="number" min="0.1" max="30" step="0.5" bind:value={cfg.options.window} /></label>
        {#if running}
          <ProgressBar value={fraction} />
          <p class="muted small ellipsis">{step}</p>
          <button class="tc-btn" onclick={() => cancel()}>{t("transcode.cancel")}</button>
        {:else}
          <button class="start" disabled={!sync.videos.length || (!sync.audios.length && !cfg.options.waveform_search)} onclick={run}>{t("sync.run")}</button>
        {/if}
        {#if error}<p class="error small">{error}</p>{/if}
      </div>
    </div>

    {#if a}
      <div class="block results">
        <div class="row">
          <h3>{t(app.mode === "audio" ? "sync.view.audio" : "sync.view.video")}</h3>
          <span class="muted small">
            {a.pairs.filter((p) => p.audio !== null).length} / {a.pairs.length} {t("sync.paired")} · {a.pairs.filter((p) => p.validated).length} {t("sync.validated")}
          </span>
        </div>
        <div class="table">
          <table>
            <thead>
              <tr>
                <th class="ok">✓</th>
                {#if app.mode === "audio"}<th>{t("sync.sound")}</th><th>TC</th><th>{t("sync.video")}</th>{:else}<th>{t("sync.video")}</th><th>TC</th><th>{t("sync.sound")}</th>{/if}
                <th>{t("sync.method")}</th>
                <th class="num">{t("sync.offset")}</th>
                <th class="num">{t("sync.confidence")}</th>
                <th class="num">{t("sync.drift")}</th>
                <th>{t("sync.note")}</th>
              </tr>
            </thead>
            <tbody>
              {#if app.mode === "audio"}
                {#each bySound as g (g.sound.path)}
                  {#each g.pairs.length ? g.pairs : [null] as row, k (k)}
                    <tr class:sel={row && row.i === sync.selected} onclick={() => row && (sync.selected = row.i)}>
                      <td class="ok">{#if row}<input type="checkbox" bind:checked={a.pairs[row.i].validated} onclick={(e) => e.stopPropagation()} />{/if}</td>
                      <td>{k === 0 ? g.sound.name : ""}</td>
                      <td class="mono">{k === 0 ? (g.sound.timecode ?? "") : ""}</td>
                      <td>{row ? a.videos[row.p.video].name : t("sync.unused")}</td>
                      <td>{row ? methodLabel(row.p) : ""}</td>
                      <td class="num mono">{row ? offsetText(row.p) : ""}</td>
                      <td class="num">{row ? conf(row.p) : ""}</td>
                      <td class="num" class:warn={row && Math.abs(row.p.drift_frames ?? 0) >= 1}>{row && row.p.drift_frames !== null ? row.p.drift_frames.toFixed(1).replace(".", ",") : ""}</td>
                      <td class="note">{row?.p.note ?? ""}</td>
                    </tr>
                  {/each}
                {/each}
              {:else}
                {#each a.pairs as p, i (i)}
                  {@const v = a.videos[p.video]}
                  <tr class:sel={i === sync.selected} class:unpaired={p.audio === null} onclick={() => (sync.selected = i)}>
                    <td class="ok"><input type="checkbox" disabled={p.audio === null} bind:checked={p.validated} onclick={(e) => e.stopPropagation()} /></td>
                    <td>{v.name}</td>
                    <td class="mono">{v.timecode ?? ""}{#if v.source === "ltc"}<span class="badge">LTC</span>{/if}</td>
                    <td>{p.audio === null ? "" : a.audios[p.audio].name}</td>
                    <td>{p.audio === null ? t("sync.none") : methodLabel(p)}</td>
                    <td class="num mono">{offsetText(p)}</td>
                    <td class="num" class:warn={p.confidence !== null && p.confidence < 0.25}>{conf(p)}</td>
                    <td class="num" class:warn={Math.abs(p.drift_frames ?? 0) >= 1}>{p.drift_frames !== null ? p.drift_frames.toFixed(1).replace(".", ",") : ""}</td>
                    <td class="note" title={p.note ?? ""}>{p.note ?? ""}</td>
                  </tr>
                {/each}
              {/if}
            </tbody>
          </table>
        </div>
        <p class="muted small">{t("sync.table.hint")}</p>
      </div>

      <div class="bottom">
        {#if sync.selected >= 0 && a.pairs[sync.selected]}
          <div class="block grow">
            {#key sync.selected}
              <PairDetail bind:analysis={sync.analysis as typeof a} index={sync.selected} />
            {/key}
          </div>
        {/if}

        <div class="block export">
          <h3>{t("sync.export")} ({exportCount})</h3>
          <label class="tc-check"><input type="checkbox" bind:checked={cfg.validatedOnly} /> {t("sync.validated.only")}</label>
          <p class="tc-sub">{t("sync.rewrap")}</p>
          <label class="tc-check"><input type="checkbox" bind:checked={cfg.nextToVideo} /> {t("sync.next.to.video")}</label>
          {#if !cfg.nextToVideo}
            <div class="tc-row">
              <span class="tc-path" title={cfg.rewrap.dest ?? ""}>{cfg.rewrap.dest ?? t("transcode.dest.none")}</span>
              <button class="tc-btn" onclick={pickDest}>{t("offload.browse")}</button>
            </div>
          {/if}
          <div class="tc-grid">
            <label class="tc-field">
              {t("sync.container")}
              <select bind:value={cfg.rewrap.format}>
                <option value="mov">MOV</option>
                <option value="mxf">{t("sync.mxf")}</option>
              </select>
            </label>
            <label class="tc-field">{t("transcode.suffix")}<input class="mono" bind:value={cfg.rewrap.suffix} /></label>
            <label class="tc-check tc-wide"><input type="checkbox" bind:checked={cfg.rewrap.keep_camera_audio} /> {t("sync.keep.camera")}</label>
          </div>
          {#if exporting}
            <ProgressBar value={exportFraction} />
            <p class="muted small ellipsis">{exportStep}</p>
            <button class="tc-btn" onclick={() => cancel()}>{t("transcode.cancel")}</button>
          {:else}
            <button class="start" disabled={!exportCount || (!cfg.nextToVideo && !cfg.rewrap.dest)} onclick={doRewrap}>{t("sync.rewrap.run")}</button>
          {/if}
          <p class="tc-sub">{t("sync.timelines")}</p>
          <div class="tc-row">
            <button class="tc-btn" disabled={!exportCount} onclick={() => doTimeline("fcpxml")}>FCPXML</button>
            <button class="tc-btn" disabled={!exportCount} onclick={() => doTimeline("otio")}>OTIO</button>
          </div>
          {#each results as r, i (i)}
            <p class="small" class:error={!!r.error}>
              {#if r.output}<button class="link" onclick={() => r.output && reveal(r.output)}>{name(r.output)}</button>{/if}
              {r.error ?? ""}
            </p>
          {/each}
          <p class="tc-hint">{t("sync.export.hint")}</p>
        </div>
      </div>
    {:else if !running}
      <p class="muted howto">{t("sync.howto")}</p>
    {/if}
  </div>
</div>

<style>
  .layout {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .sync {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-3);
    padding: var(--vf-space-3);
    overflow-y: auto;
  }
  .sync > :global(*) {
    flex-shrink: 0;
  }
  h2 {
    margin: 0;
    font-size: var(--vf-text-lg);
    letter-spacing: 0.06em;
  }
  h3 {
    margin: 0;
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .mode {
    color: var(--vf-accent);
  }
  .top,
  .bottom {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(260px, 340px);
    gap: var(--vf-space-3);
    align-items: start;
  }
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
    padding: var(--vf-space-3);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    min-width: 0;
  }
  .drop.over {
    border-color: var(--vf-accent);
    box-shadow: inset 0 0 0 1px var(--vf-accent);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--vf-space-2);
  }
  .row.left {
    justify-content: flex-start;
  }
  .lists {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--vf-space-2);
  }
  .lists ul {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 150px;
    overflow-y: auto;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
  }
  .lists li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2px var(--vf-space-2);
    font-size: var(--vf-text-sm);
    border-bottom: 1px solid var(--vf-border);
  }
  .lists li span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .x {
    background: none;
    border: 0;
    color: var(--vf-text-muted);
    cursor: pointer;
  }
  .table {
    max-height: 320px;
    overflow: auto;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--vf-text-sm);
  }
  th,
  td {
    text-align: left;
    padding: 3px 8px;
    border-bottom: 1px solid var(--vf-border);
    white-space: nowrap;
  }
  th {
    position: sticky;
    top: 0;
    background: var(--vf-surface-high);
    color: var(--vf-text-muted);
    font-weight: 600;
    font-size: var(--vf-text-xs);
  }
  tbody tr {
    cursor: pointer;
  }
  tbody tr:hover {
    background: var(--vf-surface-hover);
  }
  tr.sel {
    background: var(--vf-selection);
  }
  tr.unpaired td {
    color: var(--vf-text-muted);
  }
  .ok {
    width: 24px;
    text-align: center;
  }
  .ok input {
    accent-color: var(--vf-accent);
  }
  .num {
    text-align: right;
  }
  .note {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
  }
  .warn {
    color: var(--vf-warning);
  }
  .badge {
    margin-left: var(--vf-space-1);
    padding: 0 4px;
    border-radius: var(--vf-radius-sm);
    background: var(--vf-surface-high);
    font-size: var(--vf-text-xs);
  }
  .start {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-sm);
    font-weight: 700;
    padding: var(--vf-space-2);
    cursor: pointer;
  }
  .start:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .link {
    background: none;
    border: 0;
    color: var(--vf-accent);
    padding: 0;
    cursor: pointer;
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .small {
    font-size: var(--vf-text-xs);
  }
  .error {
    color: var(--vf-error);
  }
  .ellipsis {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  p {
    margin: 0;
  }
  .howto {
    max-width: 640px;
    line-height: 1.5;
  }
</style>
