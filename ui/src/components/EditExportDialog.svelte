<!-- Export des clips vers un logiciel de montage : choix du logiciel (formats
     proposés), de la portée et des options, puis liste des fichiers écrits. -->
<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import Modal from "./Modal.svelte";
  import { t } from "../i18n/index.svelte";
  import { reveal } from "../lib/offload";
  import {
    EDIT_FORMATS,
    EDIT_TARGETS,
    TARGET_FORMATS,
    TARGET_FCPXML,
    relativeAllowed,
    editorialExport,
    type EditFormat,
    type EditTarget,
    type EditResult,
    type FcpxmlVersion,
  } from "../lib/editorial";

  let {
    selectionPaths,
    folderPaths,
    base,
    defaultTitle,
    onClose,
  }: {
    selectionPaths: string[];
    folderPaths: string[];
    base: string;
    defaultTitle: string;
    onClose: () => void;
  } = $props();

  let target = $state<EditTarget>("fcp");
  let formats = $state<EditFormat[]>(["fcpxml"]);
  let scopeChoice = $state<"selection" | "folder" | null>(null);
  let titleEdit = $state<string | null>(null);
  const scope = $derived(scopeChoice ?? (selectionPaths.length > 0 ? "selection" : "folder"));
  const title = $derived(titleEdit ?? defaultTitle);
  let pathMode = $state<"absolute" | "relative">("absolute");
  let markers = $state(true);
  let version = $state<FcpxmlVersion>("1.11");
  let ascii = $state(false);
  let busy = $state(false);
  let error = $state("");
  let result = $state<EditResult | null>(null);

  const paths = $derived(scope === "selection" ? selectionPaths : folderPaths);
  const canRelative = $derived(relativeAllowed(formats));
  const hasFcpxml = $derived(formats.includes("fcpxml"));
  const hasAle = $derived(formats.includes("ale"));

  $effect(() => {
    if (!canRelative && pathMode === "relative") pathMode = "absolute";
  });

  function pick(next: EditTarget) {
    target = next;
    formats = [...TARGET_FORMATS[next]];
    version = TARGET_FCPXML[next];
  }

  function toggle(f: EditFormat, on: boolean) {
    formats = on ? [...formats, f] : formats.filter((x) => x !== f);
  }

  const warn = (w: { code: string; detail: string }) => {
    const k = `edit.warn.${w.code}`;
    return t(k) === k ? w.detail : t(k);
  };
  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;

  async function run() {
    if (formats.length === 0 || paths.length === 0) return;
    const r = await open({ directory: true, multiple: false, title: t("edit.out") });
    if (typeof r !== "string") return;
    busy = true;
    error = "";
    result = null;
    try {
      result = await editorialExport(
        formats,
        paths,
        { title, paths: pathMode, base, markers, fcpxml_version: version, ale_ascii: ascii },
        r,
      );
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<Modal title={t("edit.title")} {onClose} width="min(560px, 94vw)">
  <div class="form">
    {#if result}
      <p class="ok">{result.written.length} {t("edit.done")}</p>
      <ul class="files mono">
        {#each result.written as w (w)}
          <li title={w}>{name(w)}</li>
        {/each}
      </ul>
      {#each result.warnings as w, i (i)}
        <p class="warn">{warn(w)}</p>
      {/each}
      <div class="buttons">
        <button onclick={() => result?.written[0] && reveal(result.written[0]).catch(() => {})}>{t("edit.reveal")}</button>
        <button class="primary" onclick={onClose}>{t("edit.close")}</button>
      </div>
    {:else}
      <fieldset>
        <legend>{t("edit.target")}</legend>
        <div class="seg">
          {#each EDIT_TARGETS as k (k)}
            <button class:on={target === k} onclick={() => pick(k)}>{t(`edit.target.${k}`)}</button>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend>{t("edit.formats")}</legend>
        <div class="row">
          {#each EDIT_FORMATS as f (f.id)}
            <label class="check">
              <input type="checkbox" checked={formats.includes(f.id)} onchange={(e) => toggle(f.id, e.currentTarget.checked)} />
              {f.label}
            </label>
          {/each}
        </div>
      </fieldset>

      <fieldset>
        <legend>{t("edit.scope")}</legend>
        <div class="row">
          <label class="check">
            <input type="radio" name="scope" checked={scope === "selection"} onchange={() => (scopeChoice = "selection")} disabled={selectionPaths.length === 0} />
            {t("edit.scope.selection")} ({selectionPaths.length})
          </label>
          <label class="check">
            <input type="radio" name="scope" checked={scope === "folder"} onchange={() => (scopeChoice = "folder")} />
            {t("edit.scope.folder")} ({folderPaths.length})
          </label>
        </div>
      </fieldset>

      <label class="field">
        <span>{t("edit.name")}</span>
        <input type="text" value={title} oninput={(e) => (titleEdit = e.currentTarget.value)} />
      </label>

      <fieldset>
        <legend>{t("edit.paths")}</legend>
        <div class="row">
          <label class="check"><input type="radio" bind:group={pathMode} value="absolute" /> {t("edit.paths.absolute")}</label>
          <label class="check" class:off={!canRelative}>
            <input type="radio" bind:group={pathMode} value="relative" disabled={!canRelative} /> {t("edit.paths.relative")}
          </label>
        </div>
        {#if !canRelative}<p class="hint">{t("edit.paths.relative.hint")}</p>{/if}
      </fieldset>

      <div class="row">
        <label class="check"><input type="checkbox" bind:checked={markers} /> {t("edit.markers")}</label>
        {#if hasAle}
          <label class="check"><input type="checkbox" bind:checked={ascii} /> {t("edit.ale.ascii")}</label>
        {/if}
        {#if hasFcpxml}
          <label class="check">
            {t("edit.fcpxml.version")}
            <select bind:value={version} onchange={(e) => e.currentTarget.blur()}>
              <option value="1.9">1.9</option>
              <option value="1.10">1.10</option>
              <option value="1.11">1.11</option>
            </select>
          </label>
        {/if}
      </div>

      {#if error}<p class="err">{error}</p>{/if}
      <div class="buttons">
        <button onclick={onClose}>{t("dialog.cancel")}</button>
        <button class="primary" disabled={busy || formats.length === 0 || paths.length === 0} onclick={run}>
          {busy ? t("edit.running") : t("edit.run")}
        </button>
      </div>
    {/if}
  </div>
</Modal>

<style>
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-3);
    padding: var(--vf-space-3);
    font-size: var(--vf-text-sm);
  }
  fieldset {
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-2);
    margin: 0;
  }
  legend {
    color: var(--vf-text-muted);
    padding: 0 var(--vf-space-1);
  }
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--vf-space-3);
  }
  .seg {
    display: flex;
    flex-wrap: wrap;
  }
  .seg button.on {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: var(--vf-accent);
  }
  .check {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
  }
  .check input {
    accent-color: var(--vf-accent);
  }
  .check.off {
    color: var(--vf-text-disabled);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-1);
  }
  .hint {
    margin: var(--vf-space-1) 0 0;
    color: var(--vf-text-muted);
  }
  .files {
    margin: 0;
    padding: var(--vf-space-2);
    list-style: none;
    max-height: 200px;
    overflow: auto;
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
  }
  .ok {
    margin: 0;
    color: var(--vf-text);
  }
  .warn {
    margin: 0;
    color: var(--vf-warning);
  }
  .err {
    margin: 0;
    color: var(--vf-error);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--vf-space-2);
  }
  .buttons .primary {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: var(--vf-accent);
  }
  .buttons .primary:disabled {
    background: var(--vf-surface-high);
    color: var(--vf-text-disabled);
    border-color: var(--vf-border);
  }
</style>
