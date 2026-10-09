<!-- Saisie du rapport depuis le PLAYER (touche R, charte §7.3) : la ligne du
     clip en cours dans le rapport choisi, créée et pré-remplie si besoin. -->
<script lang="ts">
  import { onMount, tick } from "svelte";
  import Modal from "../../components/Modal.svelte";
  import { app } from "../../stores/app.svelte";
  import { t, i18n } from "../../i18n/index.svelte";
  import { createProject, openProject } from "../../lib/project";
  import {
    reportList,
    reportGet,
    reportCreate,
    reportSave,
    reportAddMedia,
    reportSchema,
    type Report,
    type ReportKind,
    type ReportSummary,
    type Column,
  } from "../../lib/report";

  let {
    kind,
    path,
    onClose,
  }: {
    kind: ReportKind;
    path: string;
    onClose: () => void;
  } = $props();

  let reports = $state<ReportSummary[]>([]);
  let report = $state<Report | null>(null);
  let rowIndex = $state(-1);
  let columns = $state<Column[]>([]);
  let fields = $state<Record<string, string>>({});
  let error = $state("");
  let busy = $state(false);
  let form = $state<HTMLElement | null>(null);

  const name = $derived(path.split(/[\\/]/).pop() ?? path);
  const lang = $derived(i18n.lang === "fr" ? "fr" : "en");
  // Colonnes saisies à la main (le reste vient du média).
  const auto = new Set(["file", "id", "tc_in", "tc_out", "duration", "circled"]);

  async function load(id: number | null) {
    error = "";
    busy = true;
    try {
      let r = id === null ? await reportCreate(kind) : await reportGet(id);
      let i = r.rows.findIndex((row) => row.clip === path);
      if (i < 0) {
        r = await reportAddMedia(r, [path]);
        i = r.rows.length - 1;
      }
      report = r;
      rowIndex = i;
      fields = { ...r.rows[i].fields };
      const s = await reportSchema(r.kind, r.columns ?? [], r.tracks, lang);
      columns = s.columns;
      reports = await reportList();
      await tick();
      form?.querySelector<HTMLInputElement>("input")?.focus();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  onMount(async () => {
    if (!app.project) return;
    try {
      reports = await reportList();
      // Rapport le plus récemment modifié de ce type ; sinon un nouveau.
      const mine = reports.filter((r) => r.kind === kind).sort((a, b) => b.updated_at.localeCompare(a.updated_at));
      await load(mine[0]?.id ?? null);
    } catch (e) {
      error = String(e);
    }
  });

  async function withProject(action: () => Promise<boolean>) {
    if (await action()) load(null);
  }

  async function save() {
    if (!report) return;
    busy = true;
    try {
      report.rows[rowIndex].fields = { ...fields };
      await reportSave($state.snapshot(report) as Report);
      app.status = `${t("report.popup.saved")} ${name}`;
      onClose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  const label = (r: ReportSummary) =>
    `${t(r.kind === "image" ? "report.kind.image" : "report.kind.sound")} ${t("report.number.short")} ${r.number}${r.title ? ` · ${r.title}` : ""}`;
</script>

<Modal title={`${t("report.popup.title")} : ${name}`} {onClose} width="min(640px, 94vw)">
  <div class="body">
    {#if !app.project}
      <p>{t("report.need.project")}</p>
      <div class="buttons">
        <button class="primary" onclick={() => withProject(createProject)}>{t("project.new")}</button>
        <button onclick={() => withProject(openProject)}>{t("logs.need.project.open")}</button>
        <button class="ghost" onclick={onClose}>{t("logs.cancel")}</button>
      </div>
    {:else}
      <div class="target">
        <label>
          {t("report.popup.report")}
          <select
            value={report?.id ?? 0}
            onchange={(e) => {
              const v = Number(e.currentTarget.value);
              load(v === 0 ? null : v);
            }}
          >
            {#each reports.filter((r) => r.kind === kind) as r (r.id)}
              <option value={r.id}>{label(r)}</option>
            {/each}
            <option value={0}>+ {t(kind === "image" ? "report.new.image" : "report.new.sound")}</option>
          </select>
        </label>
        {#if fields.tc_in}<span class="mono tc">{fields.tc_in}{fields.tc_out ? ` → ${fields.tc_out}` : ""}</span>{/if}
      </div>
      {#if error}<p class="error">{error}</p>{/if}
      <form
        bind:this={form}
        class="grid"
        onsubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label class="circled wide">
          <input
            type="checkbox"
            checked={!!fields.circled}
            onchange={(e) => (fields.circled = e.currentTarget.checked ? "●" : "")}
          />
          <span>{t("report.circled.take")}</span>
        </label>
        {#each columns.filter((c) => !auto.has(c.key)) as c (c.key)}
          <label class:wide={c.key === "notes"}>
            <span>{c.label}</span>
            <input bind:value={fields[c.key]} />
          </label>
        {/each}
        <button type="submit" hidden aria-hidden="true"></button>
      </form>
      <div class="buttons">
        <span class="small muted">{t("report.popup.hint")}</span>
        <button class="ghost" onclick={onClose}>{t("logs.cancel")}</button>
        <button class="primary" disabled={busy || !report} onclick={save}>{t("report.popup.save")}</button>
      </div>
    {/if}
  </div>
</Modal>

<style>
  .body {
    padding: var(--vf-space-3);
    font-size: var(--vf-text-sm);
  }
  p {
    margin: 0 0 var(--vf-space-3);
  }
  .target {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: var(--vf-space-3);
  }
  .target label {
    display: flex;
    align-items: center;
    color: var(--vf-text-muted);
  }
  .target select {
    margin-left: var(--vf-space-2);
    max-width: 360px;
  }
  .tc {
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: var(--vf-space-2) var(--vf-space-3);
    max-height: 52vh;
    overflow-y: auto;
  }
  .grid label {
    display: flex;
    flex-direction: column;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
    min-width: 0;
  }
  .grid label.wide {
    grid-column: 1 / -1;
  }
  .grid input {
    margin-top: 2px;
    width: 100%;
  }
  .grid label.circled {
    flex-direction: row;
    align-items: center;
    font-size: var(--vf-text-sm);
    color: var(--vf-text);
  }
  .grid label.circled input {
    width: auto;
    margin: 0 var(--vf-space-2) 0 0;
    accent-color: var(--vf-accent);
  }
  button,
  select,
  input {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-size: var(--vf-text-sm);
  }
  input {
    background: var(--vf-bg);
  }
  button {
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.45;
    cursor: default;
  }
  .primary {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: transparent;
    font-weight: 600;
  }
  .ghost {
    background: none;
  }
  .buttons {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    margin-top: var(--vf-space-3);
  }
  .buttons > * + * {
    margin-left: var(--vf-space-2);
  }
  .buttons .small {
    margin-right: auto;
  }
  .small {
    font-size: var(--vf-text-xs);
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .error {
    color: var(--vf-error);
  }
</style>
