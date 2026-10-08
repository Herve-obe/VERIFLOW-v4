<!-- Lecteur rapide (fenêtre déplaçable) : Échap pour fermer. Indépendant de
     l'onglet PLAYER, où le média peut être ouvert d'un clic. -->
<script lang="ts">
  import Modal from "../../components/Modal.svelte";
  import VideoView from "../player/VideoView.svelte";
  import AudioView from "../player/AudioView.svelte";
  import { openInPlayer } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import type { MediaEntry } from "../../lib/media";

  let { media, onClose }: { media: MediaEntry; onClose: () => void } = $props();

  function toPlayer() {
    openInPlayer(media.path, media.kind);
    onClose();
  }
</script>

<Modal title={media.name} {onClose} width="min(1200px, 94vw)" height="min(820px, 90vh)">
  {#snippet actions()}
    <button class="toplayer" onclick={toPlayer}>{t("media.open.player")}</button>
  {/snippet}
  <div class="content">
    {#if media.kind === "audio"}
      <AudioView slot="preview" paths={[media.path]} active={() => true} logs={false} />
    {:else}
      <VideoView slot="preview" path={media.path} active={() => true} logs={false} />
    {/if}
  </div>
</Modal>

<style>
  .content {
    height: 100%;
  }
  .toplayer {
    background: var(--vf-accent);
    color: var(--vf-on-accent);
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: 3px var(--vf-space-3);
    font-size: var(--vf-text-sm);
    font-weight: 600;
    cursor: pointer;
  }
</style>
