<!-- Vignette chargée à l'affichage ; au survol, la bande d'images suit la souris. -->
<script lang="ts">
  import { thumbnailUrl, filmstripUrl } from "../../lib/media";

  let { path, width = 96 }: { path: string; width?: number } = $props();
  let src = $state<string | null>(null);
  let hoverSrc = $state<string | null>(null);
  let failed = $state(false);
  let el = $state<HTMLDivElement | null>(null);

  $effect(() => {
    if (!el) return;
    const p = path;
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) {
        io.disconnect();
        thumbnailUrl(p).then((u) => (src = u)).catch(() => (failed = true));
      }
    });
    io.observe(el);
    return () => io.disconnect();
  });

  function onMove(e: MouseEvent) {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const i = Math.min(7, Math.max(0, Math.floor(((e.clientX - r.left) / r.width) * 8)));
    filmstripUrl(path, i).then((u) => (hoverSrc = u)).catch(() => {});
  }
</script>

<div
  class="thumb"
  bind:this={el}
  style:width={`${width}px`}
  role="img"
  onmousemove={onMove}
  onmouseleave={() => (hoverSrc = null)}
>
  {#if hoverSrc || src}
    <img src={hoverSrc ?? src} alt="" />
  {:else if failed}
    <span>?</span>
  {/if}
</div>

<style>
  .thumb {
    position: relative;
    aspect-ratio: 16 / 9;
    background: var(--vf-video-bg);
    border-radius: var(--vf-radius-sm);
    overflow: hidden;
    display: grid;
    place-items: center;
    color: var(--vf-text-disabled);
  }
  /* En position absolue : l'image ne peut pas étirer la case (défaut de
     aspect-ratio dans Safari 15, macOS Catalina). */
  img {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
  }
</style>
