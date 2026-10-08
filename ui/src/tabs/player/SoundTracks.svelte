<!-- Pistes son d'une vidéo, en tranches verticales à côté de l'image : nom,
     vumètre crête (avant le volume, donc visible même coupé), fader et
     coupure. Une piste qui porte un timecode LTC (générateur sur l'entrée
     droite de la caméra, par exemple) est coupée d'office pour protéger
     l'écoute ; un message propose de la laisser coupée, la baisser ou la
     réactiver. -->
<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import Meter from "../../components/Meter.svelte";
  import Fader from "../../components/Fader.svelte";
  import LtcNotice from "./LtcNotice.svelte";
  import { t } from "../../i18n/index.svelte";
  import { audioTrack, type AudioSlot, type LtcDetection } from "../../lib/player";

  let {
    slot,
    names,
    ltc = [],
    peaks = [],
  }: {
    slot: AudioSlot;
    names: string[];
    ltc?: (LtcDetection | null)[];
    peaks?: (number | null)[];
  } = $props();

  const LTC_LOW_DB = -30;
  const FALL_DB_PER_S = 20; // retombée des vumètres
  const HOLD_MS = 1500; // maintien de crête

  interface Track {
    gain: number;
    mute: boolean;
    level: number | null;
    hold: number | null;
    holdAt: number;
  }
  let tracks = $state<Track[]>([]);
  let notice = $state(true);
  let lastPeaks = performance.now();

  // Nouvelles pistes (nouveau clip) : réglages remis à zéro.
  $effect(() => {
    tracks = names.map(() => ({ gain: 0, mute: false, level: null, hold: null, holdAt: 0 }));
    notice = true;
  });

  // Pistes LTC coupées dès qu'elles sont détectées (une seule fois par
  // analyse : « Réactiver » n'est pas annulé ensuite).
  $effect(() => {
    const found = ltc;
    untrack(() =>
      found.forEach((d, i) => {
        if (d && tracks[i] && !tracks[i].mute) {
          tracks[i].mute = true;
          send(i);
        }
      }),
    );
  });

  // Crêtes reçues pendant la lecture : retombée progressive et maintien,
  // comme un crêtemètre matériel.
  $effect(() => {
    const p = peaks;
    untrack(() => {
      const now = performance.now();
      const dt = (now - lastPeaks) / 1000;
      lastPeaks = now;
      p.forEach((peak, i) => {
        const tr = tracks[i];
        if (!tr) return;
        const fallen = tr.level === null ? -Infinity : tr.level - FALL_DB_PER_S * dt;
        const v = Math.max(peak ?? -Infinity, fallen);
        tr.level = v < -60 ? null : v;
        if (peak !== null && (tr.hold === null || peak >= tr.hold || now - tr.holdAt > HOLD_MS)) {
          tr.hold = peak;
          tr.holdAt = now;
        } else if (tr.hold !== null && now - tr.holdAt > HOLD_MS) {
          tr.hold = null;
        }
      });
    });
  });

  // En pause ou à l'arrêt, plus aucune crête n'arrive : les vumètres
  // retombent d'eux-mêmes et les maintiens de crête s'effacent.
  let lastFall = performance.now();
  const fallTimer = window.setInterval(() => {
    const now = performance.now();
    const dt = (now - lastFall) / 1000;
    lastFall = now;
    if (now - lastPeaks < 150) return;
    for (const tr of tracks) {
      if (tr.level !== null) {
        const v = tr.level - FALL_DB_PER_S * dt;
        tr.level = v < -60 ? null : v;
      }
      if (tr.hold !== null && now - tr.holdAt > HOLD_MS) tr.hold = null;
    }
  }, 50);
  onDestroy(() => window.clearInterval(fallTimer));

  const ltcTracks = $derived(ltc.map((d, i) => (d ? i : -1)).filter((i) => i >= 0));

  function send(i: number) {
    const tr = tracks[i];
    if (!tr) return;
    audioTrack(i, tr.gain <= -60 ? -200 : tr.gain, 0, tr.mute, false, slot).catch(() => {});
  }

  function setLtc(mode: "mute" | "low" | "on") {
    for (const i of ltcTracks) {
      tracks[i].mute = mode === "mute";
      tracks[i].gain = mode === "low" ? LTC_LOW_DB : 0;
      send(i);
    }
    notice = false;
  }

  const fmtDb = (v: number) => (v <= -60 ? "-inf" : (v > 0 ? "+" : "") + v.toFixed(1));
</script>

{#if ltcTracks.length > 0 && notice}
  <LtcNotice {names} {ltc} onChoice={setLtc} />
{/if}

{#if names.length > 0}
  <div class="strips" role="group" aria-label={t("sound.tracks")}>
    {#each tracks as tr, i (i)}
      <div class="strip" class:muted={tr.mute}>
        <span class="name" title={names[i]}>{names[i]}</span>
        <span class="tag">{#if ltc[i]}<b class="ltc" title={`LTC ${ltc[i]?.timecode}`}>LTC</b>{/if}</span>
        <div class="mf">
          <Meter db={tr.level} hold={tr.hold} />
          <Fader bind:value={tr.gain} label={names[i]} onchange={() => send(i)} />
        </div>
        <span class="db mono">{fmtDb(tr.gain)}</span>
        <button class="mute" class:on={tr.mute} onclick={() => ((tr.mute = !tr.mute), send(i))} title={t("sound.mute")}>
          M
        </button>
      </div>
    {/each}
  </div>
{/if}

<style>
  .strips {
    --vf-meter-height: var(--vf-meter-height-compact);
    display: flex;
    gap: var(--vf-space-1);
    align-self: center;
  }
  .strip {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--vf-space-1);
    width: 52px;
    padding: var(--vf-space-2) var(--vf-space-1);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
  }
  .strip.muted .mf {
    opacity: 0.5;
  }
  .name {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .tag {
    display: flex;
    align-items: center;
    height: 18px;
    margin-bottom: var(--vf-space-1);
  }
  .ltc {
    padding: 0 4px;
    border-radius: 3px;
    background: var(--vf-warning);
    color: var(--vf-on-accent);
    font-size: var(--vf-text-xs);
  }
  .mf {
    display: flex;
    gap: var(--vf-space-1);
    align-items: center;
  }
  .db {
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .mute {
    width: 28px;
    height: 22px;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-xs);
    font-weight: 700;
    cursor: pointer;
  }
  .mute.on {
    background: var(--vf-error);
    border-color: var(--vf-error);
    color: var(--vf-on-accent);
  }
</style>
