<!-- Incrustations : timecode, nom du fichier, texte, logo, sous-titres. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Section from "./Section.svelte";
  import { t } from "../../i18n/index.svelte";
  import { defaultTextStyle, POSITIONS, type PresetView, type TextStyle } from "../../lib/transcode";
  import type { Form } from "../../lib/transcodeForm.svelte";

  let { form = $bindable(), preset }: { form: Form; preset: PresetView } = $props();

  const ov = $derived(form.settings.overlay);
  const subs = $derived(form.settings.subtitles);
  const active = $derived([ov.timecode, ov.filename, ov.text, ov.logo, subs.file].filter(Boolean).length);
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const pct = (v: string, d: number) => (v === "" || Number.isNaN(Number(v)) ? d : Number(v));

  async function pickLogo() {
    const r = await open({ multiple: false, directory: false, title: t("transcode.logo"), filters: [{ name: "Images", extensions: ["png", "jpg", "jpeg", "tif", "tiff", "webp", "bmp"] }] });
    if (typeof r === "string") form.settings.overlay.logo = { path: r, position: "top_right", size: 12, opacity: 1 };
  }

  async function pickSubs() {
    const r = await open({ multiple: false, directory: false, title: t("transcode.subtitles"), filters: [{ name: "SRT, VTT, ASS", extensions: ["srt", "vtt", "ass", "ssa"] }] });
    if (typeof r === "string") form.settings.subtitles.file = r;
  }

  type Key = "timecode" | "filename" | "text";
  function toggle(k: Key, on: boolean) {
    const pos = k === "timecode" ? "bottom" : k === "filename" ? "top_left" : "top";
    form.settings.overlay[k] = on ? defaultTextStyle(pos) : null;
  }
</script>

{#snippet style(s: TextStyle)}
  <div class="tc-grid four">
    <label class="tc-field">
      {t("transcode.position")}
      <select bind:value={s.position}>
        {#each POSITIONS as p (p)}<option value={p}>{t(`transcode.pos.${p}`)}</option>{/each}
      </select>
    </label>
    <label class="tc-field">{t("transcode.text.size")}<input type="number" min="1" max="30" step="0.5" value={s.size} onchange={(e) => (s.size = pct(e.currentTarget.value, 4))} /></label>
    <label class="tc-field">{t("transcode.opacity")}<input type="number" min="5" max="100" step="5" value={Math.round(s.opacity * 100)} onchange={(e) => (s.opacity = pct(e.currentTarget.value, 100) / 100)} /></label>
    <label class="tc-check"><input type="checkbox" bind:checked={s.background} /> {t("transcode.text.box")}</label>
  </div>
{/snippet}

<Section id="overlay" title={t("transcode.section.overlay")} badge={active ? String(active) : ""}>
  <label class="tc-check"><input type="checkbox" checked={!!ov.timecode} onchange={(e) => toggle("timecode", e.currentTarget.checked)} /> {t("transcode.burn.tc")}</label>
  {#if ov.timecode}
    {@render style(ov.timecode)}
    <div class="tc-grid">
      <label class="tc-field">{t("transcode.tc.start")}<input class="mono" placeholder={t("transcode.tc.source")} value={ov.tc_start ?? ""} oninput={(e) => (form.settings.overlay.tc_start = e.currentTarget.value || null)} /></label>
      <label class="tc-field">{t("transcode.tc.offset")}<input type="number" step="1" value={ov.tc_offset} onchange={(e) => (form.settings.overlay.tc_offset = Math.round(pct(e.currentTarget.value, 0)))} /></label>
    </div>
  {/if}

  <p class="tc-sub"></p>
  <label class="tc-check"><input type="checkbox" checked={!!ov.filename} onchange={(e) => toggle("filename", e.currentTarget.checked)} /> {t("transcode.burn.name")}</label>
  {#if ov.filename}{@render style(ov.filename)}{/if}

  <p class="tc-sub"></p>
  <label class="tc-check"><input type="checkbox" checked={!!ov.text} onchange={(e) => toggle("text", e.currentTarget.checked)} /> {t("transcode.burn.text")}</label>
  {#if ov.text}
    <label class="tc-field"><input bind:value={form.settings.overlay.text_value} placeholder={t("transcode.burn.text.hint")} /></label>
    {@render style(ov.text)}
  {/if}

  <p class="tc-sub">{t("transcode.logo")}</p>
  {#if ov.logo}
    <div class="tc-row">
      <span class="tc-path" title={ov.logo.path}>{name(ov.logo.path)}</span>
      <button class="tc-btn" onclick={pickLogo}>{t("offload.browse")}</button>
      <button class="tc-btn" onclick={() => (form.settings.overlay.logo = null)}>{t("transcode.remove.short")}</button>
    </div>
    <div class="tc-grid three">
      <label class="tc-field">
        {t("transcode.position")}
        <select bind:value={ov.logo.position}>
          {#each POSITIONS as p (p)}<option value={p}>{t(`transcode.pos.${p}`)}</option>{/each}
        </select>
      </label>
      <label class="tc-field">{t("transcode.logo.size")}<input type="number" min="1" max="100" step="1" value={ov.logo.size} onchange={(e) => ov.logo && (ov.logo.size = pct(e.currentTarget.value, 12))} /></label>
      <label class="tc-field">{t("transcode.opacity")}<input type="number" min="5" max="100" step="5" value={Math.round(ov.logo.opacity * 100)} onchange={(e) => ov.logo && (ov.logo.opacity = pct(e.currentTarget.value, 100) / 100)} /></label>
    </div>
  {:else}
    <button class="tc-btn" onclick={pickLogo}>+ {t("transcode.logo.add")}</button>
  {/if}

  <p class="tc-sub">{t("transcode.subtitles")}</p>
  {#if subs.file}
    <div class="tc-row">
      <span class="tc-path" title={subs.file}>{name(subs.file)}</span>
      <button class="tc-btn" onclick={pickSubs}>{t("offload.browse")}</button>
      <button class="tc-btn" onclick={() => (form.settings.subtitles.file = null)}>{t("transcode.remove.short")}</button>
    </div>
    <div class="tc-grid">
      {#if preset.kind === "video"}
        <label class="tc-field">
          {t("transcode.subtitles.mode")}
          <select value={subs.burn ? "burn" : "track"} onchange={(e) => (form.settings.subtitles.burn = e.currentTarget.value === "burn")}>
            <option value="burn">{t("transcode.subtitles.burn")}</option>
            <option value="track">{t("transcode.subtitles.track")}</option>
          </select>
        </label>
      {/if}
      {#if subs.burn || preset.kind !== "video"}
        <label class="tc-field">{t("transcode.subtitles.size")}<input type="number" min="0" max="120" step="1" placeholder={t("transcode.auto")} value={subs.size || ""} onchange={(e) => (form.settings.subtitles.size = Math.round(pct(e.currentTarget.value, 0)))} /></label>
      {/if}
    </div>
  {:else}
    <button class="tc-btn" onclick={pickSubs}>+ {t("transcode.subtitles.add")}</button>
  {/if}
  <p class="tc-hint">{t("transcode.overlay.hint")}</p>
</Section>
