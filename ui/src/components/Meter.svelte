<!-- Vumètre crête vertical en dBFS (échelle -60 à 0). -->
<script lang="ts">
  let { db = null, hold = null, min = -60 }: { db?: number | null; hold?: number | null; min?: number } = $props();

  const ratio = (v: number | null) => (v === null ? 0 : Math.min(1, Math.max(0, (v - min) / -min)));
</script>

<div class="meter" title={db === null ? "-inf dBFS" : `${db.toFixed(1)} dBFS`}>
  <div class="fill" style:height={`${ratio(db) * 100}%`}></div>
  {#if hold !== null && hold > min}
    <div class="hold" class:clip={hold >= -0.1} style:bottom={`${ratio(hold) * 100}%`}></div>
  {/if}
</div>

<style>
  .meter {
    position: relative;
    width: var(--vf-meter-width);
    height: var(--vf-meter-height);
    background: var(--vf-meter-bg);
    border-radius: 2px;
    overflow: hidden;
  }
  /* Le dégradé couvre toute la hauteur : -60 dBFS en bas, 0 dBFS en haut. */
  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    background-image: linear-gradient(
      to top,
      var(--vf-meter-low) 0%,
      var(--vf-meter-low) 70%,
      var(--vf-meter-mid) 70%,
      var(--vf-meter-mid) 90%,
      var(--vf-meter-high) 90%
    );
    background-size: 100% var(--vf-meter-height);
    background-position: bottom;
  }
  .hold {
    position: absolute;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--vf-text);
  }
  .hold.clip {
    background: var(--vf-meter-high);
  }
</style>
