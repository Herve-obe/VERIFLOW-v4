<!-- Bande de clips horizontale sous l'image. Lot 5.
     Lit : player.clips, player.current ; charge via select(i). Hauteur imposée par PlayerTab (100 % du parent).
     Carte VIDEO = Thumb hover={false}, carte AUDIO = nom + Wave ; clés player.strip.prev / .next.
     Virtualisée : seules les cartes visibles (plus une marge) sont montées, donc aucune vignette hors écran.
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { player, select, next, prev } from "../../stores/player.svelte";
  import { t } from "../../i18n/index.svelte";
  import Thumb from "../media/Thumb.svelte";
  import Wave from "../media/Wave.svelte";

  let { mode }: { mode: Mode } = $props();

  const CARD = $derived(mode === "audio" ? 168 : 112); // largeur d'une carte, marge comprise
  const OVERSCAN = 2;

  let viewport = $state<HTMLDivElement | null>(null);
  let scrollLeft = $state(0);
  let width = $state(0);

  function measure() {
    if (!viewport) return;
    width = viewport.clientWidth;
    scrollLeft = viewport.scrollLeft;
  }

  $effect(() => {
    if (!viewport) return;
    measure();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
    const ro = new ResizeObserver(measure);
    ro.observe(viewport);
    return () => ro.disconnect();
  });

  const first = $derived(Math.max(0, Math.floor(scrollLeft / CARD) - OVERSCAN));
  const last = $derived(Math.min(player.clips.length, Math.ceil((scrollLeft + width) / CARD) + OVERSCAN));
  const visible = $derived(player.clips.slice(first, last).map((clip, k) => ({ clip, i: first + k })));

  // Le clip courant reste visible.
  $effect(() => {
    const i = player.current;
    const v = viewport;
    if (!v || i < 0) return;
    const left = i * CARD;
    if (left < v.scrollLeft) v.scrollLeft = left;
    else if (left + CARD > v.scrollLeft + v.clientWidth) v.scrollLeft = left + CARD - v.clientWidth;
    measure();
  });
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.strip")}>
  <button type="button" class="nav" onclick={(e) => { prev(); e.currentTarget.blur(); }} disabled={player.current <= 0} aria-label={t("player.strip.prev")} title={t("player.strip.prev")}>&lsaquo;</button>
  <div class="viewport" bind:this={viewport} onscroll={measure}>
    <div class="track" style:width={`${player.clips.length * CARD}px`}>
      {#each visible as { clip, i } (clip.path)}
        <button
          type="button"
          class="card"
          class:on={i === player.current}
          style:left={`${i * CARD}px`}
          style:width={`${CARD}px`}
          title={clip.name}
          aria-current={i === player.current ? "true" : undefined}
          onclick={(e) => { select(i); e.currentTarget.blur(); }}
        >
          {#if mode === "audio"}
            <span class="name">{clip.name}</span>
            <Wave path={clip.path} width={CARD - 16} height={32} />
          {:else}
            <Thumb path={clip.path} width={CARD - 16} hover={false} />
          {/if}
        </button>
      {/each}
    </div>
  </div>
  <button type="button" class="nav" onclick={(e) => { next(); e.currentTarget.blur(); }} disabled={player.current >= player.clips.length - 1} aria-label={t("player.strip.next")} title={t("player.strip.next")}>&rsaquo;</button>
</div>

<style>
  .panel {
    height: 100%;
    min-height: 0;
    display: flex;
    align-items: stretch;
    background: var(--vf-surface);
    --strip-accent: var(--vf-accent-video);
  }
  .panel[data-mode="audio"] { --strip-accent: var(--vf-accent-audio); }
  .nav {
    flex: none;
    width: 24px;
    border: 0;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    cursor: pointer;
    font-size: var(--vf-text-lg);
  }
  .nav:hover:not(:disabled) { background: var(--vf-surface-hover); }
  .nav:disabled { color: var(--vf-text-disabled); cursor: default; }
  .viewport {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
  }
  .track { position: relative; height: 100%; }
  .card {
    position: absolute;
    top: 0;
    bottom: 0;
    box-sizing: border-box;
    padding: 4px 8px 2px;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 2px;
    border: 0;
    border-bottom: 3px solid transparent;
    background: transparent;
    color: var(--vf-text-muted);
    cursor: pointer;
    text-align: left;
    font-size: var(--vf-text-xs);
  }
  .card:hover { background: var(--vf-surface-hover); }
  .card.on {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border-bottom-color: var(--strip-accent);
  }
  .name {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
