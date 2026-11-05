<!-- Onglet OFFLOAD : copie sécurisée d'une source vers une ou plusieurs destinations,
     vérification bit à bit, ASC MHL et rapports (charte §7.1). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import { ask } from "../../stores/confirm.svelte";
  import JobCard from "./JobCard.svelte";
  import ExistingDialog from "./ExistingDialog.svelte";
  import VerifyDialog from "./VerifyDialog.svelte";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import { drag, dropZone, startNativeDrop } from "../../stores/drag.svelte";
  import { app } from "../../stores/app.svelte";
  import { offload, listenOffload, dismissBatteryAlert, refreshPending } from "../../stores/offload.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes, bytesBinary } from "../../lib/format";
  import { ALGORITHMS, MAX_PARALLEL_JOBS, volumes, preflight, start, batteryWarning, reveal, templatePreview, cascadeResume, cascadeDiscard, type StageChoice, type ExistingMode, type HashAlgo, type PreflightView, type BatteryWarning, type Volume, type OffloadRequest } from "../../lib/offload";

  const SETTINGS_KEY = "veriflow.offload.settings";

  interface Settings {
    destinations: string[];
    template: string;
    algorithms: HashAlgo[];
    operator: string;
    eject: boolean;
    stages: Record<string, StageChoice>;
  }

  function loadSettings(): Settings {
    const defaults: Settings = { destinations: [], template: "{date}/{carte}", algorithms: ["xxh128"], operator: "", eject: false, stages: {} };
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
  // Étape voulue par destination ; absent = Auto.
  let stageChoice = $state<Record<string, StageChoice>>(saved.stages ?? {});
  // Étape que le cœur choisit pour les destinations en Auto, quand d'autres sont forcées.
  let autoRes = $state<{ key: string; stages: Record<string, number> } | null>(null);
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
  // Taille de la source au moment du préflight qui a rempli existingFor.
  let existingBytes: number | null = null;
  // Dossiers finals imposés (reprise d'une copie trouvée dans un autre
  // dossier), valables tant que source, destinations et modèle ne changent pas.
  let rootsOverride = $state<{ key: string; roots: string[] } | null>(null);
  const formKey = () => JSON.stringify([source, destinations, template, jour, camera]);
  let summaryEl = $state<HTMLElement | null>(null);
  let revealSummary = false;

  $effect(() => {
    const s: Settings = { destinations, template, algorithms, operator, eject, stages: stageChoice };
    try {
      localStorage.setItem(SETTINGS_KEY, JSON.stringify(s));
    } catch {
      /* stockage indisponible */
    }
  });

  const choiceOf = (d: string): StageChoice => stageChoice[d] ?? "auto";
  const allAuto = $derived(destinations.every((d) => choiceOf(d) === "auto"));
  const destKey = $derived(JSON.stringify(destinations));

  /** Étape de chaque destination quand une au moins est forcée (Auto : choix du cœur, 1 par défaut). */
  function resolved(d: string): 1 | 2 {
    const c = choiceOf(d);
    if (c === "immediate") return 1;
    if (c === "cascade") return 2;
    return autoRes && autoRes.key === destKey && autoRes.stages[d] === 2 ? 2 : 1;
  }

  /** Listes envoyées au cœur et ordre des dossiers du préflight (destinations puis cascade). */
  const plan = $derived.by(() => {
    if (allAuto) return { auto: true, dests: destinations, cascade: [] as string[] };
    return {
      auto: false,
      dests: destinations.filter((d) => resolved(d) === 1),
      cascade: destinations.filter((d) => resolved(d) === 2),
    };
  });
  const planOrder = $derived([...plan.dests, ...plan.cascade]);
  /** Rang de la destination dans les résultats du préflight. */
  const preIndex = (d: string) => planOrder.indexOf(d);
  const stageOf = (d: string): number | null => pre?.stages[preIndex(d)] ?? null;

  // Destinations en Auto alors que d'autres sont forcées : on demande au cœur
  // comment il répartirait toutes les destinations.
  $effect(() => {
    const key = destKey;
    if (allAuto || !source || destinations.length === 0 || algorithms.length === 0) return;
    if (autoRes && autoRes.key === key) return;
    const dests = [...destinations];
    const r: OffloadRequest = { source, destinations: dests, template, vars: { projet: null, jour: null, camera: null }, algorithms: ["xxh128"], operator: null, notes: null, cascade: [], cascade_auto: true };
    preflight(r)
      .then((p) => {
        if (key !== JSON.stringify(destinations)) return;
        autoRes = { key, stages: Object.fromEntries(dests.map((d, i) => [d, p.stages[i] ?? 1])) };
      })
      .catch(() => {});
  });

  const request = (): OffloadRequest => ({
    source,
    destinations: plan.dests,
    cascade: plan.cascade,
    cascade_auto: plan.auto,
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
    destinations = [...r.destinations, ...(r.cascade ?? [])];
    stageChoice = Object.fromEntries([
      ...(r.cascade_auto ? [] : r.destinations.map((d) => [d, "immediate"])),
      ...(r.cascade ?? []).map((d) => [d, "cascade"]),
    ]);
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
  const noImmediate = $derived(!!pre && pre.stages.length > 0 && !pre.stages.includes(1));
  const singleCopy = $derived(!!pre && pre.stages.includes(2) && pre.stages.filter((s) => s === 1).length === 1);
  const canStart = $derived(!!pre && !noSpace && !checking && !noImmediate);

  function setChoice(d: string, c: StageChoice) {
    stageChoice = { ...stageChoice, [d]: c };
  }

  async function resumeCascade(key: string) {
    try {
      await cascadeResume(key);
    } catch (e) {
      error = String(e);
    }
    await refreshPending();
  }

  async function discardCascade(key: string, card: string) {
    const yes = await ask(t("offload.cascade.discard_confirm").replace("{card}", card), {
      title: t("offload.cascade.discard"),
      ok: t("offload.cascade.discard"),
      kind: "danger",
    });
    if (!yes) return;
    try {
      await cascadeDiscard(key);
    } catch (e) {
      error = String(e);
    }
    await refreshPending();
  }

  function batteryText(w: BatteryWarning): string {
    const key = w.reason === "short" ? "offload.battery.short" : w.reason === "low" ? "offload.battery.low" : w.percent === null ? "offload.battery.long_nopercent" : "offload.battery.long";
    return t(key)
      .replace("{percent}", String(w.percent ?? ""))
      .replace("{minutes}", String(w.estimated_minutes))
      .replace("{left}", String(w.minutes_left ?? ""));
  }

  function alertText(a: { key: string; vars: Record<string, string> }): string {
    let s = t(a.key);
    for (const [k, v] of Object.entries(a.vars)) s = s.replace(`{${k}}`, v);
    return s;
  }

  let launching = $state(false);

  /** Relevé frais : pre.battery peut être périmé si l'utilisateur a tardé. */
  async function freshBattery(): Promise<BatteryWarning | null> {
    try {
      return await batteryWarning(pre?.estimated_secs ?? 0);
    } catch {
      return pre?.battery ?? null; // repli sur le relevé du contrôle préalable
    }
  }

  async function confirm(warnings: string[]): Promise<boolean> {
    if (warnings.length === 0) return true;
    return ask(`${warnings.join("\n")}\n\n${t("offload.warn.continue")}`, {
      title: t("offload.warn.title"),
      ok: t("dialog.continue"),
    });
  }

  async function launch() {
    if (!pre || launching) return;
    // Rushes déjà présents (copie interrompue) : vérification puis choix.
    if (pre.present.some((p) => p.files > 0 || p.partial > 0)) {
      existingFor = { ...request(), date: pre.date };
      existingBytes = pre.total_bytes;
      return;
    }
    launching = true;
    try {
      const warnings: string[] = [];
      if (pre.previous.length > 0) warnings.push(`${t("offload.warn.previous")} (${pre.previous.map((p) => p.finished_at).join(", ")})`);
      pre.already_in_destination.forEach((a, i) => a && warnings.push(`${t("offload.warn.existing")} ${pre!.roots[i]}`));
      const battery = await freshBattery();
      if (!pre) return;
      if (battery) warnings.push(batteryText(battery));
      if (!(await confirm(warnings))) return;
      if (!pre) return;
      await launchWith({ ...request(), date: pre.date }, "verify", null, pre.total_bytes);
    } finally {
      launching = false;
    }
  }

  async function chooseExisting(r: OffloadRequest, mode: ExistingMode, check: number | null) {
    if (launching) return;
    existingFor = null;
    launching = true;
    try {
      const battery = await freshBattery();
      if (!pre) return;
      if (!(await confirm(battery ? [batteryText(battery)] : []))) return;
      if (!pre) return;
      await launchWith(r, mode, check, existingBytes);
    } finally {
      launching = false;
    }
  }

  async function launchWith(r: OffloadRequest, existing: ExistingMode, check: number | null, totalBytes: number | null) {
    existingFor = null;
    const req = { ...r, existing };
    try {
      const id = await start(req, eject, check, totalBytes);
      offload.requests[id] = req;
      source = "";
      notes = "";
      resumeDate = null;
      resumeSource = "";
    } catch (e) {
      error = String(e);
    }
  }

  // Dossier choisi pour une revérification ASC MHL.
  let verifyRoot = $state<string | null>(null);
  async function pickVerify() {
    const r = await open({ directory: true, multiple: false, title: t("offload.verify.button") });
    if (typeof r === "string") verifyRoot = r;
  }

  onMount(() => {
    listenOffload();
    refreshVolumes();
    void refreshPending();
  });
</script>

{#if existingFor}
  <ExistingDialog
    request={existingFor}
    onChoose={(mode, check) => existingFor && chooseExisting(existingFor, mode, check)}
    onClose={() => (existingFor = null)}
  />
{/if}

{#if verifyRoot}
  <VerifyDialog root={verifyRoot} operator={operator || null} onClose={() => (verifyRoot = null)} />
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
          {#if pre?.hdd[preIndex(d)]}<span class="warn" title={t("offload.hdd")}>HDD</span>{/if}
          {#if stageOf(d)}<span class="stage" class:s2={stageOf(d) === 2}>{t(stageOf(d) === 2 ? "offload.cascade.stage2" : "offload.cascade.stage1")}</span>{/if}
          <select
            class="stage-pick"
            aria-label={t("offload.dest.stage")}
            value={choiceOf(d)}
            onchange={(e) => {
              setChoice(d, e.currentTarget.value as StageChoice);
              e.currentTarget.blur();
            }}
          >
            <option value="auto">{t("offload.dest.auto")}</option>
            <option value="immediate">{t("offload.dest.immediate")}</option>
            <option value="cascade">{t("offload.dest.cascade")}</option>
          </select>
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
            {#if pre.stages[i] === 2}<span class="stage s2">{t("offload.cascade.stage2")}</span>{/if}
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
            <p class="note small">{t("offload.elsewhere.hint")}</p>
          </div>
        {/if}
        {#if noImmediate}<p class="error">{t("offload.cascade.need_immediate")}</p>{/if}
        {#if singleCopy}<p class="warn">{t("offload.cascade.single_copy_warning")}</p>{/if}
        {#if pre.previous.length > 0}<p class="warn">{t("offload.warn.previous")}</p>{/if}
        {#if pre.source_hdd || pre.hdd.some((h) => h)}<p class="warn">{t("offload.hdd")}</p>{/if}
        {#if pre.battery}<p class="warn">{batteryText(pre.battery)}</p>{/if}
      {:else}
        <p class="muted">{t("offload.howto")}</p>
      {/if}
      <button class="start" disabled={!canStart || launching} onclick={launch}>{t("offload.start")}</button>
    </div>
  </section>

  <section class="queue">
    <h2>{t("offload.queue")}</h2>
    <div class="verify-row">
      <button class="ghost small" onclick={pickVerify}>{t("offload.verify.button")}</button>
    </div>
    {#if offload.batteryAlert}
      <p class="warn alert">
        <span>{alertText(offload.batteryAlert)}</span>
        <button onclick={dismissBatteryAlert}>{t("offload.battery.dismiss")}</button>
      </p>
    {/if}
    {#if offload.pending.length > 0}
      <div class="pending">
        <h3>{t("offload.cascade.pending_title")}</h3>
        {#each offload.pending as p (p.key)}
          <div class="row">
            <span class="path mono" title={p.stage2_roots.join("\n")}>{p.card} : {p.stage2_roots.join(", ")}</span>
            <button onclick={() => resumeCascade(p.key)}>{t("offload.cascade.resume")}</button>
            <button class="ghost" onclick={() => discardCascade(p.key, p.card)}>{t("offload.cascade.discard")}</button>
          </div>
        {/each}
      </div>
    {/if}
    {#each offload.jobs as job (job.id)}
      <JobCard {job} />
    {:else}
      <p class="muted">{t("offload.queue.empty")}</p>
    {/each}
    <p class="muted">{t("offload.job.parallel_hint").replace("{max}", String(MAX_PARALLEL_JOBS))}</p>
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
  .alert {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
  }
  .error {
    color: var(--vf-error);
  }
  .stage {
    flex: none;
    font-size: var(--vf-text-xs);
    font-weight: 600;
    padding: 1px var(--vf-space-2);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    color: var(--vf-text-muted);
  }
  /* L'étape 2 se distingue de l'étape 1 par la couleur d'accent, pas seulement par le texte. */
  .stage.s2 {
    color: var(--vf-accent);
    border-color: var(--vf-accent);
  }
  .stage-pick {
    flex: none;
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-size: var(--vf-text-xs);
  }
  .pending {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
    padding: var(--vf-space-3);
    background: var(--vf-surface);
    border: 1px solid var(--vf-warning);
    border-radius: var(--vf-radius-md);
  }
  .pending .row {
    justify-content: flex-start;
  }
</style>
