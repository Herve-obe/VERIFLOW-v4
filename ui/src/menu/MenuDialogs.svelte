<!-- Fenêtres ouvertes depuis la barre de menus : Préférences, Raccourcis
     clavier, Licences, À propos. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../components/Modal.svelte";
  import { MENUS, command, menuState, shortcutLabel, type Entry } from "./commands.svelte";
  import { i18n, setLanguage, t, LANGUAGES, type Language } from "../i18n/index.svelte";
  import { appInfo, type AppInfo } from "../lib/api";
  import { clearAllData, resetSession, setStartupMode, startupMode, type StartupMode } from "../lib/appdata";
  import { ask } from "../stores/confirm.svelte";
  import { prefs } from "../stores/prefs.svelte";
  import { ALGORITHMS, type HashAlgo } from "../lib/offload";
  import { MARKER_COLORS, markerCss } from "../lib/logs";
  import { open as pickFolder } from "@tauri-apps/plugin-dialog";
  import ShortcutsEditor from "./ShortcutsEditor.svelte";

  let info = $state<AppInfo | null>(null);
  onMount(() => {
    appInfo().then((i) => (info = i));
  });

  const close = () => (menuState.dialog = null);

  // Préférences : une section par onglet.
  const PREF_TABS = ["general", "shortcuts", "offload", "player", "transcode"] as const;
  type PrefTab = (typeof PREF_TABS)[number];
  let prefTab = $state<PrefTab>("general");

  function toggleAlgo(id: HashAlgo) {
    const a = prefs.offload.algorithms;
    prefs.offload.algorithms = a.includes(id) ? a.filter((x) => x !== id) : [...a, id];
  }
  async function chooseTranscodeDest() {
    const r = await pickFolder({ directory: true, multiple: false });
    if (typeof r === "string") {
      prefs.transcode.dest = r;
      prefs.transcode.nextToSource = false;
    }
  }

  // Données de l'application sur ce poste.
  let startup = $state<StartupMode>(startupMode());
  function changeStartup(mode: StartupMode) {
    startup = mode;
    setStartupMode(mode);
  }
  // La fenêtre des préférences est fermée avant la confirmation (qui
  // s'affiche alors seule), puis l'interface est rechargée.
  async function wipe(all: boolean) {
    close();
    const ok = await ask(t(all ? "prefs.data.clear.confirm" : "prefs.data.reset.confirm"), {
      title: t(all ? "prefs.data.clear" : "prefs.data.reset"),
      ok: t(all ? "prefs.data.clear.ok" : "prefs.data.reset.ok"),
      kind: "danger",
    });
    if (!ok) return;
    if (all) clearAllData();
    else resetSession();
    location.reload();
  }

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
          <label class="row">
            <span>{t("prefs.startup")}</span>
            <select value={startup} onchange={(e) => changeStartup(e.currentTarget.value as StartupMode)}>
              <option value="resume">{t("prefs.startup.resume")}</option>
              <option value="fresh">{t("prefs.startup.fresh")}</option>
            </select>
          </label>
          <p class="note">{t("prefs.startup.note")}</p>
          <h3 class="spaced">{t("prefs.data")}</h3>
          <div class="row">
            <span>{t("prefs.data.reset.hint")}</span>
            <button class="ghost" onclick={() => wipe(false)}>{t("prefs.data.reset")}</button>
          </div>
          <div class="row">
            <span>{t("prefs.data.clear.hint")}</span>
            <button class="danger" onclick={() => wipe(true)}>{t("prefs.data.clear")}</button>
          </div>
          <p class="note">{t("prefs.data.note")}</p>
        {:else if prefTab === "shortcuts"}
          <h3>{t("prefs.shortcuts")}</h3>
          <ShortcutsEditor />
        {:else if prefTab === "offload"}
          <h3>{t("prefs.offload")}</h3>
          <p class="note">{t("prefs.defaults.note")}</p>
          <label class="row">
            <span>{t("prefs.offload.template")}</span>
            <input class="text mono" bind:value={prefs.offload.template} spellcheck="false" />
          </label>
          <label class="row">
            <span>{t("prefs.offload.operator")}</span>
            <input class="text" bind:value={prefs.offload.operator} />
          </label>
          <div class="row">
            <span>{t("prefs.offload.algorithms")}</span>
            <div class="checks">
              {#each ALGORITHMS as a (a.id)}
                <label><input type="checkbox" checked={prefs.offload.algorithms.includes(a.id)} onchange={() => toggleAlgo(a.id)} /> {a.label}</label>
              {/each}
            </div>
          </div>
          <label class="row">
            <span>{t("prefs.offload.eject")}</span>
            <input type="checkbox" bind:checked={prefs.offload.eject} />
          </label>
        {:else if prefTab === "player"}
          <h3>{t("prefs.player")}</h3>
          <p class="note">{t("prefs.player.note")}</p>
          <label class="row">
            <span>{t("prefs.player.tc")}</span>
            <input type="checkbox" bind:checked={prefs.player.tcOverlay} />
          </label>
          <label class="row">
            <span>{t("prefs.player.scopes")}</span>
            <input type="checkbox" bind:checked={prefs.player.scopes} />
          </label>
          <div class="row">
            <span>{t("prefs.player.marker")}</span>
            <div class="colors">
              {#each MARKER_COLORS as c (c)}
                <button
                  class="dot"
                  class:on={prefs.player.markerColor === c}
                  style:background={markerCss(c)}
                  onclick={() => (prefs.player.markerColor = c)}
                  aria-label={c}
                  title={c}
                ></button>
              {/each}
            </div>
          </div>
          <label class="row">
            <span>{t("prefs.player.reportAuto")}</span>
            <input type="checkbox" bind:checked={prefs.player.reportAuto} />
          </label>
          <p class="note">{t("prefs.player.reportAuto.note")}</p>
        {:else if prefTab === "transcode"}
          <h3>{t("prefs.transcode")}</h3>
          <p class="note">{t("prefs.transcode.note")}</p>
          <label class="row">
            <span>{t("prefs.transcode.next")}</span>
            <input type="checkbox" bind:checked={prefs.transcode.nextToSource} />
          </label>
          <div class="row">
            <span>{t("prefs.transcode.dest")}</span>
            <span class="path" title={prefs.transcode.dest}>{prefs.transcode.dest || t("transcode.dest.none")}</span>
            <button class="ghost" onclick={chooseTranscodeDest}>{t("offload.browse")}</button>
            {#if prefs.transcode.dest}<button class="ghost" onclick={() => (prefs.transcode.dest = "")}>×</button>{/if}
          </div>
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
  <Modal title={t("menu.help.licenses")} onClose={close} width="min(680px, 94vw)">
    <div class="scroll text">
      <h3>VERIFLOW</h3>
      <p>{t("licenses.veriflow")}</p>
      <h3>FFmpeg</h3>
      <p>{t("licenses.ffmpeg")}</p>
      <p class="mono">https://ffmpeg.org · https://git.ffmpeg.org/ffmpeg.git</p>
      <h3>{t("licenses.others.title")}</h3>
      <p>{t("licenses.others")}</p>
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
      {#if info}<p class="mono muted">{info.os} · {info.arch}</p>{/if}
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
  .text {
    width: 260px;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-size: var(--vf-text-sm);
  }
  .checks {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-1) var(--vf-space-3);
    justify-content: flex-end;
    max-width: 300px;
  }
  .checks label {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
  }
  .colors {
    display: flex;
    gap: var(--vf-space-1);
  }
  .colors .dot {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid var(--vf-border);
    cursor: pointer;
    padding: 0;
  }
  .colors .dot.on {
    border-color: var(--vf-text);
  }
  .path {
    flex: 1;
    min-width: 0;
    text-align: right;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  h3.spaced {
    margin-top: var(--vf-space-6);
  }
  .row > span {
    flex: 1;
  }
  .row button {
    flex-shrink: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .ghost {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
  }
  .ghost:hover {
    background: var(--vf-surface-hover);
  }
  .danger {
    background: none;
    color: var(--vf-error);
    border: 1px solid var(--vf-error);
  }
  .danger:hover {
    background: var(--vf-error);
    color: var(--vf-on-error);
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
  .text p {
    font-size: var(--vf-text-sm);
    line-height: 1.5;
    margin: 0 0 var(--vf-space-2);
  }
  .mono {
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-xs);
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
