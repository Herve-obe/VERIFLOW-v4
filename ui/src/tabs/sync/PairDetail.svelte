<!-- Détail d'une paire : formes d'onde du son témoin et du son de
     l'enregistreur superposées, décalage réglable, autre son, validation. -->
<script lang="ts">
  import { t } from "../../i18n/index.svelte";
  import { fps, refine, timecodeAt, waveforms, type Analysis, type Pair, type Waves } from "../../lib/sync";

  let { analysis = $bindable(), index }: { analysis: Analysis; index: number } = $props();

  const pair = $derived(analysis.pairs[index] as Pair);
  const video = $derived(analysis.videos[pair.video]);
  const audio = $derived(pair.audio === null ? null : analysis.audios[pair.audio]);
  const frame = $derived(1 / fps(video.rate));

  let length = $state(4);
  let at = $state(-1);
  let waves = $state<Waves | null>(null);
  let busy = $state(false);
  let error = $state("");
  let canvas = $state<HTMLCanvasElement | null>(null);

  // Zone commune (temps de la vidéo) et position par défaut au milieu.
  const common = $derived.by(() => {
    if (!audio) return null;
    const from = Math.max(0, pair.offset);
    const to = Math.min(video.duration, pair.offset + audio.duration);
    return to > from ? { from, to } : null;
  });
  const position = $derived(at >= 0 ? at : common ? common.from + Math.max(0, (common.to - common.from - length) / 2) : 0);

  let req = 0;
  $effect(() => {
    const a = audio;
    const off = pair.offset;
    const p = position;
    const len = length;
    waves = null;
    error = "";
    if (!a || video.channels === 0) return;
    const r = ++req;
    waveforms($state.snapshot(video), $state.snapshot(a), off, p, len, 800)
      .then((w) => r === req && (waves = w))
      .catch((e) => r === req && (error = String(e)));
  });

  // Dessin : son témoin en haut, enregistreur en bas, même échelle de temps.
  $effect(() => {
    const w = waves;
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    const css = getComputedStyle(document.documentElement);
    const color = (name: string) => css.getPropertyValue(name).trim();
    const W = (c.width = c.clientWidth * devicePixelRatio);
    const H = (c.height = c.clientHeight * devicePixelRatio);
    ctx.clearRect(0, 0, W, H);
    ctx.fillStyle = color("--vf-meter-bg");
    ctx.fillRect(0, 0, W, H);
    ctx.strokeStyle = color("--vf-border");
    ctx.beginPath();
    ctx.moveTo(0, H / 2);
    ctx.lineTo(W, H / 2);
    ctx.stroke();
    if (!w) return;
    const draw = (env: number[], top: boolean, fill: string) => {
      const peak = Math.max(1e-6, ...env);
      ctx.fillStyle = fill;
      const half = H / 2 - 4;
      env.forEach((v, i) => {
        const x = (i / env.length) * W;
        const h = (v / peak) * half;
        ctx.fillRect(x, top ? H / 2 - 2 - h : H / 2 + 2, Math.max(1, W / env.length), h);
      });
    };
    draw(w.video, true, color("--vf-accent-video"));
    draw(w.audio, false, color("--vf-accent-audio"));
  });

  function nudge(seconds: number) {
    pair.offset = Math.round((pair.offset + seconds) * 48000) / 48000;
    pair.method = "manual";
    pair.validated = false;
  }

  async function recompute() {
    if (!audio) return;
    busy = true;
    error = "";
    try {
      const r = await refine($state.snapshot(video), $state.snapshot(audio), pair.offset, 1.0);
      pair.offset = r.offset;
      pair.confidence = r.confidence;
      pair.refined = true;
      pair.method = pair.method === "manual" ? "waveform" : pair.method;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function chooseAudio(v: string) {
    pair.audio = v === "" ? null : Number(v);
    pair.validated = false;
    pair.confidence = null;
    pair.refined = false;
    const a = pair.audio === null ? null : analysis.audios[pair.audio];
    // Décalage par l'heure si les deux en ont une ; sinon début contre début.
    pair.offset = a && a.start !== null && video.start !== null ? a.start - video.start : 0;
    pair.method = a && a.start !== null && video.start !== null ? "timecode" : "manual";
    at = -1;
  }

  // Au-delà d'une seconde : en secondes, à 0,1 ms près.
  const ms = (s: number) =>
    `${s >= 0 ? "+" : ""}${Math.abs(s) >= 1 ? `${s.toFixed(4).replace(".", ",")} s` : `${(s * 1000).toFixed(1).replace(".", ",")} ms`}`;
  const frames = (s: number) => `${s >= 0 ? "+" : ""}${(s / frame).toFixed(2).replace(".", ",")}`;
</script>

<div class="detail">
  <div class="head">
    <h3>{video.name}</h3>
    <label class="tc-field">
      {t("sync.sound")}
      <select class="tc-input" value={pair.audio ?? ""} onchange={(e) => chooseAudio(e.currentTarget.value)}>
        <option value="">{t("sync.no.sound")}</option>
        {#each analysis.audios as a, i (a.path)}<option value={i}>{a.name}{a.timecode ? ` (${a.timecode})` : ""}</option>{/each}
      </select>
    </label>
  </div>

  {#if audio}
    <div class="offset">
      <span class="mono big">{ms(pair.offset)}</span>
      <span class="muted">({frames(pair.offset)} {t("sync.frames")})</span>
      <span class="muted">{t("sync.sound.starts")} {video.start !== null ? timecodeAt(video.start + pair.offset, video.rate) : ""}</span>
    </div>
    <div class="nudges">
      <button class="tc-btn" onclick={() => nudge(-frame)} title={t("sync.nudge.frame")}>−1 {t("sync.frame")}</button>
      <button class="tc-btn" onclick={() => nudge(-0.001)}>−1 ms</button>
      <button class="tc-btn" onclick={() => nudge(-1 / 48000)} title={t("sync.nudge.sample")}>−1 {t("sync.sample")}</button>
      <button class="tc-btn" onclick={() => nudge(1 / 48000)} title={t("sync.nudge.sample")}>+1 {t("sync.sample")}</button>
      <button class="tc-btn" onclick={() => nudge(0.001)}>+1 ms</button>
      <button class="tc-btn" onclick={() => nudge(frame)} title={t("sync.nudge.frame")}>+1 {t("sync.frame")}</button>
      <span class="spacer"></span>
      <button class="tc-btn" disabled={busy} onclick={recompute}>{t(busy ? "sync.recomputing" : "sync.recompute")}</button>
    </div>

    <canvas bind:this={canvas} aria-label={t("sync.waves")}></canvas>
    <div class="legend small">
      <span class="v">■ {t("sync.legend.camera")}</span>
      <span class="a">■ {t("sync.legend.recorder")}</span>
      <span class="spacer"></span>
      <label>
        {t("sync.zoom")}
        <select class="tc-input" bind:value={length}>
          {#each [0.5, 1, 2, 4, 10, 30] as l (l)}<option value={l}>{String(l).replace(".", ",")} s</option>{/each}
        </select>
      </label>
    </div>
    {#if common}
      <input
        type="range"
        min={common.from}
        max={Math.max(common.from, common.to - length)}
        step="0.04"
        value={position}
        oninput={(e) => (at = Number(e.currentTarget.value))}
        aria-label={t("sync.position")}
      />
    {:else}
      <p class="tc-warn small">{t("sync.no.common")}</p>
    {/if}
    {#if video.channels === 0}<p class="muted small">{t("sync.no.scratch")}</p>{/if}
    {#if error}<p class="tc-warn small">{error}</p>{/if}
    {#if pair.drift_frames !== null && Math.abs(pair.drift_frames) >= 1}
      <p class="tc-warn small">{t("sync.drift.warn")} {pair.drift_frames.toFixed(1).replace(".", ",")} {t("sync.frames")}</p>
    {/if}
    <label class="tc-check"><input type="checkbox" bind:checked={pair.validated} /> {t("sync.validate")}</label>
  {:else}
    <p class="muted">{pair.note ?? t("sync.no.sound.hint")}</p>
  {/if}
</div>

<style>
  .detail {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
  }
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--vf-space-3);
  }
  .head .tc-field {
    min-width: 260px;
  }
  h3 {
    margin: 0;
    font-size: var(--vf-text-md);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .offset {
    display: flex;
    align-items: baseline;
    gap: var(--vf-space-2);
    flex-wrap: wrap;
  }
  .big {
    font-size: var(--vf-text-lg);
    font-weight: 700;
  }
  .nudges,
  .legend {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    flex-wrap: wrap;
  }
  .spacer {
    flex: 1;
  }
  canvas {
    width: 100%;
    height: 140px;
    border-radius: var(--vf-radius-sm);
    border: 1px solid var(--vf-border);
  }
  input[type="range"] {
    width: 100%;
    accent-color: var(--vf-accent);
  }
  .v {
    color: var(--vf-accent-video);
  }
  .a {
    color: var(--vf-accent-audio);
  }
  .legend label {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    color: var(--vf-text-muted);
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .small {
    font-size: var(--vf-text-xs);
  }
  p {
    margin: 0;
  }
</style>
