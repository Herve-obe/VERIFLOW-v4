<!-- Suivi d'une copie : progression, débit, temps restant, tableau d'avancement, résultat. -->
<script lang="ts">
  import ProgressBar from "../../components/ProgressBar.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes, bytesBinary, rate, duration } from "../../lib/format";
  import { cancel, reveal, type DestStatus } from "../../lib/offload";
  import type { Job } from "../../stores/offload.svelte";

  let { job }: { job: Job } = $props();
  let showAll = $state(false);
  let openError = $state("");

  const open = (path: string) => {
    openError = "";
    reveal(path).catch((e) => (openError = String(e)));
  };

  const progress = $derived(job.total > 0 ? job.done / job.total : 0);
  const eta = $derived(job.rate > 0 && job.total > 0 ? ((job.total - job.done) / (job.rate * (1 + job.roots.length))) : NaN);
  const ok = $derived(job.result ? !job.result.summary.cancelled && job.result.summary.failed_files === 0 : false);
  const barState = $derived(job.state === "failed" || (job.result && !ok) ? "error" : job.state === "done" ? "ok" : "running");
  const rows = $derived(showAll ? job.recent : job.recent.slice(0, 12));

  const label = (d: DestStatus) =>
    d.state === "verified" ? t("offload.status.verified") : d.state === "resumed_verified" ? t("offload.status.resumed") : t("offload.status.failed");
</script>

<article class="job" class:ok={job.state === "done" && ok} class:ko={job.state === "failed" || (job.state === "done" && !ok)}>
  <header>
    <div>
      <h3>{job.result?.source_name ?? job.source}</h3>
      <span class="state">{t(`offload.job.${job.state}`)}</span>
    </div>
    {#if job.state === "queued" || job.state === "running"}
      <button class="ghost" onclick={() => cancel(job.id)}>{t("offload.cancel")}</button>
    {/if}
  </header>

  <ProgressBar value={job.state === "done" ? 1 : progress} state={barState} />

  <div class="stats mono">
    {#if job.state === "running"}
      <span>{Math.round(progress * 100)} %</span>
      <span>{job.verified} / {job.files} {t("offload.files")}</span>
      <span>{rate(job.rate)}</span>
      <span>{t("offload.eta")} {duration(eta)}</span>
    {:else if job.result}
      <span>{job.result.summary.files.length} {t("offload.files")}</span>
      <span>{bytes(job.result.summary.total_bytes)} ({bytesBinary(job.result.summary.total_bytes)})</span>
      <span>{duration(job.result.summary.duration_s)}</span>
      <span>{rate(job.result.summary.total_bytes / Math.max(job.result.summary.duration_s, 0.001))}</span>
    {/if}
  </div>

  {#if job.current}
    <div class="current mono" title={job.current}>{job.current}</div>
  {/if}

  {#if job.state === "done" && job.result}
    <div class="verdict" class:good={ok}>
      {#if job.result.summary.cancelled}
        {t("offload.verdict.cancelled")}
      {:else if ok}
        {t("offload.verdict.ok")}
      {:else}
        {job.result.summary.failed_files} {t("offload.verdict.failed")}
      {/if}
    </div>
    {#if job.ejected}
      <div class="small">{job.ejected === "ok" ? t("offload.ejected") : `${t("offload.eject.error")} : ${job.ejected}`}</div>
    {/if}
    <div class="actions">
      {#each job.result.reports as r, i (r.pdf)}
        <button onclick={() => open(r.pdf)}>{t("offload.report.pdf")} {i + 1}</button>
      {/each}
      {#each job.result.roots as root, i (root)}
        <button class="ghost" onclick={() => open(root)}>{t("offload.open.folder")} {i + 1}</button>
      {/each}
    </div>
    {#if openError}<div class="small err">{openError}</div>{/if}
  {/if}

  {#if job.error}
    <div class="verdict">{job.error}</div>
  {/if}

  {#if job.failed.length > 0}
    <h4 class="err">{t("offload.failures")}</h4>
    <ul class="failures mono">
      {#each job.failed as f (f.rel)}
        <li>
          {f.rel}
          {#each f.destinations as d, i (i)}
            {#if d.state === "failed"}<div>{t("offload.destination")} {i + 1} : {d.detail}</div>{/if}
          {/each}
        </li>
      {/each}
    </ul>
  {/if}

  {#if job.recent.length > 0}
    <table>
      <thead>
        <tr><th>{t("offload.file")}</th><th>{t("offload.size")}</th><th>{t("offload.hash")}</th>
          {#each job.roots as _, i (i)}<th>{t("offload.dest.short")} {i + 1}</th>{/each}</tr>
      </thead>
      <tbody>
        {#each rows as f (f.rel)}
          <tr>
            <td class="mono path" title={f.rel}>{f.rel}</td>
            <td class="mono num">{bytes(f.size)}</td>
            <td class="mono hash">{f.hashes[0]?.[1] ?? ""}</td>
            {#each f.destinations as d, i (i)}
              <td class="st {d.state}" title={d.state === "failed" ? d.detail : ""}>{label(d)}</td>
            {/each}
          </tr>
        {/each}
      </tbody>
    </table>
    {#if job.recent.length > 12}
      <button class="link" onclick={() => (showAll = !showAll)}>{showAll ? t("offload.less") : t("offload.more")}</button>
    {/if}
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
    align-items: center;
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
  .verdict {
    font-weight: 700;
    color: var(--vf-error);
  }
  .verdict.good {
    color: var(--vf-success);
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-2);
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
  button.link {
    background: none;
    color: var(--vf-accent);
    padding: 0;
    align-self: flex-start;
  }
  .err {
    margin: var(--vf-space-2) 0 0;
    color: var(--vf-error);
    font-size: var(--vf-text-sm);
  }
  .failures {
    margin: 0;
    padding-left: var(--vf-space-4);
    font-size: var(--vf-text-xs);
    color: var(--vf-error);
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
  }
  th {
    color: var(--vf-text-muted);
    font-weight: 600;
  }
  .path {
    width: 40%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    direction: rtl;
    text-align: left;
  }
  .num {
    width: 80px;
  }
  .hash {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--vf-text-muted);
  }
  .st.verified,
  .st.resumed_verified {
    color: var(--vf-success);
  }
  .st.failed {
    color: var(--vf-error);
    font-weight: 700;
  }
</style>
