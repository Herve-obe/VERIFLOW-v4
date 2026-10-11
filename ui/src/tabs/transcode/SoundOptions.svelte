<!-- Son : pistes des vidéos, fréquence, résolution, débit, normalisation. -->
<script lang="ts">
  import Section from "./Section.svelte";
  import { t, i18n } from "../../i18n/index.svelte";
  import { LOUDNESS_TARGETS, SAMPLE_RATES, type AudioMode, type BitDepth, type PresetView } from "../../lib/transcode";
  import type { Form } from "../../lib/transcodeForm.svelte";

  let { form = $bindable(), preset }: { form: Form; preset: PresetView } = $props();

  const lang = $derived(i18n.lang === "en" ? "en" : "fr");
  const reference = $derived(preset.analysis);
  const mode = $derived(form.settings.audio_mode ?? preset.audio_mode);
  const active = $derived(
    [form.loudness !== "none", preset.audio && form.settings.sample_rate, preset.audio && form.settings.bit_depth, preset.kind === "video" && form.settings.audio_mode].filter(Boolean)
      .length,
  );
  const depthLabel = (d: BitDepth) => (d === "32f" ? t("transcode.depth.float") : `${d} bits`);
</script>

<Section id="sound" title={t("transcode.section.sound")} badge={active ? String(active) : ""}>
  <div class="tc-grid">
    {#if preset.kind === "video"}
      <label class="tc-field tc-wide">
        {t("transcode.audio.mode")}
        <select value={mode} onchange={(e) => (form.settings.audio_mode = e.currentTarget.value as AudioMode)}>
          {#each ["keep", "first_two", "mix", "none"] as m (m)}<option value={m}>{t(`transcode.audio.mode.${m}`)}</option>{/each}
        </select>
      </label>
    {/if}
    {#if preset.audio}
      <label class="tc-field">
        {t("transcode.rate")}
        <select value={form.settings.sample_rate ?? 0} onchange={(e) => (form.settings.sample_rate = Number(e.currentTarget.value) || null)}>
          <option value={0}>{t("transcode.same")}</option>
          {#each SAMPLE_RATES as r (r)}<option value={r}>{(r / 1000).toLocaleString(lang)} kHz</option>{/each}
        </select>
      </label>
      {#if preset.bit_depths.length}
        <label class="tc-field">
          {t("transcode.depth")}
          <select value={form.settings.bit_depth ?? ""} onchange={(e) => (form.settings.bit_depth = (e.currentTarget.value || null) as BitDepth | null)}>
            <option value="">{t("transcode.same")}</option>
            {#each preset.bit_depths as d (d)}<option value={d}>{depthLabel(d)}</option>{/each}
          </select>
        </label>
      {/if}
      {#if preset.audio_bitrates.length}
        <label class="tc-field">
          {t("transcode.audio.bitrate")}
          <select value={form.settings.audio_kbps ?? preset.default_audio_bitrate} onchange={(e) => (form.settings.audio_kbps = Number(e.currentTarget.value))}>
            {#each preset.audio_bitrates as r (r)}<option value={r}>{r} kbit/s</option>{/each}
          </select>
        </label>
      {/if}
    {/if}
    {#if mode !== "none" || preset.kind !== "video"}
      <label class="tc-field tc-wide">
        {t(reference ? "transcode.loudness.reference" : "transcode.loudness")}
        <select bind:value={form.loudness}>
          <option value="none">{t("transcode.loudness.none")}</option>
          {#each LOUDNESS_TARGETS as l (l.id)}
            <option value={l.id}>{t(`transcode.loudness.${l.id}`)} ({l.target.integrated} LUFS, {l.target.true_peak} dBTP)</option>
          {/each}
          <option value="custom">{t("transcode.loudness.custom")}</option>
        </select>
      </label>
      {#if form.loudness === "custom"}
        <label class="tc-field">{t("transcode.loudness.integrated")}<input type="number" step="0.5" bind:value={form.customI} /></label>
        <label class="tc-field">{t("transcode.loudness.tp")}<input type="number" step="0.5" bind:value={form.customTp} /></label>
      {/if}
    {/if}
  </div>
  {#if form.loudness !== "none" && !reference}
    <p class="tc-hint">{t(preset.kind === "video" ? "transcode.loudness.hint.video" : "transcode.loudness.hint")}</p>
  {/if}
  {#if preset.kind === "video" && mode === "mix"}<p class="tc-hint">{t("transcode.audio.mode.mix.hint")}</p>{/if}
  {#if preset.audio && preset.audio_bitrates.length}<p class="tc-hint">{t("transcode.lossy.channels")}</p>{/if}
</Section>
