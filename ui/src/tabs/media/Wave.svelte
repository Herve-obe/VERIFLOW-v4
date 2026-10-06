<!-- Forme d'onde miniature (crêtes), dessinée sur un canvas. -->
<script lang="ts">
  import { waveform } from "../../lib/media";

  let { path, width = 160, height = 36 }: { path: string; width?: number; height?: number } = $props();
  let canvas = $state<HTMLCanvasElement | null>(null);

  $effect(() => {
    const c = canvas;
    if (!c) return;
    const p = path;
    const io = new IntersectionObserver(async (entries) => {
      if (!entries.some((e) => e.isIntersecting)) return;
      io.disconnect();
      try {
        const w = await waveform(p);
        const ctx = c.getContext("2d");
        if (!ctx) return;
        const color = getComputedStyle(c).getPropertyValue("--vf-accent-audio").trim() || "#ffb020";
        ctx.clearRect(0, 0, c.width, c.height);
        ctx.fillStyle = color;
        const mid = c.height / 2;
        const step = c.width / w.peaks.length;
        w.peaks.forEach((v, i) => {
          const h = Math.max(1, v * c.height);
          ctx.fillRect(i * step, mid - h / 2, Math.max(1, step), h);
        });
      } catch {
        /* fichier illisible : pas de forme d'onde */
      }
    });
    io.observe(c);
    return () => io.disconnect();
  });
</script>

<canvas bind:this={canvas} {width} {height}></canvas>

<style>
  canvas {
    display: block;
    background: var(--vf-meter-bg);
    border-radius: var(--vf-radius-sm);
  }
</style>
