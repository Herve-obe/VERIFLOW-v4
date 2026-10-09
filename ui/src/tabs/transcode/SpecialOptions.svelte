<!-- Réglages propres à certains préréglages : images, conformation, son de
     remplacement, insert, sous-titres, VMAF, seuils de détection. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Section from "./Section.svelte";
  import { t } from "../../i18n/index.svelte";
  import { FRAME_RATES, THRESHOLDS, type PresetView } from "../../lib/transcode";
  import type { Form } from "../../lib/transcodeForm.svelte";

  let { form = $bindable(), preset }: { form: Form; preset: PresetView } = $props();

  const st = $derived(form.settings);
  const k = $derived(preset.kind);
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;
  const SOUND = ["wav", "bwf", "aif", "aiff", "flac", "mp3", "m4a", "aac", "ogg", "opus", "mov", "mp4", "mxf"];

  async function pickAudio() {
    const r = await open({ multiple: true, directory: false, title: t("transcode.replace.files"), filters: [{ name: t("transcode.media"), extensions: SOUND }] });
    const list = Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
    form.settings.audio_files = [...new Set([...form.settings.audio_files, ...list])];
  }

  async function pickFile(field: "insert_file" | "reference", directory = false) {
    const r = await open({ multiple: false, directory, title: t(field === "insert_file" ? "transcode.insert.file" : "transcode.vmaf.reference") });
    if (typeof r === "string") form.settings[field] = r;
  }

  async function pickSubs() {
    const r = await open({ multiple: false, directory: false, title: t("transcode.subtitles"), filters: [{ name: "SRT, VTT, ASS", extensions: ["srt", "vtt", "ass", "ssa"] }] });
    if (typeof r === "string") form.settings.subtitles.file = r;
  }

  const shown = $derived(
    ["image", "conform", "replace_audio", "insert", "subtitles", "vmaf", "merge", "cut_detect", "black_detect", "silence_detect", "offline_detect", "framemd5", "extract_tracks", "cut"].includes(k),
  );
</script>

{#if shown}
  <Section id="special" title={t("transcode.section.special")}>
    {#if k === "image"}
      <div class="tc-grid">
        <label class="tc-field tc-wide">
          {t("transcode.seq.mode")}
          <select
            value={st.sequence.single ? "single" : st.sequence.every ? "every" : "all"}
            onchange={(e) => {
              const v = e.currentTarget.value;
              form.settings.sequence.single = v === "single";
              form.settings.sequence.every = v === "every" ? (st.sequence.every ?? 1) : null;
            }}
          >
            <option value="single">{t("transcode.seq.single")}</option>
            <option value="every">{t("transcode.seq.every")}</option>
            <option value="all">{t("transcode.seq.all")}</option>
          </select>
        </label>
        {#if st.sequence.single}
          <label class="tc-field tc-wide">{t("transcode.seq.position")}<input class="mono" placeholder={t("transcode.seq.middle")} value={st.sequence.position ?? ""} oninput={(e) => (form.settings.sequence.position = e.currentTarget.value || null)} /></label>
        {:else if st.sequence.every}
          <label class="tc-field">{t("transcode.seq.interval")}<input type="number" min="0.04" step="0.5" value={st.sequence.every} onchange={(e) => (form.settings.sequence.every = Math.max(0.04, Number(e.currentTarget.value) || 1))} /></label>
        {:else}
          <label class="tc-check tc-wide"><input type="checkbox" bind:checked={form.settings.sequence.tc_numbering} /> {t("transcode.seq.tc")}</label>
        {/if}
      </div>
      <p class="tc-hint">{t("transcode.seq.hint")}</p>
    {:else if k === "conform"}
      <div class="tc-grid">
        <label class="tc-field">
          {t("transcode.conform.rate")}
          <select value={st.conform_rate ?? ""} onchange={(e) => (form.settings.conform_rate = e.currentTarget.value || null)}>
            <option value="">-</option>
            {#each FRAME_RATES as r (r)}<option value={r}>{r.replace(".", ",")} i/s</option>{/each}
          </select>
        </label>
        <label class="tc-check"><input type="checkbox" bind:checked={form.settings.keep_pitch} /> {t("transcode.conform.pitch")}</label>
      </div>
      <p class="tc-hint">{t("transcode.conform.hint")}</p>
    {:else if k === "replace_audio"}
      <p class="tc-sub">{t("transcode.replace.files")} ({st.audio_files.length})</p>
      {#each st.audio_files as f (f)}
        <div class="tc-row">
          <span class="tc-path" title={f}>{name(f)}</span>
          <button class="tc-btn" onclick={() => (form.settings.audio_files = st.audio_files.filter((x) => x !== f))}>{t("transcode.remove.short")}</button>
        </div>
      {/each}
      <div class="tc-row"><button class="tc-btn" onclick={pickAudio}>+ {t("transcode.add.files")}</button></div>
      <div class="tc-grid">
        <label class="tc-field">{t("transcode.replace.offset")}<input type="number" step="0.01" bind:value={form.settings.audio_offset} /></label>
        <label class="tc-check"><input type="checkbox" bind:checked={form.settings.sync_tc} /> {t("transcode.replace.sync")}</label>
      </div>
      <p class="tc-hint">{t("transcode.replace.hint")}</p>
    {:else if k === "insert"}
      <div class="tc-field">
        {t("transcode.insert.file")}
        <div class="tc-row">
          <span class="tc-path">{st.insert_file ? name(st.insert_file) : t("transcode.none")}</span>
          <button class="tc-btn" onclick={() => pickFile("insert_file")}>{t("offload.browse")}</button>
        </div>
      </div>
      <label class="tc-field">{t("transcode.insert.at")}<input class="mono" placeholder="10:00:12:00" value={st.insert_at ?? ""} oninput={(e) => (form.settings.insert_at = e.currentTarget.value || null)} /></label>
      <p class="tc-hint">{t("transcode.insert.hint")}</p>
    {:else if k === "subtitles"}
      <div class="tc-row">
        <span class="tc-path">{st.subtitles.file ? name(st.subtitles.file) : t("transcode.none")}</span>
        <button class="tc-btn" onclick={pickSubs}>{t("offload.browse")}</button>
      </div>
      <p class="tc-hint">{t("transcode.subtitles.track.hint")}</p>
    {:else if k === "vmaf"}
      <div class="tc-field">
        {t("transcode.vmaf.reference")}
        <div class="tc-row">
          <span class="tc-path" title={st.reference ?? ""}>{st.reference ?? t("transcode.none")}</span>
          <button class="tc-btn" onclick={() => pickFile("reference", true)}>{t("transcode.add.folder")}</button>
          <button class="tc-btn" onclick={() => pickFile("reference")}>{t("transcode.file")}</button>
        </div>
      </div>
      <p class="tc-hint">{t("transcode.vmaf.hint")}</p>
    {:else if THRESHOLDS[k]}
      <div class="tc-grid">
        <label class="tc-field">
          {t("transcode.threshold")} {THRESHOLDS[k].unit ? `(${THRESHOLDS[k].unit})` : ""}
          <input type="number" step={THRESHOLDS[k].step} placeholder={String(THRESHOLDS[k].value)} value={st.threshold ?? ""} oninput={(e) => (form.settings.threshold = e.currentTarget.value === "" ? null : Number(e.currentTarget.value))} />
        </label>
      </div>
      <p class="tc-hint">{t(`transcode.hint.kind.${k}`)}</p>
    {:else}
      <p class="tc-hint">{t(`transcode.hint.kind.${k}`)}</p>
    {/if}
  </Section>
{/if}
