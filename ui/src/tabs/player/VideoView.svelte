<!-- PLAYER VIDEO : lecture précise à l'image, shuttle J/K/L, points d'entrée et de sortie. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import Transport from "../../components/Transport.svelte";
  import { app } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import { resolvePlayerAction } from "../../shortcuts";
  import { framesToTc, fps, type FrameRate } from "../../lib/timecode";
  import { pickVideo, videoOpen, videoFrame, videoClose, type VideoClip, type Slot } from "../../lib/player";

  // `slot` : emplacement du lecteur ; `path` : clip à ouvrir directement ;
  // `active` : vrai quand les raccourcis clavier doivent agir sur cette vue.
  let {
    slot = "player",
    path = null,
    active = () => app.tab === "player" && app.mode === "video",
  }: { slot?: Slot; path?: string | null; active?: () => boolean } = $props();

  let clip = $state<VideoClip | null>(null);
  let loading = $state(false);
  let shown = $state(0); // image affichée
  let speed = $state(0); // 0 = pause, négatif = arrière
  let markIn = $state<number | null>(null);
  let markOut = $state<number | null>(null);
  let displayFps = $state(0);
  let ipcMs = $state(0); // temps de transfert d'une image depuis le cœur
  let drawMs = $state(0); // temps de dessin
  let canvas = $state<HTMLCanvasElement | null>(null);

  let position = 0; // position fractionnaire (images) pendant la lecture
  let requested = 0; // dernière image demandée (sert de base au pas à pas)
  let inFlight = false;
  let pending: number | null = null;
  let timer = 0;
  let lastTs = 0;
  let fpsCount = 0;
  let fpsSince = performance.now();

  const rate = $derived<FrameRate>(clip?.info.video?.rate ?? { num: 25, den: 1 });
  const dropFrame = $derived(clip?.info.start_timecode?.includes(";") ?? false);
  const last = $derived(Math.max(0, (clip?.frame_count ?? 1) - 1));
  const tc = (i: number) => framesToTc((clip?.start_frame ?? 0) + i, rate, dropFrame);

  async function open() {
    const picked = await pickVideo(t("player.filter.video"));
    if (picked) await load(picked);
  }

  async function load(path: string) {
    stop();
    loading = true;
    try {
      clip = await videoOpen(path, slot);
      markIn = markOut = null;
      await show(0);
      app.status = path;
    } catch (err) {
      app.status = String(err);
    } finally {
      loading = false;
    }
  }

  /** Affiche l'image `i` ; une seule requête à la fois, la plus récente est conservée. */
  async function show(i: number) {
    if (!clip) return;
    i = Math.min(Math.max(0, Math.round(i)), last);
    requested = i;
    if (inFlight) {
      pending = i;
      return;
    }
    inFlight = true;
    try {
      const t0 = performance.now();
      const buf = await videoFrame(i, slot);
      const t1 = performance.now();
      // Décodage JPEG natif du moteur web (rapide et asynchrone).
      const bitmap = await createImageBitmap(new Blob([buf], { type: "image/jpeg" }));
      canvas?.getContext("2d")?.drawImage(bitmap, 0, 0);
      bitmap.close();
      const t2 = performance.now();
      ipcMs = ipcMs * 0.8 + (t1 - t0) * 0.2;
      drawMs = drawMs * 0.8 + (t2 - t1) * 0.2;
      shown = i;
      fpsCount++;
      const now = performance.now();
      if (now - fpsSince > 1000) {
        displayFps = (fpsCount * 1000) / (now - fpsSince);
        fpsCount = 0;
        fpsSince = now;
      }
    } catch (err) {
      app.status = String(err);
    } finally {
      inFlight = false;
    }
    if (pending !== null && pending !== shown) {
      const next = pending;
      pending = null;
      await show(next);
    }
    pending = null;
  }

  // Horloge de lecture : minuterie courte plutôt que requestAnimationFrame,
  // qui peut être suspendu par certains systèmes quand la fenêtre n'a pas le focus.
  function tick() {
    if (speed === 0) return;
    const ts = performance.now();
    const dt = lastTs ? (ts - lastTs) / 1000 : 0;
    lastTs = ts;
    position += dt * fps(rate) * speed;
    // Fin de clip (en avant) ou début (en arrière) : arrêt sur la dernière image.
    if ((speed > 0 && position >= last) || (speed < 0 && position <= 0)) {
      position = Math.min(Math.max(position, 0), last);
      show(position);
      speed = 0;
      return;
    }
    // On ne demande une nouvelle image que lorsque la précédente est affichée :
    // si le décodage ne suit pas, des images sont sautées mais le temps reste juste.
    if (!inFlight && Math.round(position) !== shown) show(position);
    timer = window.setTimeout(tick, 4);
  }

  function setSpeed(s: number) {
    const wasStopped = speed === 0;
    speed = s;
    if (s !== 0 && wasStopped) {
      position = shown;
      lastTs = 0;
      fpsCount = 0;
      fpsSince = performance.now();
      timer = window.setTimeout(tick, 0);
    }
  }

  function pause() {
    speed = 0;
    window.clearTimeout(timer);
  }

  function stop() {
    pause();
    if (clip) show(0);
  }

  function step(delta: number) {
    pause();
    show(requested + delta);
  }

  const toggle = () => (speed === 0 ? setSpeed(1) : pause());
  const forward = () => setSpeed(speed <= 0 ? 1 : Math.min(speed * 2, 8));
  const back = () => setSpeed(speed >= 0 ? -1 : Math.max(speed * 2, -8));

  function onKeydown(e: KeyboardEvent) {
    if (!clip || !active()) return;
    const action = resolvePlayerAction(e);
    if (!action) return;
    e.preventDefault();
    switch (action) {
      case "play.toggle": toggle(); break;
      case "play.stop": stop(); break;
      case "shuttle.forward": forward(); break;
      case "shuttle.back": back(); break;
      case "shuttle.pause": pause(); break;
      case "step.forward": step(1); break;
      case "step.back": step(-1); break;
      case "mark.in": markIn = shown; break;
      case "mark.out": markOut = shown; break;
    }
  }

  $effect(() => {
    if (path) load(path);
  });

  onDestroy(() => {
    pause();
    videoClose(slot).catch(() => {});
  });
</script>

<svelte:window onkeydown={onKeydown} />

{#if !clip}
  <div class="empty">
    <button class="open" onclick={open} disabled={loading}>{loading ? t("player.loading") : t("player.open.video")}</button>
    <p>{t("player.open.video.hint")}</p>
    <p class="hint">{t("player.shortcuts")}</p>
  </div>
{:else}
  <div class="video">
    <div class="screen">
      <canvas bind:this={canvas} width={clip.display_width} height={clip.display_height}></canvas>
    </div>

    <div class="bar">
      <span class="tc mono">{tc(shown)}</span>
      <input
        class="scrub"
        type="range"
        min="0"
        max={last}
        value={shown}
        oninput={(e) => {
          pause();
          show(Number(e.currentTarget.value));
        }}
      />
      <span class="mono small">{t("player.frame")} {shown} / {last}</span>
    </div>

    <div class="controls">
      <Transport playing={speed !== 0} {speed} onToggle={toggle} onStop={stop} onBack={back} onForward={forward} />
      <div class="marks mono">
        <span>{t("player.in")} {markIn === null ? "--:--:--:--" : tc(markIn)}</span>
        <span>{t("player.out")} {markOut === null ? "--:--:--:--" : tc(markOut)}</span>
        {#if markIn !== null && markOut !== null && markOut >= markIn}
          <span>{t("player.duration")} {framesToTc(markOut - markIn + 1, rate, dropFrame)}</span>
        {/if}
      </div>
      <button class="change" onclick={open}>{t("player.change")}</button>
    </div>

    <dl class="info">
      <dt>{t("player.codec")}</dt><dd>{clip.info.video?.codec} ({clip.info.video?.pix_fmt})</dd>
      <dt>{t("player.resolution")}</dt><dd>{clip.info.video?.width} x {clip.info.video?.height}</dd>
      <dt>{t("player.rate")}</dt><dd>{fps(rate).toFixed(3)} i/s{dropFrame ? " DF" : ""}</dd>
      <dt>{t("player.tc.start")}</dt><dd class="mono">{clip.info.start_timecode ?? tc(0)}</dd>
      <dt>{t("player.display.fps")}</dt>
      <dd class="mono" title="transfert / décodage et dessin">
        {speed !== 0 ? displayFps.toFixed(1) : "-"} i/s ({ipcMs.toFixed(0)} + {drawMs.toFixed(0)} ms)
      </dd>
    </dl>
  </div>
{/if}

<style>
  .empty {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--vf-space-3);
    color: var(--vf-text-muted);
    text-align: center;
    padding: var(--vf-space-6);
  }
  .hint {
    font-size: var(--vf-text-sm);
    color: var(--vf-text-disabled);
  }
  .open,
  .change {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-md);
    padding: var(--vf-space-2) var(--vf-space-4);
    font-weight: 600;
    cursor: pointer;
  }
  .change {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    font-weight: 400;
    margin-left: auto;
  }
  .video {
    display: grid;
    grid-template-rows: 1fr auto auto auto;
    height: 100%;
    gap: var(--vf-space-2);
    padding: var(--vf-space-3);
  }
  .screen {
    min-height: 0;
    background: var(--vf-video-bg);
    border-radius: var(--vf-radius-md);
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  canvas {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
  }
  .bar,
  .controls {
    display: flex;
    align-items: center;
    gap: var(--vf-space-4);
  }
  .tc {
    font-size: var(--vf-text-timecode);
    color: var(--vf-accent);
    min-width: 11ch;
  }
  .scrub {
    flex: 1;
    accent-color: var(--vf-accent);
  }
  .small {
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  .marks {
    display: flex;
    gap: var(--vf-space-4);
    font-size: var(--vf-text-sm);
    color: var(--vf-mark);
  }
  .info {
    display: grid;
    grid-template-columns: repeat(5, auto 1fr);
    gap: var(--vf-space-1) var(--vf-space-2);
    margin: 0;
    font-size: var(--vf-text-sm);
  }
  dt {
    color: var(--vf-text-muted);
  }
  dd {
    margin: 0;
  }
</style>
