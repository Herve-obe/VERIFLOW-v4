<!-- Fenêtre de sortie moniteur : l'image du lecteur, plein écran, sans
     interface. Échap ferme la fenêtre. -->
<script lang="ts">
  import { onDestroy } from "svelte";
  import { videoFrame } from "./lib/player";
  import { closeMonitor, onFrame, sendReady, type MonitorFrame } from "./lib/monitor";

  let canvas = $state<HTMLCanvasElement | null>(null);
  let busy = false;
  let next: MonitorFrame | null = null;

  async function show(f: MonitorFrame) {
    if (busy) {
      next = f;
      return;
    }
    busy = true;
    try {
      const ctx = canvas?.getContext("2d");
      if (f.index === null) {
        ctx?.clearRect(0, 0, canvas?.width ?? 0, canvas?.height ?? 0);
      } else {
        const buf = await videoFrame(f.index, f.slot);
        const bitmap = await createImageBitmap(new Blob([buf], { type: "image/jpeg" }));
        if (canvas && (canvas.width !== bitmap.width || canvas.height !== bitmap.height)) {
          canvas.width = bitmap.width;
          canvas.height = bitmap.height;
        }
        canvas?.getContext("2d")?.drawImage(bitmap, 0, 0);
        bitmap.close();
      }
    } catch {
      /* image plus disponible : la suivante arrive */
    } finally {
      busy = false;
    }
    if (next) {
      const n = next;
      next = null;
      await show(n);
    }
  }

  // Curseur visible quand la souris bouge, masqué après 2 s d'immobilité :
  // l'image reste propre sans que la souris se « perde » sur cet écran.
  const IDLE_MS = 2000;
  let idle = $state(false);
  let idleTimer: number | undefined;
  function wake() {
    idle = false;
    window.clearTimeout(idleTimer);
    idleTimer = window.setTimeout(() => (idle = true), IDLE_MS);
  }
  wake();
  onDestroy(() => window.clearTimeout(idleTimer));

  const stop = onFrame(show);
  // Prête à recevoir : le lecteur envoie alors l'image en cours.
  stop.then(() => sendReady());
  onDestroy(() => stop.then((u) => u()));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && closeMonitor()} />

<div class="monitor" class:idle onpointermove={wake} role="presentation">
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  .monitor {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--vf-video-bg);
  }
  /* Curseur masqué sur cette fenêtre seulement, après 2 s d'immobilité (et
     pas via body : la feuille de style pourrait se retrouver dans la
     fenêtre principale). */
  .monitor.idle {
    cursor: none;
  }
  canvas {
    max-width: 100%;
    max-height: 100%;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
