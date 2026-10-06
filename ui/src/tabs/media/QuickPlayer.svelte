<!-- Lecteur rapide (pop-up) : Échap pour fermer. Indépendant de l'onglet PLAYER. -->
<script lang="ts">
  import VideoView from "../player/VideoView.svelte";
  import AudioView from "../player/AudioView.svelte";
  import type { MediaEntry } from "../../lib/media";

  let { media, onClose }: { media: MediaEntry; onClose: () => void } = $props();
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="backdrop" role="presentation" onclick={(e) => e.target === e.currentTarget && onClose()}>
  <div class="dialog" role="dialog" aria-modal="true" aria-label={media.name}>
    <header>
      <span class="mono">{media.name}</span>
      <button onclick={onClose} aria-label="Fermer">✕</button>
    </header>
    <div class="body">
      {#if media.kind === "audio"}
        <AudioView slot="preview" paths={[media.path]} active={() => true} />
      {:else}
        <VideoView slot="preview" path={media.path} active={() => true} />
      {/if}
    </div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.6);
    display: grid;
    place-items: center;
    z-index: 10;
  }
  .dialog {
    width: min(1100px, 92vw);
    height: min(760px, 86vh);
    display: grid;
    grid-template-rows: auto 1fr;
    background: var(--vf-bg);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-lg);
    overflow: hidden;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--vf-space-2) var(--vf-space-3);
    background: var(--vf-surface);
    border-bottom: 1px solid var(--vf-border);
  }
  button {
    background: none;
    border: 0;
    color: var(--vf-text);
    cursor: pointer;
  }
  .body {
    min-height: 0;
    overflow: auto;
  }
</style>
