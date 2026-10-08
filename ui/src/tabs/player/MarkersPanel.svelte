<!-- Logs du PLAYER : marqueurs (point ou plage), couleur, commentaire, scène,
     prise ; exports EDL, ALE, CSV, FCPXML, OTIO. Enregistrés dans le projet. -->
<script lang="ts">
  import { tick } from "svelte";
  import { save } from "@tauri-apps/plugin-dialog";
  import { app } from "../../stores/app.svelte";
  import Modal from "../../components/Modal.svelte";
  import { createProject, openProject } from "../../lib/project";
  import { t } from "../../i18n/index.svelte";
  import {
    MARKER_COLORS,
    LOG_FORMATS,
    markersList,
    markerSave,
    markerDelete,
    logsExport,
    markerCss,
    type Marker,
    type MarkerColor,
    type LogFormat,
  } from "../../lib/logs";

  let {
    path,
    frame,
    tc,
    markIn = null,
    markOut = null,
    onSeek,
    onRangeUsed = () => {},
    onChange = () => {},
  }: {
    path: string | null;
    frame: number;
    tc: (frame: number) => string;
    markIn?: number | null;
    markOut?: number | null;
    onSeek: (frame: number) => void;
    onRangeUsed?: () => void;
    onChange?: (markers: Marker[]) => void;
  } = $props();

  const COLOR_KEY = "veriflow.marker.color";
  const FORMAT_KEY = "veriflow.logs.format";
  const read = (k: string, d: string) => {
    try {
      return localStorage.getItem(k) ?? d;
    } catch {
      return d;
    }
  };
  const write = (k: string, v: string) => {
    try {
      localStorage.setItem(k, v);
    } catch {
      /* stockage indisponible */
    }
  };

  let markers = $state<Marker[]>([]);
  let color = $state<MarkerColor>(read(COLOR_KEY, "red") as MarkerColor);
  let format = $state<LogFormat>(read(FORMAT_KEY, "edl") as LogFormat);
  let allClips = $state(false);
  let error = $state("");

  const sorted = $derived([...markers].sort((a, b) => (a.in_frame ?? a.frame) - (b.in_frame ?? b.frame) || a.id - b.id));

  async function reload(p: string | null) {
    error = "";
    markers = p && app.project ? await markersList(p).catch(() => []) : [];
    onChange(markers);
  }

  // Relecture au changement de média ou de projet.
  $effect(() => {
    const p = path;
    void app.project?.path;
    reload(p);
  });

  async function persist(m: Marker): Promise<Marker | null> {
    try {
      const saved = await markerSave(m);
      error = "";
      return saved;
    } catch (e) {
      error = String(e);
      return null;
    }
  }

  // Sans projet, les marqueurs ne peuvent pas être enregistrés : une fenêtre
  // le dit clairement et propose de créer ou d'ouvrir un projet, puis le
  // marqueur demandé est posé.
  let needProject = $state(false);

  async function withProject(action: () => Promise<boolean>) {
    if (await action()) {
      needProject = false;
      await add();
    }
  }

  /** Ajoute un marqueur à l'image courante, ou une plage si entrée et sortie sont posées. */
  export async function add() {
    if (!path) return;
    if (!app.project) {
      needProject = true;
      return;
    }
    const range = markIn !== null && markOut !== null;
    const m: Marker = {
      id: 0,
      path,
      frame: range ? Math.min(markIn!, markOut!) : frame,
      in_frame: range ? Math.min(markIn!, markOut!) : null,
      out_frame: range ? Math.max(markIn!, markOut!) : null,
      color,
      comment: "",
      scene: "",
      take: "",
    };
    const saved = await persist(m);
    if (!saved) return;
    markers = [...markers, saved];
    onChange(markers);
    if (range) onRangeUsed();
    // Saisie du commentaire tout de suite, dès que la ligne est affichée.
    await tick();
    document.getElementById(`marker-comment-${saved.id}`)?.focus();
  }

  async function update(m: Marker, patch: Partial<Marker>) {
    const saved = await persist({ ...m, ...patch });
    if (!saved) return;
    markers = markers.map((x) => (x.id === saved.id ? saved : x));
    onChange(markers);
  }

  async function remove(m: Marker) {
    try {
      await markerDelete(m.id);
      markers = markers.filter((x) => x.id !== m.id);
      onChange(markers);
    } catch (e) {
      error = String(e);
    }
  }

  function pickColor(c: MarkerColor) {
    color = c;
    write(COLOR_KEY, c);
  }

  const nextColor = (c: MarkerColor) => MARKER_COLORS[(MARKER_COLORS.indexOf(c) + 1) % MARKER_COLORS.length];

  async function runExport() {
    write(FORMAT_KEY, format);
    const stem = allClips ? (app.project?.name ?? "logs") : (path?.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? "logs");
    const ext = format;
    const dest = await save({ defaultPath: `${stem}.${ext}`, filters: [{ name: ext.toUpperCase(), extensions: [ext] }] });
    if (!dest) return;
    try {
      const written = await logsExport(format, allClips || !path ? [] : [path], dest, stem);
      app.status = `${t("logs.exported")} ${written}`;
      error = "";
    } catch (e) {
      error = String(e);
    }
  }
</script>

<aside class="markers">
  <header>
    <h3>{t("logs.title")}</h3>
    <div class="colors" role="radiogroup" aria-label={t("logs.color")}>
      {#each MARKER_COLORS as c (c)}
        <button
          class="swatch"
          class:on={c === color}
          style:background={markerCss(c)}
          role="radio"
          aria-checked={c === color}
          aria-label={t(`logs.color.${c}`)}
          title={t(`logs.color.${c}`)}
          onclick={() => pickColor(c)}
        ></button>
      {/each}
    </div>
  </header>

  <button class="add" onclick={add} disabled={!path}>
    + {markIn !== null && markOut !== null ? t("logs.add.range") : t("logs.add")} (M)
  </button>
  {#if !app.project}
    <p class="note">{t("logs.need.project")}</p>
  {/if}
  {#if error}<p class="err">{error}</p>{/if}

  <ul>
    {#each sorted as m (m.id)}
      <li>
        <div class="line">
          <button
            class="swatch"
            style:background={markerCss(m.color)}
            title={t("logs.color.next")}
            aria-label={t("logs.color.next")}
            onclick={() => update(m, { color: nextColor(m.color) })}
          ></button>
          <button class="tc mono" onclick={() => onSeek(m.in_frame ?? m.frame)} title={t("logs.go")}>
            {tc(m.in_frame ?? m.frame)}
          </button>
          {#if m.in_frame !== null && m.out_frame !== null}
            <span class="range mono">&gt; {tc(m.out_frame)}</span>
          {/if}
          <button class="del" onclick={() => remove(m)} aria-label={t("logs.delete")} title={t("logs.delete")}>
            <svg viewBox="0 0 16 16"><path d="M4 4 L12 12 M12 4 L4 12" stroke="currentColor" stroke-width="1.5" /></svg>
          </button>
        </div>
        <input
          id={`marker-comment-${m.id}`}
          class="comment"
          placeholder={t("logs.comment")}
          value={m.comment}
          onchange={(e) => update(m, { comment: e.currentTarget.value })}
          onkeydown={(e) => e.key === "Enter" && e.currentTarget.blur()}
        />
        <div class="line small">
          <input placeholder={t("player.scene")} value={m.scene} onchange={(e) => update(m, { scene: e.currentTarget.value })} />
          <input placeholder={t("player.take")} value={m.take} onchange={(e) => update(m, { take: e.currentTarget.value })} />
        </div>
      </li>
    {:else}
      <li class="empty">{t("logs.none")}</li>
    {/each}
  </ul>

  <footer>
    <select bind:value={format} aria-label={t("logs.format")}>
      {#each LOG_FORMATS as f (f.id)}
        <option value={f.id}>{f.label}</option>
      {/each}
    </select>
    <label class="scope"><input type="checkbox" bind:checked={allClips} /> {t("logs.all.clips")}</label>
    <button onclick={runExport} disabled={!app.project || (!allClips && markers.length === 0)}>{t("logs.export")}</button>
  </footer>
</aside>

{#if needProject}
  <Modal title={t("logs.need.project.title")} onClose={() => (needProject = false)} width="min(460px, 92vw)">
    <div class="need">
      <p>{t("logs.need.project.text")}</p>
      <div class="buttons">
        <button class="primary" onclick={() => withProject(createProject)}>{t("project.new")}</button>
        <button onclick={() => withProject(openProject)}>{t("logs.need.project.open")}</button>
        <button class="ghost" onclick={() => (needProject = false)}>{t("logs.cancel")}</button>
      </div>
    </div>
  </Modal>
{/if}

<style>
  .markers {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
    min-height: 0;
    padding: var(--vf-space-2);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    font-size: var(--vf-text-sm);
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--vf-space-2);
  }
  h3 {
    margin: 0;
    font-size: var(--vf-text-sm);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--vf-text-muted);
  }
  .colors {
    display: flex;
    gap: 3px;
  }
  .swatch {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 1px solid var(--vf-border);
    padding: 0;
    cursor: pointer;
    flex: none;
  }
  .swatch.on {
    outline: 2px solid var(--vf-accent);
    outline-offset: 1px;
  }
  .add,
  footer button {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-2);
    font-weight: 600;
    cursor: pointer;
  }
  .add:disabled,
  footer button:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .note {
    margin: 0;
    color: var(--vf-warning);
  }
  .err {
    margin: 0;
    color: var(--vf-error);
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    overflow: auto;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
  }
  li {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding: var(--vf-space-1);
    border-radius: var(--vf-radius-sm);
    background: var(--vf-surface-high);
  }
  li.empty {
    background: none;
    color: var(--vf-text-disabled);
  }
  .line {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
  }
  .tc {
    background: none;
    border: 0;
    color: var(--vf-accent);
    cursor: pointer;
    padding: 0;
    font-size: var(--vf-text-sm);
  }
  .range {
    color: var(--vf-text-muted);
  }
  .del {
    margin-left: auto;
    background: none;
    border: 0;
    color: var(--vf-text-muted);
    width: 18px;
    height: 18px;
    padding: 2px;
    cursor: pointer;
  }
  input {
    min-width: 0;
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px var(--vf-space-1);
    font-size: var(--vf-text-sm);
  }
  .small input {
    flex: 1;
  }
  footer {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: var(--vf-space-2);
    padding-top: var(--vf-space-2);
    border-top: 1px solid var(--vf-border);
  }
  select {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-sm);
  }
  .scope {
    display: flex;
    align-items: center;
    gap: 4px;
    color: var(--vf-text-muted);
  }
  .need {
    padding: var(--vf-space-4);
    font-size: var(--vf-text-md);
  }
  .need p {
    margin: 0 0 var(--vf-space-4);
    line-height: 1.5;
  }
  .need .buttons {
    display: flex;
    gap: var(--vf-space-2);
    justify-content: flex-end;
  }
  .need button {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    cursor: pointer;
  }
  .need button.primary {
    background: var(--vf-accent);
    border-color: var(--vf-accent);
    color: var(--vf-on-accent);
    font-weight: 600;
  }
  .need button.ghost {
    background: none;
  }
</style>
