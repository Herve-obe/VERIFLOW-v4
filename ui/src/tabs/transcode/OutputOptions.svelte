<!-- Destination, nommage des fichiers produits et vérifications. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Section from "./Section.svelte";
  import { t } from "../../i18n/index.svelte";
  import type { PresetView } from "../../lib/transcode";
  import type { Form } from "../../lib/transcodeForm.svelte";

  let { form = $bindable(), preset, example }: { form: Form; preset: PresetView; example: string } = $props();

  const writes = $derived(!preset.analysis || ["cut_detect", "black_detect", "offline_detect", "silence_detect", "framemd5"].includes(preset.kind));

  async function pickDest() {
    const r = await open({ directory: true, multiple: false, title: t("transcode.dest.pick") });
    if (typeof r === "string") {
      form.dest = r;
      form.nextToSource = false;
    }
  }

  // Aperçu du nom produit à partir du premier fichier de la liste.
  const preview = $derived.by(() => {
    if (!example) return "";
    let stem = example.replace(/\.[^.]*$/, "");
    if (form.replaceFrom) stem = stem.split(form.replaceFrom).join(form.replaceTo);
    let n = `${form.prefix}${stem}${form.suffix || preset.suffix}`;
    if (preset.kind === "merge" && !form.suffix) n += "_fusion";
    if (form.numbering) n += `_${String(Number(form.numberStart) || 1).padStart(Number(form.numberDigits) || 3, "0")}`;
    const ext = preset.ext === "*" ? (example.split(".").pop() ?? "") : preset.ext;
    if (preset.kind === "image" && !form.settings.sequence.single) return `${n}/`;
    return ext ? `${n}.${ext}` : n;
  });
</script>

{#if writes}
  <Section id="output" title={t("transcode.section.output")}>
    <label class="tc-check"><input type="checkbox" bind:checked={form.nextToSource} /> {t("transcode.next.to.source")}</label>
    {#if !form.nextToSource}
      <div class="tc-row">
        <span class="tc-path" title={form.dest}>{form.dest || t("transcode.dest.none")}</span>
        <button class="tc-btn" onclick={pickDest}>{t("offload.browse")}</button>
      </div>
    {/if}
    <p class="tc-sub">{t("transcode.naming")}</p>
    <div class="tc-grid">
      <label class="tc-field">{t("transcode.prefix")}<input class="mono" bind:value={form.prefix} /></label>
      <label class="tc-field">{t("transcode.suffix")}<input class="mono" placeholder={preset.suffix || t("transcode.suffix.none")} bind:value={form.suffix} /></label>
      <label class="tc-field">{t("transcode.replace.from")}<input class="mono" bind:value={form.replaceFrom} /></label>
      <label class="tc-field">{t("transcode.replace.to")}<input class="mono" bind:value={form.replaceTo} /></label>
      <label class="tc-check"><input type="checkbox" bind:checked={form.numbering} /> {t("transcode.numbering")}</label>
      {#if form.numbering}
        <div class="tc-row">
          <label class="tc-field">{t("transcode.numbering.start")}<input type="number" min="0" bind:value={form.numberStart} /></label>
          <label class="tc-field">{t("transcode.numbering.digits")}<input type="number" min="1" max="8" bind:value={form.numberDigits} /></label>
        </div>
      {/if}
      <label class="tc-field tc-wide">
        {t("transcode.existing")}
        <select bind:value={form.existing}>
          <option value="rename">{t("transcode.existing.rename")}</option>
          <option value="skip">{t("transcode.existing.skip")}</option>
          <option value="overwrite">{t("transcode.existing.overwrite")}</option>
        </select>
      </label>
    </div>
    {#if preview}<p class="tc-hint">{t("transcode.naming.example")} <span class="mono">{preview}</span></p>{/if}
    <p class="tc-hint">{t("transcode.never.overwrite")}</p>

    <p class="tc-sub">{t("transcode.verify")}</p>
    {#if !preset.analysis}
      <label class="tc-check"><input type="checkbox" bind:checked={form.checksum} /> {t("transcode.checksum")}</label>
    {/if}
    {#if preset.kind === "video"}
      <label class="tc-check"><input type="checkbox" bind:checked={form.settings.vmaf_after} /> {t("transcode.vmaf.after")}</label>
    {/if}
    <label class="tc-check"><input type="checkbox" bind:checked={form.report} /> {t("transcode.report")}</label>
  </Section>
{:else}
  <Section id="output" title={t("transcode.section.output")}>
    <label class="tc-check"><input type="checkbox" bind:checked={form.report} /> {t("transcode.report")}</label>
    <p class="tc-hint">{t("transcode.report.next")}</p>
  </Section>
{/if}
