<!-- Choix de la sortie audio : pilote et carte son, paire de canaux. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { t } from "../i18n/index.svelte";
  import { output, refreshOutputs, setOutput } from "../stores/output.svelte";

  const HOST_LABELS: Record<string, string> = {
    wasapi: "WASAPI",
    asio: "ASIO",
    coreaudio: "CoreAudio",
    alsa: "ALSA",
    jack: "JACK",
    pulseaudio: "PulseAudio",
    pipewire: "PipeWire",
  };
  const hostLabel = (h: string) => HOST_LABELS[h.toLowerCase()] ?? h;

  const hosts = $derived([...new Set(output.devices.map((d) => d.host))]);
  const selected = $derived(output.devices.find((d) => d.id === output.choice.device) ?? null);
  const pairs = $derived(Math.max(1, Math.floor((selected?.channels ?? 2) / 2)));

  onMount(() => {
    if (output.devices.length === 0) refreshOutputs();
  });

  function pickDevice(id: string) {
    const dev = output.devices.find((d) => d.id === id) ?? null;
    const maxFirst = dev ? Math.max(0, dev.channels - 2) : 0;
    setOutput({ device: dev ? dev.id : null, first_channel: Math.min(output.choice.first_channel, maxFirst) & ~1 });
  }
</script>

<div class="picker">
  <label>
    <span>{t("audio.output")}</span>
    <select value={output.choice.device ?? ""} onchange={(e) => pickDevice(e.currentTarget.value)} disabled={output.loading}>
      <option value="">{t("audio.output.default")}</option>
      {#each hosts as h (h)}
        <optgroup label={hostLabel(h)}>
          {#each output.devices.filter((d) => d.host === h) as d (d.id)}
            <option value={d.id}>{d.name}{d.default ? ` (${t("audio.output.system")})` : ""}</option>
          {/each}
        </optgroup>
      {/each}
    </select>
  </label>
  {#if pairs > 1}
    <label>
      <span>{t("audio.output.pair")}</span>
      <select
        value={output.choice.first_channel}
        onchange={(e) => setOutput({ device: output.choice.device, first_channel: Number(e.currentTarget.value) })}
      >
        {#each Array.from({ length: pairs }, (_, i) => i * 2) as c (c)}
          <option value={c}>{c + 1}-{c + 2}</option>
        {/each}
      </select>
    </label>
  {/if}
  <button class="ghost" onclick={refreshOutputs} title={t("audio.output.refresh")} aria-label={t("audio.output.refresh")}>
    <svg viewBox="0 0 16 16"><path d="M13 8a5 5 0 1 1-1.5-3.6M13 2.5V5h-2.5" fill="none" stroke="currentColor" stroke-width="1.5" /></svg>
  </button>
</div>

<style>
  .picker {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    font-size: var(--vf-text-sm);
  }
  label {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    color: var(--vf-text-muted);
  }
  select {
    max-width: 260px;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px var(--vf-space-1);
    font-size: var(--vf-text-sm);
  }
  .ghost {
    background: none;
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    color: var(--vf-text-muted);
    width: 24px;
    height: 24px;
    padding: 3px;
    cursor: pointer;
  }
  .ghost svg {
    width: 100%;
    height: 100%;
  }
</style>
