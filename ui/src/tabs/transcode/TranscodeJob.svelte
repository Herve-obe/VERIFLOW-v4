<!-- Suivi d'un lot de conversions : progression, vitesse, résultat par fichier,
     mesures de loudness. -->
<script lang="ts">
  import ProgressBar from "../../components/ProgressBar.svelte";
  import { t } from "../../i18n/index.svelte";
  import { duration } from "../../lib/format";
  import { reveal } from "../../lib/offload";
  import { cancel, type FileResult } from "../../lib/transcode";
  import { ask } from "../../stores/confirm.svelte";
  import { progressOf, removeJob, type Job } from "../../stores/transcode.svelte";

  let { job }: { job: Job } = $props();
  let openError = $state("");

  const active = $derived(job.state === "queued" || job.state === "running");
  const failed = $derived(job.results.filter((r) => r.status === "failed").length);
  const done = $derived(job.results.filter((r) => r.status === "done").length);
  const ok = $derived(job.state === "done" && !job.cancelled && failed === 0);
  const progress = $derived(job.state === "done" ? 1 : progressOf(job));
  const barState = $derived(job.state === "failed" || (job.state === "done" && !ok) ? "error" : job.state === "done" ? "ok" : "running");
  // Temps restant estimé à partir du temps écoulé et de l'avancement global.
  const eta = $derived.by(() => {
    if (job.state !== "running" || !job.startedAt || progress <= 0.02) return NaN;
    const elapsed = (Date.now() - job.startedAt) / 1000;
    return (elapsed * (1 - progress)) / progress;
  });
  const target = $derived(job.request.settings?.loudness ?? null);
  const measured = $derived(job.results.some((r) => r.loudness));
  const ANALYSES = ["analyze", "vmaf", "cut_detect", "black_detect", "offline_detect", "silence_detect", "framemd5"];
  const analysis = $derived(ANALYSES.includes(job.request.settings?.preset ?? ""));
  const scored = $derived(job.results.some((r) => r.vmaf !== null && r.vmaf !== undefined));
  const detections = $derived(job.results.filter((r) => r.analysis && ["cut_detect", "black_detect", "offline_detect", "silence_detect"].includes(job.request.settings?.preset ?? "")));
  const span = (a: number, b: number | null) => (b === null ? "" : `${(b - a).toFixed(2).replace(".", ",")} s`);
  const firstOutput = $derived(job.results.find((r) => r.output)?.output ?? null);

  const name = (p: string | null) => (p ? (p.split(/[\\/]/).pop() ?? p) : "");
  const folder = (p: string) => p.slice(0, Math.max(0, p.length - name(p).length - 1));
  const db = (v: number | null | undefined, digits = 1) =>
    v === null || v === undefined || !Number.isFinite(v) ? "-" : v.toFixed(digits).replace(".", ",").replace(/^-0,0$/, "0,0");
  const signed = (v: number) => `${v > 0 ? "+" : ""}${db(v)}`;

  function openFolder() {
    openError = "";
    const dir = job.request.dest ?? (firstOutput ? folder(firstOutput) : null);
    if (dir) reveal(dir).catch((e) => (openError = String(e)));
  }

  async function remove() {
    const yes = await ask(t(active ? "transcode.remove.active" : "transcode.remove.done"), {
      title: t("transcode.remove.title"),
      ok: t(active ? "transcode.remove.do.active" : "transcode.remove.do"),
      kind: active ? "danger" : "warning",
    });
    if (!yes) return;
    if (active) await cancel(job.id).catch(() => {});
    removeJob(job.id);
  }

  const statusLabel = (r: FileResult) => t(`transcode.status.${r.status}`);
  const stateLabel = $derived(
    job.state === "done" && job.cancelled ? t("transcode.job.cancelled") : t(`transcode.job.${job.state}`),
  );
</script>

<article class="job" class:ok class:ko={job.state === "failed" || (job.state === "done" && !ok)}>
  <header>
    <div>
      <h3>{job.label || `#${job.id}`}</h3>
      <span class="state">{stateLabel}</span>
    </div>
    <div class="tools">
      {#if active}
        <button class="ghost" onclick={() => cancel(job.id)}>{t("transcode.cancel")}</button>
      {:else if done > 0 && firstOutput}
        <button class="ghost" onclick={openFolder}>{t("transcode.open.folder")}</button>
      {/if}
      {#if job.report}
        <button class="ghost" onclick={() => job.report && reveal(job.report).catch((e) => (openError = String(e)))}>{t("transcode.open.report")}</button>
      {/if}
      <button class="close" onclick={remove} title={t("transcode.remove.title")} aria-label={t("transcode.remove.title")}>
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 2.5 L9.5 9.5 M9.5 2.5 L2.5 9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
      </button>
    </div>
  </header>

  <ProgressBar value={progress} state={barState} />

  <div class="stats mono">
    {#if job.state === "running"}
      <span>{Math.round(progress * 100)} %</span>
      <span>{t("transcode.file")} {Math.min(job.index + 1, job.files)} / {job.files}</span>
      {#if job.speed > 0}<span>× {job.speed.toFixed(1).replace(".", ",")}</span>{/if}
      <span>{t("offload.eta")} {duration(eta)}</span>
    {:else if job.state === "done"}
      <span>{done} / {job.files} {t(analysis ? "transcode.files.measured" : "transcode.files.done")}</span>
      {#if failed}<span class="bad">{failed} {t("transcode.files.failed")}</span>{/if}
      <span>{duration(job.seconds)}</span>
    {/if}
  </div>
  {#if job.state === "running" && job.current}
    <p class="current" title={job.current}>{name(job.current)}</p>
  {/if}
  {#if job.error}<p class="err">{job.error}</p>{/if}
  {#if openError}<p class="err">{openError}</p>{/if}

  {#if job.results.length}
    <table>
      <thead>
        <tr>
          <th class="file">{t("transcode.col.file")}</th>
          <th class="st">{t("transcode.col.status")}</th>
          {#if scored}<th class="num">VMAF</th>{/if}
          {#if measured}
            <th class="num">LUFS</th>
            <th class="num">LRA</th>
            <th class="num">dBTP</th>
            {#if target}<th class="num">{t(analysis ? "transcode.col.gap" : "transcode.col.gain")}</th>{/if}
          {/if}
        </tr>
      </thead>
      <tbody>
        {#each job.results as r (r.source)}
          <tr>
            <td class="file" title={r.output ?? r.source}>
              {name(r.source)}{#if r.output && name(r.output) !== name(r.source)}<span class="muted"> → {name(r.output)}</span>{/if}
              {#if r.outputs.length > 1}<span class="muted"> ({r.outputs.length} {t("transcode.files")})</span>{/if}
              {#if r.message}<div class="msg" class:info={r.status === "done"}>{r.message}</div>{/if}
              {#if r.checksum}<div class="sum mono" title="XXH128">XXH128 {r.checksum}</div>{/if}
            </td>
            <td class="st {r.status}">{statusLabel(r)}</td>
            {#if scored}<td class="num">{db(r.vmaf, 2)}</td>{/if}
            {#if measured}
              {@const l = r.loudness}
              <td class="num">{db(l?.integrated)}</td>
              <td class="num">{db(l?.range)}</td>
              <td class="num" class:bad={!!l && !!target && l.true_peak > target.true_peak && l.gain === null}>{db(l?.true_peak)}</td>
              {#if target}
                <td class="num" title={l?.peak_limited ? t("transcode.peak.limited") : ""}>
                  {#if l?.gain !== null && l?.gain !== undefined}{signed(l.gain)}{#if l.peak_limited}<span class="bad"> *</span>{/if}
                  {:else if l && Number.isFinite(l.integrated)}{signed(l.integrated - target.integrated)}{/if}
                </td>
              {/if}
            {/if}
          </tr>
        {/each}
      </tbody>
    </table>
    {#if measured && target}
      <p class="small">{t(job.results.some((r) => r.loudness?.gain !== null && r.loudness?.gain !== undefined) ? "transcode.gain.legend" : "transcode.gap.legend")}</p>
    {/if}
    {#if job.results.some((r) => r.loudness?.peak_limited)}
      <p class="small bad">* {t("transcode.peak.limited")}</p>
    {/if}
    {#each detections as r (r.source)}
      <details class="found">
        <summary>
          {name(r.source)} : {r.analysis?.segments.length ?? 0} {t("transcode.found")}
          {#if r.output}<button class="link" onclick={() => r.output && reveal(r.output).catch((e) => (openError = String(e)))}>{t("transcode.open.file")}</button>{/if}
        </summary>
        <table>
          <tbody>
            {#each (r.analysis?.segments ?? []).slice(0, 200) as sg, i (i)}
              <tr>
                <td class="num">{i + 1}</td>
                <td class="mono">{sg.start_tc ?? sg.start.toFixed(2)}</td>
                <td class="mono">{sg.end_tc ?? (sg.end !== null ? sg.end.toFixed(2) : "")}</td>
                <td class="num">{span(sg.start, sg.end)}</td>
              </tr>
            {/each}
          </tbody>
        </table>
      </details>
    {/each}
  {/if}
</article>

<style>
  .job {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
    padding: var(--vf-space-3);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-left: 3px solid var(--vf-accent);
    border-radius: var(--vf-radius-md);
  }
  .job.ok {
    border-left-color: var(--vf-success);
  }
  .job.ko {
    border-left-color: var(--vf-error);
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: var(--vf-space-2);
  }
  .tools {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    flex: none;
  }
  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 24px;
    height: 24px;
    padding: 0;
    border: 0;
    background: none;
    color: var(--vf-text-muted);
    border-radius: var(--vf-radius-sm);
    cursor: pointer;
  }
  .close:hover {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  .close svg {
    width: 12px;
    height: 12px;
  }
  h3 {
    margin: 0;
    font-size: var(--vf-text-lg);
  }
  .state,
  .small {
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  p {
    margin: 0;
  }
  .stats {
    display: flex;
    gap: var(--vf-space-4);
    font-size: var(--vf-text-sm);
  }
  .current {
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  button {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  button.ghost {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
  }
  .err {
    color: var(--vf-error);
    font-size: var(--vf-text-sm);
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--vf-text-xs);
    table-layout: fixed;
  }
  th,
  td {
    text-align: left;
    padding: 3px 6px;
    border-bottom: 1px solid var(--vf-border);
    vertical-align: top;
  }
  th {
    color: var(--vf-text-muted);
    font-weight: 600;
  }
  .file {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .msg {
    white-space: normal;
    color: var(--vf-error);
  }
  .msg.info {
    color: var(--vf-text-muted);
  }
  .sum {
    color: var(--vf-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .found summary {
    cursor: pointer;
    font-size: var(--vf-text-sm);
  }
  .found table {
    margin-top: var(--vf-space-1);
  }
  button.link {
    background: none;
    border: 0;
    color: var(--vf-accent);
    padding: 0 var(--vf-space-2);
    font-size: var(--vf-text-xs);
  }
  .st {
    width: 90px;
  }
  .num {
    width: 64px;
    text-align: right;
    font-family: var(--vf-font-mono);
  }
  .st.done {
    color: var(--vf-success);
  }
  .st.failed,
  .bad {
    color: var(--vf-error);
  }
  .st.skipped,
  .st.cancelled,
  .muted {
    color: var(--vf-text-muted);
  }
</style>
