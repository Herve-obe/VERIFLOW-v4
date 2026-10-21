<!-- Lecteur rapide de MEDIA, dans sa propre fenêtre du système : déplaçable
     sur un autre écran, redimensionnable. Échap ferme la fenêtre. -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import VideoView from "./tabs/player/VideoView.svelte";
  import AudioView from "./tabs/player/AudioView.svelte";
  import { t } from "./i18n/index.svelte";
  import { isTyping } from "./shortcuts";
  import { setMode } from "./stores/app.svelte";
  import { onQuickLoad, pendingQuick, saveGeometry, sendToPlayer, type QuickMedia } from "./lib/quick";

  const win = getCurrentWindow();
  let media = $state<QuickMedia | null>(null);
  const unlisten: Promise<() => void>[] = [];
  // Couleurs du mode du média (VIDEO ou AUDIO), comme dans le PLAYER.
  $effect(() => {
    if (media) setMode(media.kind);
  });

  /** Mémorise la place de la fenêtre (rouverte au même endroit, même écran). */
  async function remember() {
    try {
      const scale = await win.scaleFactor();
      const pos = (await win.outerPosition()).toLogical(scale);
      const size = (await win.innerSize()).toLogical(scale);
      if (!(await win.isMinimized()) && !(await win.isMaximized())) {
        saveGeometry({ x: pos.x, y: pos.y, width: size.width, height: size.height });
      }
    } catch {
      /* fenêtre en cours de fermeture */
    }
  }

  let timer = 0;
  const later = () => {
    window.clearTimeout(timer);
    timer = window.setTimeout(remember, 300);
  };

  onMount(async () => {
    unlisten.push(onQuickLoad((m) => (media = m)));
    unlisten.push(win.onMoved(later), win.onResized(later));
    media = await pendingQuick();
  });

  onDestroy(() => {
    for (const u of unlisten) u.then((f) => f());
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" && !isTyping(e.target)) {
      e.preventDefault();
      win.close();
    }
  }

  async function toPlayer() {
    if (!media) return;
    await sendToPlayer(media);
    win.close();
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="quick">
  <header>
    <span class="name" title={media?.path}>{media?.name ?? ""}</span>
    <button class="toplayer" onclick={toPlayer} disabled={!media}>{t("media.open.player")}</button>
  </header>
  <div class="content">
    {#if media}
      {#key media.path}
        {#if media.kind === "audio"}
          <AudioView slot="preview" paths={[media.path]} active={() => true} logs={false} />
        {:else}
          <VideoView slot="preview" path={media.path} active={() => true} logs={false} />
        {/if}
      {/key}
    {/if}
  </div>
</div>

<style>
  .quick {
    display: flex;
    flex-direction: column;
    height: 100vh;
    background: var(--vf-bg);
    color: var(--vf-text);
  }
  header {
    display: flex;
    align-items: center;
    gap: var(--vf-space-3);
    padding: var(--vf-space-2) var(--vf-space-3);
    border-bottom: 1px solid var(--vf-border);
  }
  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 600;
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
  .content {
    flex: 1;
    min-height: 0;
    padding: var(--vf-space-2);
  }
</style>
