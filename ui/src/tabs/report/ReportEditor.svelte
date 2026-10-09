<!-- Éditeur d'un rapport : en-tête (champs du rapport papier), tableau d'une
     ligne par prise, exports. Enregistrement automatique dans le projet. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import ChoiceField from "./ChoiceField.svelte";
  import ColumnsDialog from "./ColumnsDialog.svelte";
  import { app } from "../../stores/app.svelte";
  import { ask } from "../../stores/confirm.svelte";
  import { t, i18n } from "../../i18n/index.svelte";
  import { reveal } from "../../lib/offload";
  import {
    reportSave,
    reportDelete,
    reportSchema,
    reportAddMedia,
    reportExport,
    pickMedia,
    pickExportPath,
    EXPORT_FORMATS,
    MAX_TRACKS,
    type Report,
    type ReportSchema,
    type ExportFormat,
    type ReportRow,
  } from "../../lib/report";

  let {
    report = $bindable(),
    onSaved,
    onDeleted,
  }: {
    report: Report;
    onSaved: () => void;
    onDeleted: () => void;
  } = $props();

  let schema = $state<ReportSchema | null>(null);
  let saving = $state<"" | "pending" | "saved" | "error">("");
  let error = $state("");
  let exported = $state("");
  let busy = $state(false);

  const lang = $derived(i18n.lang === "fr" ? "fr" : "en");
  const fields = $derived(new Map((schema?.header ?? []).map((f) => [f.key, f])));
  const label = (key: string) => {
    const f = fields.get(key);
    return f ? (lang === "fr" ? f.label_fr : f.label_en) : key;
  };

  $effect(() => {
    const k = report.kind;
    const keys = [...(report.columns ?? [])];
    const n = report.tracks;
    reportSchema(k, keys, n, lang)
      .then((s) => (schema = s))
      .catch((e) => (error = String(e)));
  });

  // Enregistrement automatique, une demi-seconde après la dernière saisie.
  let timer = 0;
  function touch() {
    saving = "pending";
    window.clearTimeout(timer);
    timer = window.setTimeout(save, 500);
  }
  async function save() {
    window.clearTimeout(timer);
    try {
      const saved = await reportSave($state.snapshot(report) as Report);
      if (report.number === 0) report.number = saved.number;
      saving = "saved";
      onSaved();
    } catch (e) {
      saving = "error";
      error = String(e);
    }
  }
  onDestroy(() => {
    if (saving === "pending") save();
  });

  // Champs d'en-tête : groupes du rapport papier.
  const groups = $derived(
    report.kind === "image"
      ? [
          { title: "report.group.production", keys: ["date", "title", "director", "dop", "operator"] },
          { title: "report.group.camera", keys: ["camera", "image_format", "sound_ref", "definition", "fps", "media"] },
        ]
      : [
          { title: "report.group.production", keys: ["date", "title", "director", "sound_engineer", "boom"] },
          { title: "report.group.recorder", keys: ["recorder", "timecode", "sound_ref", "recorder_other", "film", "fps", "sample_rate", "bits"] },
        ],
  );

  function setHeader(key: string, value: string) {
    report.header[key] = value;
    touch();
  }

  // ---------- Lignes ----------

  function addRow() {
    report.rows.push({ clip: null, fields: {} });
    touch();
  }

  function removeRow(i: number) {
    report.rows.splice(i, 1);
    touch();
  }

  function moveRow(i: number, delta: number) {
    const j = i + delta;
    if (j < 0 || j >= report.rows.length) return;
    const [r] = report.rows.splice(i, 1);
    report.rows.splice(j, 0, r);
    touch();
  }

  function setCell(row: ReportRow, key: string, value: string) {
    row.fields[key] = value;
    touch();
  }

  async function addMedia() {
    const paths = await pickMedia(report.kind, t(report.kind === "image" ? "report.media.video" : "report.media.sound"));
    if (paths.length === 0) return;
    busy = true;
    error = "";
    try {
      const updated = await reportAddMedia($state.snapshot(report) as Report, paths);
      report.rows = updated.rows;
      report.header = updated.header;
      report.tracks = updated.tracks;
      await save();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  function setTracks(v: number) {
    report.tracks = Math.max(1, Math.min(MAX_TRACKS, Math.round(v) || 1));
    touch();
  }

  // ---------- Exports et suppression ----------

  const fileName = () =>
    `${t(report.kind === "image" ? "report.file.image" : "report.file.sound")}_${report.number}` +
    (report.header.title ? `_${report.header.title.replace(/[\\/:*?"<>|]+/g, "_").trim()}` : "");

  async function exportAs(format: ExportFormat) {
    const dest = await pickExportPath(fileName(), format);
    if (!dest) return;
    busy = true;
    error = "";
    try {
      if (saving === "pending") await save();
      exported = await reportExport($state.snapshot(report) as Report, format, dest, lang);
      app.status = `${t("report.exported")} ${exported}`;
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function remove() {
    const ok = await ask(t("report.delete.confirm"), { title: t("report.delete"), ok: t("report.delete"), kind: "danger" });
    if (!ok) return;
    window.clearTimeout(timer);
    saving = "";
    try {
      await reportDelete(report.id);
      onDeleted();
    } catch (e) {
      error = String(e);
    }
  }

  // ---------- Colonnes et tri ----------

  let columnsOpen = $state(false);

  function applyColumns(keys: string[]) {
    columnsOpen = false;
    report.columns = keys;
    touch();
  }

  // Tri des prises par une colonne (clic sur son titre) : croissant, puis
  // décroissant. Comparaison « naturelle » : 2 avant 10, 12/2 avant 12/10.
  let sortKey = $state<string | null>(null);
  let sortDesc = $state(false);
  const collator = new Intl.Collator(undefined, { numeric: true, sensitivity: "base" });

  function sortBy(key: string) {
    sortDesc = sortKey === key ? !sortDesc : false;
    sortKey = key;
    const dir = sortDesc ? -1 : 1;
    const value = (r: ReportRow) => r.fields[key] ?? "";
    report.rows = [...report.rows].sort((a, b) => {
      const va = value(a);
      const vb = value(b);
      // Cellules vides toujours en fin de liste.
      if (!va || !vb) return va ? -1 : vb ? 1 : 0;
      return collator.compare(va, vb) * dir;
    });
    touch();
  }

  // Prise cerclée : case à cocher en tête de ligne (entourée sur le PDF).
  const tableCols = $derived((schema?.columns ?? []).filter((c) => c.key !== "circled"));

  const monoKeys = new Set(["tc_in", "tc_out", "duration", "sound_tc"]);
</script>

{#snippet arrow(key: string)}
  {#if sortKey === key}
    <svg class="arrow" viewBox="0 0 10 10" aria-hidden="true">
      <path d={sortDesc ? "M2 3.5 L5 6.5 L8 3.5" : "M2 6.5 L5 3.5 L8 6.5"} fill="none" stroke="currentColor" stroke-width="1.4" />
    </svg>
  {/if}
{/snippet}

{#if columnsOpen && schema}
  <ColumnsDialog catalog={schema.catalog} keys={schema.keys} {lang} onApply={applyColumns} onClose={() => (columnsOpen = false)} />
{/if}

<div class="editor">
  <div class="toolbar">
    <h3>
      {t(report.kind === "image" ? "report.kind.image" : "report.kind.sound")}
      {t("report.number.short")}
      <input
        class="num mono"
        type="number"
        min="1"
        value={report.number}
        onchange={(e) => ((report.number = Math.max(1, Number(e.currentTarget.value) || 1)), touch())}
        aria-label={t("report.number")}
      />
    </h3>
    <button onclick={() => (columnsOpen = true)} disabled={!schema} title={t("report.columns.hint")}>{t("report.columns")}</button>
    {#if report.kind === "sound"}
      <label class="inline">
        {t("report.tracks")}
        <input class="num" type="number" min="1" max={MAX_TRACKS} value={report.tracks} onchange={(e) => setTracks(Number(e.currentTarget.value))} />
      </label>
    {/if}
    <span class="status small">
      {#if saving === "pending"}{t("report.saving")}{:else if saving === "saved"}{t("report.saved")}{:else if saving === "error"}<span class="error">{t("report.save.error")}</span>{/if}
    </span>
    <span class="spacer"></span>
    <span class="small muted">{t("report.export")}</span>
    {#each EXPORT_FORMATS.filter((f) => f.id !== "edl" || report.kind === "image") as f (f.id)}
      <button disabled={busy} onclick={() => exportAs(f.id)} title={f.id === "edl" ? t("report.edl.hint") : ""}>{f.label}</button>
    {/each}
    <button class="ghost danger" onclick={remove} title={t("report.delete")}>{t("report.delete")}</button>
  </div>

  {#if error}<p class="error banner">{error}</p>{/if}
  {#if exported}
    <p class="banner ok">
      {t("report.exported")} <span class="mono">{exported}</span>
      <button class="link" onclick={() => reveal(exported).catch((e) => (error = String(e)))}>{t("report.open")}</button>
    </p>
  {/if}

  <div class="scroll">
    {#if schema}
      <div class="header">
        {#each groups as g (g.title)}
          <fieldset>
            <legend>{t(g.title)}</legend>
            {#each g.keys as key (key)}
              {@const f = fields.get(key)}
              {#if f}
                <label>
                  <span>{label(key)}</span>
                  {#if key === "date"}
                    <input type="date" value={report.header[key] ?? ""} oninput={(e) => setHeader(key, e.currentTarget.value)} />
                  {:else if f.options.length + f.extra.length > 0}
                    <ChoiceField
                      value={report.header[key] ?? ""}
                      options={[...f.options, ...f.extra]}
                      unit={f.unit}
                      onChange={(v) => setHeader(key, v)}
                    />
                  {:else}
                    <span class="value">
                      <input value={report.header[key] ?? ""} oninput={(e) => setHeader(key, e.currentTarget.value)} />
                      {#if f.unit}<span class="unit">{f.unit}</span>{/if}
                    </span>
                  {/if}
                </label>
              {/if}
            {/each}
          </fieldset>
        {/each}
        <fieldset class="side">
          <legend>{t("report.group.sheet")}</legend>
          <label>
            <span>{label("backup")}</span>
            <input value={report.header.backup ?? ""} oninput={(e) => setHeader("backup", e.currentTarget.value)} />
          </label>
          <label class="grow">
            <span>{label("remarks")}</span>
            <textarea rows="4" value={report.header.remarks ?? ""} oninput={(e) => setHeader("remarks", e.currentTarget.value)}></textarea>
          </label>
        </fieldset>
      </div>

      <div class="rows-bar">
        <h4>{t("report.takes")} <span class="muted">({report.rows.length})</span></h4>
        <button class="primary" disabled={busy} onclick={addMedia}>+ {t(report.kind === "image" ? "report.add.video" : "report.add.sound")}</button>
        <button onclick={addRow}>+ {t("report.add.row")}</button>
        <span class="small muted">{t("report.add.hint")}</span>
      </div>

      <div class="table-wrap">
        <table>
          <colgroup>
            <col style:width="64px" />
            {#each tableCols as c (c.key)}<col style:width={`${Math.max(72, Math.round(c.width * 80))}px`} />{/each}
            <col style:width="78px" />
          </colgroup>
          <thead>
            <tr>
              <th title={t("report.circled.hint")}>
                <button class="sort" onclick={() => sortBy("circled")}>{t("report.circled")}{@render arrow("circled")}</button>
              </th>
              {#each tableCols as c (c.key)}
                <th><button class="sort" onclick={() => sortBy(c.key)} title={t("report.sort")}>{c.label}{@render arrow(c.key)}</button></th>
              {/each}
              <th></th>
            </tr>
          </thead>
          <tbody>
            {#each report.rows as row, i (i)}
              <tr>
                <td class="circ">
                  <input
                    type="checkbox"
                    checked={!!row.fields.circled}
                    onchange={(e) => setCell(row, "circled", e.currentTarget.checked ? "●" : "")}
                    aria-label={t("report.circled")}
                  />
                </td>
                {#each tableCols as c (c.key)}
                  <td>
                    <input
                      class:mono={monoKeys.has(c.key)}
                      value={row.fields[c.key] ?? ""}
                      oninput={(e) => setCell(row, c.key, e.currentTarget.value)}
                      title={c.key === "file" || c.key === "id" ? (row.clip ?? "") : ""}
                    />
                  </td>
                {/each}
                <td class="acts">
                  <button class="icon" onclick={() => moveRow(i, -1)} disabled={i === 0} aria-label={t("report.row.up")} title={t("report.row.up")}>
                    <svg viewBox="0 0 12 12"><path d="M3 7.5 L6 4.5 L9 7.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
                  </button>
                  <button class="icon" onclick={() => moveRow(i, 1)} disabled={i === report.rows.length - 1} aria-label={t("report.row.down")} title={t("report.row.down")}>
                    <svg viewBox="0 0 12 12"><path d="M3 4.5 L6 7.5 L9 4.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
                  </button>
                  <button class="icon" onclick={() => removeRow(i)} aria-label={t("report.row.delete")} title={t("report.row.delete")}>
                    <svg viewBox="0 0 12 12"><path d="M3 3 L9 9 M9 3 L3 9" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
                  </button>
                </td>
              </tr>
            {:else}
              <tr><td class="empty" colspan={tableCols.length + 2}>{t("report.rows.empty")}</td></tr>
            {/each}
          </tbody>
        </table>
      </div>
    {/if}
  </div>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .toolbar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    padding: var(--vf-space-2) var(--vf-space-3);
    border-bottom: 1px solid var(--vf-border);
    background: var(--vf-surface);
  }
  .toolbar > * {
    margin: 2px var(--vf-space-2) 2px 0;
  }
  h3 {
    margin: 0 var(--vf-space-3) 0 0;
    font-size: var(--vf-text-md);
    display: flex;
    align-items: center;
  }
  h3 input {
    margin-left: var(--vf-space-1);
  }
  h4 {
    margin: 0 var(--vf-space-2) 0 0;
    font-size: var(--vf-text-md);
  }
  .inline {
    display: flex;
    align-items: center;
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  .inline > * {
    margin-left: var(--vf-space-1);
  }
  .spacer {
    flex: 1;
  }
  button,
  input,
  textarea {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-size: var(--vf-text-sm);
  }
  input,
  textarea {
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
  .danger {
    color: var(--vf-error);
  }
  .link {
    background: none;
    border: 0;
    color: var(--vf-accent);
    text-decoration: underline;
    padding: 0 var(--vf-space-1);
  }
  .num {
    width: 64px;
  }
  .status {
    min-width: 90px;
    color: var(--vf-text-muted);
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
  .banner {
    margin: 0;
    padding: var(--vf-space-1) var(--vf-space-3);
    font-size: var(--vf-text-sm);
    border-bottom: 1px solid var(--vf-border);
    overflow-wrap: anywhere;
  }
  .banner.ok {
    color: var(--vf-success);
  }
  .scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: var(--vf-space-3);
  }
  .header {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1fr) minmax(0, 1fr);
    gap: var(--vf-space-3);
  }
  fieldset {
    margin: 0;
    padding: var(--vf-space-2) var(--vf-space-3) var(--vf-space-3);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  legend {
    padding: 0 var(--vf-space-1);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  fieldset label {
    display: grid;
    grid-template-columns: 120px minmax(0, 1fr);
    align-items: center;
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
    margin-top: var(--vf-space-1);
  }
  fieldset label input,
  fieldset label textarea {
    width: 100%;
    color: var(--vf-text);
  }
  fieldset label input {
    height: 26px;
  }
  fieldset.side label {
    grid-template-columns: minmax(0, 1fr);
  }
  fieldset.side label span {
    margin-bottom: 2px;
  }
  .grow textarea {
    resize: vertical;
    min-height: 70px;
  }
  .value {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  .value input {
    flex: 1;
    min-width: 0;
  }
  .unit {
    margin-left: var(--vf-space-1);
    font-size: var(--vf-text-xs);
    white-space: nowrap;
  }
  .rows-bar {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    margin: var(--vf-space-4) 0 var(--vf-space-2);
  }
  .rows-bar > * {
    margin-right: var(--vf-space-2);
  }
  .table-wrap {
    overflow-x: auto;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
  }
  table {
    border-collapse: collapse;
    table-layout: fixed;
    min-width: 100%;
    font-size: var(--vf-text-sm);
  }
  th {
    position: sticky;
    top: 0;
    background: var(--vf-surface-high);
    text-align: left;
    font-weight: 600;
    padding: var(--vf-space-1) var(--vf-space-2);
    border-bottom: 1px solid var(--vf-border);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  td {
    padding: 1px;
    border-bottom: 1px solid var(--vf-border);
  }
  td input {
    width: 100%;
    border-color: transparent;
    background: none;
    padding: 3px var(--vf-space-1);
  }
  td input:focus {
    border-color: var(--vf-accent);
    background: var(--vf-bg);
  }
  th .sort {
    display: inline-flex;
    align-items: center;
    width: 100%;
    padding: 0;
    border: 0;
    background: none;
    font: inherit;
    font-weight: 600;
    color: inherit;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
  }
  th .sort:hover {
    color: var(--vf-accent);
  }
  .arrow {
    width: 10px;
    height: 10px;
    margin-left: 3px;
    flex: none;
    color: var(--vf-accent);
  }
  td.circ {
    text-align: center;
  }
  td.circ input {
    width: auto;
    accent-color: var(--vf-accent);
  }
  td.empty {
    padding: var(--vf-space-4);
    text-align: center;
    color: var(--vf-text-muted);
  }
  .acts {
    white-space: nowrap;
    text-align: right;
  }
  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 22px;
    padding: 0;
    background: none;
    border-color: transparent;
    color: var(--vf-text-muted);
  }
  .icon:hover:not(:disabled) {
    color: var(--vf-text);
    background: var(--vf-surface-hover);
  }
  .icon svg {
    width: 12px;
    height: 12px;
  }
</style>
