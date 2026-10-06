<!-- Onglet OFFLOAD : copie sécurisée d'une source vers une ou plusieurs destinations,
     vérification bit à bit, ASC MHL et rapports (charte §7.1). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import JobCard from "./JobCard.svelte";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import { DRAG_TYPE } from "../../lib/explorer";
  import { app } from "../../stores/app.svelte";
  import { offload, listenOffload } from "../../stores/offload.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes, bytesBinary } from "../../lib/format";
  import { ALGORITHMS, volumes, preflight, start, reveal, type HashAlgo, type PreflightView, type Volume, type OffloadRequest } from "../../lib/offload";

  const SETTINGS_KEY = "veriflow.offload.settings";

  interface Settings {
    destinations: string[];
    template: string;
    algorithms: HashAlgo[];
    operator: string;
    eject: boolean;
  }

  function loadSettings(): Settings {
    const defaults: Settings = { destinations: [], template: "{date}/{carte}", algorithms: ["xxh128"], operator: "", eject: false };
    try {
      return { ...defaults, ...JSON.parse(localStorage.getItem(SETTINGS_KEY) ?? "{}") };
    } catch {
      return defaults;
    }
  }

  const saved = loadSettings();
  let source = $state("");
  let destinations = $state<string[]>(saved.destinations);
  let template = $state(saved.template);
  let algorithms = $state<HashAlgo[]>(saved.algorithms);
  let operator = $state(saved.operator);
  let eject = $state(saved.eject);
  let jour = $state("");
  let camera = $state("");
  let notes = $state("");
  let vols = $state<Volume[]>([]);
  let pre = $state<PreflightView | null>(null);
  let checking = $state(false);
  let error = $state("");

  $effect(() => {
    const s: Settings = { destinations, template, algorithms, operator, eject };
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
    } catch {
      /* stockage indisponible */
    }
  });

  const request = (): OffloadRequest => ({
    source,
    destinations,
    template,
    vars: { projet: app.project?.name ?? null, jour: jour || null, camera: camera || null },
    algorithms,
    operator: operator || null,
    notes: notes || null,
  });

  async function refreshVolumes() {
    try {
      vols = await volumes();
    } catch {
      vols = [];
    }
  }

  let timer = 0;
  // Contrôle automatique (espace, doublons, chemins finaux) à chaque modification.
  $effect(() => {
    const r = request();
    window.clearTimeout(timer);
    pre = null;
    error = "";
    if (!r.source || r.destinations.length === 0 || r.algorithms.length === 0) return;
    timer = window.setTimeout(async () => {
      checking = true;
      try {
        pre = await preflight(r);
      } catch (e) {
        error = String(e);
      } finally {
        checking = false;
      }
    }, 300);
  });

  async function pickSource() {
    const r = await open({ directory: true, multiple: false, title: t("offload.source.pick") });
    if (typeof r === "string") source = r;
  }

  async function addDestination() {
    const r = await open({ directory: true, multiple: false, title: t("offload.dest.pick") });
    if (typeof r === "string" && !destinations.includes(r)) destinations = [...destinations, r];
  }

  let picked = $state<string | null>(null);
  let dropOver = $state<"source" | "dest" | null>(null);

  function addDestPath(p: string) {
    if (p && p !== source && !destinations.includes(p)) destinations = [...destinations, p];
  }

  function dropped(e: DragEvent): string | null {
    e.preventDefault();
    dropOver = null;
    return e.dataTransfer?.getData(DRAG_TYPE) || e.dataTransfer?.getData("text/plain") || null;
  }

  const explorerActions = [
    { label: t("explorer.use.source"), run: (p: string) => (source = p) },
    { label: t("explorer.add.dest"), run: addDestPath },
    { label: t("explorer.reveal"), run: (p: string) => reveal(p).catch(() => {}) },
  ];

  function toggleAlgo(id: HashAlgo) {
    algorithms = algorithms.includes(id) ? algorithms.filter((a) => a !== id) : [...algorithms, id];
  }

  const noSpace = $derived(pre?.missing_space.some((m) => m !== null) ?? false);
  const canStart = $derived(!!pre && !noSpace && !checking);

  async function launch() {
    if (!pre) return;
    const warnings: string[] = [];
    if (pre.previous.length > 0) warnings.push(`${t("offload.warn.previous")} (${pre.previous.map((p) => p.finished_at).join(", ")})`);
    pre.already_in_destination.forEach((a, i) => a && warnings.push(`${t("offload.warn.existing")} ${pre!.roots[i]}`));
    if (warnings.length > 0) {
      const go = await confirm(`${warnings.join("\n")}\n\n${t("offload.warn.continue")}`, { title: t("offload.warn.title"), kind: "warning" });
      if (!go) return;
    }
    try {
      await start(request(), eject);
      source = "";
      notes = "";
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    listenOffload();
    refreshVolumes();
  });
</script>

<div class="layout">
<Explorer
  owner="offload"
  selected={picked}
  onSelect={(p) => (picked = p)}
  onActivate={(p) => (source = p)}
  actions={explorerActions}
/>
<div class="offload">
  <section class="setup">
    <h2>{t("offload.title")} <span class="mode">{t("mode." + app.mode)}</span></h2>

    <div
      class="block drop"
      class:over={dropOver === "source"}
      role="region"
      aria-label={t("offload.source")}
      ondragover={(e) => {
        e.preventDefault();
        dropOver = "source";
      }}
      ondragleave={() => (dropOver = null)}
      ondrop={(e) => {
        const p = dropped(e);
        if (p) source = p;
      }}
    >
      <div class="row">
        <h3>{t("offload.source")}</h3>
        <button class="ghost small" onclick={refreshVolumes}>{t("offload.refresh")}</button>
      </div>
      <div class="volumes">
        {#each vols.filter((v) => v.removable) as v (v.mount_point)}
          <button class="vol" class:sel={source === v.mount_point} onclick={() => (source = v.mount_point)} title={v.mount_point}>
            <b>{v.name}</b>
            <span>{bytes(v.total - v.available)} / {bytes(v.total)} · {v.file_system}</span>
          </button>
        {:else}
          <p class="muted">{t("offload.no.removable")}</p>
        {/each}
      </div>
      <div class="row">
        <input class="path" readonly value={source} placeholder={t("offload.source.none")} />
        <button onclick={pickSource}>{t("offload.browse")}</button>
      </div>
      <p class="muted small">{t("offload.drop.source")}</p>
    </div>

    <div
      class="block drop"
      class:over={dropOver === "dest"}
      role="region"
      aria-label={t("offload.destinations")}
      ondragover={(e) => {
        e.preventDefault();
        dropOver = "dest";
      }}
      ondragleave={() => (dropOver = null)}
      ondrop={(e) => {
        const p = dropped(e);
        if (p) addDestPath(p);
      }}
    >
      <h3>{t("offload.destinations")}</h3>
      {#each destinations as d, i (d)}
        <div class="row dest">
          <span class="num">{i + 1}</span>
          <span class="path mono" title={d}>{d}</span>
          {#if pre?.hdd[i]}<span class="warn" title={t("offload.hdd")}>HDD</span>{/if}
          <button class="ghost small" onclick={() => (destinations = destinations.filter((x) => x !== d))} aria-label={t("offload.remove")}>✕</button>
        </div>
      {/each}
      <button class="add" onclick={addDestination}>+ {t("offload.dest.add")}</button>
      <p class="muted small">{t("offload.drop.dest")}</p>
    </div>

    <div class="block grid">
      <label>{t("offload.template")}<input class="mono" bind:value={template} /></label>
      <label>{t("offload.day")}<input bind:value={jour} placeholder="J01" /></label>
      <label>{t("offload.camera")}<input bind:value={camera} placeholder="A" /></label>
      <label>{t("offload.operator")}<input bind:value={operator} /></label>
      <label class="wide">{t("offload.notes")}<input bind:value={notes} /></label>
    </div>

    <div class="block">
      <h3>{t("offload.algorithms")}</h3>
      <div class="algos">
        {#each ALGORITHMS as a (a.id)}
          <label class="check" title={a.mhl ? "" : t("offload.not.mhl")}>
            <input type="checkbox" checked={algorithms.includes(a.id)} onchange={() => toggleAlgo(a.id)} />
            {a.label}{a.mhl ? "" : " *"}
          </label>
        {/each}
      </div>
      <p class="muted small">{t("offload.algo.hint")}</p>
      <label class="check"><input type="checkbox" bind:checked={eject} /> {t("offload.eject.after")}</label>
    </div>

    <div class="block summary">
      {#if checking}
        <p class="muted">{t("offload.checking")}</p>
      {:else if error}
        <p class="error">{error}</p>
      {:else if pre}
        <p><b>{pre.source_name}</b> : {pre.files} {t("offload.files")}, {bytes(pre.total_bytes)} ({bytesBinary(pre.total_bytes)})</p>
        {#each pre.roots as r, i (r)}
          <p class="mono small">
            {i + 1}. {r}
            {#if pre.missing_space[i] !== null}<span class="error"> {t("offload.space.missing")} {bytes(pre.missing_space[i] ?? 0)}</span>{/if}
            {#if pre.already_in_destination[i]}<span class="warn"> {t("offload.warn.existing.short")}</span>{/if}
          </p>
        {/each}
        {#if pre.previous.length > 0}<p class="warn">{t("offload.warn.previous")}</p>{/if}
        {#if pre.source_hdd || pre.hdd.some((h) => h)}<p class="warn">{t("offload.hdd")}</p>{/if}
      {:else}
        <p class="muted">{t("offload.howto")}</p>
      {/if}
      <button class="start" disabled={!canStart} onclick={launch}>{t("offload.start")}</button>
    </div>
  </section>

  <section class="queue">
    <h2>{t("offload.queue")}</h2>
    {#each offload.jobs as job (job.id)}
      <JobCard {job} />
    {:else}
      <p class="muted">{t("offload.queue.empty")}</p>
    {/each}
  </section>
</div>
</div>

<style>
  .layout {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  .drop.over {
    border-color: var(--vf-accent);
    box-shadow: inset 0 0 0 1px var(--vf-accent);
  }
  .offload {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(340px, 440px) minmax(0, 1fr);
    gap: var(--vf-space-4);
    height: 100%;
    padding: var(--vf-space-3);
  }
  section {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-3);
    overflow-y: auto;
    min-height: 0;
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
  .block {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
    padding: var(--vf-space-3);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    justify-content: space-between;
  }
  .volumes {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-2);
  }
  .vol {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    padding: var(--vf-space-2) var(--vf-space-3);
    font-size: var(--vf-text-xs);
    cursor: pointer;
  }
  .vol.sel {
    border-color: var(--vf-accent);
    box-shadow: inset 0 0 0 1px var(--vf-accent);
  }
  .vol b {
    font-size: var(--vf-text-sm);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  input {
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
  }
  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
  }
  .grid label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .grid label:first-child,
  .wide {
    grid-column: 1 / -1;
  }
  .algos {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-2) var(--vf-space-4);
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
  button {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    cursor: pointer;
  }
  .small {
    font-size: var(--vf-text-xs);
  }
  .add {
    align-self: flex-start;
    border-style: dashed;
  }
  .dest .num {
    color: var(--vf-accent);
    font-weight: 700;
  }
  .start {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    font-weight: 700;
    padding: var(--vf-space-2);
    margin-top: var(--vf-space-2);
  }
  .start:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .summary p {
    margin: 0;
  }
  .muted {
    color: var(--vf-text-muted);
    margin: 0;
  }
  .warn {
    color: var(--vf-warning);
    font-size: var(--vf-text-xs);
    font-weight: 600;
  }
  .error {
    color: var(--vf-error);
  }
</style>
