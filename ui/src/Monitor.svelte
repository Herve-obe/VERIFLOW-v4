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

  const stop = onFrame(show);
  // Prête à recevoir : le lecteur envoie alors l'image en cours.
  stop.then(() => sendReady());
  onDestroy(() => stop.then((u) => u()));
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && closeMonitor()} />

<div class="monitor">
  <canvas bind:this={canvas}></canvas>
</div>

<style>
  /* Curseur caché sur cette fenêtre seulement (et pas via body : la
     feuille de style pourrait se retrouver dans la fenêtre principale). */
  .monitor {
    position: fixed;
    inset: 0;
    cursor: none;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--vf-video-bg);
  }
  canvas {
    max-width: 100%;
    max-height: 100%;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
