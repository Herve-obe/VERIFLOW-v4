<!-- Onglet PLAYER : panneaux redimensionnables et masquables autour de la vue VIDEO ou AUDIO
     (charte §3.1 et §7.3). Gauche : explorateur au-dessus de la liste de clips. Centre : bandeau,
     lecteur, bande de clips. Droite : inspecteur. Les deux vues restent montées pour conserver
     leur état lors des bascules ; la disposition est mémorisée par mode (stores/playerLayout). -->
<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { app } from "../../stores/app.svelte";
  import { isBeginner } from "../../stores/level.svelte";
  import {
    CENTER_MIN,
    LAYOUT_LIMITS as L,
    DEFAULT_SIZES as D,
    playerLayout,
    saveLayout,
  } from "../../stores/playerLayout.svelte";
  import { t } from "../../i18n/index.svelte";
  import Explorer from "../../components/explorer/Explorer.svelte";
  import Resizer from "../../components/Resizer.svelte";
  import VideoView from "./VideoView.svelte";
  import AudioView from "./AudioView.svelte";
  import PlayerToolbar from "./PlayerToolbar.svelte";
  import ClipList from "./ClipList.svelte";
  import ClipStrip from "./ClipStrip.svelte";
  import ClipBanner from "./ClipBanner.svelte";
  import MetaInspector from "./MetaInspector.svelte";
  import { loadFolder, next, playerSources, prev } from "../../stores/player.svelte";

  const mode = $derived(app.mode);
  const lay = $derived(playerLayout[mode]);
  const vis = $derived(lay.visible);

  let body: HTMLDivElement | undefined = $state();
  let width = $state(1400);

  onMount(() => {
    if (!body) return;
    const measure = () => (width = body?.clientWidth ?? width);
    measure();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
    const ro = new ResizeObserver(measure);
    ro.observe(body);
    return () => ro.disconnect();
  });

  // Mémorise les deux dispositions, avec un délai : pas d'écriture à chaque mouvement de poignée.
  const saveBoth = () => {
    saveLayout("video");
    saveLayout("audio");
  };
  $effect(() => {
    void JSON.stringify($state.snapshot(playerLayout));
    const id = window.setTimeout(saveBoth, 300);
    return () => window.clearTimeout(id);
  });
  onDestroy(saveBoth);
  // Fermeture de la fenêtre : dernière écriture sans attendre le délai.
  onMount(() => {
    window.addEventListener("pagehide", saveBoth);
    window.addEventListener("beforeunload", saveBoth);
    return () => {
      window.removeEventListener("pagehide", saveBoth);
      window.removeEventListener("beforeunload", saveBoth);
    };
  });

  // Le bandeau ou l'inspecteur affichent déjà codec, cadence, TC et iXML : les vues les masquent alors.
  const infoPanels = $derived(vis.banner || vis.inspector);

  // Flèches haut/bas : clip précédent/suivant, sauf pendant une saisie, sur un curseur ou une poignée.
  function onArrows(e: KeyboardEvent) {
    if (app.tab !== "player" || e.defaultPrevented) return;
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    if (e.ctrlKey || e.metaKey || e.altKey || e.shiftKey) return;
    const el = e.target instanceof HTMLElement ? e.target : null;
    if (el?.closest('input, textarea, select, [contenteditable="true"], [role="slider"], [role="separator"], [role="tree"], [role="menu"], [role="dialog"], [aria-modal="true"]')) return;
    e.preventDefault();
    if (e.key === "ArrowUp") prev();
    else next();
  }

  // Colonne des marqueurs (300 px) en Expert : elle est dans la zone centrale.
  const centerMin = $derived(CENTER_MIN + (isBeginner() ? 0 : 300));

  const showLeft = $derived(vis.explorer || vis.clips);
  const HANDLE = 5;
  const leftFloor = L.left.min;
  const rightFloor = 120;
  // Le centre garde au moins centerMin : l'inspecteur se réduit d'abord, puis la colonne de gauche.
  const rightW = $derived(
    vis.inspector
      ? Math.max(rightFloor, Math.min(lay.sizes.right, width - (showLeft ? lay.sizes.left + HANDLE : 0) - centerMin - HANDLE))
      : 0,
  );
  const leftW = $derived(
    showLeft
      ? Math.max(leftFloor, Math.min(lay.sizes.left, width - (vis.inspector ? rightW + HANDLE : 0) - centerMin - HANDLE))
      : 0,
  );
  const leftMax = $derived(Math.min(L.left.max, Math.max(leftFloor, width - (vis.inspector ? rightW + HANDLE : 0) - centerMin - HANDLE)));
  const rightMax = $derived(Math.min(L.right.max, Math.max(rightFloor, width - (showLeft ? leftW + HANDLE : 0) - centerMin - HANDLE)));
</script>

<svelte:window onkeydown={onArrows} />

<div class="root">
  <PlayerToolbar {mode} />
  <div class="body" bind:this={body}>
    {#if showLeft}
      <aside class="left" style:width="{leftW}px" aria-label={t("player.panel.explorer")}>
        {#if vis.explorer}
          <div class="explorer" style:height={vis.clips ? `${lay.sizes.leftSplit}px` : undefined} style:flex={vis.clips ? "none" : "1"}>
            <Explorer
              owner="player"
              docked
              selected={playerSources[mode].dir || null}
              onSelect={(p) => void loadFolder(mode, p)}
            />
          </div>
        {/if}
        {#if vis.explorer && vis.clips}
          <Resizer
            orientation="horizontal"
            bind:size={lay.sizes.leftSplit}
            min={L.leftSplit.min}
            max={L.leftSplit.max}
            def={D.leftSplit}
          />
        {/if}
        {#if vis.clips}
          <div class="clips"><ClipList {mode} /></div>
        {/if}
      </aside>
      <Resizer bind:size={lay.sizes.left} min={L.left.min} max={leftMax} def={D.left} />
    {/if}

    <main class="center">
      {#if vis.banner}
        <div class="banner"><ClipBanner {mode} /></div>
      {/if}
      <div class="views">
        <div class="view" class:hidden={mode !== "video"}><VideoView path={app.playerVideo} logs={!isBeginner()} details={!infoPanels} tracksBelow /></div>
        <div class="view" class:hidden={mode !== "audio"}><AudioView paths={app.playerAudio} logs={!isBeginner()} details={!infoPanels} /></div>
      </div>
      {#if vis.strip}
        <Resizer
          orientation="horizontal"
          grow="before"
          bind:size={lay.sizes.strip}
          min={L.strip.min}
          max={L.strip.max}
          def={D.strip}
        />
        <div class="strip" style:height="{lay.sizes.strip}px"><ClipStrip {mode} /></div>
      {/if}
    </main>

    {#if vis.inspector}
      <Resizer grow="before" bind:size={lay.sizes.right} min={L.right.min} max={rightMax} def={D.right} />
      <aside class="right" style:width="{rightW}px" aria-label={t("player.panel.inspector")}>
        <MetaInspector {mode} />
      </aside>
    {/if}
  </div>
</div>

<style>
  .root {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .body {
    flex: 1;
    display: flex;
    min-height: 0;
    min-width: 0;
  }
  .left,
  .right {
    flex: none;
    display: flex;
    flex-direction: column;
    min-width: var(--vf-player-panel-min);
    min-height: 0;
    overflow: hidden;
    background: var(--vf-surface);
  }
  .explorer {
    min-height: var(--vf-player-panel-min);
    overflow: hidden;
    display: flex;
  }
  .clips {
    flex: 1;
    min-height: var(--vf-player-panel-min);
    overflow: hidden;
  }
  .center {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }
  .banner {
    flex: none;
    color: var(--vf-player-banner-text);
  }
  .views {
    flex: 1;
    min-height: 0;
  }
  .view {
    height: 100%;
  }
  .hidden {
    display: none;
  }
  .strip {
    flex: none;
    min-height: var(--vf-player-strip-height);
    overflow: hidden;
  }
  .right {
    overflow-y: auto;
  }
</style>
