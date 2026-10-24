<!-- Fenêtres ouvertes depuis la barre de menus : Préférences, Raccourcis
     clavier, Licences, À propos. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../components/Modal.svelte";
  import { MENUS, command, menuState, shortcutLabel, type Entry } from "./commands.svelte";
  import { i18n, setLanguage, t, LANGUAGES, type Language } from "../i18n/index.svelte";
  import { appInfo, type AppInfo } from "../lib/api";
  import { legalText, ffmpegInfo, type FfmpegInfo, type LegalDoc } from "../lib/legal";

  let info = $state<AppInfo | null>(null);
  onMount(() => {
    appInfo().then((i) => (info = i));
  });

  const close = () => (menuState.dialog = null);

  // Licences : une rubrique par document livré avec VERIFLOW.
  const LEGAL_TABS: { id: LegalDoc; label: string }[] = [
    { id: "gpl", label: "VERIFLOW (GPLv3)" },
    { id: "cluf", label: "CLUF" },
    { id: "cgu", label: "CGU" },
    { id: "ffmpeg", label: "FFmpeg" },
    { id: "tiers", label: "licenses.tab.tiers" },
    { id: "third_party", label: "licenses.tab.texts" },
  ];
  let legalTab = $state<LegalDoc>("gpl");
  let legal = $state<Partial<Record<LegalDoc, string>>>({});
  let ffmpeg = $state<FfmpegInfo | null>(null);
  let ffmpegError = $state("");
  $effect(() => {
    if (menuState.dialog !== "licenses") return;
    const id = legalTab;
    if (legal[id] === undefined) {
      legalText(id)
        .then((text) => (legal[id] = text))
        .catch((e) => (legal[id] = String(e)));
    }
    if (id === "ffmpeg" && !ffmpeg && !ffmpegError) {
      ffmpegInfo()
        .then((f) => (ffmpeg = f))
        .catch((e) => (ffmpegError = String(e)));
    }
  });
  const tabLabel = (l: string) => (l.startsWith("licenses.") ? t(l as never) : l);

  // Préférences : une section par onglet (d'autres viendront : raccourcis
  // modifiables, OFFLOAD, PLAYER, TRANSCODE).
  const PREF_TABS = ["general"] as const;
  type PrefTab = (typeof PREF_TABS)[number];
  let prefTab = $state<PrefTab>("general");

  // Raccourcis : tirés des menus, dans leur ordre.
  const flat = (items: Entry[]): string[] =>
    items.flatMap((e) => (e === "-" ? [] : typeof e === "string" ? [e] : flat(e.items())));
  const groups = $derived(
    MENUS.map((m) => ({
      label: m.label(),
      rows: flat(m.items())
        .map((id) => command(id))
        .filter((c) => c?.shortcut)
        .map((c) => ({ label: c!.label(), keys: shortcutLabel(c!.shortcut) })),
    })).filter((g) => g.rows.length > 0),
  );
</script>

{#if menuState.dialog === "prefs"}
  <Modal title={t("menu.edit.prefs").replace("…", "")} onClose={close} width="min(760px, 94vw)" height="min(520px, 90vh)">
    <div class="prefs">
      <nav class="sections" aria-label={t("menu.edit.prefs")}>
        {#each PREF_TABS as id (id)}
          <button class:on={prefTab === id} onclick={() => (prefTab = id)}>{t(`prefs.${id}` as never)}</button>
        {/each}
      </nav>
      <section class="pane">
        {#if prefTab === "general"}
          <h3>{t("prefs.general")}</h3>
          <label class="row">
            <span>{t("settings.language")}</span>
            <select value={i18n.lang} onchange={(e) => setLanguage(e.currentTarget.value as Language)}>
              {#each Object.keys(LANGUAGES) as lang (lang)}
                <option value={lang}>{t(`language.${lang}` as never)}</option>
              {/each}
            </select>
          </label>
          <p class="note">{t("prefs.more")}</p>
        {/if}
      </section>
    </div>
  </Modal>
{:else if menuState.dialog === "shortcuts"}
  <Modal title={t("menu.help.shortcuts")} onClose={close} width="min(720px, 94vw)" height="min(640px, 90vh)">
    <div class="scroll">
      {#each groups as g (g.label)}
        <h3>{g.label}</h3>
        <table>
          <tbody>
            {#each g.rows as r (r.label)}
              <tr><td>{r.label}</td><td class="keys">{r.keys}</td></tr>
            {/each}
          </tbody>
        </table>
      {/each}
      <p class="note">{t("shortcuts.note")}</p>
    </div>
  </Modal>
{:else if menuState.dialog === "licenses"}
  <Modal title={t("menu.help.licenses")} onClose={close} width="min(960px, 96vw)" height="min(680px, 92vh)">
    <div class="prefs">
      <nav class="sections" aria-label={t("menu.help.licenses")}>
        {#each LEGAL_TABS as tab (tab.id)}
          <button class:on={legalTab === tab.id} onclick={() => (legalTab = tab.id)}>{tabLabel(tab.label)}</button>
        {/each}
      </nav>
      <section class="pane legal">
        {#if legalTab === "gpl"}
          <p class="intro">{t("licenses.veriflow")}</p>
        {:else if legalTab === "ffmpeg"}
          <p class="intro">{t("licenses.ffmpeg")}</p>
          {#if ffmpeg}
            <p class="mono">{ffmpeg.version}</p>
            <pre class="conf">{ffmpeg.buildconf}</pre>
          {:else if ffmpegError}
            <p class="note">{ffmpegError}</p>
          {/if}
        {:else if legalTab === "third_party"}
          <p class="intro">{t("licenses.others")}</p>
        {/if}
        {#if i18n.lang !== "fr" && (legalTab === "cluf" || legalTab === "cgu" || legalTab === "tiers")}
          <p class="note">{t("licenses.french")}</p>
        {/if}
        <pre class="doc">{legal[legalTab] ?? "…"}</pre>
      </section>
    </div>
  </Modal>
{:else if menuState.dialog === "about"}
  <Modal title={t("menu.help.about")} onClose={close} width="min(440px, 94vw)">
    <div class="about">
      <div class="name">VERIFLOW</div>
      <div class="mono">v{info?.version ?? ""}</div>
      <p>{t("about.tagline")}</p>
      <p class="muted">{t("about.author")}</p>
      <p class="muted">{t("about.license")}</p>
      <p class="muted">{t("about.ffmpeg")}</p>
      {#if info}<p class="mono muted">{info.os} · {info.arch}</p>{/if}
      <button class="legal-btn" onclick={() => (menuState.dialog = "licenses")}>{t("about.legal")}</button>
    </div>
  </Modal>
{/if}

<style>
  .prefs {
    display: grid;
    grid-template-columns: 180px 1fr;
    height: 100%;
    min-height: 0;
  }
  .sections {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--vf-space-2);
    border-right: 1px solid var(--vf-border);
  }
  .sections button {
    background: none;
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-2) var(--vf-space-3);
    color: var(--vf-text-muted);
    text-align: left;
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .sections button:hover {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  .sections button.on {
    background: var(--vf-selection);
    color: var(--vf-accent);
    font-weight: 600;
  }
  .pane,
  .scroll {
    padding: var(--vf-space-4);
    overflow: auto;
    min-height: 0;
  }
  .scroll {
    height: 100%;
  }
  h3 {
    margin: 0 0 var(--vf-space-2);
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .scroll h3:not(:first-child) {
    margin-top: var(--vf-space-4);
  }
  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--vf-space-4);
    padding: var(--vf-space-2) 0;
    font-size: var(--vf-text-sm);
  }
  select {
    min-width: 180px;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--vf-text-sm);
  }
  td {
    padding: 3px 0;
    border-bottom: 1px solid var(--vf-border);
  }
  .keys {
    text-align: right;
    font-family: var(--vf-font-mono);
    color: var(--vf-accent);
    white-space: nowrap;
  }
  .note,
  .muted {
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
  }
  .mono {
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-xs);
  }
  .legal {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
    min-height: 0;
  }
  .legal .intro {
    font-size: var(--vf-text-sm);
    line-height: 1.5;
    margin: 0;
  }
  .legal pre {
    margin: 0;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-xs);
    line-height: 1.5;
    color: var(--vf-text);
  }
  .legal .conf {
    color: var(--vf-text-muted);
  }
  .legal-btn {
    margin-top: var(--vf-space-3);
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    cursor: pointer;
  }
  .about {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--vf-space-1);
    padding: var(--vf-space-6) var(--vf-space-4);
    text-align: center;
  }
  .about .name {
    font-size: var(--vf-text-xl);
    font-weight: 700;
    letter-spacing: 0.14em;
  }
  .about p {
    margin: var(--vf-space-1) 0 0;
    font-size: var(--vf-text-sm);
  }
</style>
