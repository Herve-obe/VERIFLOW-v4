<!-- Scopes calculés sur l'image affichée (aperçu JPEG 4:4:4, LUT comprise) :
     forme d'onde luma, parade RGB, vectorscope, histogramme. Luma et
     chrominance selon la Rec. 709. -->
<script lang="ts">
  import { t } from "../../i18n/index.svelte";

  type Kind = "waveform" | "parade" | "vectorscope" | "histogram";

  let { source, tick }: { source: HTMLCanvasElement | null | undefined; tick: number } = $props();

  const KEY = "veriflow.player.scope";
  const read = (): Kind => {
    try {
      const v = localStorage.getItem(KEY);
      if (v === "waveform" || v === "parade" || v === "vectorscope" || v === "histogram") return v;
    } catch {
      /* stockage indisponible */
    }
    return "waveform";
  };
  let kind = $state<Kind>(read());
  $effect(() => {
    try {
      localStorage.setItem(KEY, kind);
    } catch {
      /* stockage indisponible */
    }
  });

  let canvas = $state<HTMLCanvasElement | null>(null);
  // Image réduite pour l'analyse : assez fine pour un scope, assez légère pour la lecture.
  const SW = 256;
  const SH = 144;
  let sample: HTMLCanvasElement | null = null;

  const luma = (r: number, g: number, b: number) => 0.2126 * r + 0.7152 * g + 0.0722 * b;

  function pixels(): Uint8ClampedArray | null {
    if (!source || !source.width) return null;
    sample ??= document.createElement("canvas");
    sample.width = SW;
    sample.height = SH;
    const ctx = sample.getContext("2d", { willReadFrequently: true });
    if (!ctx) return null;
    ctx.drawImage(source, 0, 0, SW, SH);
    return ctx.getImageData(0, 0, SW, SH).data;
  }

  $effect(() => {
    void tick;
    void kind;
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext("2d");
    if (!ctx) return;
    const css = getComputedStyle(document.documentElement);
    const color = (n: string) => css.getPropertyValue(n).trim();
    const W = (c.width = Math.round(c.clientWidth * devicePixelRatio));
    const H = (c.height = Math.round(c.clientHeight * devicePixelRatio));
    ctx.fillStyle = color("--vf-scope-bg");
    ctx.fillRect(0, 0, W, H);
    const data = pixels();
    if (data) {
      if (kind === "waveform") waveform(ctx, data, W, H, [[luma, color("--vf-scope-trace")]]);
      else if (kind === "parade")
        waveform(ctx, data, W, H, [
          [(r) => r, color("--vf-scope-red")],
          [(_r, g) => g, color("--vf-scope-green")],
          [(_r, _g, b) => b, color("--vf-scope-blue")],
        ]);
      else if (kind === "vectorscope") vectorscope(ctx, data, W, H, color);
      else histogram(ctx, data, W, H, color);
    }
    if (kind === "waveform" || kind === "parade") levels(ctx, W, H, color);
  });

  type Channel = [(r: number, g: number, b: number) => number, string];

  /** Forme d'onde : colonne de l'image en abscisse, niveau en ordonnée. */
  function waveform(ctx: CanvasRenderingContext2D, data: Uint8ClampedArray, W: number, H: number, channels: Channel[]) {
    const part = W / channels.length;
    const bins = new Uint32Array(SW * 256);
    channels.forEach(([value, fill], k) => {
      bins.fill(0);
      for (let y = 0; y < SH; y++)
        for (let x = 0; x < SW; x++) {
          const i = (y * SW + x) * 4;
          bins[x * 256 + Math.round(value(data[i], data[i + 1], data[i + 2]))]++;
        }
      ctx.fillStyle = fill;
      const cw = part / SW;
      const ch = (H - 2) / 256;
      for (let x = 0; x < SW; x++)
        for (let v = 0; v < 256; v++) {
          const n = bins[x * 256 + v];
          if (!n) continue;
          ctx.globalAlpha = Math.min(1, 0.15 + n / 12);
          ctx.fillRect(k * part + x * cw, H - 1 - (v + 1) * ch, Math.max(1, cw), Math.max(1, ch));
        }
      ctx.globalAlpha = 1;
    });
  }

  /** Graduation 0 à 100 % (pas de 25 %). */
  function levels(ctx: CanvasRenderingContext2D, W: number, H: number, color: (n: string) => string) {
    ctx.strokeStyle = color("--vf-scope-grid");
    ctx.fillStyle = color("--vf-scope-label");
    ctx.font = `${10 * devicePixelRatio}px sans-serif`;
    for (const pct of [0, 25, 50, 75, 100]) {
      const y = Math.round(H - 1 - (pct / 100) * (H - 2)) + 0.5;
      ctx.beginPath();
      ctx.moveTo(0, y);
      ctx.lineTo(W, y);
      ctx.stroke();
      ctx.fillText(String(pct), 2, Math.max(10 * devicePixelRatio, y - 2));
    }
  }

  /** Cb et Cr (Rec. 709), de -0,5 à 0,5. */
  function chroma(r: number, g: number, b: number): [number, number] {
    const y = luma(r, g, b) / 255;
    return [(b / 255 - y) / 1.8556, (r / 255 - y) / 1.5748];
  }

  function vectorscope(
    ctx: CanvasRenderingContext2D,
    data: Uint8ClampedArray,
    W: number,
    H: number,
    color: (n: string) => string,
  ) {
    const size = Math.min(W, H) - 8;
    const cx = W / 2;
    const cy = H / 2;
    const scale = size; // 0,5 de chrominance = rayon size / 2
    const at = (cb: number, cr: number): [number, number] => [cx + cb * scale, cy - cr * scale];
    // Graticule : cercle extérieur, axes, ligne des tons chair, cibles des barres à 75 %.
    ctx.strokeStyle = color("--vf-scope-grid");
    ctx.beginPath();
    ctx.arc(cx, cy, size / 2, 0, Math.PI * 2);
    ctx.moveTo(cx - size / 2, cy);
    ctx.lineTo(cx + size / 2, cy);
    ctx.moveTo(cx, cy - size / 2);
    ctx.lineTo(cx, cy + size / 2);
    ctx.stroke();
    ctx.strokeStyle = color("--vf-scope-skin");
    ctx.beginPath();
    ctx.moveTo(cx, cy);
    // Ligne des tons chair (ligne I) : 123° depuis l'axe Cb.
    const skin = (123 * Math.PI) / 180;
    ctx.lineTo(cx + Math.cos(skin) * (size / 2), cy - Math.sin(skin) * (size / 2));
    ctx.stroke();
    ctx.fillStyle = color("--vf-scope-label");
    ctx.font = `${10 * devicePixelRatio}px sans-serif`;
    const bars: [string, number, number, number][] = [
      ["R", 191, 0, 0],
      ["Mg", 191, 0, 191],
      ["B", 0, 0, 191],
      ["Cy", 0, 191, 191],
      ["G", 0, 191, 0],
      ["Yl", 191, 191, 0],
    ];
    ctx.strokeStyle = color("--vf-scope-grid");
    for (const [name, r, g, b] of bars) {
      const [x, y] = at(...chroma(r, g, b));
      const box = 6 * devicePixelRatio;
      ctx.strokeRect(x - box / 2, y - box / 2, box, box);
      ctx.fillText(name, x + box, y - box / 2);
    }
    // Points de l'image.
    ctx.fillStyle = color("--vf-scope-trace");
    ctx.globalAlpha = 0.25;
    for (let i = 0; i < data.length; i += 4) {
      const [x, y] = at(...chroma(data[i], data[i + 1], data[i + 2]));
      ctx.fillRect(x, y, devicePixelRatio, devicePixelRatio);
    }
    ctx.globalAlpha = 1;
  }

  function histogram(
    ctx: CanvasRenderingContext2D,
    data: Uint8ClampedArray,
    W: number,
    H: number,
    color: (n: string) => string,
  ) {
    const series: [number[], string][] = [
      [new Array(256).fill(0), color("--vf-scope-red")],
      [new Array(256).fill(0), color("--vf-scope-green")],
      [new Array(256).fill(0), color("--vf-scope-blue")],
      [new Array(256).fill(0), color("--vf-scope-trace")],
    ];
    for (let i = 0; i < data.length; i += 4) {
      series[0][0][data[i]]++;
      series[1][0][data[i + 1]]++;
      series[2][0][data[i + 2]]++;
      series[3][0][Math.round(luma(data[i], data[i + 1], data[i + 2]))]++;
    }
    const peak = Math.max(1, ...series.flatMap(([h]) => h));
    ctx.lineWidth = devicePixelRatio;
    for (const [h, stroke] of series) {
      ctx.strokeStyle = stroke;
      ctx.beginPath();
      h.forEach((n, v) => {
        const x = (v / 255) * W;
        const y = H - 1 - Math.sqrt(n / peak) * (H - 4);
        if (v === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      });
      ctx.stroke();
    }
    ctx.lineWidth = 1;
  }
</script>

<div class="scopes">
  <select bind:value={kind} onchange={(e) => e.currentTarget.blur()} aria-label={t("player.scope")}>
    <option value="waveform">{t("player.scope.waveform")}</option>
    <option value="parade">{t("player.scope.parade")}</option>
    <option value="vectorscope">{t("player.scope.vectorscope")}</option>
    <option value="histogram">{t("player.scope.histogram")}</option>
  </select>
  <canvas bind:this={canvas} class:square={kind === "vectorscope"}></canvas>
  <p class="hint">{t("player.scope.hint")}</p>
</div>

<style>
  .scopes {
    width: var(--vf-scope-width);
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--vf-space-1);
    min-height: 0;
  }
  select {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    padding: 2px 6px;
    font-size: var(--vf-text-xs);
  }
  canvas {
    width: 100%;
    height: 180px;
    border-radius: var(--vf-radius-sm);
    border: 1px solid var(--vf-border);
  }
  canvas.square {
    height: var(--vf-scope-width);
  }
  .hint {
    margin: 0;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-xs);
  }
</style>
