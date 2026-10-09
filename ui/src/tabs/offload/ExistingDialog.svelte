<!-- Rushes déjà présents en destination (copie interrompue) : vérification de
     leurs empreintes, puis choix entre compléter la copie et tout recopier. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { ask } from "../../stores/confirm.svelte";
  import Modal from "../../components/Modal.svelte";
  import ProgressBar from "../../components/ProgressBar.svelte";
  import { t } from "../../i18n/index.svelte";
  import { bytes } from "../../lib/format";
  import { checkExisting, checkCancel, onCheckProgress, type CheckView, type ExistingMode, type OffloadRequest } from "../../lib/offload";

  let {
    request,
    onChoose,
    onClose,
  }: {
    request: OffloadRequest;
    onChoose: (mode: ExistingMode, check: number | null) => void;
    onClose: () => void;
  } = $props();

  let progress = $state(0);
  let result = $state<CheckView | null>(null);
  let error = $state("");
  let unlisten: (() => void) | null = null;

  const present = $derived(result ? result.destinations.reduce((n, d) => n + d.identical + d.different.length, 0) : 0);
  const complete = $derived(result ? result.destinations.every((d) => d.different.length === 0 && d.missing === 0) : false);

  onMount(async () => {
    unlisten = await onCheckProgress((p) => (progress = p.total > 0 ? p.done / p.total : 1)).catch(() => null);
    try {
      result = await checkExisting(request);
    } catch (e) {
      error = String(e);
    }
  });

  onDestroy(() => unlisten?.());

  function close() {
    if (!result && !error) checkCancel().catch(() => {});
    onClose();
  }

  async function replace() {
    const ok = await ask(t("offload.existing.replace.confirm").replace("{n}", String(present)), {
      title: t("offload.existing.do.replace"),
      ok: t("offload.existing.do.replace"),
      kind: "danger",
    });
    if (ok) onChoose("replace", null);
  }
</script>

<Modal title={t("offload.existing.title")} onClose={close} width="min(620px, 94vw)">
  <div class="body">
    {#if error}
      <p class="error">{error}</p>
      <div class="buttons">
        <button class="ghost" onclick={onClose}>{t("offload.existing.cancel")}</button>
      </div>
    {:else if !result}
      <p>{t("offload.existing.checking")}</p>
      <ProgressBar value={progress} />
      <p class="muted small">{t("offload.existing.checking.hint")}</p>
      <div class="buttons">
        <button class="ghost" onclick={close}>{t("offload.existing.cancel")}</button>
      </div>
    {:else}
      <p>{complete ? t("offload.existing.complete") : t("offload.existing.found")}</p>
      <table>
        <thead>
          <tr>
            <th>{t("offload.destination")}</th>
            <th>{t("offload.existing.identical")}</th>
            <th>{t("offload.existing.different")}</th>
            <th>{t("offload.existing.missing")}</th>
          </tr>
        </thead>
        <tbody>
          {#each result.destinations as d, i (i)}
            <tr>
              <td class="mono path" title={result.roots[i]}>{i + 1}. {result.roots[i]}</td>
              <td class="num ok">{d.identical} ({bytes(d.identical_bytes)})</td>
              <td class="num" class:warn={d.different.length > 0}>{d.different.length}</td>
              <td class="num">{d.missing}</td>
            </tr>
          {/each}
        </tbody>
      </table>
      {#if result.destinations.some((d) => d.different.length > 0)}
        <details>
          <summary class="small">{t("offload.existing.different.list")}</summary>
          <ul class="mono small">
            {#each result.destinations as d, i (i)}
              {#each d.different as rel (rel)}<li>{i + 1}. {rel}</li>{/each}
            {/each}
          </ul>
        </details>
      {/if}
      <div class="choices">
        <button class="primary" onclick={() => onChoose("complete", result!.id)}>
          <b>{t("offload.existing.do.complete")}</b>
          <span>{t("offload.existing.do.complete.hint")}</span>
        </button>
        <button onclick={replace}>
          <b>{t("offload.existing.do.replace")}</b>
          <span>{t("offload.existing.do.replace.hint")}</span>
        </button>
      </div>
      <div class="buttons">
        <button class="ghost" onclick={onClose}>{t("offload.existing.cancel")}</button>
      </div>
    {/if}
  </div>
</Modal>

<style>
  .body {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-3);
    padding: var(--vf-space-3);
  }
  p {
    margin: 0;
  }
  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--vf-text-sm);
    table-layout: fixed;
  }
  th,
  td {
    padding: 4px 6px;
    border-bottom: 1px solid var(--vf-border);
    text-align: left;
  }
  th {
    color: var(--vf-text-muted);
    font-weight: normal;
  }
  th:first-child {
    width: 46%;
  }
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .num {
    font-variant-numeric: tabular-nums;
  }
  .ok {
    color: var(--vf-success);
  }
  .warn {
    color: var(--vf-warning);
  }
  .error {
    color: var(--vf-error);
  }
  ul {
    margin: var(--vf-space-1) 0 0;
    padding-left: var(--vf-space-4);
    max-height: 140px;
    overflow-y: auto;
  }
  .choices {
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-2);
  }
  .choices button {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: var(--vf-space-2) var(--vf-space-3);
    text-align: left;
    height: auto;
  }
  .choices .primary {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: transparent;
  }
  .choices span {
    font-size: var(--vf-text-xs);
    opacity: 0.85;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
  }
</style>
