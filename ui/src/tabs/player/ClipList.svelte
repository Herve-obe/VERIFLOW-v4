<!-- Liste des clips (dossier ou playlist), sous l'explorateur. Lot 3.
     Lit/écrit : player.clips, player.current, player.source, player.details (stores/player.svelte.ts) ;
     charge via select(i) ; ne touche pas la disposition (la colonne gauche lui donne sa taille).
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu).
     Liste virtualisée : lignes de hauteur fixe (--vf-player-row-height), seules les lignes visibles
     sont dans le DOM. Clic = charge le clip ; en AUDIO, Ctrl+clic en marque plusieurs puis Entrée
     les charge ensemble (pistes mono). -->
<script lang="ts">
  import { onMount, untrack } from "svelte";
  import { app, type Mode } from "../../stores/app.svelte";
  import {
    player,
    playerSources,
    setClips,
    select,
    loadFolder,
    checkPlaylist,
    describeNow,
    resetChecked,
    addToPlaylist,
    removeFromPlaylist,
    moveInPlaylist,
    clearPlaylist,
  } from "../../stores/player.svelte";
  import type { Described, MediaEntry } from "../../lib/media";
  import { explorer } from "../../stores/explorer.svelte";
  import { t } from "../../i18n/index.svelte";

  let { mode }: { mode: Mode } = $props();

  const src = $derived(playerSources[mode]);
  const loadedPaths = $derived<string[]>(
    mode === "video" ? (app.playerVideo ? [app.playerVideo] : []) : (app.playerAudio ?? []),
  );

  let filter = $state("");
  let sortBy = $state<"name" | "tc">("name");
  let picked = $state<string[]>([]);

  // --- Contenu affiché -------------------------------------------------------
  const base = $derived(src.source === "folder" ? src.entries : src.playlist);

  const pad = (n: number) => String(n).padStart(2, "0");
  const clock = (sec: number) => {
    const s = Math.floor(sec);
    return `${pad(Math.floor(s / 3600) % 24)}:${pad(Math.floor(s / 60) % 60)}:${pad(s % 60)}`;
  };
  const dur = (d: Described | undefined): number | null => {
    const p = d?.details.probe;
    if (p && p.duration > 0) return p.duration;
    const w = d?.details.wav;
    return w && w.sample_rate > 0 ? w.frames / w.sample_rate : null;
  };
  const tcOf = (d: Described | undefined): string => {
    const p = d?.details.probe?.start_timecode;
    if (p) return p;
    const w = d?.details.wav;
    if (w?.time_reference != null && w.sample_rate > 0) return clock(w.time_reference / w.sample_rate);
    return "";
  };
  const short = (sec: number | null) => {
    if (sec === null) return "";
    const s = Math.round(sec);
    return `${Math.floor(s / 60)}:${pad(s % 60)}`;
  };
  const sceneTake = (d: Described | undefined) => [d?.values.scene, d?.values.take].filter(Boolean).join("/");

  const shown = $derived.by<MediaEntry[]>(() => {
    const q = filter.trim().toLowerCase();
    let out = base;
    if (q) {
      out = out.filter((e) => {
        const v = player.details.get(e.path)?.values ?? {};
        return [e.name, v.scene, v.take, v.reel].some((s) => s?.toLowerCase().includes(q));
      });
    }
    if (src.source === "folder" && sortBy === "tc") {
      const key = (e: MediaEntry) => tcOf(player.details.get(e.path)) || "￿";
      out = [...out].sort((a, b) => key(a).localeCompare(key(b)) || a.rel.localeCompare(b.rel));
    }
    return out;
  });

  // La source active alimente le store partagé (bande, bandeau, flèches haut/bas).
  $effect(() => {
    const list = shown;
    const source = src.source;
    const loaded = loadedPaths[0];
    untrack(() => setClips(list, source, loaded ? list.findIndex((e) => e.path === loaded) : -1));
  });

  // Ouvert depuis MEDIA (ou ailleurs) : charge le dossier parent du clip et le surligne.
  // Seulement quand le chemin chargé du mode change, jamais sur une simple bascule de mode.
  const lastSeen: Record<Mode, string | undefined> = { video: undefined, audio: undefined };
  const parentOf = (p: string) => {
    const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    if (i < 0) return "";
    const head = p.slice(0, i);
    if (head === "") return p.slice(0, 1); // « /clip.mov » : la racine
    return /^[A-Za-z]:$/.test(head) ? `${head}\\` : head; // « C:\clip.mov » : C:\
  };
  $effect(() => {
    const p = loadedPaths[0];
    const m = mode;
    untrack(() => {
      if (p === lastSeen[m]) return;
      lastSeen[m] = p;
      if (!p) return;
      const s = playerSources[m];
      if (s.entries.some((e) => e.path === p)) return;
      if (s.source === "playlist" && s.playlist.some((e) => e.path === p)) return;
      const dir = parentOf(p);
      if (dir) void loadFolder(m, dir);
    });
  });

  // Ouverture de la playlist ou volume éjecté : les fichiers sont réexaminés.
  $effect(() => {
    void explorer.volumes.length;
    void (src.source === "playlist");
    untrack(() => {
      resetChecked(mode);
      if (src.source === "playlist") void checkPlaylist(mode);
    });
  });

  // Playlist : repère les fichiers disparus à l'ouverture et à chaque changement de contenu.
  $effect(() => {
    void src.playlist.length;
    untrack(() => void checkPlaylist(mode));
  });

  const missing = $derived(new Set(src.missing));
  const unreadable = $derived(new Set(src.unreadable));
  const loadedSet = $derived(new Set(loadedPaths));

  // --- Virtualisation --------------------------------------------------------
  let rowH = $state(24);
  let viewH = $state(200);
  let top = $state(0);
  let scroller: HTMLDivElement | undefined = $state();
  const OVER = 4;
  const first = $derived(Math.max(0, Math.floor(top / rowH) - OVER));
  const last = $derived(Math.min(shown.length, Math.ceil((top + viewH) / rowH) + OVER));
  const visibleRows = $derived(shown.slice(first, last));

  // Les lignes visibles sans détails sont analysées tout de suite, sans attendre les lots.
  // Attend ~150 ms après l'arrêt du défilement.
  $effect(() => {
    const paths = visibleRows.map((e) => e.path);
    const id = window.setTimeout(() => describeNow(paths), 150);
    return () => window.clearTimeout(id);
  });

  onMount(() => {
    if (!scroller) return;
    const h = parseFloat(getComputedStyle(scroller).getPropertyValue("--vf-player-row-height"));
    if (h > 0) rowH = h;
    const measure = () => (viewH = scroller?.clientHeight ?? viewH);
    measure();
    if (typeof ResizeObserver === "undefined") {
      window.addEventListener("resize", measure);
      return () => window.removeEventListener("resize", measure);
    }
    const ro = new ResizeObserver(measure);
    ro.observe(scroller);
    return () => ro.disconnect();
  });

  // Le clip chargé reste visible (flèches haut/bas, ouverture depuis MEDIA).
  $effect(() => {
    const i = player.current;
    untrack(() => {
      if (!scroller || i < 0) return;
      const y = i * rowH;
      if (y < scroller.scrollTop) scroller.scrollTop = y;
      else if (y + rowH > scroller.scrollTop + scroller.clientHeight) scroller.scrollTop = y + rowH - scroller.clientHeight;
    });
  });

  // --- Actions ---------------------------------------------------------------
  function click(e: MouseEvent, index: number) {
    const clip = shown[index];
    if (!clip || missing.has(clip.path) || unreadable.has(clip.path)) return;
    if (e.ctrlKey || e.metaKey) {
      picked = picked.includes(clip.path) ? picked.filter((p) => p !== clip.path) : [...picked, clip.path];
      return;
    }
    picked = [clip.path];
    select(index);
  }

  function key(e: KeyboardEvent) {
    if (e.key !== "Enter" || mode !== "audio" || picked.length < 2) return;
    e.preventDefault();
    const list = picked.filter((p) => !missing.has(p));
    if (list.length > 0) app.playerAudio = list;
  }

  const chosen = $derived(
    picked.length > 0 ? shown.filter((e) => picked.includes(e.path)) : shown.filter((e) => loadedSet.has(e.path)),
  );
  const canAdd = $derived(src.source === "folder" && chosen.length > 0);
  const canRemove = $derived(src.source === "playlist" && chosen.length > 0);
  const single = $derived(src.source === "playlist" && chosen.length === 1 ? chosen[0].path : null);
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.clips")}>
  <div class="tabs" role="group" aria-label={t("player.panel.clips")}>
    <button class="tab" class:on={src.source === "folder"} aria-pressed={src.source === "folder"} onclick={() => (src.source = "folder")}>
      {t("player.list.folder")}
    </button>
    <button class="tab" class:on={src.source === "playlist"} aria-pressed={src.source === "playlist"} onclick={() => (src.source = "playlist")}>
      {t("player.list.playlist")}
    </button>
    <span class="spacer"></span>
    <span class="count mono">{shown.length}{shown.length !== base.length ? ` / ${base.length}` : ""} {t("player.list.count")}</span>
  </div>

  <div class="tools">
    <input class="filter" type="search" placeholder={t("player.list.filter")} aria-label={t("player.list.filter")} bind:value={filter} />
    {#if src.source === "folder"}
      <select class="sort" aria-label={t("player.list.sort")} bind:value={sortBy} onchange={(e) => e.currentTarget.blur()}>
        <option value="name">{t("player.list.sort.name")}</option>
        <option value="tc">{t("player.list.sort.tc")}</option>
      </select>
      <button class="act" disabled={!canAdd} onclick={() => addToPlaylist(mode, chosen)}>{t("player.playlist.add")}</button>
    {:else}
      <button class="act" disabled={!canRemove} onclick={() => removeFromPlaylist(mode, chosen.map((e) => e.path))}>
        {t("player.playlist.remove")}
      </button>
      <button
        class="act"
        disabled={!single}
        aria-label={t("player.playlist.up")}
        title={t("player.playlist.up")}
        onclick={() => single && moveInPlaylist(mode, single, -1)}>&uarr;</button
      >
      <button
        class="act"
        disabled={!single}
        aria-label={t("player.playlist.down")}
        title={t("player.playlist.down")}
        onclick={() => single && moveInPlaylist(mode, single, 1)}>&darr;</button
      >
      <button class="act" disabled={src.playlist.length === 0} onclick={() => clearPlaylist(mode)}>{t("player.playlist.clear")}</button>
    {/if}
  </div>

  <div class="head" aria-hidden="true">
    <span>{t("field.clip_name")}</span>
    <span class="r">{t("player.tc.start")}</span>
    <span class="r">{t("player.duration")}</span>
    <span class="r">{t("player.scene")}/{t("player.take")}</span>
    <span></span>
  </div>

  <div class="scroller" bind:this={scroller} onscroll={(e) => (top = e.currentTarget.scrollTop)}>
    {#if shown.length === 0}
      <p class="empty">
        {#if src.loading}{t("player.loading")}
        {:else if src.error}{src.error}
        {:else if src.source === "folder" && !src.dir}{t("player.list.pick")}
        {:else}{t("player.list.empty")}{/if}
      </p>
    {/if}
    <div class="space" style:height="{shown.length * rowH}px">
      {#each visibleRows as e, k (e.path)}
        {@const i = first + k}
        {@const d = player.details.get(e.path)}
        <button
          class="row"
          class:cur={loadedSet.has(e.path)}
          class:pick={picked.includes(e.path)}
          class:gone={missing.has(e.path) || unreadable.has(e.path)}
          style:top="{i * rowH}px"
          style:height="{rowH}px"
          title={missing.has(e.path)
            ? `${t("player.playlist.missing")} : ${e.path}`
            : unreadable.has(e.path)
              ? `${t("player.playlist.unreadable")} : ${e.path}`
              : e.rel || e.name}
          onclick={(ev) => click(ev, i)}
          onkeydown={key}
        >
          <span class="name">{e.name}</span>
          <span class="r mono">{tcOf(d)}</span>
          <span class="r mono">{short(dur(d))}</span>
          <span class="r mono">{sceneTake(d)}</span>
          <span class="circ" class:yes={d?.values.circled === "true"} title={d?.values.circled === "true" ? t("field.circled") : undefined}></span>
        </button>
      {/each}
    </div>
  </div>
</div>

<style>
  .panel {
    --cols: minmax(0, 1fr) 62px 38px 40px 10px;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
    background: var(--vf-surface);
    font-size: var(--vf-text-xs);
  }
  .tabs,
  .tools {
    display: flex;
    align-items: center;
    gap: var(--vf-space-1);
    padding: var(--vf-space-1) var(--vf-space-2);
  }
  .spacer {
    flex: 1;
  }
  .count {
    color: var(--vf-text-muted);
  }
  .tabs button.tab {
    padding: 2px var(--vf-space-2);
    background: transparent;
    color: var(--vf-text-muted);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-xs);
    cursor: pointer;
  }
  .tabs button.tab:hover {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  /* Plus précis que « .tabs button.tab:hover » ? Même poids, déclaré après : l'onglet actif se voit (accent plein). */
  .tabs button.tab.on {
    background: var(--vf-accent);
    border-color: var(--vf-accent);
    color: var(--vf-on-accent);
    font-weight: 600;
  }
  .filter {
    flex: 1;
    min-width: 0;
    padding: 2px var(--vf-space-2);
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-xs);
  }
  .sort {
    max-width: 90px;
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-xs);
  }
  .tools button.act {
    padding: 2px var(--vf-space-2);
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-xs);
    cursor: pointer;
    white-space: nowrap;
  }
  .tools button.act:hover:not(:disabled) {
    background: var(--vf-surface-hover);
  }
  .tools button.act:disabled {
    color: var(--vf-text-disabled);
    cursor: default;
  }
  .head,
  .row {
    display: grid;
    grid-template-columns: var(--cols);
    align-items: center;
    gap: var(--vf-space-2);
    padding: 0 var(--vf-space-2);
  }
  .head {
    color: var(--vf-text-muted);
    border-bottom: 1px solid var(--vf-border);
    padding-block: 2px;
    white-space: nowrap;
    overflow: hidden;
  }
  .r {
    text-align: right;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .scroller {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .space {
    position: relative;
  }
  .empty {
    margin: var(--vf-space-3);
    color: var(--vf-text-disabled);
  }
  .row {
    position: absolute;
    left: 0;
    right: 0;
    width: 100%;
    background: transparent;
    color: var(--vf-text);
    border: 0;
    border-left: 2px solid transparent;
    text-align: left;
    font-size: var(--vf-text-xs);
    cursor: pointer;
  }
  .row:hover {
    background: var(--vf-surface-hover);
  }
  /* Même poids que « .row:hover », déclarés après : marqué et chargé l'emportent. */
  .row.pick {
    background: var(--vf-selection);
  }
  .row.cur {
    background: var(--vf-selection);
    border-left-color: var(--vf-accent);
    font-weight: 600;
  }
  .row.gone {
    color: var(--vf-text-disabled);
    text-decoration: line-through;
    cursor: default;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .circ {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    border: 1px solid transparent;
  }
  .circ.yes {
    border-color: var(--vf-accent);
    background: var(--vf-accent);
  }
</style>
