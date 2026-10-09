<!-- PLAYER AUDIO : multipiste jusqu'à 32 pistes, SOLO / MUTE / niveau / panoramique, crêtes et LUFS. -->
<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import Meter from "../../components/Meter.svelte";
  import Fader from "../../components/Fader.svelte";
  import Transport from "../../components/Transport.svelte";
  import OutputPicker from "../../components/OutputPicker.svelte";
  import MarkersPanel from "./MarkersPanel.svelte";
  import ReportPopup from "./ReportPopup.svelte";
  import LtcNotice from "./LtcNotice.svelte";
  import { output, currentOutput } from "../../stores/output.svelte";
  import { AUDIO_LOG_FPS, markerCss, type Marker } from "../../lib/logs";
  import { app } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import { resolvePlayerAction } from "../../shortcuts";
  import { secondsToClock, framesToTc } from "../../lib/timecode";
  import {
    pickAudio,
    audioOpen,
    audioClose,
    audioTransport,
    audioSeek,
    audioTrack,
    audioStatus,
    audioLtcScan,
    type LtcDetection,
    type AudioOpened,
    type Slot,
  } from "../../lib/player";

  let {
    slot = "player",
    paths: initialPaths = null,
    active = () => app.tab === "player" && app.mode === "audio",
    logs = true,
  }: { slot?: Slot; paths?: string[] | null; active?: () => boolean; logs?: boolean } = $props();

  interface Strip {
    name: string;
    gain: number;
    pan: number;
    mute: boolean;
    solo: boolean;
    level: number | null;
    hold: number | null;
    holdAt: number;
  }

  const FALL_DB_PER_S = 20; // retombée des vumètres
  const HOLD_MS = 1500; // maintien de crête

  let session = $state<AudioOpened | null>(null);
  let strips = $state<Strip[]>([]);
  let master = $state({ gain: 0, l: null as number | null, r: null as number | null, holdL: null as number | null, holdR: null as number | null, holdAt: 0 });
  let lufs = $state({ m: null as number | null, s: null as number | null, i: null as number | null });
  let playing = $state(false);
  let position = $state(0);
  let loading = $state(false);
  let timer = 0;
  let lastPoll = performance.now();
  let loaded = $state<string[]>([]);
  let markers = $state<Marker[]>([]);
  let markIn = $state<number | null>(null);
  let markOut = $state<number | null>(null);
  let panel = $state<{ add: () => Promise<void> } | null>(null);
  // Saisie du rapport (touche R).
  let reportOpen = $state(false);


  const duration = $derived(session?.session.duration ?? 0);
  const first = $derived(session?.session.files[0]);
  const tcStart = $derived(first?.time_reference != null ? first.time_reference / first.sample_rate : null);

  // Marqueurs sur une grille de 25 i/s (comme le cœur), TC de départ BWF.
  const LOG_RATE = { num: AUDIO_LOG_FPS, den: 1 };
  const logFrame = $derived(Math.round(position * AUDIO_LOG_FPS));
  const lastFrame = $derived(Math.max(1, Math.round(duration * AUDIO_LOG_FPS)));
  const startFrame = $derived(Math.round((tcStart ?? 0) * AUDIO_LOG_FPS));
  const tc = (f: number) => framesToTc(startFrame + f, LOG_RATE, false);

  async function open() {
    const picked = await pickAudio(t("player.filter.audio"));
    if (picked.length > 0) await load(picked);
  }

  // Pistes portant un timecode LTC : coupées d'office pour protéger l'écoute.
  let ltc = $state<(LtcDetection | null)[]>([]);
  let ltcNotice = $state(false);
  const ltcTracks = $derived(ltc.map((d, i) => (d ? i : -1)).filter((i) => i >= 0));

  function scanLtc(paths: string[]) {
    ltc = [];
    ltcNotice = false;
    audioLtcScan(paths)
      .then((r) => {
        if (loaded !== paths) return;
        ltc = r;
        r.forEach((d, i) => {
          if (d && strips[i]) {
            strips[i].mute = true;
            send(i);
          }
        });
        ltcNotice = r.some((d) => d);
      })
      .catch(() => {});
  }

  function setLtc(mode: "mute" | "low" | "on") {
    for (const i of ltcTracks) {
      strips[i].mute = mode === "mute";
      strips[i].gain = mode === "low" ? -30 : 0;
      send(i);
    }
    ltcNotice = false;
  }

  async function load(paths: string[], scan = true) {
    loading = true;
    try {
      await close();
      session = await audioOpen(paths, slot, currentOutput());
      loaded = paths;
      markIn = markOut = null;
      strips = session.session.tracks.map((tr) => ({
        name: tr.name,
        gain: 0,
        pan: 0,
        mute: false,
        solo: false,
        level: null,
        hold: null,
        holdAt: 0,
      }));
      position = 0;
      timer = window.setInterval(poll, 33);
      app.status = paths.join(", ");
      if (scan) scanLtc(paths);
    } catch (err) {
      app.status = String(err);
    } finally {
      loading = false;
    }
  }

  async function close() {
    window.clearInterval(timer);
    if (session) await audioClose(slot).catch(() => {});
    session = null;
  }

  /** Retombée progressive + maintien de crête, comme un crêtemètre matériel. */
  function ballistics(prev: number | null, peak: number | null, dt: number): number | null {
    const fallen = prev === null ? null : prev - FALL_DB_PER_S * dt;
    const value = Math.max(peak ?? -Infinity, fallen ?? -Infinity);
    return value < -60 ? null : value;
  }

  async function poll() {
    try {
      const s = await audioStatus(slot);
      const now = performance.now();
      const dt = (now - lastPoll) / 1000;
      lastPoll = now;
      playing = s.playing;
      position = s.position;
      s.track_peaks.forEach((p, i) => {
        const st = strips[i];
        if (!st) return;
        st.level = ballistics(st.level, p, dt);
        if (p !== null && (st.hold === null || p >= st.hold || now - st.holdAt > HOLD_MS)) {
          st.hold = p;
          st.holdAt = now;
        } else if (st.hold !== null && now - st.holdAt > HOLD_MS) {
          st.hold = null;
        }
      });
      master.l = ballistics(master.l, s.master_peaks[0], dt);
      master.r = ballistics(master.r, s.master_peaks[1], dt);
      const mp = Math.max(s.master_peaks[0] ?? -Infinity, s.master_peaks[1] ?? -Infinity);
      if (Number.isFinite(mp) && (master.holdL === null || mp >= master.holdL || now - master.holdAt > HOLD_MS)) {
        master.holdL = master.holdR = mp;
        master.holdAt = now;
      }
      lufs = { m: s.lufs_momentary, s: s.lufs_short_term, i: s.lufs_integrated };
    } catch {
      /* session fermée entre-temps */
    }
  }

  const send = (i: number) => {
    const s = strips[i];
    audioTrack(i, s.gain <= -60 ? -200 : s.gain, s.pan, s.mute, s.solo, slot).catch((e) => (app.status = String(e)));
  };
  const sendMaster = () => audioTrack(-1, master.gain <= -60 ? -200 : master.gain, 0, false, false, slot);

  const toggle = () => audioTransport(playing ? "pause" : "play", slot).then(() => (playing = !playing));
  const stop = () => audioTransport("stop", slot).then(() => (playing = false));
  const seekBy = (s: number) => audioSeek(Math.min(Math.max(0, position + s), duration), slot);

  function onKeydown(e: KeyboardEvent) {
    if (!session || !active()) return;
    const action = resolvePlayerAction(e);
    if (!action) return;
    e.preventDefault();
    switch (action) {
      case "play.toggle": toggle(); break;
      case "play.stop": stop(); break;
      case "shuttle.forward": audioTransport("play", slot); break;
      case "shuttle.pause": audioTransport("pause", slot); break;
      case "shuttle.back": seekBy(-5); break;
      case "step.forward": seekBy(1); break;
      case "step.back": seekBy(-1); break;
      case "mark.in": markIn = logFrame; break;
      case "mark.out": markOut = logFrame; break;
      case "mark.add": panel?.add(); break;
      case "report.edit": if (logs && loaded[0]) reportOpen = true; break;
    }
  }

  const fmt = (v: number | null) => (v === null ? "-inf" : v.toFixed(1));
  const fmtDb = (v: number) => (v <= -60 ? "-inf" : (v > 0 ? "+" : "") + v.toFixed(1));

  /** Rouvre la session sur une autre sortie en gardant position et réglages. */
  async function reopen() {
    if (loaded.length === 0) return;
    const keep = strips.map((s) => ({ ...s }));
    const gain = master.gain;
    const at = position;
    // Même fichiers : l'analyse LTC et les réglages de l'utilisateur sont conservés.
    await load(loaded, false);
    if (!session) return;
    keep.forEach((k, i) => {
      if (!strips[i]) return;
      Object.assign(strips[i], { gain: k.gain, pan: k.pan, mute: k.mute, solo: k.solo });
      send(i);
    });
    master.gain = gain;
    sendMaster();
    audioSeek(at, slot);
  }

  $effect(() => {
    void output.revision;
    untrack(() => {
      if (session) reopen();
    });
  });

  // Ouverture au changement de fichiers uniquement (voir VideoView).
  $effect(() => {
    const p = initialPaths;
    if (p && p.length > 0) untrack(() => load(p));
  });

  onDestroy(close);
</script>

<svelte:window onkeydown={onKeydown} />

{#if reportOpen && loaded[0]}
  <ReportPopup kind="sound" path={loaded[0]} onClose={() => (reportOpen = false)} />
{/if}

{#if !session}
  <div class="empty">
    <button class="open" onclick={open} disabled={loading}>{loading ? t("player.loading") : t("player.open.audio")}</button>
    <p>{t("player.open.audio.hint")}</p>
    <p class="hint">{t("player.shortcuts")}</p>
  </div>
{:else}
  <div class="layout" class:withlogs={logs}>
  <div class="audio">
    <div class="top">
    {#if ltcNotice}
      <LtcNotice names={strips.map((st) => st.name)} {ltc} onChoice={setLtc} />
    {/if}
    <header>
      <div class="clock">
        <span class="tc mono">{secondsToClock((tcStart ?? 0) + position)}</span>
        <span class="small mono">{secondsToClock(position)} / {secondsToClock(duration)}</span>
      </div>
      <Transport {playing} onToggle={toggle} onStop={stop} />
      <div class="meta">
        {#if first?.ixml.scene}<span>{t("player.scene")} <b>{first.ixml.scene}</b></span>{/if}
        {#if first?.ixml.take}<span>{t("player.take")} <b>{first.ixml.take}</b></span>{/if}
        <span>{strips.length} {t("player.tracks")}, {session.session.sample_rate / 1000} kHz, {first?.bits} bits{first?.format === "Float" ? " float" : ""}</span>
        <span class:warn={session.output.resampling}>
          {t("player.output")} : {session.output.device}{session.output.first_channel > 0
            ? ` (${session.output.first_channel + 1}-${session.output.first_channel + 2})`
            : ""}, {session.output.sample_rate / 1000} kHz
          ({session.output.resampling ? t("player.resampling") : t("player.native")})
        </span>
        <OutputPicker />
        {#if markIn !== null || markOut !== null}
          <span class="marks mono">
            {t("player.in")} {markIn === null ? "--:--:--:--" : tc(markIn)} · {t("player.out")} {markOut === null ? "--:--:--:--" : tc(markOut)}
          </span>
        {/if}
      </div>
      <button class="change" onclick={open}>{t("player.change")}</button>
    </header>
    </div>

    <div class="scrubwrap">
      <input
        class="scrub"
        type="range"
        min="0"
        max={duration}
        step="0.01"
        value={position}
        oninput={(e) => audioSeek(Number(e.currentTarget.value), slot)}
      />
      {#each markers as m (m.id)}
        {#if m.in_frame !== null && m.out_frame !== null}
          <span
            class="span"
            style:left={`${(Math.min(m.in_frame, m.out_frame) / lastFrame) * 100}%`}
            style:width={`${(Math.abs(m.out_frame - m.in_frame) / lastFrame) * 100}%`}
            style:background={markerCss(m.color)}
          ></span>
        {:else}
          <span class="tick" style:left={`${(m.frame / lastFrame) * 100}%`} style:background={markerCss(m.color)}></span>
        {/if}
      {/each}
    </div>

    <div class="console">
      <div class="strips">
        {#each strips as s, i (i)}
          <div class="strip" class:muted={s.mute} class:soloed={s.solo}>
            <span class="name" title={s.name}>{#if ltc[i]}<b class="ltc" title={`LTC ${ltc[i]?.timecode}`}>LTC</b> {/if}{s.name}</span>
            <div class="meter-fader">
              <Meter db={s.level} hold={s.hold} />
              <Fader bind:value={s.gain} label={s.name} onchange={() => send(i)} />
            </div>
            <span class="db mono">{fmtDb(s.gain)}</span>
            <input
              class="pan"
              type="range"
              min="-1"
              max="1"
              step="0.05"
              title={t("player.pan")}
              bind:value={s.pan}
              oninput={() => send(i)}
              ondblclick={() => ((s.pan = 0), send(i))}
            />
            <div class="buttons">
              <button class="solo" class:on={s.solo} onclick={() => ((s.solo = !s.solo), send(i))}>{t("player.solo")}</button>
              <button class="mute" class:on={s.mute} onclick={() => ((s.mute = !s.mute), send(i))}>{t("player.mute")}</button>
            </div>
            <span class="num mono">{i + 1}</span>
          </div>
        {/each}
      </div>

      <div class="strip masterstrip">
        <span class="name">{t("player.master")}</span>
        <div class="meter-fader">
          <Meter db={master.l} hold={master.holdL} />
          <Meter db={master.r} hold={master.holdR} />
          <Fader bind:value={master.gain} label={t("player.master")} onchange={sendMaster} />
        </div>
        <span class="db mono">{fmtDb(master.gain)}</span>
        <dl class="lufs mono">
          <dt>{t("player.lufs.m")}</dt><dd>{fmt(lufs.m)}</dd>
          <dt>{t("player.lufs.s")}</dt><dd>{fmt(lufs.s)}</dd>
          <dt>{t("player.lufs.i")}</dt><dd>{fmt(lufs.i)}</dd>
        </dl>
      </div>
    </div>
  </div>
  {#if logs}
    <MarkersPanel
      bind:this={panel}
      onReport={() => (reportOpen = true)}
      path={loaded[0] ?? null}
      frame={logFrame}
      {tc}
      {markIn}
      {markOut}
      onSeek={(f) => audioSeek(f / AUDIO_LOG_FPS, slot)}
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
  .open {
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
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    padding: var(--vf-space-2) var(--vf-space-4);
    cursor: pointer;
    margin-left: auto;
  }
  .layout {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    height: 100%;
    min-height: 0;
  }
  .layout.withlogs {
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: var(--vf-space-2);
    padding-right: var(--vf-space-3);
    padding-block: var(--vf-space-3);
  }
  .layout.withlogs .audio {
    padding: 0 0 0 var(--vf-space-3);
  }
  .scrubwrap {
    position: relative;
    display: flex;
    align-items: center;
  }
  .scrubwrap .scrub {
    flex: 1;
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
  .marks {
    color: var(--vf-mark);
  }
  .top {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
  }
  .ltc {
    padding: 0 3px;
    border-radius: 3px;
    background: var(--vf-warning);
    color: var(--vf-on-accent);
    font-size: var(--vf-text-xs);
  }
  .audio {
    position: relative;
    min-height: 0;
    display: grid;
    grid-template-rows: auto auto 1fr;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--vf-space-3);
    height: 100%;
    padding: var(--vf-space-3);
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--vf-space-6);
    min-width: 0;
  }
  .clock {
    display: flex;
    flex-direction: column;
  }
  .tc {
    font-size: var(--vf-text-timecode);
    color: var(--vf-accent);
  }
  .small {
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  .meta {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  .meta b {
    color: var(--vf-text);
  }
  .warn {
    color: var(--vf-warning);
  }
  .scrub {
    width: 100%;
    accent-color: var(--vf-accent);
  }
  .console {
    display: flex;
    align-items: flex-start;
    gap: var(--vf-space-3);
    min-height: 0;
    min-width: 0;
  }
  .strips {
    display: flex;
    align-items: flex-start;
    gap: var(--vf-space-1);
    overflow-x: auto;
    flex: 1;
    min-width: 0;
    padding-bottom: var(--vf-space-2);
  }
  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--vf-space-2);
    width: 64px;
    flex: none;
    padding: var(--vf-space-2) var(--vf-space-1);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
  }
  .strip.soloed {
    border-color: var(--vf-warning);
  }
  .strip.muted .name {
    color: var(--vf-text-disabled);
  }
  .name {
    width: 100%;
    font-size: var(--vf-text-xs);
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meter-fader {
    display: flex;
    gap: var(--vf-space-1);
    align-items: center;
  }
  .db,
  .num {
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .pan {
    width: 52px;
    accent-color: var(--vf-text-muted);
  }
  .buttons {
    display: flex;
    gap: 2px;
  }
  .buttons button {
    width: 26px;
    height: 22px;
    font-size: var(--vf-text-xs);
    font-weight: 700;
    background: var(--vf-surface-high);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    cursor: pointer;
  }
  .solo.on {
    background: var(--vf-warning);
    color: var(--vf-on-accent);
  }
  .mute.on {
    background: var(--vf-error);
    color: var(--vf-on-accent);
  }
  .masterstrip {
    width: 120px;
  }
  .lufs {
    display: grid;
    grid-template-columns: auto auto;
    gap: 2px var(--vf-space-2);
    margin: 0;
    font-size: var(--vf-text-xs);
  }
  .lufs dt {
    color: var(--vf-text-muted);
  }
  .lufs dd {
    margin: 0;
    text-align: right;
  }
</style>
