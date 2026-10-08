<!-- PLAYER VIDEO : lecture précise à l'image, shuttle J/K/L, points d'entrée et de sortie. -->
<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import Transport from "../../components/Transport.svelte";
  import OutputPicker from "../../components/OutputPicker.svelte";
  import MarkersPanel from "./MarkersPanel.svelte";
  import SoundTracks from "./SoundTracks.svelte";
  import { app } from "../../stores/app.svelte";
  import { output, currentOutput } from "../../stores/output.svelte";
  import { t } from "../../i18n/index.svelte";
  import { resolvePlayerAction } from "../../shortcuts";
  import { framesToTc, fps, type FrameRate } from "../../lib/timecode";
  import { markerCss, type Marker } from "../../lib/logs";
  import {
    pickVideo,
    pickLut,
    videoOpen,
    videoFrame,
    videoClose,
    videoLut,
    drawJpeg,
    audioOpen,
    audioClose,
    audioSeek,
    audioTransport,
    audioStatus,
    audioLtcScan,
    type LtcDetection,
    type VideoClip,
    type Slot,
    type AudioSlot,
  } from "../../lib/player";

  // `slot` : emplacement du lecteur ; `path` : clip à ouvrir directement ;
  // `active` : vrai quand les raccourcis clavier doivent agir sur cette vue ;
  // `logs` : affiche le panneau des marqueurs.
  let {
    slot = "player",
    path = null,
    active = () => app.tab === "player" && app.mode === "video",
    logs = true,
  }: { slot?: Slot; path?: string | null; active?: () => boolean; logs?: boolean } = $props();

  const avSlot = $derived(`${slot}-av` as AudioSlot);
  const PREF_TC = "veriflow.player.tc.overlay";

  let clip = $state<VideoClip | null>(null);
  let clipPath = $state<string | null>(null);
  let loading = $state(false);
  let shown = $state(0); // image affichée
  let speed = $state(0); // 0 = pause, négatif = arrière
  let markIn = $state<number | null>(null);
  let markOut = $state<number | null>(null);
  let displayFps = $state(0);
  let ipcMs = $state(0); // temps de transfert d'une image depuis le cœur
  let drawMs = $state(0); // temps de dessin
  let canvas = $state<HTMLCanvasElement | null>(null);
  let markers = $state<Marker[]>([]);
  let panel = $state<{ add: () => Promise<void> } | null>(null);
  let lut = $state<string | null>(null);
  let tcOverlay = $state(readPref(PREF_TC) === "1");

  // Son de la vidéo : ouvert dans le moteur audio, il sert d'horloge en lecture normale.
  let avReady = $state(false);
  let avError = $state("");
  let soundOn = $state(true);
  let audioPlaying = false;
  let audioPos = 0; // position du son (s) au moment `audioAt`
  let audioAt = 0;
  let audioPolling = false;
  let lastAudioPoll = 0;
  // Le son ne sert d'horloge qu'une fois qu'il avance réellement : sinon
  // (sortie muette, pilote bloqué) l'image resterait figée sur sa première image.
  let audioTrusted = false;
  let audioStartSecs = 0;
  let audioStartAt = 0;
  // Au premier lancement, FFmpeg (et l'antivirus sous Windows) peut mettre
  // quelques secondes à démarrer : l'image attend le son jusqu'à ce délai.
  const AUDIO_STALL_MS = 5000;
  let avTracks = $state<string[]>([]);
  let avLtc = $state<(LtcDetection | null)[]>([]);
  let trackPeaks = $state<(number | null)[]>([]);

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
  const hasSound = $derived((clip?.info.audio.length ?? 0) > 0);
  const lutName = $derived(lut?.split(/[\\/]/).pop() ?? "");

  function readPref(k: string): string | null {
    try {
      return localStorage.getItem(k);
    } catch {
      return null;
    }
  }
  function writePref(k: string, v: string) {
    try {
      localStorage.setItem(k, v);
    } catch {
      /* stockage indisponible */
    }
  }

  async function open() {
    const picked = await pickVideo(t("player.filter.video"));
    if (picked) await load(picked);
  }

  async function load(p: string) {
    stop();
    loading = true;
    try {
      await closeSound();
      clip = await videoOpen(p, slot);
      clipPath = p;
      markIn = markOut = null;
      if (lut) await videoLut(lut, slot).catch((e) => ((app.status = String(e)), (lut = null)));
      await show(0);
      app.status = p;
      openSound(p);
    } catch (err) {
      app.status = String(err);
    } finally {
      loading = false;
    }
  }

  async function openSound(p: string) {
    avReady = false;
    avError = "";
    if (!clip || clip.info.audio.length === 0) return;
    try {
      const opened = await audioOpen([p], avSlot, currentOutput());
      if (clipPath !== p) return;
      avTracks = opened.session.tracks.map((tr) => tr.name);
      avLtc = [];
      avReady = true;
      // Timecode LTC sur une piste ? Elle sera coupée (protection de l'écoute).
      audioLtcScan([p])
        .then((r) => {
          if (clipPath === p) avLtc = r;
        })
        .catch(() => {});
    } catch (e) {
      avError = String(e);
    }
  }

  async function closeSound() {
    stopAudio();
    if (avReady) await audioClose(avSlot).catch(() => {});
    avReady = false;
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
      // Décodage JPEG natif du moteur web.
      await drawJpeg(canvas?.getContext("2d"), buf);
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

  // ---------- Son synchronisé ----------

  const audioClock = () => avReady && soundOn && speed === 1;

  async function startAudio(fromFrame: number) {
    if (!audioClock() || audioPlaying) return;
    audioPlaying = true;
    audioTrusted = false;
    const secs = fromFrame / fps(rate);
    audioPos = secs;
    audioAt = performance.now();
    audioStartSecs = secs;
    audioStartAt = audioAt;
    try {
      await audioSeek(secs, avSlot);
      await audioTransport("play", avSlot);
      audioAt = performance.now();
    } catch (e) {
      audioPlaying = false;
      app.status = String(e);
    }
  }

  function stopAudio() {
    if (!audioPlaying) return;
    audioPlaying = false;
    audioTransport("pause", avSlot).catch(() => {});
  }

  /** Relève la position réelle du son (au plus 10 fois par seconde). */
  function pollAudio(now: number) {
    if (audioPolling || now - lastAudioPoll < 100) return;
    audioPolling = true;
    lastAudioPoll = now;
    const asked = performance.now();
    audioStatus(avSlot)
      .then((st) => {
        if (!audioPlaying) return;
        const at = performance.now();
        // La position a été lue pendant l'aller-retour : datée au milieu, pour
        // ne pas dépendre du temps de réponse (variable pendant la lecture).
        const stamp = (asked + at) / 2;
        trackPeaks = st.track_peaks;
        if (!audioTrusted) {
          // Le son est parti si sa position avance depuis le point de départ,
          // sans dépasser ce qui a pu être joué depuis (position périmée).
          const elapsed = (at - audioStartAt) / 1000;
          if (st.playing && st.position > audioStartSecs + 0.02 && st.position < audioStartSecs + elapsed + 0.5) {
            if (avError === t("player.sound.stalled")) avError = "";
            audioTrusted = true;
            audioPos = st.position;
            audioAt = stamp;
          } else if (at - audioStartAt > AUDIO_STALL_MS) {
            // Le son n'avance pas : lecture poursuivie sur l'horloge de l'image.
            stopAudio();
            avError = t("player.sound.stalled");
            app.status = avError;
          }
          return;
        }
        if (!st.playing) return;
        // Horloge lissée : un petit écart est rattrapé en douceur, un grand
        // écart (plus d'une demi-seconde) est repris tel quel.
        const predicted = audioPos + (stamp - audioAt) / 1000;
        const err = st.position - predicted;
        audioPos = Math.abs(err) > 0.5 ? st.position : predicted + err * 0.2;
        audioAt = stamp;
      })
      .catch(() => {})
      .finally(() => (audioPolling = false));
  }

  // Horloge de lecture : minuterie courte plutôt que requestAnimationFrame,
  // qui peut être suspendu par certains systèmes quand la fenêtre n'a pas le focus.
  // En lecture normale avec le son, la position suit l'horloge de la carte son.
  function tick() {
    if (speed === 0) return;
    const ts = performance.now();
    const dt = lastTs ? (ts - lastTs) / 1000 : 0;
    lastTs = ts;
    if (audioPlaying) pollAudio(ts);
    if (audioPlaying && !audioTrusted) {
      // Le son n'a pas encore démarré : l'image l'attend, pour partir synchrone.
      timer = window.setTimeout(tick, 4);
      return;
    }
    if (audioPlaying && audioTrusted) {
      // Pas de recul pour une correction de quelques millisecondes : l'image
      // attend simplement que l'horloge la rattrape.
      const p = (audioPos + (ts - audioAt) / 1000) * fps(rate);
      if (p > position || position - p > fps(rate) / 2) position = p;
    } else {
      position += dt * fps(rate) * speed;
    }
    // Fin de clip (en avant) ou début (en arrière) : arrêt sur la dernière image.
    if ((speed > 0 && position >= last) || (speed < 0 && position <= 0)) {
      position = Math.min(Math.max(position, 0), last);
      show(position);
      speed = 0;
      stopAudio();
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
    if (audioClock()) startAudio(Math.round(position));
    else stopAudio();
  }

  function pause() {
    speed = 0;
    window.clearTimeout(timer);
    stopAudio();
  }

  function stop() {
    pause();
    if (clip) show(0);
  }

  function step(delta: number) {
    pause();
    show(requested + delta);
  }

  function seek(frame: number) {
    pause();
    show(frame);
  }

  const toggle = () => (speed === 0 ? setSpeed(1) : pause());
  const forward = () => setSpeed(speed <= 0 ? 1 : Math.min(speed * 2, 8));
  const back = () => setSpeed(speed >= 0 ? -1 : Math.max(speed * 2, -8));

  function toggleSound() {
    soundOn = !soundOn;
    if (soundOn && speed === 1) {
      position = shown;
      startAudio(shown);
    } else stopAudio();
  }

  // ---------- LUT et TC incrusté ----------

  async function chooseLut() {
    const picked = await pickLut(t("player.lut.filter"));
    if (!picked || !clip) return;
    try {
      await videoLut(picked, slot);
      lut = picked;
      show(shown);
    } catch (e) {
      app.status = String(e);
    }
  }

  async function removeLut() {
    lut = null;
    if (!clip) return;
    await videoLut(null, slot).catch(() => {});
    show(shown);
  }

  function toggleTc() {
    tcOverlay = !tcOverlay;
    writePref(PREF_TC, tcOverlay ? "1" : "0");
  }

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
      // Marqueurs, entrée et sortie : seulement dans l'onglet PLAYER (pas dans le lecteur rapide).
      case "mark.in": if (logs) markIn = shown; break;
      case "mark.out": if (logs) markOut = shown; break;
      case "mark.add": if (logs) panel?.add(); break;
    }
  }

  // Ouverture au changement de chemin uniquement : sans `untrack`, l'effet
  // dépendrait aussi du clip lu dans `load` et rouvrirait le fichier en boucle
  // (chaque réouverture arrêtant la lecture).
  $effect(() => {
    const p = path;
    if (p) untrack(() => load(p));
  });

  // Changement de sortie audio : le son de la vidéo est rouvert sur la nouvelle sortie.
  $effect(() => {
    void output.revision;
    untrack(() => {
      if (clipPath && hasSound) {
        const p = clipPath;
        closeSound().then(() => openSound(p));
      }
    });
  });

  onDestroy(() => {
    pause();
    videoClose(slot).catch(() => {});
    audioClose(avSlot).catch(() => {});
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
  <div class="layout" class:withlogs={logs}>
  <div class="video">
    <div class="viewer">
      <div class="screen">
        <canvas bind:this={canvas} width={clip.display_width} height={clip.display_height}></canvas>
        {#if tcOverlay}<div class="tcover mono">{tc(shown)}</div>{/if}
      </div>
      {#if avReady}
        <SoundTracks slot={avSlot} names={avTracks} ltc={avLtc} peaks={trackPeaks} />
      {/if}
    </div>

    <div class="bar">
      <span class="tc mono">{tc(shown)}</span>
      <div class="scrubwrap">
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
        {#each markers as m (m.id)}
          {#if m.in_frame !== null && m.out_frame !== null}
            <span
              class="span"
              style:left={`${(Math.min(m.in_frame, m.out_frame) / Math.max(1, last)) * 100}%`}
              style:width={`${(Math.abs(m.out_frame - m.in_frame) / Math.max(1, last)) * 100}%`}
              style:background={markerCss(m.color)}
            ></span>
          {:else}
            <span class="tick" style:left={`${(m.frame / Math.max(1, last)) * 100}%`} style:background={markerCss(m.color)}></span>
          {/if}
        {/each}
      </div>
      <span class="mono small">{t("player.frame")} {shown} / {last}</span>
    </div>

    <div class="controls">
      <Transport playing={speed !== 0} {speed} onToggle={toggle} onStop={stop} onBack={back} onForward={forward} />
      {#if logs}
        <div class="marks mono">
        <span>{t("player.in")} {markIn === null ? "--:--:--:--" : tc(markIn)}</span>
        <span>{t("player.out")} {markOut === null ? "--:--:--:--" : tc(markOut)}</span>
        {#if markIn !== null && markOut !== null && markOut >= markIn}
          <span>{t("player.duration")} {framesToTc(markOut - markIn + 1, rate, dropFrame)}</span>
        {/if}
      </div>
      {/if}
      <div class="tools">
        {#if hasSound}
          <button class="opt" class:on={soundOn && avReady} onclick={toggleSound} title={avError || t("player.sound.hint")} disabled={!avReady}>
            {t("player.sound")}
          </button>
        {/if}
        <button class="opt" class:on={tcOverlay} onclick={toggleTc} title={t("player.tc.overlay.hint")}>{t("player.tc.overlay")}</button>
        {#if lut}
          <span class="lut" title={lut}>LUT {lutName}</span>
          <button class="opt" onclick={removeLut} aria-label={t("player.lut.remove")} title={t("player.lut.remove")}>✕</button>
        {:else}
          <button class="opt" onclick={chooseLut} title={t("player.lut.hint")}>LUT</button>
        {/if}
      </div>
      <button class="change" onclick={open}>{t("player.change")}</button>
    </div>
    {#if hasSound}
      <div class="sound">
        <OutputPicker />
        {#if avError}<span class="err">{avError}</span>{/if}
      </div>
    {/if}

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
  {#if logs}
    <MarkersPanel
      bind:this={panel}
      path={clipPath}
      frame={shown}
      {tc}
      {markIn}
      {markOut}
      onSeek={seek}
      onRangeUsed={() => (markIn = markOut = null)}
      onChange={(m) => (markers = m)}
    />
  {/if}
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
  }
  .layout {
    display: grid;
    grid-template-columns: 1fr;
    height: 100%;
    min-height: 0;
  }
  .layout.withlogs {
    grid-template-columns: 1fr 300px;
    gap: var(--vf-space-2);
    padding-right: var(--vf-space-3);
    padding-block: var(--vf-space-3);
  }
  .layout.withlogs .video {
    padding: 0 0 0 var(--vf-space-3);
  }
  .video {
    display: grid;
    grid-template-rows: 1fr;
    grid-auto-rows: auto;
    height: 100%;
    min-height: 0;
    gap: var(--vf-space-2);
    padding: var(--vf-space-3);
  }
  /* Image et pistes son côte à côte ; le message LTC se pose sur l'image. */
  .viewer {
    position: relative;
    display: flex;
    gap: var(--vf-space-2);
    min-height: 0;
  }
  .screen {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    background: var(--vf-video-bg);
    border-radius: var(--vf-radius-md);
    overflow: hidden;
  }
  /* Image ajustée à la zone disponible, sans jamais l'agrandir : en
     position absolue, sa taille réelle ne pousse pas la mise en page
     (indispensable avec le moteur web de macOS Catalina). */
  canvas {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
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
  .scrubwrap {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
  }
  .scrub {
    flex: 1;
    accent-color: var(--vf-accent);
  }
  .tick,
  .span {
    position: absolute;
    top: -6px;
    height: 5px;
    pointer-events: none;
    border-radius: 1px;
  }
  .tick {
    width: 3px;
    margin-left: -1px;
  }
  .span {
    opacity: 0.8;
  }
  .tcover {
    position: absolute;
    bottom: var(--vf-space-3);
    left: 50%;
    transform: translateX(-50%);
    padding: 2px var(--vf-space-2);
    background: var(--vf-tc-overlay-bg);
    color: var(--vf-text);
    font-size: var(--vf-text-lg);
    border-radius: var(--vf-radius-sm);
    pointer-events: none;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    margin-left: auto;
  }
  .opt {
    background: var(--vf-surface-high);
    color: var(--vf-text-muted);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px var(--vf-space-2);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .opt.on {
    color: var(--vf-on-accent);
    background: var(--vf-accent);
    border-color: var(--vf-accent);
  }
  .opt:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .lut {
    font-size: var(--vf-text-sm);
    color: var(--vf-accent);
    max-width: 180px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sound {
    display: flex;
    align-items: center;
    gap: var(--vf-space-3);
  }
  .err {
    color: var(--vf-error);
    font-size: var(--vf-text-sm);
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
    min-width: 0;
    /* Une seule ligne : la hauteur ne change pas pendant la lecture. */
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
