<!-- Confirmation au style de VERIFLOW : icône, titre, message, deux boutons.
     Entrée valide, Échap annule ; les autres fenêtres ouvertes ne réagissent
     pas à ces touches tant que la confirmation est affichée. -->
<script lang="ts">
  import { tick } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { confirmQueue, answer } from "../stores/confirm.svelte";

  const current = $derived(confirmQueue.items[0] ?? null);
  let okButton = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (current) tick().then(() => okButton?.focus());
  });

  function onKeydown(e: KeyboardEvent) {
    if (!current) return;
    if (e.key === "Escape" || e.key === "Enter") {
      e.preventDefault();
      e.stopImmediatePropagation();
      answer(e.key === "Enter");
    }
  }
</script>

<svelte:window onkeydowncapture={onKeydown} />

{#if current}
  <div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && answer(false)}>
    <div class="card" role="alertdialog" aria-modal="true" aria-labelledby="vf-confirm-title">
      <div class="head">
        <svg viewBox="0 0 24 24" class="icon" class:danger={current.kind === "danger"} aria-hidden="true">
          <path d="M12 3 L22 20 H2 Z" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round" />
          <path d="M12 9 V14 M12 16.6 V17" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" />
        </svg>
        <h4 id="vf-confirm-title">{current.title}</h4>
      </div>
      <p>{current.message}</p>
      <div class="buttons">
        <button class="ghost" onclick={() => answer(false)}>{current.cancel ?? t("dialog.cancel")}</button>
        <button class="ok" class:danger={current.kind === "danger"} bind:this={okButton} onclick={() => answer(true)}>
          {current.ok ?? t("dialog.ok")}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .backdrop {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    z-index: 20;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--vf-backdrop);
  }
  .card {
    width: 440px;
    max-width: 92vw;
    padding: var(--vf-space-4);
    background: var(--vf-surface);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-lg);
    box-shadow: 0 12px 40px var(--vf-shadow);
    color: var(--vf-text);
    font-size: var(--vf-text-sm);
  }
  .head {
    display: flex;
    align-items: center;
    margin-bottom: var(--vf-space-3);
  }
  .icon {
    width: 22px;
    height: 22px;
    flex: none;
    margin-right: var(--vf-space-2);
    color: var(--vf-warning);
  }
  .icon.danger {
    color: var(--vf-error);
  }
  h4 {
    margin: 0;
    font-size: var(--vf-text-md);
    font-weight: 600;
  }
  p {
    margin: 0 0 var(--vf-space-4);
    line-height: 1.5;
    white-space: pre-line;
    color: var(--vf-text-muted);
  }
  .buttons {
    display: flex;
    justify-content: flex-end;
  }
  .buttons button + button {
    margin-left: var(--vf-space-2);
  }
  .ok {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border-color: transparent;
    font-weight: 600;
  }
  .ok.danger {
    background: var(--vf-error);
    color: var(--vf-on-error);
  }
</style>
