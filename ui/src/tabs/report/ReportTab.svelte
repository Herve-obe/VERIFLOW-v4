<!-- Onglet REPORT : rapports image (mode VIDEO) et son (mode AUDIO) du projet.
     Liste des rapports à gauche, éditeur à droite (charte §7.4). -->
<script lang="ts">
  import ReportEditor from "./ReportEditor.svelte";
  import { app } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import { createProject, openProject } from "../../lib/project";
  import {
    reportList,
    reportGet,
    reportCreate,
    reportAddMedia,
    reportSave,
    kindOfMode,
    brandingGet,
    brandingSet,
    pickLogo,
    type Report,
    type ReportSummary,
    type ReportTemplate,
    type ReportBranding,
  } from "../../lib/report";

  let list = $state<ReportSummary[]>([]);
  let current = $state<Report | null>(null);
  let template = $state<ReportTemplate>("school");
  let branding = $state<ReportBranding>({ logo: null, organization: "" });
  let error = $state("");

  const kind = $derived(kindOfMode(app.mode));
  const visible = $derived(list.filter((r) => r.kind === kind));

  async function refresh() {
    try {
      list = app.project ? await reportList() : [];
      branding = app.project ? await brandingGet() : { logo: null, organization: "" };
    } catch (e) {
      error = String(e);
    }
  }

  // Projet ouvert ou fermé : liste rechargée, éditeur vidé.
  $effect(() => {
    void app.project?.path;
    current = null;
    refresh();
  });

  // Changement de mode : on n'affiche que les rapports du type correspondant.
  $effect(() => {
    if (current && current.kind !== kind) current = null;
  });

  async function select(id: number) {
    error = "";
    try {
      current = await reportGet(id);
    } catch (e) {
      error = String(e);
    }
  }

  async function create() {
    error = "";
    try {
      current = await reportCreate(kind, template);
      await refresh();
    } catch (e) {
      error = String(e);
    }
  }

  // Médias envoyés depuis MEDIA : ajoutés au rapport ouvert, ou à un nouveau.
  $effect(() => {
    const paths = app.reportMedia;
    if (!paths || !app.project) return;
    app.reportMedia = null;
    (async () => {
      try {
        if (!current || current.kind !== kind) current = await reportCreate(kind, template);
        current = await reportSave(await reportAddMedia(current, paths));
        await refresh();
      } catch (e) {
        error = String(e);
      }
    })();
  });

  async function saveBranding() {
    try {
      await brandingSet(branding);
    } catch (e) {
      error = String(e);
    }
  }

  async function chooseLogo() {
    const p = await pickLogo(t("report.logo"));
    if (!p) return;
    branding.logo = p;
    saveBranding();
  }

  const label = (r: ReportSummary) =>
    `${t(r.kind === "image" ? "report.kind.image" : "report.kind.sound")} ${t("report.number.short")} ${r.number}`;
</script>

{#if !app.project}
  <section class="empty">
    <h2>{t("tab.report")} <span class="mode">{t("mode." + app.mode)}</span></h2>
    <p>{t("report.need.project")}</p>
    <div class="buttons">
      <button class="primary" onclick={createProject}>{t("project.new")}</button>
      <button onclick={openProject}>{t("logs.need.project.open")}</button>
    </div>
  </section>
{:else}
  <div class="layout">
    <aside>
      <h2>{t("tab.report")} <span class="mode">{t("mode." + app.mode)}</span></h2>
      <div class="new">
        <select bind:value={template} aria-label={t("report.template")}>
          <option value="school">{t("report.template.school")}</option>
          <option value="pro">{t("report.template.pro")}</option>
        </select>
        <button class="primary" onclick={create}>+ {t(kind === "image" ? "report.new.image" : "report.new.sound")}</button>
      </div>
      <ul class="list">
        {#each visible as r (r.id)}
          <li>
            <button class:sel={current?.id === r.id} onclick={() => select(r.id)}>
              <span class="name">{label(r)}</span>
              <span class="sub">{r.title || t("report.untitled")}</span>
              <span class="sub mono">{r.date} · {r.rows} {t("report.rows")}</span>
            </button>
          </li>
        {:else}
          <li class="muted small">{t("report.none")}</li>
        {/each}
      </ul>
      <div class="branding">
        <h3>{t("report.branding")}</h3>
        <label>
          {t("report.organization")}
          <input bind:value={branding.organization} onchange={saveBranding} placeholder={t("report.organization.hint")} />
        </label>
        <div class="logo">
          <span class="small" title={branding.logo ?? ""}>{branding.logo ? branding.logo.split(/[\\/]/).pop() : t("report.logo.none")}</span>
          <button onclick={chooseLogo}>{t("report.logo.choose")}</button>
          {#if branding.logo}
            <button class="ghost" onclick={() => ((branding.logo = null), saveBranding())}>{t("report.logo.remove")}</button>
          {/if}
        </div>
      </div>
      {#if error}<p class="error small">{error}</p>{/if}
    </aside>
    <main>
      {#if current}
        {#key current.id}
          <ReportEditor
            bind:report={current}
            onSaved={refresh}
            onDeleted={() => ((current = null), refresh())}
          />
        {/key}
      {:else}
        <div class="hint">
          <p>{t(kind === "image" ? "report.hint.image" : "report.hint.sound")}</p>
        </div>
      {/if}
    </main>
  </div>
{/if}

<style>
  .layout {
    display: flex;
    height: 100%;
    min-height: 0;
  }
  aside {
    width: 270px;
    flex: none;
    display: flex;
    flex-direction: column;
    padding: var(--vf-space-3);
    border-right: 1px solid var(--vf-border);
    background: var(--vf-surface);
    overflow-y: auto;
  }
  aside > * {
    flex-shrink: 0;
  }
  aside > * + * {
    margin-top: var(--vf-space-3);
  }
  main {
    flex: 1;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  h2 {
    margin: 0;
    font-size: var(--vf-text-lg);
    letter-spacing: 0.06em;
  }
  h3 {
    margin: 0 0 var(--vf-space-2);
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
  .mode {
    color: var(--vf-accent);
  }
  .new {
    display: flex;
  }
  .new select {
    margin-right: var(--vf-space-2);
    min-width: 0;
  }
  .new button {
    flex: 1;
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
  button {
    cursor: pointer;
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
  input {
    background: var(--vf-bg);
    width: 100%;
  }
  .list {
    list-style: none;
    margin: 0;
    padding: 0;
    flex: 1 0 auto;
  }
  .list li + li {
    margin-top: var(--vf-space-1);
  }
  .list button {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    text-align: left;
    background: none;
    border-color: transparent;
    padding: var(--vf-space-2);
  }
  .list button:hover {
    background: var(--vf-surface-hover);
  }
  .list button.sel {
    background: var(--vf-selection);
    border-color: var(--vf-border);
  }
  .name {
    font-weight: 600;
  }
  .sub {
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .branding {
    border-top: 1px solid var(--vf-border);
    padding-top: var(--vf-space-3);
  }
  .branding label {
    display: flex;
    flex-direction: column;
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .branding label input {
    margin-top: 2px;
  }
  .logo {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    margin-top: var(--vf-space-2);
  }
  .logo > * {
    margin: 0 var(--vf-space-2) var(--vf-space-1) 0;
  }
  .logo span {
    max-width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
  .empty,
  .hint {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: var(--vf-space-8);
    color: var(--vf-text-muted);
  }
  .empty h2 {
    color: var(--vf-text);
    margin-bottom: var(--vf-space-2);
  }
  .hint p {
    max-width: 460px;
    line-height: 1.5;
  }
  .buttons {
    display: flex;
    margin-top: var(--vf-space-3);
  }
  .buttons button + button {
    margin-left: var(--vf-space-2);
  }
  @supports (background: color-mix(in srgb, red 50%, transparent)) {
    .list button.sel {
      background: color-mix(in srgb, var(--vf-accent) 22%, transparent);
    }
  }
</style>
