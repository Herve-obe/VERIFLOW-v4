<!-- Réglages d'image : taille, recadrage, rotation, désentrelacement, cadence,
     LUT, couleurs, débit et encodeur. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Section from "./Section.svelte";
  import { t } from "../../i18n/index.svelte";
  import { ASPECTS, FRAME_RATES, SCALES, type PresetView } from "../../lib/transcode";
  import type { Form } from "../../lib/transcodeForm.svelte";

  let { form = $bindable(), preset }: { form: Form; preset: PresetView } = $props();

  const img = $derived(form.settings.image);
  const active = $derived(
    [
      img.crop.top || img.crop.bottom || img.crop.left || img.crop.right,
      img.aspect,
      img.frame,
      img.rotate !== "none",
      img.deinterlace,
      img.fps,
      img.decimate,
      img.lut,
      img.brightness !== 0 || img.contrast !== 1 || img.saturation !== 1 || img.gamma !== 1,
      img.color_tags,
      form.settings.scale && form.settings.scale !== preset.scale,
    ].filter(Boolean).length,
  );

  const scaleLabel = (v: string) =>
    v === "source" ? t("transcode.scale.source") : v === "1/2" ? t("transcode.scale.half") : v === "1/4" ? t("transcode.scale.quarter") : `${v}p`;
  const num = (v: string, d = 0) => (v === "" || Number.isNaN(Number(v)) ? d : Number(v));
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;

  async function pickLut() {
    const r = await open({ multiple: false, directory: false, title: t("transcode.lut"), filters: [{ name: "LUT", extensions: ["cube", "3dl", "dat", "m3d", "csp"] }] });
    if (typeof r === "string") form.settings.image.lut = r;
  }

  function resetColor() {
    Object.assign(form.settings.image, { brightness: 0, contrast: 1, saturation: 1, gamma: 1 });
  }

  function setFrame(i: 0 | 1, v: string) {
    const f: [number, number] = form.settings.image.frame ? [...form.settings.image.frame] : [1920, 1080];
    f[i] = Math.max(16, Math.round(num(v, f[i]) / 2) * 2);
    form.settings.image.frame = f;
  }
</script>

<Section id="image" title={t("transcode.section.image")} badge={active ? String(active) : ""}>
  {#if preset.video_bitrate || preset.encoder_choice}
    <p class="tc-sub">{t("transcode.encoding")}</p>
    <div class="tc-grid">
      {#if preset.video_bitrate}
        <label class="tc-field">
          {t("transcode.video.bitrate")}
          <input type="number" min="0.1" step="0.5" placeholder={t("transcode.auto")} value={form.settings.video_mbps ?? ""} oninput={(e) => (form.settings.video_mbps = e.currentTarget.value ? Number(e.currentTarget.value) : null)} />
        </label>
      {/if}
      {#if preset.encoder_choice}
        <label class="tc-check tc-wide"><input type="checkbox" bind:checked={form.settings.software} /> {t("transcode.software")}</label>
      {/if}
    </div>
  {/if}

  <p class="tc-sub">{t("transcode.size")}</p>
  <div class="tc-grid">
    {#if preset.scale_free}
      <label class="tc-field">
        {t("transcode.scale")}
        <select
          value={img.frame ? "custom" : (form.settings.scale ?? preset.scale)}
          onchange={(e) => {
            const v = e.currentTarget.value;
            if (v === "custom") form.settings.image.frame = [1920, 1080];
            else {
              form.settings.image.frame = null;
              form.settings.scale = v === preset.scale ? null : v;
            }
          }}
        >
          {#each SCALES as v (v)}<option value={v}>{scaleLabel(v)}</option>{/each}
          <option value="custom">{t("transcode.size.custom")}</option>
        </select>
      </label>
      {#if img.frame}
        <label class="tc-field">
          {t("transcode.fit")}
          <select bind:value={form.settings.image.fit}>
            <option value="pad">{t("transcode.fit.pad")}</option>
            <option value="crop">{t("transcode.fit.crop")}</option>
          </select>
        </label>
        <label class="tc-field">{t("transcode.width")}<input type="number" min="16" step="2" value={img.frame[0]} onchange={(e) => setFrame(0, e.currentTarget.value)} /></label>
        <label class="tc-field">{t("transcode.height")}<input type="number" min="16" step="2" value={img.frame[1]} onchange={(e) => setFrame(1, e.currentTarget.value)} /></label>
      {/if}
    {:else}
      <p class="tc-hint tc-wide">{t("transcode.size.fixed")}</p>
    {/if}
    <label class="tc-field">
      {t("transcode.aspect")}
      <select value={img.aspect ?? ""} onchange={(e) => (form.settings.image.aspect = e.currentTarget.value || null)}>
        <option value="">{t("transcode.none")}</option>
        {#each ASPECTS as a (a)}<option value={a}>{a}</option>{/each}
      </select>
    </label>
    <label class="tc-field">
      {t("transcode.rotate")}
      <select bind:value={form.settings.image.rotate}>
        {#each ["none", "cw90", "ccw90", "half", "flip_h", "flip_v"] as r (r)}<option value={r}>{t(`transcode.rotate.${r}`)}</option>{/each}
      </select>
    </label>
  </div>

  <p class="tc-sub">{t("transcode.crop")}</p>
  <div class="tc-grid four">
    {#each ["top", "bottom", "left", "right"] as side (side)}
      <label class="tc-field">
        {t(`transcode.crop.${side}`)}
        <input
          type="number"
          min="0"
          step="2"
          value={img.crop[side as "top"]}
          onchange={(e) => (form.settings.image.crop[side as "top"] = Math.max(0, Math.round(num(e.currentTarget.value))))}
        />
      </label>
    {/each}
  </div>

  <p class="tc-sub">{t("transcode.motion")}</p>
  <div class="tc-grid">
    <label class="tc-field">
      {t("transcode.deinterlace")}
      <select value={img.deinterlace ?? ""} onchange={(e) => (form.settings.image.deinterlace = (e.currentTarget.value || null) as typeof img.deinterlace)}>
        <option value="">{t("transcode.none")}</option>
        <option value="yadif">{t("transcode.deinterlace.yadif")}</option>
        <option value="bwdif">{t("transcode.deinterlace.bwdif")}</option>
        <option value="bwdif_field">{t("transcode.deinterlace.field")}</option>
      </select>
    </label>
    <label class="tc-field">
      {t("transcode.fps")}
      <select value={img.fps ?? ""} onchange={(e) => (form.settings.image.fps = e.currentTarget.value || null)}>
        <option value="">{t("transcode.same")}</option>
        {#each FRAME_RATES as r (r)}<option value={r}>{r.replace(".", ",")} i/s</option>{/each}
      </select>
    </label>
    {#if img.fps}
      <label class="tc-field tc-wide">
        {t("transcode.fps.method")}
        <select bind:value={form.settings.image.fps_method}>
          <option value="duplicate">{t("transcode.fps.duplicate")}</option>
          <option value="blend">{t("transcode.fps.blend")}</option>
          <option value="interpolate">{t("transcode.fps.interpolate")}</option>
        </select>
      </label>
    {/if}
    <label class="tc-check tc-wide"><input type="checkbox" bind:checked={form.settings.image.decimate} /> {t("transcode.decimate")}</label>
  </div>

  <p class="tc-sub">{t("transcode.color")}</p>
  <div class="tc-grid">
    <div class="tc-field tc-wide">
      {t("transcode.lut")}
      <div class="tc-row">
        <span class="tc-path" title={img.lut ?? ""}>{img.lut ? name(img.lut) : t("transcode.none")}</span>
        <button class="tc-btn" onclick={pickLut}>{t("offload.browse")}</button>
        {#if img.lut}<button class="tc-btn" onclick={() => (form.settings.image.lut = null)}>{t("transcode.remove.short")}</button>{/if}
      </div>
    </div>
    {#each [["brightness", -1, 1, 0.02], ["contrast", 0, 3, 0.05], ["saturation", 0, 3, 0.05], ["gamma", 0.1, 3, 0.05]] as [k, min, max, step] (k)}
      <label class="tc-field">
        {t(`transcode.color.${k}`)}
        <input
          type="number"
          {min}
          {max}
          {step}
          value={img[k as "gamma"]}
          onchange={(e) => (form.settings.image[k as "gamma"] = num(e.currentTarget.value, k === "brightness" ? 0 : 1))}
        />
      </label>
    {/each}
    <label class="tc-field">
      {t("transcode.color.tags")}
      <select value={img.color_tags ?? ""} onchange={(e) => (form.settings.image.color_tags = e.currentTarget.value || null)}>
        <option value="">{t("transcode.same")}</option>
        <option value="bt709">Rec. 709</option>
        <option value="bt2020">Rec. 2020</option>
        <option value="bt601">Rec. 601</option>
      </select>
    </label>
    <div class="tc-field"><button class="tc-btn" onclick={resetColor}>{t("transcode.color.reset")}</button></div>
  </div>
</Section>
