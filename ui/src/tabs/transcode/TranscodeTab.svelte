<!-- Onglet TRANSCODE : conversion de lots de fichiers par préréglages, traitements
     sans réencodage, analyses, file d'attente (charte §7.6). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import TranscodeJob from "./TranscodeJob.svelte";
  import ImageOptions from "./ImageOptions.svelte";
  import OverlayOptions from "./OverlayOptions.svelte";
  import SoundOptions from "./SoundOptions.svelte";
  import SpecialOptions from "./SpecialOptions.svelte";
  import OutputOptions from "./OutputOptions.svelte";
  import UserPresets from "./UserPresets.svelte";
  import FileInfo from "./FileInfo.svelte";
  import "./form.css";
  import { drag, dropZone, startNativeDrop } from "../../stores/drag.svelte";
  import { app } from "../../stores/app.svelte";
  import { transcode, listenTranscode } from "../../stores/transcode.svelte";
  import { t, i18n } from "../../i18n/index.svelte";
  import { reveal } from "../../lib/offload";
  import { presets as fetchPresets, encoderFor, expand, start, CATEGORIES, type EncoderChoice, type PresetView, type Trim } from "../../lib/transcode";
  import { loadForm, saveForm, toRequest, type Form } from "../../lib/transcodeForm.svelte";

  // Extensions acceptées (voir core/src/media/catalog.rs).
  const MEDIA_EXT = ["mov", "mp4", "mxf", "mts", "m2ts", "mkv", "avi", "m4v", "mpg", "mpeg", "webm", "3gp", "wav", "bwf", "rf64", "w64", "aif", "aiff", "flac", "mp3", "m4a", "aac", "ogg", "opus"];
  // Traitements où les points d'entrée et de sortie n'ont pas de sens.
  const NO_TRIM = ["merge", "vmaf", "insert", "replace_audio", "conform", "subtitles", "loudness"];

  let list = $state<PresetView[]>([]);
  let sources = $state<string[]>([]);
  let trims = $state<Record<string, Trim>>({});
  let trimOpen = $state<string | null>(null);
  let infoFor = $state<string | null>(null);
  let form = $state<Form>(loadForm(app.mode));
  let loadedMode = app.mode;
  let encoder = $state<EncoderChoice | null>(null);
  let encoderBusy = $state(false);
  let error = $state("");
  let selected = $state<string | null>(null);

  // Réglages propres à chaque mode (VIDEO / AUDIO), mémorisés sur le poste.
  $effect(() => {
    const mode = app.mode;
    if (mode !== loadedMode) {
      loadedMode = mode;
      form = loadForm(mode);
    }
  });
  $effect(() => {
    const snapshot = $state.snapshot(form) as Form;
    untrack(() => saveForm(loadedMode, snapshot));
  });

  const domain = $derived(app.mode === "audio" ? "audio" : "video");
  const shown = $derived(list.filter((p) => p.domain === domain || p.domain === "both"));
  const preset = $derived(shown.find((p) => p.id === form.settings.preset) ?? shown.find((p) => !p.unavailable) ?? null);
  const lang = $derived(i18n.lang === "en" ? "en" : "fr");
  const trimmable = $derived(!!preset && !NO_TRIM.includes(preset.kind));

  $effect(() => {
    fetchPresets(lang)
      .then((p) => (list = p))
      .catch((e) => (error = String(e)));
  });

  // Encodeur réellement utilisé sur ce poste (essai en arrière-plan).
  let encoderReq = 0;
  $effect(() => {
    const p = preset;
    const soft = form.settings.software;
    encoder = null;
    if (!p || p.unavailable || !p.video) return;
    const req = ++encoderReq;
    encoderBusy = true;
    encoderFor(p.id, soft)
      .then((e) => req === encoderReq && (encoder = e))
      .catch(() => {})
      .finally(() => req === encoderReq && (encoderBusy = false));
  });

  const ENCODER_LABELS: [string, string][] = [
    ["videotoolbox", "Apple VideoToolbox"],
    ["nvenc", "NVIDIA NVENC"],
    ["qsv", "Intel Quick Sync"],
    ["amf", "AMD AMF"],
    ["_mf", "Windows Media Foundation"],
    ["libx264", "x264"],
    ["libx265", "x265"],
    ["libsvtav1", "SVT-AV1"],
    ["libaom", "libaom AV1"],
    ["prores_ks", "FFmpeg prores_ks"],
  ];
  const encoderLabel = (e: EncoderChoice) => ENCODER_LABELS.find(([k]) => e.name.includes(k))?.[1] ?? e.name;

  async function add(paths: string[]) {
    error = "";
    try {
      const files = await expand(paths);
      const known = new Set(sources);
      sources = [...sources, ...files.filter((f) => !known.has(f))];
      if (files.length === 0) error = t("transcode.no.media");
    } catch (e) {
      error = String(e);
    }
  }

  async function pickFiles() {
    const r = await open({ multiple: true, directory: false, title: t("transcode.add.files"), filters: [{ name: t("transcode.media"), extensions: MEDIA_EXT }] });
    if (Array.isArray(r)) add(r);
    else if (typeof r === "string") add([r]);
  }

  async function pickFolder() {
    const r = await open({ directory: true, multiple: false, title: t("transcode.add.folder") });
    if (typeof r === "string") add([r]);
  }

  function removeSource(f: string) {
    sources = sources.filter((x) => x !== f);
    delete trims[f];
  }

  function move(i: number, d: number) {
    const j = i + d;
    if (j < 0 || j >= sources.length) return;
    const next = [...sources];
    [next[i], next[j]] = [next[j], next[i]];
    sources = next;
  }

  function setTrim(f: string, k: "start" | "end", v: string) {
    const cur = trims[f] ?? { start: null, end: null };
    trims[f] = { ...cur, [k]: v.trim() || null };
  }
  const hasTrim = (f: string) => !!(trims[f]?.start || trims[f]?.end);

  // Médias envoyés depuis MEDIA (« Envoyer vers TRANSCODE »).
  $effect(() => {
    const m = app.transcodeMedia;
    if (!m) return;
    app.transcodeMedia = null;
    untrack(() => add(m));
  });

  startNativeDrop();
  const sourcesZone = { name: "transcode", accept: (paths: string[]) => add(paths) };
  const explorerActions = [
    { label: t("transcode.explorer.add"), run: (p: string) => add([p]) },
    { label: t("transcode.explorer.dest"), run: (p: string) => ((form.dest = p), (form.nextToSource = false)) },
    { label: t("explorer.reveal"), run: (p: string) => reveal(p).catch(() => {}) },
  ];

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const folder = (p: string) => p.slice(0, Math.max(0, p.length - name(p).length - 1));

  const writes = $derived(!!preset && (!preset.analysis || ["cut_detect", "black_detect", "offline_detect", "silence_detect", "framemd5"].includes(preset.kind)));
  const needsDest = $derived(writes && !form.nextToSource && !form.dest);
  const enoughFiles = $derived(preset?.kind === "merge" ? sources.length >= 2 : sources.length > 0);
  const canStart = $derived(!!preset && !preset.unavailable && enoughFiles && !needsDest);

  async function launch() {
    if (!preset) return;
    error = "";
    const used: Record<string, Trim> = {};
    if (trimmable) for (const f of sources) if (hasTrim(f)) used[f] = { ...trims[f] };
    const req = toRequest(form, preset, [...sources], used);
    const label = `${preset.label} · ${req.sources.length} ${t(req.sources.length > 1 ? "transcode.files" : "transcode.file")}`;
    try {
      const id = await start(req);
      const j = transcode.jobs.find((x) => x.id === id);
      if (j) Object.assign(j, { request: req, label });
      else transcode.pending[id] = { request: req, label };
      sources = [];
      trims = {};
    } catch (e) {
      error = String(e);
    }
  }

  const startLabel = $derived(preset?.analysis ? "transcode.start.analyze" : "transcode.start");

  onMount(() => listenTranscode());
</script>

{#if infoFor}<FileInfo path={infoFor} onClose={() => (infoFor = null)} />{/if}

<div class="layout">
  <Explorer owner="transcode" {selected} onSelect={(p) => (selected = p)} actions={explorerActions} />
  <div class="transcode">
    <section class="setup">
      <h2>{t("transcode.title")} <span class="mode">{t("mode." + app.mode)}</span></h2>

      <div class="block drop" class:over={drag.over === "transcode"} role="region" aria-label={t("transcode.sources")} use:dropZone={sourcesZone}>
        <div class="row">
          <h3>{t("transcode.sources")} {#if sources.length}<span class="count">({sources.length})</span>{/if}</h3>
          {#if sources.length}<button class="ghost small" onclick={() => ((sources = []), (trims = {}))}>{t("transcode.clear")}</button>{/if}
        </div>
        {#if sources.length}
          <ul class="files">
            {#each sources as f, i (f)}
              <li title={f}>
                <div class="line">
                  <span class="name">{name(f)}</span>
                  <span class="dir mono">{"‎" + folder(f) + "‎"}</span>
                  {#if preset?.kind === "merge"}
                    <button class="x" onclick={() => move(i, -1)} disabled={i === 0} aria-label={t("report.row.up")}>
                      <svg viewBox="0 0 12 12"><path d="M3 7.5 L6 4.5 L9 7.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
                    </button>
                    <button class="x" onclick={() => move(i, 1)} disabled={i === sources.length - 1} aria-label={t("report.row.down")}>
                      <svg viewBox="0 0 12 12"><path d="M3 4.5 L6 7.5 L9 4.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
                    </button>
                  {/if}
                  {#if trimmable}
                    <button class="x" class:on={hasTrim(f)} onclick={() => (trimOpen = trimOpen === f ? null : f)} title={t("transcode.trim")} aria-label={t("transcode.trim")}>
                      <svg viewBox="0 0 12 12"><circle cx="3" cy="9" r="1.6" fill="none" stroke="currentColor" stroke-width="1.2" /><circle cx="9" cy="9" r="1.6" fill="none" stroke="currentColor" stroke-width="1.2" /><path d="M4.2 7.8 L9 2 M7.8 7.8 L3 2" stroke="currentColor" stroke-width="1.2" /></svg>
                    </button>
                  {/if}
                  <button class="x" onclick={() => (infoFor = f)} title={t("transcode.info")} aria-label={t("transcode.info")}>
                    <svg viewBox="0 0 12 12"><circle cx="6" cy="6" r="4.8" fill="none" stroke="currentColor" stroke-width="1.2" /><path d="M6 5.4 V8.6 M6 3.4 V3.6" stroke="currentColor" stroke-width="1.4" stroke-linecap="round" /></svg>
                  </button>
                  <button class="x del" onclick={() => removeSource(f)} aria-label={t("transcode.remove")} title={t("transcode.remove")}>
                    <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3 L9 9 M9 3 L3 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
                  </button>
                </div>
                {#if trimOpen === f && trimmable}
                  <div class="trim tc-grid">
                    <label class="tc-field">{t("transcode.trim.in")}<input class="mono" placeholder="10:00:12:00" value={trims[f]?.start ?? ""} oninput={(e) => setTrim(f, "start", e.currentTarget.value)} /></label>
                    <label class="tc-field">{t("transcode.trim.out")}<input class="mono" placeholder="10:01:30:00" value={trims[f]?.end ?? ""} oninput={(e) => setTrim(f, "end", e.currentTarget.value)} /></label>
                    <p class="tc-hint tc-wide">{t("transcode.trim.hint")}</p>
                  </div>
                {/if}
              </li>
            {/each}
          </ul>
        {/if}
        <div class="buttons">
          <button onclick={pickFiles}>+ {t("transcode.add.files")}</button>
          <button onclick={pickFolder}>+ {t("transcode.add.folder")}</button>
        </div>
        <p class="muted small">{t("transcode.drop")}</p>
      </div>

      <div class="block">
        <h3>{t("transcode.preset")}</h3>
        <select class="tc-input" value={preset?.id ?? ""} onchange={(e) => (form.settings.preset = e.currentTarget.value)}>
          {#each CATEGORIES as c (c)}
            {#if shown.some((p) => p.category === c)}
              <optgroup label={t(`transcode.cat.${c}`)}>
                {#each shown.filter((p) => p.category === c) as p (p.id)}
                  <option value={p.id} disabled={!!p.unavailable} title={p.unavailable ?? ""}>{p.label}{p.unavailable ? ` (${t("transcode.unavailable")})` : ""}</option>
                {/each}
              </optgroup>
            {/if}
          {/each}
        </select>
        {#if preset}
          <p class="muted small">{t(`transcode.hint.${preset.category}`)}</p>
          {#if preset.unavailable}<p class="warn small">{t("transcode.unavailable")} : {preset.unavailable}</p>{/if}
        {/if}
        {#if encoderBusy}
          <p class="muted small">{t("transcode.encoder.testing")}</p>
        {:else if encoder}
          <p class="small">
            {t("transcode.encoder")} <b>{encoderLabel(encoder)}</b>
            ({t(encoder.hardware ? "transcode.encoder.hardware" : "transcode.encoder.software")})
          </p>
          {#if encoder.uncertified_prores}<p class="warn small">{t("transcode.prores.uncertified")}</p>{/if}
        {/if}
        <UserPresets {form} onApply={(f) => (form = f)} />
      </div>

      {#if preset && !preset.unavailable}
        {#if preset.video}
          <ImageOptions bind:form {preset} />
          <OverlayOptions bind:form {preset} />
        {/if}
        {#if preset.audio || preset.kind === "video" || preset.kind === "loudness"}
          <SoundOptions bind:form {preset} />
        {/if}
        <SpecialOptions bind:form {preset} />
        <OutputOptions bind:form {preset} example={sources[0] ? name(sources[0]) : ""} />
      {/if}

      <div class="block">
        {#if error}<p class="error">{error}</p>{/if}
        {#if needsDest}<p class="muted small">{t("transcode.dest.needed")}</p>{/if}
        {#if preset?.kind === "merge" && sources.length === 1}<p class="muted small">{t("transcode.merge.two")}</p>{/if}
        <button class="start" disabled={!canStart} onclick={launch}>
          {t(startLabel)}{sources.length ? ` (${sources.length})` : ""}
        </button>
      </div>
    </section>

    <section class="queue">
      <h2>{t("transcode.queue")}</h2>
      {#each transcode.jobs as job (job.id)}
        <TranscodeJob {job} />
      {:else}
        <p class="muted">{t("transcode.queue.empty")}</p>
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
  .transcode {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-template-columns: minmax(360px, 460px) minmax(0, 1fr);
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
  section > :global(*) {
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
  .count {
    color: var(--vf-text);
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
  .drop.over {
    border-color: var(--vf-accent);
    box-shadow: inset 0 0 0 1px var(--vf-accent);
  }
  .row {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    justify-content: space-between;
  }
  .buttons {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-2);
  }
  .files {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 260px;
    overflow-y: auto;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
  }
  .files li {
    padding: 2px var(--vf-space-2);
    font-size: var(--vf-text-sm);
    border-bottom: 1px solid var(--vf-border);
  }
  .files li:last-child {
    border-bottom: 0;
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
  }
  .files .name {
    flex-shrink: 0;
    max-width: 55%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .files .dir {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
    direction: rtl;
    text-align: left;
  }
  .trim {
    padding: var(--vf-space-1) 0 var(--vf-space-2);
  }
  .x {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    padding: 0;
    background: none;
    border: 0;
    color: var(--vf-text-muted);
    flex: none;
  }
  .x:hover,
  .x.on {
    color: var(--vf-accent);
  }
  .x.del:hover {
    color: var(--vf-error);
  }
  .x:disabled {
    opacity: 0.3;
  }
  .x svg {
    width: 11px;
    height: 11px;
  }
  button {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    cursor: pointer;
  }
  .ghost {
    background: none;
  }
  .small {
    font-size: var(--vf-text-xs);
  }
  p {
    margin: 0;
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .warn {
    color: var(--vf-warning);
  }
  .error {
    color: var(--vf-error);
  }
  .start {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    font-weight: 700;
    padding: var(--vf-space-2);
  }
  .start:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
</style>
