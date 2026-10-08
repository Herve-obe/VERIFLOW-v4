<!-- Onglet OFFLOAD : copie sécurisée d'une source vers une ou plusieurs destinations,
     vérification bit à bit, ASC MHL et rapports (charte §7.1). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open, confirm } from "@tauri-apps/plugin-dialog";
  import JobCard from "./JobCard.svelte";
  import ExistingDialog from "./ExistingDialog.svelte";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import { drag, dropZone, startNativeDrop } from "../../stores/drag.svelte";
  import { app } from "../../stores/app.svelte";
  import { offload, listenOffload } from "../../stores/offload.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes, bytesBinary } from "../../lib/format";
  import { ALGORITHMS, volumes, preflight, start, reveal, templatePreview, type ExistingMode, type HashAlgo, type PreflightView, type Volume, type OffloadRequest } from "../../lib/offload";

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
  // Reprise d'une copie interrompue : même date, donc même dossier final,
  // tant que la source reste celle de la copie reprise.
  let resumeDate = $state<string | null>(null);
  let resumeSource = "";
  // Demande en attente du choix « compléter / tout recopier ».
  let existingFor = $state<OffloadRequest | null>(null);
  // Dossiers finals imposés (reprise d'une copie trouvée dans un autre
  // dossier), valables tant que source, destinations et modèle ne changent pas.
  let rootsOverride = $state<{ key: string; roots: string[] } | null>(null);
  const formKey = () => JSON.stringify([source, destinations, template, jour, camera]);
  let summaryEl = $state<HTMLElement | null>(null);
  let revealSummary = false;

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
    date: source && source === resumeSource ? resumeDate : null,
    roots: rootsOverride && rootsOverride.key === formKey() ? rootsOverride.roots : null,
  });

  /** Reprend la copie dans les dossiers où elle a été trouvée. */
  function useElsewhere() {
    if (!pre) return;
    rootsOverride = { key: formKey(), roots: pre.roots.map((r, i) => pre!.elsewhere[i][0]?.root ?? r) };
  }

  // Aperçu du chemin produit par le modèle, sous le champ.
  let preview = $state("");
  let previewTimer = 0;
  $effect(() => {
    const vars = { projet: app.project?.name ?? null, jour: jour || null, camera: camera || null };
    const card = source.split(/[\\/]/).filter(Boolean).pop() ?? "A001";
    const base = destinations[0] ?? "";
    const sep = base.includes("\\") ? "\\" : "/";
    const tpl = template;
    const date = source && source === resumeSource ? resumeDate : null;
    window.clearTimeout(previewTimer);
    previewTimer = window.setTimeout(() => {
      templatePreview(tpl, vars, card, date)
        .then((rel) => {
          const relNative = rel.split(/[\\/]/).join(sep);
          preview = base ? `${base.replace(/[\\/]+$/, "")}${sep}${relNative}` : relNative;
        })
        .catch(() => (preview = ""));
    }, 150);
  });

  // « Reprendre la copie » depuis la file d'attente : formulaire rempli avec
  // la demande d'origine.
  $effect(() => {
    const r = offload.resume;
    if (!r) return;
    offload.resume = null;
    source = r.source;
    resumeSource = r.source;
    resumeDate = r.date ?? null;
    destinations = [...r.destinations];
    template = r.template;
    algorithms = [...r.algorithms];
    operator = r.operator ?? "";
    notes = r.notes ?? "";
    jour = r.vars.jour ?? "";
    camera = r.vars.camera ?? "";
    rootsOverride = r.roots ? { key: formKey(), roots: [...r.roots] } : null;
    revealSummary = true;
  });

  // Après une reprise, le résumé et le bouton de lancement sont amenés à l'écran.
  $effect(() => {
    if (pre && summaryEl && revealSummary) {
      revealSummary = false;
      summaryEl.scrollIntoView({ block: "end", behavior: "smooth" });
    }
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

  function addDestPath(p: string) {
    if (p && p !== source && !destinations.includes(p)) destinations = [...destinations, p];
  }

  // Dépôts depuis l'explorateur de VERIFLOW ou depuis le système.
  startNativeDrop();
  const sourceZone = { name: "source", accept: (paths: string[]) => (source = paths[0]) };
  const destZone = { name: "dest", accept: (paths: string[]) => paths.forEach(addDestPath) };

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
    // Rushes déjà présents (copie interrompue) : vérification puis choix.
    if (pre.present.some((p) => p.files > 0 || p.partial > 0)) {
      existingFor = { ...request(), date: pre.date };
      return;
    }
    const warnings: string[] = [];
    if (pre.previous.length > 0) warnings.push(`${t("offload.warn.previous")} (${pre.previous.map((p) => p.finished_at).join(", ")})`);
    pre.already_in_destination.forEach((a, i) => a && warnings.push(`${t("offload.warn.existing")} ${pre!.roots[i]}`));
    if (warnings.length > 0) {
      const go = await confirm(`${warnings.join("\n")}\n\n${t("offload.warn.continue")}`, { title: t("offload.warn.title"), kind: "warning" });
      if (!go) return;
    }
    await launchWith({ ...request(), date: pre.date }, "verify", null);
  }

  async function launchWith(r: OffloadRequest, existing: ExistingMode, check: number | null) {
    existingFor = null;
    const req = { ...r, existing };
    try {
      const id = await start(req, eject, check);
      offload.requests[id] = req;
      source = "";
      notes = "";
      resumeDate = null;
      resumeSource = "";
    } catch (e) {
      error = String(e);
    }
  }

  onMount(() => {
    listenOffload();
    refreshVolumes();
  });
</script>

{#if existingFor}
  <ExistingDialog
    request={existingFor}
    onChoose={(mode, check) => existingFor && launchWith(existingFor, mode, check)}
    onClose={() => (existingFor = null)}
  />
{/if}

<div class="layout">
<Explorer
  owner="offload"
  selected={picked}
  onSelect={(p) => (picked = p)}
  actions={explorerActions}
/>
<div class="offload">
  <section class="setup">
    <h2>{t("offload.title")} <span class="mode">{t("mode." + app.mode)}</span></h2>

    <div
      class="block drop"
      class:over={drag.over === "source"}
      role="region"
      aria-label={t("offload.source")}
      use:dropZone={sourceZone}
    >
      <div class="row">
        <h3>{t("offload.source")}</h3>
        <button class="ghost small" onclick={refreshVolumes}>{t("offload.refresh")}</button>
      </div>
      {#if vols.some((v) => v.removable)}
        <p class="muted small">{t("offload.removable.hint")}</p>
      {/if}
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
      class:over={drag.over === "dest"}
      role="region"
      aria-label={t("offload.destinations")}
      use:dropZone={destZone}
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
      {#if preview}
        <p class="wide preview small">{t("offload.template.example")} <span class="mono">{request().roots?.[0] ?? preview}</span></p>
      {/if}
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

    <div class="block summary" bind:this={summaryEl}>
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
            {#if pre.present[i].files > 0 || pre.present[i].partial > 0}
              <span class="warn"> {t("offload.present.short")} {pre.present[i].files} ({bytes(pre.present[i].bytes)})</span>
            {:else if pre.already_in_destination[i]}<span class="warn"> {t("offload.warn.existing.short")}</span>{/if}
          </p>
        {/each}
        {#if request().roots}
          <p class="note small">
            {t("offload.elsewhere.forced")}
            <button class="link" onclick={() => (rootsOverride = null)}>{t("offload.elsewhere.back")}</button>
          </p>
        {:else if pre.elsewhere.some((l) => l.length > 0)}
          <div class="elsewhere">
            <p class="warn">{t("offload.elsewhere.found")}</p>
            {#each pre.elsewhere as list, i (i)}
              {#if list[0]}
                <p class="mono small">{i + 1}. {list[0].root} : {list[0].files} {t("offload.files")} ({bytes(list[0].bytes)})</p>
              {/if}
            {/each}
            <button onclick={useElsewhere}>{t("offload.elsewhere.use")}</button>
          </div>
        {/if}
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
  /* Fenêtre basse : les blocs gardent leur hauteur et la colonne défile, au
     lieu de se comprimer et de se chevaucher. */
  section > * {
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
  /* Colonnes minmax(0, 1fr) et champs à 100 % : un champ ne peut jamais
     élargir sa colonne au-delà du cadre (moteur web de Catalina). */
  .grid {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--vf-space-2) var(--vf-space-3);
  }
  .grid label {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .grid input {
    width: 100%;
    min-width: 0;
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
  .preview {
    margin: -2px 0 0;
    color: var(--vf-text-muted);
    overflow-wrap: anywhere;
  }
  .preview .mono {
    color: var(--vf-text);
  }
  .elsewhere {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: var(--vf-space-1);
    padding: var(--vf-space-2);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
  }
  .elsewhere p {
    margin: 0;
    overflow-wrap: anywhere;
  }
  .note {
    margin: 0;
    color: var(--vf-text-muted);
  }
  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--vf-accent);
    cursor: pointer;
    text-decoration: underline;
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
