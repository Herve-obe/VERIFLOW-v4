<!-- Revérification d'un disque d'après son historique ASC MHL, sans la source :
     résumé de l'historique, choix d'un sous-dossier si besoin, lancement. -->
<script lang="ts">
  import { onMount } from "svelte";
  import Modal from "../../components/Modal.svelte";
  import { t, i18n } from "../../i18n/index.svelte";
  import { mhlInfo, mhlVerify, type MhlError, type MhlInfo } from "../../lib/offload";
  import { markVerify } from "../../stores/offload.svelte";

  let { root, operator = null, onClose }: { root: string; operator?: string | null; onClose: () => void } = $props();

  let info = $state<MhlInfo | null>(null);
  let error = $state("");
  let loading = $state(true);
  let starting = $state(false);
  let choice = $state("");

  const when = (d: string | null) => {
    if (!d) return "";
    const x = new Date(d);
    return Number.isNaN(x.getTime()) ? d : x.toLocaleString(i18n.lang);
  };
  const algoLabel = (a: string) => a.toUpperCase();

  async function load(path: string) {
    loading = true;
    error = "";
    info = null;
    try {
      info = await mhlInfo(path);
    } catch (e) {
      const err = e as Partial<MhlError>;
      error = err?.code ? t(`offload.verify.${err.code}`) : String(err?.message ?? e);
    } finally {
      loading = false;
    }
  }

  onMount(() => load(root));

  const ready = $derived(!!info && info.generations > 0);

  async function launch() {
    if (!info || !ready) return;
    starting = true;
    try {
      markVerify(await mhlVerify(info.root, operator));
      onClose();
    } catch (e) {
      const err = e as Partial<MhlError>;
      error = err?.code ? t(`offload.verify.${err.code}`) : String(err?.message ?? e);
      starting = false;
    }
  }
</script>

<Modal title={t("offload.verify.title")} onClose={onClose} width="min(620px, 94vw)">
  <div class="body" aria-live="polite" aria-busy={loading}>
    <p class="mono path" title={info?.root ?? root}>{info?.root ?? root}</p>
    {#if loading}
      <p class="muted" role="status">{t("offload.verify.loading")}</p>
    {:else if error}
      <p class="error">{error}</p>
    {:else if info && info.generations === 0 && info.histories.length > 0}
      <p>{t("offload.verify.histories_found")}</p>
      <ul class="histories">
        {#each info.histories as h (h)}
          <li>
            <label class="pick">
              <input type="radio" name="history" value={h} bind:group={choice} />
              <span class="mono" title={h}>{h}</span>
            </label>
          </li>
        {/each}
      </ul>
      <div class="buttons">
        <button class="ghost" onclick={onClose}>{t("offload.cancel")}</button>
        <button disabled={!choice} onclick={() => load(choice)}>{t("offload.verify.choose")}</button>
      </div>
    {:else if info}
      <dl>
        <dt>{t("offload.verify.generations")}</dt><dd>{info.generations}</dd>
        <dt>{t("offload.verify.last_date")}</dt><dd>{when(info.last_date)}</dd>
        <dt>{t("offload.verify.files")}</dt><dd>{info.files}</dd>
        <dt>{t("offload.verify.algos")}</dt><dd>{info.algos.map(algoLabel).join(", ")}</dd>
      </dl>
      {#each info.chain_issues as c (c.kind + c.name)}
        <p class="error">{t(`offload.verify.chain_${c.kind}`)} : <span class="mono">{c.name}</span></p>
      {/each}
      <p class="muted small">{t("offload.verify.hint")}</p>
    {/if}
    {#if !(info && info.generations === 0 && info.histories.length > 0)}
      <div class="buttons">
        <button class="ghost" onclick={onClose}>{t("offload.cancel")}</button>
        {#if ready}
          <button disabled={starting} onclick={launch}>{t("offload.verify.start")}</button>
        {/if}
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
  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: var(--vf-text-sm);
    color: var(--vf-text-muted);
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .small {
    font-size: var(--vf-text-sm);
  }
  .error {
    color: var(--vf-error);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: var(--vf-space-1) var(--vf-space-4);
    margin: 0;
    font-size: var(--vf-text-sm);
  }
  dt {
    color: var(--vf-text-muted);
  }
  dd {
    margin: 0;
  }
  .histories {
    list-style: none;
    margin: 0;
    padding: 0;
    max-height: 200px;
    overflow-y: auto;
  }
  .pick {
    display: flex;
    gap: var(--vf-space-2);
    align-items: center;
    padding: 3px 0;
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .pick span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: var(--vf-space-2);
  }
  button {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-1) var(--vf-space-3);
    cursor: pointer;
  }
  button:disabled {
    opacity: 0.5;
    cursor: default;
  }
  button.ghost {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
  }
</style>
