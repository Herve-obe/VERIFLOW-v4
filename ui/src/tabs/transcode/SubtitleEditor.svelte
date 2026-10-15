<!-- Éditeur de sous-titres SRT et WebVTT : temps, texte, contrôles de
     lisibilité (longueur des lignes, vitesse de lecture, durée, écart). -->
<script lang="ts">
  import { untrack } from "svelte";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import Modal from "../../components/Modal.svelte";
  import { t } from "../../i18n/index.svelte";
  import { checkSubtitles, formatTime, loadSubtitles, parseTime, saveSubtitles, type Cue, type Issue } from "../../lib/subtitles";

  let {
    path = null,
    onClose,
    onSaved,
  }: { path?: string | null; onClose: () => void; onSaved: (path: string) => void } = $props();

  // Fichier en cours : celui reçu à l'ouverture, puis celui d'« Enregistrer sous ».
  let file = $state<string | null>(untrack(() => path));
  let cues = $state<Cue[]>([]);
  let issues = $state<Issue[][]>([]);
  let fps = $state(25);
  let selected = $state(-1);
  let dirty = $state(false);
  let error = $state("");
  let shift = $state("0");

  const name = (p: string) => p.split(/[\\/]/).pop() ?? p;

  $effect(() => {
    const p = path;
    if (!p) return;
    loadSubtitles(p)
      .then((c) => {
        cues = c;
        dirty = false;
      })
      .catch((e) => (error = String(e)));
  });

  // Contrôles relancés à chaque modification.
  let req = 0;
  $effect(() => {
    const list = $state.snapshot(cues);
    const rate = fps;
    const r = ++req;
    checkSubtitles(list, { fps: rate })
      .then((i) => r === req && (issues = i))
      .catch(() => {});
  });

  function touch() {
    dirty = true;
  }

  function setTime(i: number, key: "start" | "end", value: string) {
    const v = parseTime(value);
    if (v === null) {
      error = `${t("sub.bad.time")} : ${value}`;
      return;
    }
    error = "";
    cues[i][key] = v;
    touch();
  }

  function insertAfter(i: number) {
    const gap = 2 / fps;
    const start = i >= 0 ? cues[i].end + gap : 0;
    cues.splice(i + 1, 0, { start, end: start + 2, text: "" });
    selected = i + 1;
    touch();
  }

  function remove(i: number) {
    cues.splice(i, 1);
    selected = Math.min(selected, cues.length - 1);
    touch();
  }

  /** Coupe en deux : texte au saut de ligne (sinon au milieu), temps au prorata. */
  function split(i: number) {
    const c = cues[i];
    const lines = c.text.split("\n");
    let a: string;
    let b: string;
    if (lines.length > 1) {
      a = lines.slice(0, Math.ceil(lines.length / 2)).join("\n");
      b = lines.slice(Math.ceil(lines.length / 2)).join("\n");
    } else {
      const words = c.text.split(" ");
      a = words.slice(0, Math.ceil(words.length / 2)).join(" ");
      b = words.slice(Math.ceil(words.length / 2)).join(" ");
    }
    const total = Math.max(1, a.length + b.length);
    const cut = c.start + ((c.end - c.start) * a.length) / total;
    const gap = 2 / fps;
    cues.splice(i, 1, { start: c.start, end: cut, text: a }, { start: Math.min(c.end, cut + gap), end: c.end, text: b });
    touch();
  }

  function mergeNext(i: number) {
    if (i + 1 >= cues.length) return;
    const n = cues[i + 1];
    cues[i].text = `${cues[i].text}\n${n.text}`.trim();
    cues[i].end = Math.max(cues[i].end, n.end);
    cues.splice(i + 1, 1);
    touch();
  }

  function applyShift() {
    const s = Number(shift.replace(",", "."));
    if (!Number.isFinite(s) || s === 0) return;
    for (const c of cues) {
      c.start = Math.max(0, c.start + s);
      c.end = Math.max(0, c.end + s);
    }
    touch();
  }

  function sort() {
    cues.sort((a, b) => a.start - b.start);
    touch();
  }

  async function write(target: string | null) {
    let p = target;
    if (!p) {
      const r = await saveDialog({ title: t("sub.save.as"), defaultPath: file ?? "sous-titres.srt", filters: [{ name: "SRT", extensions: ["srt"] }, { name: "WebVTT", extensions: ["vtt"] }] });
      if (!r) return;
      p = r;
    }
    try {
      await saveSubtitles(p, $state.snapshot(cues));
      file = p;
      dirty = false;
      error = "";
      onSaved(p);
    } catch (e) {
      error = String(e);
    }
  }

  function close() {
    if (dirty && !confirm(t("sub.unsaved"))) return;
    onClose();
  }

  const speed = (c: Cue) => {
    const chars = c.text.replace(/<[^>]*>|\{[^}]*\}/g, "").replace(/\n/g, "").length;
    const d = c.end - c.start;
    return d > 0 ? chars / d : Infinity;
  };

  function label(i: Issue): string {
    switch (i.kind) {
      case "gap":
        return `${t("sub.issue.gap")} (${i.frames.toFixed(1).replace(".", ",")} ${t("sub.frames")})`;
      case "too_short":
      case "too_long":
        return `${t(`sub.issue.${i.kind}`)} (${i.seconds.toFixed(2).replace(".", ",")} s)`;
      case "too_fast":
        return `${t("sub.issue.too_fast")} (${i.cps.toFixed(0)} ${t("sub.cps")})`;
      case "line_too_long":
        return `${t("sub.issue.line_too_long")} (${i.chars})`;
      case "too_many_lines":
        return `${t("sub.issue.too_many_lines")} (${i.lines})`;
      default:
        return t(`sub.issue.${i.kind}`);
    }
  }

  // Tant que l'éditeur est ouvert, Tab passe d'un champ à l'autre sans
  // basculer le mode VIDEO/AUDIO (ce qui fermerait l'éditeur).
  $effect(() => {
    const keep = (e: KeyboardEvent) => {
      if (e.key === "Tab" && !e.ctrlKey) e.stopPropagation();
    };
    window.addEventListener("keydown", keep, true);
    return () => window.removeEventListener("keydown", keep, true);
  });

  const problems = $derived(issues.reduce((n, l) => n + (l.length ? 1 : 0), 0));
</script>

<Modal title={`${t("sub.title")}${file ? ` : ${name(file)}` : ""}${dirty ? " *" : ""}`} onClose={close} width="min(1100px, 96vw)" height="min(760px, 92vh)">
  <div class="editor">
    <div class="bar">
      <button class="tc-btn" onclick={() => insertAfter(selected >= 0 ? selected : cues.length - 1)}>+ {t("sub.add")}</button>
      <button class="tc-btn" onclick={sort}>{t("sub.sort")}</button>
      <label class="inline">
        {t("sub.shift")}
        <input class="tc-input num" bind:value={shift} />
        s
        <button class="tc-btn" onclick={applyShift}>{t("sub.apply")}</button>
      </label>
      <label class="inline">
        {t("sub.rate")}
        <select class="tc-input" bind:value={fps}>
          {#each [23.976, 24, 25, 29.97, 30, 50, 59.94, 60] as r (r)}<option value={r}>{String(r).replace(".", ",")}</option>{/each}
        </select>
      </label>
      <span class="spacer"></span>
      <span class="small" class:warn={problems > 0}>{cues.length} {t("sub.count")} · {problems} {t("sub.problems")}</span>
    </div>

    <div class="list">
      {#if !cues.length}
        <p class="muted empty">{t("sub.empty")}</p>
      {/if}
      {#each cues as c, i (i)}
        <div class="cue" class:sel={i === selected} class:bad={issues[i]?.length} onfocusin={() => (selected = i)}>
          <span class="n mono">{i + 1}</span>
          <div class="times">
            <input class="tc-input mono" value={formatTime(c.start)} onchange={(e) => setTime(i, "start", e.currentTarget.value)} aria-label={t("sub.start")} />
            <input class="tc-input mono" value={formatTime(c.end)} onchange={(e) => setTime(i, "end", e.currentTarget.value)} aria-label={t("sub.end")} />
            <span class="small muted mono">{(c.end - c.start).toFixed(2).replace(".", ",")} s · {Number.isFinite(speed(c)) ? speed(c).toFixed(0) : "∞"} {t("sub.cps")}</span>
          </div>
          <textarea class="tc-input" rows="2" bind:value={c.text} oninput={touch} aria-label={t("sub.text")}></textarea>
          <div class="acts">
            <button class="tc-btn" onclick={() => insertAfter(i)} title={t("sub.add.after")}>+</button>
            <button class="tc-btn" onclick={() => split(i)} title={t("sub.split")}>✂</button>
            <button class="tc-btn" onclick={() => mergeNext(i)} disabled={i + 1 >= cues.length} title={t("sub.merge")}>⇣</button>
            <button class="tc-btn" onclick={() => remove(i)} title={t("sub.delete")}>×</button>
          </div>
          {#if issues[i]?.length}
            <p class="issues small">{issues[i].map(label).join(" · ")}</p>
          {/if}
        </div>
      {/each}
    </div>

    <div class="foot">
      {#if error}<span class="error small">{error}</span>{/if}
      <span class="muted small">{t("sub.rules")}</span>
      <span class="spacer"></span>
      <button class="tc-btn" onclick={() => write(null)}>{t("sub.save.as")}</button>
      <button class="primary" disabled={!file || !dirty} onclick={() => write(file)}>{t("sub.save")}</button>
    </div>
  </div>
</Modal>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    padding: var(--vf-space-3);
    gap: var(--vf-space-2);
    font-size: var(--vf-text-sm);
  }
  .bar,
  .foot {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    flex-wrap: wrap;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    color: var(--vf-text-muted);
  }
  .num {
    width: 64px;
  }
  .spacer {
    flex: 1;
  }
  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-1);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1);
  }
  .cue {
    display: grid;
    grid-template-columns: 32px 210px 1fr auto;
    gap: var(--vf-space-2);
    align-items: start;
    padding: var(--vf-space-1);
    border-radius: var(--vf-radius-sm);
    border-left: 3px solid transparent;
  }
  .cue.sel {
    background: var(--vf-selection);
  }
  .cue.bad {
    border-left-color: var(--vf-warning);
  }
  .n {
    color: var(--vf-text-muted);
    text-align: right;
    padding-top: 4px;
  }
  .times {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  textarea {
    resize: vertical;
    font-family: inherit;
    min-height: 44px;
  }
  .acts {
    display: flex;
    gap: 2px;
  }
  .issues {
    grid-column: 2 / -1;
    margin: 0;
    color: var(--vf-warning);
  }
  .warn {
    color: var(--vf-warning);
  }
  .error {
    color: var(--vf-error);
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .small {
    font-size: var(--vf-text-xs);
  }
  .empty {
    padding: var(--vf-space-3);
    margin: 0;
  }
  .primary {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: none;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    font-weight: 600;
    cursor: pointer;
  }
  .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }
</style>
