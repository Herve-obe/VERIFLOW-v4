<!-- Inspecteur : informations techniques et édition des métadonnées (un média ou un lot). -->
<script lang="ts">
  import { t } from "../../i18n/index.svelte";
  import { bytes } from "../../lib/format";
  import { clock, setMeta, type Described, type FieldDef, type MediaEntry } from "../../lib/media";
  import { app } from "../../stores/app.svelte";

  let {
    selection,
    details,
    fieldDefs,
    onSaved,
  }: {
    selection: MediaEntry[];
    details: Map<string, Described>;
    fieldDefs: FieldDef[];
    onSaved: (paths: string[]) => void;
  } = $props();

  const MIXED = "\u0000mixte";
  let draft = $state<Record<string, string>>({});
  let dirty = $state<Set<string>>(new Set());
  let saving = $state(false);
  let message = $state("");

  const kindFilter = $derived(app.mode === "audio" ? "audio" : "video");
  const visibleFields = $derived(fieldDefs.filter((f) => f.applies === "both" || f.applies === kindFilter));
  const groups = $derived([...new Set(visibleFields.map((f) => f.group))]);
  const single = $derived(selection.length === 1 ? details.get(selection[0].path) : undefined);
  const channels = $derived(single?.details.wav?.channels ?? single?.details.probe?.audio.reduce((n, a) => n + a.channels, 0) ?? 0);

  // Valeurs communes de la sélection (ou « plusieurs valeurs »).
  $effect(() => {
    const sel = selection;
    const next: Record<string, string> = {};
    const ids = [...fieldDefs.map((f) => f.id), ...Array.from({ length: 64 }, (_, i) => `track.${i + 1}`)];
    for (const id of ids) {
      const vals = new Set(sel.map((m) => details.get(m.path)?.values[id] ?? ""));
      next[id] = vals.size === 1 ? [...vals][0] : MIXED;
    }
    draft = next;
    dirty = new Set();
    message = "";
  });

  function edit(id: string, value: string) {
    draft[id] = value;
    dirty = new Set(dirty).add(id);
  }

  async function save() {
    if (dirty.size === 0) return;
    saving = true;
    try {
      const values: Record<string, string> = {};
      for (const id of dirty) values[id] = draft[id] === MIXED ? "" : draft[id];
      const paths = selection.map((m) => m.path);
      await setMeta(paths, values);
      message = t("media.saved");
      dirty = new Set();
      onSaved(paths);
    } catch (e) {
      message = String(e);
    } finally {
      saving = false;
    }
  }

  const tech = $derived.by(() => {
    if (!single) return [];
    const p = single.details.probe;
    const v = p?.video;
    const w = single.details.wav;
    const a = p?.audio[0];
    const rows: [string, string][] = [
      [t("media.tech.format"), p?.format ?? (w ? "wav" : "")],
      [t("media.tech.duration"), clock(p?.duration ?? (w ? w.frames / w.sample_rate : 0))],
      [t("media.tech.size"), bytes(selection[0].size)],
    ];
    if (v) {
      rows.push([t("player.codec"), `${v.codec} (${v.pix_fmt})`]);
      rows.push([t("player.resolution"), `${v.width} x ${v.height}`]);
      rows.push([t("player.rate"), `${(v.rate.num / v.rate.den).toFixed(3)} i/s`]);
    }
    if (p?.start_timecode) rows.push([t("player.tc.start"), p.start_timecode]);
    if (w) rows.push([t("media.tech.audio"), `${w.channels} ${t("player.tracks")}, ${w.sample_rate / 1000} kHz, ${w.bits} bits${w.format === "Float" ? " float" : ""}`]);
    else if (a) rows.push([t("media.tech.audio"), `${a.codec}, ${a.channels} ch, ${a.sample_rate / 1000} kHz`]);
    if (w?.time_reference != null) rows.push(["BWF time reference", String(w.time_reference)]);
    if (single.details.error) rows.push([t("media.tech.error"), single.details.error]);
    return rows;
  });
</script>

<aside class="inspector">
  {#if selection.length === 0}
    <p class="muted">{t("media.select.hint")}</p>
  {:else}
    <h3 title={selection[0].path}>
      {selection.length === 1 ? selection[0].name : `${selection.length} ${t("media.selected")}`}
    </h3>

    {#if single}
      <dl class="tech">
        {#each tech as [k, v] (k)}<dt>{k}</dt><dd class="mono">{v}</dd>{/each}
      </dl>
    {/if}

    {#if !app.project}
      <p class="warn">{t("media.need.project")}</p>
    {/if}

    {#each groups as g (g)}
      <fieldset>
        <legend>{t(`media.group.${g}`)}</legend>
        {#each visibleFields.filter((f) => f.group === g) as f (f.id)}
          <label class:edited={dirty.has(f.id)}>
            <span>{t(`field.${f.id}`)}</span>
            {#if f.kind === "bool"}
              <select value={draft[f.id]} onchange={(e) => edit(f.id, e.currentTarget.value)}>
                {#if draft[f.id] === MIXED}<option value={MIXED}>{t("media.mixed")}</option>{/if}
                <option value="">-</option>
                <option value="true">{t("media.yes")}</option>
                <option value="false">{t("media.no")}</option>
              </select>
            {:else}
              <input
                type={f.kind === "number" ? "number" : "text"}
                value={draft[f.id] === MIXED ? "" : draft[f.id]}
                placeholder={draft[f.id] === MIXED ? t("media.mixed") : ""}
                oninput={(e) => edit(f.id, e.currentTarget.value)}
              />
            {/if}
          </label>
        {/each}
      </fieldset>
    {/each}

    {#if single && channels > 0}
      <fieldset>
        <legend>{t("media.group.tracks")}</legend>
        {#each Array.from({ length: Math.min(channels, 64) }, (_, i) => i + 1) as n (n)}
          <label class:edited={dirty.has(`track.${n}`)}>
            <span>{t("media.track")} {n}</span>
            <input value={draft[`track.${n}`] ?? ""} oninput={(e) => edit(`track.${n}`, e.currentTarget.value)} />
          </label>
        {/each}
      </fieldset>
    {/if}

    <div class="save">
      <button disabled={dirty.size === 0 || saving || !app.project} onclick={save}>
        {selection.length > 1 ? `${t("media.save.batch")} (${selection.length})` : t("media.save")}
      </button>
      {#if message}<span class="small">{message}</span>{/if}
    </div>
  {/if}
</aside>

<style>
  .inspector {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-3);
    padding: var(--vf-space-3);
    background: var(--vf-surface);
    border-left: 1px solid var(--vf-border);
    overflow-y: auto;
    min-height: 0;
  }
  h3 {
    margin: 0;
    font-size: var(--vf-text-md);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .tech {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 2px var(--vf-space-2);
    margin: 0;
    font-size: var(--vf-text-xs);
  }
  dt {
    color: var(--vf-text-muted);
  }
  dd {
    margin: 0;
    word-break: break-all;
  }
  fieldset {
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-md);
    padding: var(--vf-space-2);
    display: grid;
    grid-template-columns: 1fr;
    gap: var(--vf-space-1);
  }
  legend {
    font-size: var(--vf-text-xs);
    color: var(--vf-accent);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    padding: 0 var(--vf-space-1);
  }
  label {
    display: grid;
    grid-template-columns: 42% 1fr;
    align-items: center;
    gap: var(--vf-space-2);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  label.edited span {
    color: var(--vf-accent);
  }
  input,
  select {
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px var(--vf-space-1);
    font-size: var(--vf-text-sm);
    min-width: 0;
  }
  .save {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    position: sticky;
    bottom: 0;
    background: var(--vf-surface);
    padding-top: var(--vf-space-2);
  }
  button {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    font-weight: 600;
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  .muted,
  .small {
    color: var(--vf-text-muted);
    font-size: var(--vf-text-sm);
    margin: 0;
  }
  .warn {
    color: var(--vf-warning);
    font-size: var(--vf-text-xs);
    margin: 0;
  }
</style>
