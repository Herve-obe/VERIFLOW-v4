<!-- Carte d'alerte LTC, centrée sur la zone de lecture (image ou console) :
     pistes concernées, timecode lu, et trois choix centrés. -->
<script lang="ts">
  import { t } from "../../i18n/index.svelte";
  import type { LtcDetection } from "../../lib/player";

  let {
    names,
    ltc,
    onChoice,
  }: {
    names: string[];
    ltc: (LtcDetection | null)[];
    onChoice: (mode: "mute" | "low" | "on") => void;
  } = $props();

  const ltcTracks = $derived(ltc.map((d, i) => (d ? i : -1)).filter((i) => i >= 0));
</script>

<div class="notice" role="alertdialog" aria-label={t("sound.ltc.title")}>
  <div class="head">
    <svg viewBox="0 0 24 24" class="warn" aria-hidden="true">
      <path d="M12 3 L22 20 H2 Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
      <path d="M12 9 V14 M12 16.6 V17" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
    </svg>
    <h4>{t("sound.ltc.title")}</h4>
  </div>
  <ul>
    {#each ltcTracks as i (i)}
      <li><b>{names[i]}</b><span class="mono">{ltc[i]?.timecode}</span><span>{ltc[i]?.fps} i/s</span></li>
    {/each}
  </ul>
  <p>{t("sound.ltc.muted")}</p>
  <div class="choices">
    <button class="primary" onclick={() => onChoice("mute")}>{t("sound.ltc.keep")}</button>
    <button onclick={() => onChoice("low")}>{t("sound.ltc.low")}</button>
    <button onclick={() => onChoice("on")}>{t("sound.ltc.on")}</button>
  </div>
</div>

<style>
  .notice {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    z-index: 2;
    width: 380px;
    max-width: 90%;
    padding: var(--vf-space-4);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-lg);
    box-shadow: 0 12px 40px var(--vf-shadow);
    font-size: var(--vf-text-sm);
    color: var(--vf-text);
    text-align: center;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--vf-space-2);
    margin-bottom: var(--vf-space-3);
  }
  .warn {
    width: 20px;
    height: 20px;
    flex: none;
    color: var(--vf-warning);
  }
  h4 {
    margin: 0;
    font-size: var(--vf-text-md);
    font-weight: 600;
  }
  ul {
    list-style: none;
    margin: 0 0 var(--vf-space-2);
    padding: 0;
  }
  li {
    display: flex;
    justify-content: center;
    gap: var(--vf-space-3);
    padding: 2px 0;
    color: var(--vf-text-muted);
  }
  li b {
    color: var(--vf-text);
    font-weight: 600;
  }
  .notice p {
    margin: 0 0 var(--vf-space-4);
    color: var(--vf-text-muted);
  }
  .choices {
    display: flex;
    justify-content: center;
    gap: var(--vf-space-2);
  }
  .choices button {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .choices button.primary {
    background: var(--vf-accent);
    border-color: var(--vf-accent);
    color: var(--vf-on-accent);
    font-weight: 600;
  }
</style>
