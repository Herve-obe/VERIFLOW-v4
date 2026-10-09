<!-- Champ à choix : liste déroulante des valeurs proposées, et « Autre » pour
     en saisir une à la main. -->
<script lang="ts">
  import { tick } from "svelte";
  import { t } from "../../i18n/index.svelte";

  let {
    value,
    options,
    unit = "",
    onChange,
  }: {
    value: string;
    options: string[];
    unit?: string;
    onChange: (v: string) => void;
  } = $props();

  const OTHER = "\u0000autre";
  // « Autre » choisi sans valeur encore saisie.
  let forceOther = $state(false);
  let input = $state<HTMLInputElement | null>(null);

  const isOther = $derived(forceOther || (value !== "" && !options.includes(value)));
  const selected = $derived(isOther ? OTHER : value);

  async function pick(v: string) {
    if (v === OTHER) {
      forceOther = true;
      onChange("");
      await tick();
      input?.focus();
    } else {
      forceOther = false;
      onChange(v);
    }
  }
</script>

<span class="choice">
  <select value={selected} onchange={(e) => pick(e.currentTarget.value)}>
    <option value="">-</option>
    {#each options as o (o)}<option value={o}>{o}</option>{/each}
    <option value={OTHER}>{t("report.other")}</option>
  </select>
  {#if isOther}
    <input bind:this={input} {value} placeholder={t("report.other.hint")} oninput={(e) => onChange(e.currentTarget.value)} />
  {/if}
  {#if unit}<span class="unit">{unit}</span>{/if}
</span>

<style>
  .choice {
    display: flex;
    align-items: center;
    min-width: 0;
  }
  select {
    flex: 1;
    min-width: 0;
  }
  input {
    flex: 1;
    min-width: 0;
    margin-left: var(--vf-space-1);
  }
  select,
  input {
    background: var(--vf-bg);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 0 var(--vf-space-2);
    font-size: var(--vf-text-sm);
    height: 26px;
  }
  .unit {
    margin-left: var(--vf-space-1);
    font-size: var(--vf-text-xs);
    white-space: nowrap;
    color: var(--vf-text-muted);
  }
</style>
