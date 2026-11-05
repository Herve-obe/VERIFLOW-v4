<!-- Bandeau d'infos du clip courant (grands caractères). Lot 4.
     Lit : currentClip() et currentDetails() (stores/player.svelte.ts). Champ absent = « - ».
     Hauteur libre (auto) : PlayerTab ne la contraint pas.
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { currentClip, currentDetails } from "../../stores/player.svelte";
  import { t } from "../../i18n/index.svelte";
  import { framesToTc, tcToFrames } from "../../lib/timecode";

  let { mode }: { mode: Mode } = $props();

  const NONE = "-";
  const clip = $derived(currentClip());
  const d = $derived(currentDetails());
  const val = (k: string) => d?.values[k] || d?.details.embedded[k] || NONE;
  const probe = $derived(d?.details.probe ?? null);
  const video = $derived(probe?.video ?? null);
  const wav = $derived(d?.details.wav ?? null);

  const rate = $derived(video ? `${parseFloat((video.rate.num / video.rate.den).toFixed(3))}` : NONE);
  const format = $derived(video ? `${video.codec}  ${video.width}x${video.height}` : NONE);

  const tcIn = $derived(probe?.start_timecode ?? null);
  const tcOut = $derived.by(() => {
    if (!video || !tcIn) return NONE;
    const start = tcToFrames(tcIn, video.rate);
    if (start === null) return NONE;
    return framesToTc(start + video.frame_count, video.rate, tcIn.includes(";"));
  });
  // BWF : référence temporelle en échantillons, affichée à 25 i/s comme dans MEDIA.
  const bwf = $derived.by(() => {
    if (!wav || wav.time_reference === null || !wav.sample_rate) return NONE;
    return framesToTc(Math.floor((wav.time_reference / wav.sample_rate) * 25), { num: 25, den: 1 });
  });
  const exposure = $derived([val("iso"), val("white_balance"), val("shutter")].join("  "));
  const audioFmt = $derived.by(() => {
    if (wav) return { hz: `${wav.sample_rate / 1000} kHz`, bits: `${wav.bits}${wav.format === "Float" ? " float" : ""} bits`, ch: String(wav.channels) };
    const a = probe?.audio[0];
    if (!a) return null;
    return { hz: `${a.sample_rate / 1000} kHz`, bits: a.bits ? `${a.bits} bits` : NONE, ch: String(probe?.audio.reduce((n, x) => n + x.channels, 0) ?? a.channels) };
  });
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.banner")}>
  {#if !clip}
    <p class="empty">{NONE}</p>
  {:else}
    <div class="name" title={clip.path}>{clip.name}</div>
    <div class="grid">
      <div class="cell"><span class="lab">{t("player.banner.scene")}</span><span class="big">{val("scene")}</span></div>
      <div class="cell"><span class="lab">{t("player.banner.shot")}</span><span class="big">{val("shot")}</span></div>
      <div class="cell"><span class="lab">{t("player.banner.take")}</span><span class="big">{val("take")}</span></div>
      {#if mode === "video"}
        <div class="cell"><span class="lab">{t("player.banner.camera")}</span><span class="big">{val("camera")}</span></div>
        <div class="cell"><span class="lab">{t("player.banner.rate")}</span><span class="big mono">{rate}</span></div>
        <div class="cell wide"><span class="lab">{t("player.banner.tc")}</span><span class="big mono">{tcIn ?? NONE} - {tcOut}</span></div>
        <div class="cell"><span class="lab">{t("player.banner.exposure")}</span><span class="big mono">{exposure}</span></div>
        <div class="cell"><span class="lab">{t("player.banner.format")}</span><span class="big mono">{format}</span></div>
      {:else}
        <div class="cell wide"><span class="lab">{t("player.banner.tc")}</span><span class="big mono">{bwf}</span></div>
        <div class="cell"><span class="lab">{t("player.banner.freq")}</span><span class="big mono">{audioFmt?.hz ?? NONE}</span></div>
        <div class="cell"><span class="lab">{t("player.banner.bits")}</span><span class="big mono">{audioFmt?.bits ?? NONE}</span></div>
        <div class="cell"><span class="lab">{t("player.banner.tracks")}</span><span class="big mono">{audioFmt?.ch ?? NONE}</span></div>
      {/if}
    </div>
  {/if}
</div>

<style>
  .panel {
    min-height: 0;
    padding: var(--vf-space-2, 8px) var(--vf-space-3, 12px);
    background: var(--vf-surface);
    border-bottom: 1px solid var(--vf-border);
    color: var(--vf-player-banner-text);
  }
  .panel[data-mode="video"] .name { color: var(--vf-accent-video); }
  .panel[data-mode="audio"] .name { color: var(--vf-accent-audio); }
  .name {
    font-size: var(--vf-text-sm);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    margin-bottom: 4px;
  }
  .grid { display: flex; flex-wrap: wrap; gap: 4px 24px; }
  .cell { display: flex; flex-direction: column; min-width: 0; }
  .lab { font-size: var(--vf-text-xs); color: var(--vf-text-muted); }
  .big { font-size: var(--vf-text-lg); white-space: nowrap; }
  .mono { font-variant-numeric: tabular-nums; }
  .empty { margin: 0; color: var(--vf-text-disabled); }
</style>
