<!-- Préréglages personnels : réglages complets enregistrés sous un nom,
     exportés ou importés en fichier .vfpreset pour les partager. -->
<script lang="ts">
  import { open, save } from "@tauri-apps/plugin-dialog";
  import { t } from "../../i18n/index.svelte";
  import { ask } from "../../stores/confirm.svelte";
  import { app } from "../../stores/app.svelte";
  import { readPreset, writePreset } from "../../lib/transcode";
  import { exportPreset, importPreset, loadUserPresets, saveUserPresets, type Form, type UserPreset } from "../../lib/transcodeForm.svelte";

  let { form, onApply }: { form: Form; onApply: (f: Form) => void } = $props();

  let list = $state<UserPreset[]>(loadUserPresets());
  let current = $state("");
  let naming = $state(false);
  let newName = $state("");
  let message = $state("");

  const mine = $derived(list.filter((p) => p.mode === app.mode));

  function apply(name: string) {
    current = name;
    const p = mine.find((x) => x.name === name);
    if (p) onApply(JSON.parse(JSON.stringify(p.form)));
  }

  function store() {
    const name = newName.trim();
    if (!name) return;
    const entry: UserPreset = { name, mode: app.mode, form: JSON.parse(JSON.stringify(form)) };
    entry.form.settings.trims = {};
    list = [...list.filter((p) => !(p.mode === app.mode && p.name === name)), entry];
    saveUserPresets(list);
    current = name;
    naming = false;
    newName = "";
    message = t("transcode.user.saved");
  }

  async function remove() {
    if (!current) return;
    const ok = await ask(`${t("transcode.user.delete.confirm")} « ${current} » ?`, { title: t("transcode.user.delete"), ok: t("transcode.user.delete"), kind: "warning" });
    if (!ok) return;
    list = list.filter((p) => !(p.mode === app.mode && p.name === current));
    saveUserPresets(list);
    current = "";
  }

  async function exportFile() {
    const p = mine.find((x) => x.name === current);
    if (!p) return;
    const path = await save({ defaultPath: `${p.name}.vfpreset`, filters: [{ name: "VERIFLOW", extensions: ["vfpreset"] }] });
    if (!path) return;
    try {
      await writePreset(path, exportPreset(p));
      message = t("transcode.user.exported");
    } catch (e) {
      message = String(e);
    }
  }

  async function importFile() {
    const path = await open({ multiple: false, directory: false, filters: [{ name: "VERIFLOW", extensions: ["vfpreset", "json"] }] });
    if (typeof path !== "string") return;
    try {
      const p = importPreset(await readPreset(path));
      if (p.mode !== app.mode) {
        message = t(p.mode === "audio" ? "transcode.user.wrong.audio" : "transcode.user.wrong.video");
        return;
      }
      list = [...list.filter((x) => !(x.mode === p.mode && x.name === p.name)), p];
      saveUserPresets(list);
      apply(p.name);
      message = t("transcode.user.imported");
    } catch (e) {
      message = String(e);
    }
  }
</script>

<div class="user">
  <div class="tc-row">
    <select class="tc-input" value={current} onchange={(e) => apply(e.currentTarget.value)} aria-label={t("transcode.user")}>
      <option value="">{t("transcode.user.choose")} ({mine.length})</option>
      {#each mine as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
    </select>
    <button class="tc-btn" onclick={() => ((naming = !naming), (newName = current))} title={t("transcode.user.save")}>{t("transcode.user.save")}</button>
  </div>
  {#if naming}
    <div class="tc-row">
      <input
        class="tc-input"
        placeholder={t("transcode.user.name")}
        bind:value={newName}
        onkeydown={(e) => {
          if (e.key === "Enter") store();
          if (e.key === "Escape") naming = false;
        }}
      />
      <button class="tc-btn" disabled={!newName.trim()} onclick={store}>OK</button>
    </div>
  {/if}
  <div class="tc-row">
    <button class="tc-btn" onclick={importFile}>{t("transcode.user.import")}</button>
    <button class="tc-btn" disabled={!current} onclick={exportFile}>{t("transcode.user.export")}</button>
    <button class="tc-btn" disabled={!current} onclick={remove}>{t("transcode.user.delete")}</button>
  </div>
  {#if message}<p class="tc-hint">{message}</p>{/if}
</div>

<style>
  .user > :global(* + *) {
    margin-top: var(--vf-space-2);
  }
</style>
