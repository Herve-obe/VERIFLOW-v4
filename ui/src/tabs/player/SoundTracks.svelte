<!-- Pistes son d'une vidéo : niveau, volume et coupure par piste. Une piste
     qui porte un timecode LTC (générateur sur l'entrée droite de la caméra,
     par exemple) est coupée d'office pour protéger l'écoute. -->
<script lang="ts">
  import { untrack } from "svelte";
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

  interface Track {
    gain: number;
    mute: boolean;
  }
  let tracks = $state<Track[]>([]);
  let notice = $state(true);

  // Nouvelles pistes (nouveau clip) : réglages remis à zéro.
  $effect(() => {
    tracks = names.map(() => ({ gain: 0, mute: false }));
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

  const ratio = (db: number | null | undefined) => (db == null ? 0 : Math.min(1, Math.max(0, (db + 60) / 60)));
  const fmtDb = (v: number) => (v <= -60 ? "-inf" : (v > 0 ? "+" : "") + v.toFixed(0));
</script>

{#if names.length > 0}
  <div class="sound">
    {#if ltcTracks.length > 0 && notice}
      <div class="notice" role="status">
        <span>
          {t("sound.ltc.found")}
          {ltcTracks.map((i) => `${names[i]} (${ltc[i]?.timecode}, ${ltc[i]?.fps} i/s)`).join(", ")}.
          {t("sound.ltc.muted")}
        </span>
        <button onclick={() => setLtc("mute")}>{t("sound.ltc.keep")}</button>
        <button onclick={() => setLtc("low")}>{t("sound.ltc.low")}</button>
        <button onclick={() => setLtc("on")}>{t("sound.ltc.on")}</button>
      </div>
    {/if}
    <div class="tracks">
      {#each tracks as tr, i (i)}
        <div class="track" class:muted={tr.mute}>
          <span class="name" title={names[i]}>
            {names[i]}
            {#if ltc[i]}<b class="ltc" title={`LTC ${ltc[i]?.timecode}`}>LTC</b>{/if}
          </span>
          <div class="bar" title={peaks[i] == null ? "-inf dBFS" : `${peaks[i]?.toFixed(1)} dBFS`}>
            <div class="fill" style:width={`${ratio(peaks[i]) * 100}%`}></div>
          </div>
          <input
            type="range"
            min="-60"
            max="12"
            step="1"
            bind:value={tr.gain}
            oninput={() => send(i)}
            ondblclick={() => ((tr.gain = 0), send(i))}
            aria-label={`${t("sound.gain")} ${names[i]}`}
            title={`${fmtDb(tr.gain)} dB`}
          />
          <span class="db mono">{fmtDb(tr.gain)}</span>
          <button class="mute" class:on={tr.mute} onclick={() => ((tr.mute = !tr.mute), send(i))} title={t("sound.mute")}>M</button>
        </div>
      {/each}
    </div>
  </div>
{/if}

<style>
  .sound {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-1);
    font-size: var(--vf-text-sm);
  }
  .notice {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--vf-space-2);
    padding: var(--vf-space-1) var(--vf-space-2);
    border: 1px solid var(--vf-warning);
    border-radius: var(--vf-radius-sm);
    color: var(--vf-warning);
  }
  .notice button,
  .mute {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 1px var(--vf-space-2);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .tracks {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-1) var(--vf-space-4);
  }
  .track {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
  }
  .track.muted .name,
  .track.muted .bar {
    opacity: 0.45;
  }
  .name {
    min-width: 64px;
    color: var(--vf-text-muted);
    white-space: nowrap;
  }
  .ltc {
    margin-left: 4px;
    padding: 0 4px;
    border-radius: 3px;
    background: var(--vf-warning);
    color: var(--vf-on-accent);
    font-size: var(--vf-text-xs);
  }
  .bar {
    position: relative;
    width: 90px;
    height: 6px;
    background: var(--vf-meter-bg);
    border-radius: 2px;
    overflow: hidden;
  }
  .fill {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    background-image: linear-gradient(
      to right,
      var(--vf-meter-low) 0px,
      var(--vf-meter-low) 63px,
      var(--vf-meter-mid) 63px,
      var(--vf-meter-mid) 81px,
      var(--vf-meter-high) 81px
    );
    background-size: 90px 100%;
  }
  input[type="range"] {
    width: 80px;
    accent-color: var(--vf-accent);
  }
  .db {
    min-width: 3ch;
    color: var(--vf-text-muted);
  }
  .mute.on {
    background: var(--vf-error);
    border-color: var(--vf-error);
    color: var(--vf-on-accent);
  }
</style>
