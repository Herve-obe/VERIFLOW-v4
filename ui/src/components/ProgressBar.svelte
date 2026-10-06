<!-- Barre de progression (0 à 1). -->
<script lang="ts">
  let { value = 0, state = "running" }: { value?: number; state?: "running" | "ok" | "error" } = $props();
</script>

<div class="bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" aria-valuenow={Math.round(value * 100)}>
  <div class="fill {state}" style:width={`${Math.min(1, Math.max(0, value)) * 100}%`}></div>
</div>

<style>
  .bar {
    height: 8px;
    background: var(--vf-meter-bg);
    border-radius: 4px;
    overflow: hidden;
  }
  .fill {
    height: 100%;
    background: var(--vf-accent);
    transition: width var(--vf-transition);
  }
  .fill.ok {
    background: var(--vf-success);
  }
  .fill.error {
    background: var(--vf-error);
  }
</style>
