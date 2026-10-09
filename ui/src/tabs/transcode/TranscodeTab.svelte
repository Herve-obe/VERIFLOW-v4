<!-- Onglet TRANSCODE : conversion de lots de fichiers par préréglages, file
     d'attente, normalisation et mesure du loudness (charte §7.6). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import TranscodeJob from "./TranscodeJob.svelte";
  import { drag, dropZone, startNativeDrop } from "../../stores/drag.svelte";
  import { app } from "../../stores/app.svelte";
  import { transcode, listenTranscode } from "../../stores/transcode.svelte";
  import { t, i18n } from "../../i18n/index.svelte";
  import { reveal } from "../../lib/offload";
  import {
    presets as fetchPresets,
    encoderFor,
    expand,
    start,
    LOUDNESS_TARGETS,
    SAMPLE_RATES,
    SCALES,
    type BitDepth,
    type Category,
    type EncoderChoice,
    type Existing,
    type PresetView,
    type Settings,
    type TranscodeRequest,
  } from "../../lib/transcode";

  // Extensions acceptées (voir core/src/media/catalog.rs).
  const MEDIA_EXT = ["mov", "mp4", "mxf", "mts", "m2ts", "mkv", "avi", "m4v", "mpg", "mpeg", "webm", "3gp", "wav", "bwf", "rf64", "w64", "aif", "aiff", "flac", "mp3", "m4a", "aac", "ogg", "opus"];
  const CATEGORIES: Category[] = ["intermediate", "delivery", "proxy", "no_reencode", "audio", "analysis"];

  interface Saved {
    preset: string;
    scale: string | null;
    videoMbps: number | null;
    software: boolean;
    sampleRate: number | null;
    bitDepth: BitDepth | null;
    audioKbps: number | null;
    loudness: string;
    customI: number;
    customTp: number;
    dest: string;
    nextToSource: boolean;
    suffix: string;
    existing: Existing;
  }

  const key = (mode: string) => `veriflow.transcode.${mode}`;
  function load(mode: string): Saved {
    const defaults: Saved = {
      preset: mode === "audio" ? "wav" : "prores_422",
      scale: null,
      videoMbps: null,
      software: false,
      sampleRate: null,
      bitDepth: null,
      audioKbps: null,
      loudness: "none",
      customI: -23,
      customTp: -1,
      dest: "",
      nextToSource: false,
      suffix: "",
      existing: "rename",
    };
    try {
      return { ...defaults, ...JSON.parse(localStorage.getItem(key(mode)) ?? "{}") };
    } catch {
      return defaults;
    }
  }

  let list = $state<PresetView[]>([]);
  let sources = $state<string[]>([]);
  let s = $state<Saved>(load(app.mode));
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
      s = load(mode);
    }
  });
  $effect(() => {
    const snapshot = JSON.stringify(s);
    try {
      localStorage.setItem(key(untrack(() => loadedMode)), snapshot);
    } catch {
      /* stockage indisponible */
    }
  });

  const domain = $derived(app.mode === "audio" ? "audio" : "video");
  const shown = $derived(list.filter((p) => p.domain === domain));
  const preset = $derived(shown.find((p) => p.id === s.preset) ?? shown[0] ?? null);
  const lang = $derived(i18n.lang === "en" ? "en" : "fr");

  $effect(() => {
    fetchPresets(lang)
      .then((p) => (list = p))
      .catch((e) => (error = String(e)));
  });

  // Encodeur réellement utilisé sur ce poste (essai en arrière-plan).
  let encoderReq = 0;
  $effect(() => {
    const p = preset;
    const soft = s.software;
    encoder = null;
    if (!p || !p.video || (!p.encoder_choice && !p.video_bitrate && !p.id.includes("prores"))) return;
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
    ["prores_ks", "FFmpeg prores_ks"],
  ];
  const encoderLabel = (e: EncoderChoice) => ENCODER_LABELS.find(([k]) => e.name.includes(k))?.[1] ?? e.name;

  async function add(paths: string[]) {
    error = "";
    try {
      const files = await expand(paths);
      const known = new Set(sources);
      const fresh = files.filter((f) => !known.has(f));
      sources = [...sources, ...fresh];
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

  async function pickDest() {
    const r = await open({ directory: true, multiple: false, title: t("transcode.dest.pick") });
    if (typeof r === "string") {
      s.dest = r;
      s.nextToSource = false;
    }
  }

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
    { label: t("transcode.explorer.dest"), run: (p: string) => ((s.dest = p), (s.nextToSource = false)) },
    { label: t("explorer.reveal"), run: (p: string) => reveal(p).catch(() => {}) },
  ];

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const folder = (p: string) => p.slice(0, Math.max(0, p.length - name(p).length - 1));

  function loudnessTarget() {
    if (s.loudness === "none") return null;
    if (s.loudness === "custom") return { integrated: Number(s.customI), true_peak: Number(s.customTp) };
    return LOUDNESS_TARGETS.find((l) => l.id === s.loudness)?.target ?? null;
  }

  const settings = (): Settings => ({
    preset: preset?.id ?? s.preset,
    scale: preset?.video ? s.scale : null,
    video_mbps: preset?.video_bitrate && s.videoMbps ? Number(s.videoMbps) : null,
    software: !!preset?.encoder_choice && s.software,
    sample_rate: preset?.audio ? s.sampleRate : null,
    bit_depth: preset?.audio && preset.bit_depths.includes(s.bitDepth as BitDepth) ? s.bitDepth : null,
    audio_kbps: preset?.audio && preset.audio_bitrates.length ? s.audioKbps : null,
    loudness: preset?.audio || preset?.analysis ? loudnessTarget() : null,
  });

  const suffix = $derived(s.suffix || preset?.suffix || "");
  const needsDest = $derived(!!preset && !preset.analysis && !s.nextToSource && !s.dest);
  const canStart = $derived(!!preset && sources.length > 0 && !needsDest);

  async function launch() {
    if (!preset) return;
    error = "";
    const req: TranscodeRequest = {
      sources: [...sources],
      settings: settings(),
      dest: preset.analysis || s.nextToSource ? null : s.dest,
      suffix,
      existing: s.existing,
    };
    const label = `${preset.label} · ${req.sources.length} ${t(req.sources.length > 1 ? "transcode.files" : "transcode.file")}`;
    try {
      const id = await start(req);
      const j = transcode.jobs.find((x) => x.id === id);
      if (j) Object.assign(j, { request: req, label });
      else transcode.pending[id] = { request: req, label };
      sources = [];
    } catch (e) {
      error = String(e);
    }
  }

  const scaleLabel = (v: string) => (v === "source" ? t("transcode.scale.source") : v === "1/2" ? t("transcode.scale.half") : v === "1/4" ? t("transcode.scale.quarter") : `${v}p`);
  const depthLabel = (d: BitDepth) => (d === "32f" ? t("transcode.depth.float") : `${d} bits`);

  onMount(() => listenTranscode());
</script>

<div class="layout">
  <Explorer owner="transcode" {selected} onSelect={(p) => (selected = p)} actions={explorerActions} />
  <div class="transcode">
    <section class="setup">
      <h2>{t("transcode.title")} <span class="mode">{t("mode." + app.mode)}</span></h2>

      <div class="block drop" class:over={drag.over === "transcode"} role="region" aria-label={t("transcode.sources")} use:dropZone={sourcesZone}>
        <div class="row">
          <h3>{t("transcode.sources")} {#if sources.length}<span class="count">({sources.length})</span>{/if}</h3>
          {#if sources.length}<button class="ghost small" onclick={() => (sources = [])}>{t("transcode.clear")}</button>{/if}
        </div>
        {#if sources.length}
          <ul class="files">
            {#each sources as f (f)}
              <li title={f}>
                <span class="name">{name(f)}</span>
                <span class="dir mono">{"\u200e" + folder(f) + "\u200e"}</span>
                <button class="x" onclick={() => (sources = sources.filter((x) => x !== f))} aria-label={t("transcode.remove")} title={t("transcode.remove")}>
                  <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3 3 L9 9 M9 3 L3 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
                </button>
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
        <select value={preset?.id ?? ""} onchange={(e) => (s.preset = e.currentTarget.value)}>
          {#each CATEGORIES as c (c)}
            {#if shown.some((p) => p.category === c)}
              <optgroup label={t(`transcode.cat.${c}`)}>
                {#each shown.filter((p) => p.category === c) as p (p.id)}
                  <option value={p.id}>{p.label}</option>
                {/each}
              </optgroup>
            {/if}
          {/each}
        </select>
        {#if preset}
          <p class="muted small">{t(`transcode.hint.${preset.category}`)}</p>
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
      </div>

      {#if preset && (preset.video || preset.audio || preset.analysis)}
        <div class="block grid">
          <h3 class="wide">{t("transcode.options")}</h3>
          {#if preset.video}
            <label>
              {t("transcode.scale")}
              <select value={s.scale ?? preset.scale} onchange={(e) => (s.scale = e.currentTarget.value === preset.scale ? null : e.currentTarget.value)}>
                {#each SCALES as v (v)}<option value={v}>{scaleLabel(v)}</option>{/each}
              </select>
            </label>
          {/if}
          {#if preset.video_bitrate}
            <label>
              {t("transcode.video.bitrate")}
              <input type="number" min="1" step="1" placeholder={t("transcode.auto")} value={s.videoMbps ?? ""} oninput={(e) => (s.videoMbps = e.currentTarget.value ? Number(e.currentTarget.value) : null)} />
            </label>
          {/if}
          {#if preset.encoder_choice}
            <label class="check wide"><input type="checkbox" bind:checked={s.software} /> {t("transcode.software")}</label>
          {/if}
          {#if preset.audio}
            <label>
              {t("transcode.rate")}
              <select value={s.sampleRate ?? 0} onchange={(e) => (s.sampleRate = Number(e.currentTarget.value) || null)}>
                <option value={0}>{t("transcode.same")}</option>
                {#each SAMPLE_RATES as r (r)}<option value={r}>{(r / 1000).toLocaleString(lang)} kHz</option>{/each}
              </select>
            </label>
            {#if preset.bit_depths.length}
              <label>
                {t("transcode.depth")}
                <select value={s.bitDepth ?? ""} onchange={(e) => (s.bitDepth = (e.currentTarget.value || null) as BitDepth | null)}>
                  <option value="">{t("transcode.same")}</option>
                  {#each preset.bit_depths as d (d)}<option value={d}>{depthLabel(d)}</option>{/each}
                </select>
              </label>
            {/if}
            {#if preset.audio_bitrates.length}
              <label>
                {t("transcode.audio.bitrate")}
                <select value={s.audioKbps ?? preset.default_audio_bitrate} onchange={(e) => (s.audioKbps = Number(e.currentTarget.value))}>
                  {#each preset.audio_bitrates as r (r)}<option value={r}>{r} kbit/s</option>{/each}
                </select>
              </label>
            {/if}
          {/if}
          {#if preset.audio || preset.analysis}
            <label class="wide">
              {t(preset.analysis ? "transcode.loudness.reference" : "transcode.loudness")}
              <select bind:value={s.loudness}>
                <option value="none">{t(preset.analysis ? "transcode.loudness.noref" : "transcode.loudness.none")}</option>
                {#each LOUDNESS_TARGETS as l (l.id)}
                  <option value={l.id}>{t(`transcode.loudness.${l.id}`)} ({l.target.integrated} LUFS, {l.target.true_peak} dBTP)</option>
                {/each}
                <option value="custom">{t("transcode.loudness.custom")}</option>
              </select>
            </label>
            {#if s.loudness === "custom"}
              <label>{t("transcode.loudness.integrated")}<input type="number" step="0.5" bind:value={s.customI} /></label>
              <label>{t("transcode.loudness.tp")}<input type="number" step="0.5" bind:value={s.customTp} /></label>
            {/if}
            {#if s.loudness !== "none" && !preset.analysis}
              <p class="wide muted small">{t("transcode.loudness.hint")}</p>
            {/if}
          {/if}
          {#if preset.audio && preset.audio_bitrates.length}
            <p class="wide muted small">{t("transcode.lossy.channels")}</p>
          {/if}
        </div>
      {/if}

      {#if preset && !preset.analysis}
        <div class="block grid">
          <h3 class="wide">{t("transcode.dest")}</h3>
          <label class="check wide"><input type="checkbox" bind:checked={s.nextToSource} /> {t("transcode.next.to.source")}</label>
          {#if !s.nextToSource}
            <div class="row wide">
              <span class="path mono" title={s.dest}>{s.dest || t("transcode.dest.none")}</span>
              <button onclick={pickDest}>{t("offload.browse")}</button>
            </div>
          {/if}
          <label>
            {t("transcode.suffix")}
            <input class="mono" placeholder={preset.suffix || t("transcode.suffix.none")} bind:value={s.suffix} />
          </label>
          <label>
            {t("transcode.existing")}
            <select bind:value={s.existing}>
              <option value="rename">{t("transcode.existing.rename")}</option>
              <option value="skip">{t("transcode.existing.skip")}</option>
              <option value="overwrite">{t("transcode.existing.overwrite")}</option>
            </select>
          </label>
          <p class="wide muted small">{t("transcode.never.overwrite")}</p>
        </div>
      {/if}

      <div class="block">
        {#if error}<p class="error">{error}</p>{/if}
        {#if needsDest}<p class="muted small">{t("transcode.dest.needed")}</p>{/if}
        <button class="start" disabled={!canStart} onclick={launch}>
          {t(preset?.analysis ? "transcode.start.analyze" : "transcode.start")}{sources.length ? ` (${sources.length})` : ""}
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
    max-height: 220px;
    overflow-y: auto;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
  }
  .files li {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    padding: 2px var(--vf-space-2);
    font-size: var(--vf-text-sm);
    border-bottom: 1px solid var(--vf-border);
  }
  .files li:last-child {
    border-bottom: 0;
  }
  .files .name {
    flex-shrink: 0;
    max-width: 60%;
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
  }
  .x:hover {
    color: var(--vf-error);
  }
  .x svg {
    width: 10px;
    height: 10px;
  }
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
  .grid input,
  .grid select {
    width: 100%;
    min-width: 0;
  }
  .wide {
    grid-column: 1 / -1;
  }
  .grid label.check {
    flex-direction: row;
    align-items: center;
    gap: var(--vf-space-1);
    font-size: var(--vf-text-sm);
    color: var(--vf-text);
  }
  .grid label.check input {
    width: auto;
    accent-color: var(--vf-accent);
  }
  .path {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--vf-text-sm);
  }
  input,
  select {
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-size: var(--vf-text-sm);
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
