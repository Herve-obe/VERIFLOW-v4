<!-- Suivi d'une copie : progression, débit, temps restant, tableau d'avancement, résultat. -->
<script lang="ts">
  import ProgressBar from "../../components/ProgressBar.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes, bytesBinary, rate, duration } from "../../lib/format";
  import { MAX_PARALLEL_JOBS, cancel, remainingSecs, reveal, type DestStatus } from "../../lib/offload";
  import { ask } from "../../stores/confirm.svelte";
  import { offload, interrupted, removeJob, type Job } from "../../stores/offload.svelte";

  let { job }: { job: Job } = $props();
  let showAll = $state(false);
  let openError = $state("");

  const open = (path: string) => {
    openError = "";
    reveal(path).catch((e) => (openError = String(e)));
  };

  const isVerify = $derived(job.kind === "verify");
  const vs = $derived(job.verify);
  const vOk = $derived(!!vs && vs.conform && !vs.cancelled);
  const progress = $derived(job.total > 0 ? job.done / job.total : 0);
  const eta = $derived(remainingSecs(job.total, job.done, job.rate, job.roots.length));
  const ok = $derived(job.result ? !job.result.summary.cancelled && job.result.summary.failed_files === 0 : false);
  const barState = $derived((job.state === "failed" && !job.dropped) || (job.result && !ok) || (isVerify && job.state === "done" && !vOk) ? "error" : job.state === "done" ? "ok" : "running");
  const GAP_MAX = 200;
  const gaps = $derived(vs ? vs.files.filter((f) => f.status.state === "different" || f.status.state === "missing" || f.status.state === "unreadable") : []);
  const news = $derived(vs ? vs.files.filter((f) => f.status.state === "new") : []);
  const rows = $derived(showAll ? job.recent : job.recent.slice(0, 12));

  // Reprise proposée tant que la carte n'a pas été recopiée avec succès depuis.
  const resumable = $derived(
    interrupted(job) &&
      !!offload.requests[job.id] &&
      !offload.jobs.some((o) => o.id > job.id && o.source === job.source && o.state === "done" && !interrupted(o)),
  );

  /** Retire la fiche après confirmation ; une copie en cours est d'abord annulée. */
  async function remove() {
    const active = job.state === "queued" || job.state === "waiting" || job.state === "starting" || job.state === "running";
    const ok = await ask(t(active ? "offload.remove.active" : "offload.remove.done"), {
      title: t("offload.remove.title"),
      ok: t(active ? "offload.remove.do.active" : "offload.remove.do"),
      kind: active ? "danger" : "warning",
    });
    if (!ok) return;
    if (active) await cancel(job.id).catch(() => {});
    removeJob(job.id);
  }

  const label = (d: DestStatus) =>
    d.state === "verified" ? t("offload.status.verified") : d.state === "resumed_verified" ? t("offload.status.resumed") : t("offload.status.failed");
</script>

<article class="job" class:ok={job.state === "done" && (isVerify ? vOk : ok)} class:ko={(job.state === "failed" && !job.dropped) || (job.state === "done" && !(isVerify ? vOk : ok))}>
  <header>
    <div>
      <h3>{isVerify ? (vs?.root ?? job.roots[0] ?? job.source) : (job.result?.source_name ?? job.source)}</h3>
      <span class="state">{t(job.dropped ? "offload.job.dropped" : `offload.job.${job.result?.summary.cancelled || vs?.cancelled ? "cancelled" : job.state}`)}</span>
      {#if job.state === "waiting" && job.waitReason}
        <span class="wait">{t(`offload.wait.${job.waitReason}`).replace("{max}", String(MAX_PARALLEL_JOBS))}</span>
      {/if}
    </div>
    <div class="tools">
      {#if job.state === "queued" || job.state === "waiting" || job.state === "starting" || job.state === "running"}
        <button class="ghost" onclick={() => cancel(job.id)}>{t("offload.cancel")}</button>
      {:else if resumable && !isVerify}
        <button onclick={() => (offload.resume = offload.requests[job.id])}>{t(job.dropped ? "offload.relaunch" : "offload.resume")}</button>
      {/if}
      <button class="close" onclick={remove} title={t("offload.remove.title")} aria-label={t("offload.remove.title")}>
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.5 2.5 L9.5 9.5 M9.5 2.5 L2.5 9.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" /></svg>
      </button>
    </div>
  </header>

  <ProgressBar value={job.state === "done" ? 1 : progress} state={barState} />

  <div class="stats mono">
    {#if job.state === "running"}
      <span>{Math.round(progress * 100)} %</span>
      {#if !isVerify}<span>{job.verified} / {job.files} {t("offload.files")}</span>{/if}
      <span>{rate(job.rate)}</span>
      <span>{t("offload.eta")} {duration(eta)}</span>
    {:else if vs}
      <span>{vs.files.length} {t("offload.files")}</span>
      <span>{bytes(vs.total_bytes)} ({bytesBinary(vs.total_bytes)})</span>
      <span>{duration(vs.duration_s)}</span>
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

  {#if job.state === "done" && vs}
    <div class="verdict" class:good={vOk}>
      {vs.cancelled ? t("offload.verdict.cancelled") : vOk ? t("offload.verify.verdict.ok") : t("offload.verify.verdict.gap")}
    </div>
    <div class="counts mono">
      <span class="good-n">{t("offload.verify.ok")} : {vs.ok}</span>
      <span class:bad-n={vs.different > 0}>{t("offload.verify.different")} : {vs.different}</span>
      <span class:bad-n={vs.missing > 0}>{t("offload.verify.missing")} : {vs.missing}</span>
      <span>{t("offload.verify.new")} : {vs.new}</span>
      <span class:bad-n={vs.unreadable > 0}>{t("offload.verify.unreadable")} : {vs.unreadable}</span>
    </div>
    {#if job.reportError || vs.write_error || (!job.verifyReports && !vs.cancelled)}
      <div class="small err">{t("offload.verify.readonly")}</div>
    {/if}
    <div class="actions">
      {#if job.verifyReports}
        <button onclick={() => open(job.verifyReports!.pdf)}>{t("offload.report.pdf")}</button>
        <button class="ghost" onclick={() => open(job.verifyReports!.html)}>{t("offload.verify.report.html")}</button>
        <button class="ghost" onclick={() => open(job.verifyReports!.csv)}>{t("offload.verify.report.csv")}</button>
      {/if}
      <button class="ghost" onclick={() => open(vs.root)}>{t("offload.open.folder")}</button>
    </div>
    {#if openError}<div class="small err">{openError}</div>{/if}
    {#if gaps.length > 0}
      <details>
        <summary class="small err">{t("offload.verify.gaps")} ({gaps.length})</summary>
        <ul class="failures mono">
          {#each gaps.slice(0, GAP_MAX) as f (f.rel)}
            <li>{f.rel} : {t(`offload.verify.${f.status.state === "unreadable" ? "unreadable" : f.status.state}`)}{f.status.state === "unreadable" ? ` (${f.status.detail})` : ""}</li>
          {/each}
        </ul>
        {#if gaps.length > GAP_MAX}<div class="small">{t("offload.verify.more").replace("{n}", String(gaps.length - GAP_MAX))}</div>{/if}
      </details>
    {/if}
    {#if news.length > 0}
      <details>
        <summary class="small">{t("offload.verify.new")} ({news.length})</summary>
        <ul class="newfiles mono">
          {#each news.slice(0, GAP_MAX) as f (f.rel)}<li>{f.rel}</li>{/each}
        </ul>
        {#if news.length > GAP_MAX}<div class="small">{t("offload.verify.more").replace("{n}", String(news.length - GAP_MAX))}</div>{/if}
      </details>
    {/if}
  {:else if job.state === "done" && job.result}
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
      <div class="small">{job.ejected === "ok" ? t("offload.ejected") : `${t("offload.eject.error")} : ${job.ejectCode ? t(`offload.eject.code.${job.ejectCode}`) : job.ejected}`}</div>
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
    <div class="verdict">{job.errorCode ? t(`offload.error.${job.errorCode}`) : job.error}</div>
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
  .wait {
    display: block;
    font-size: var(--vf-text-sm);
    color: var(--vf-warning);
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
  .counts {
    display: flex;
    flex-wrap: wrap;
    gap: var(--vf-space-4);
    font-size: var(--vf-text-sm);
  }
  .counts .good-n {
    color: var(--vf-success);
  }
  .counts .bad-n {
    color: var(--vf-error);
    font-weight: 700;
  }
  .newfiles {
    margin: 0;
    padding-left: var(--vf-space-4);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  details {
    max-height: 260px;
    overflow-y: auto;
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
