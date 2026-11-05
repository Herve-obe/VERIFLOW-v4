<!-- Inspecteur de métadonnées en lecture seule, colonne de droite. Lot 6.
     Lit : currentClip() et currentDetails(). Aucun champ éditable. Largeur et défilement : 100 % du parent,
     overflow-y auto à gérer par le composant.
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";
  import { currentClip, currentDetails } from "../../stores/player.svelte";
  import { isBeginner } from "../../stores/level.svelte";
  import { bytes } from "../../lib/format";
  import { clock } from "../../lib/media";
  import { fps } from "../../lib/timecode";

  let { mode }: { mode: Mode } = $props();

  type Row = { label: string; value: string };
  type Section = { id: string; title: string; rows: Row[] };

  const KEY = "veriflow.player.inspector.sections";
  function load(): Record<string, boolean> {
    try {
      const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
      return v && typeof v === "object" ? v : {};
    } catch {
      return {};
    }
  }
  // true = replié
  let folded = $state<Record<string, boolean>>(load());
  let filter = $state("");

  function toggle(id: string) {
    folded[id] = !folded[id];
    try {
      localStorage.setItem(KEY, JSON.stringify(folded));
    } catch {
      /* stockage indisponible : l'état vaut pour la session */
    }
  }

  const clip = $derived(currentClip());
  const d = $derived(currentDetails());

  const sections = $derived.by<Section[]>(() => {
    if (!clip) return [];
    const det = d?.details;
    const probe = det?.probe ?? null;
    const val = (id: string) => d?.values[id] ?? det?.embedded[id] ?? "";
    const fieldRows = (ids: string[]): Row[] =>
      ids.map((id) => ({ label: t(`field.${id}`), value: val(id) }));
    const row = (key: string, value: string | number | null | undefined): Row => ({
      label: t(key),
      value: value === null || value === undefined ? "" : String(value),
    });

    const video = probe?.video ?? null;
    const audio = probe?.audio ?? [];
    const wav = det?.wav ?? null;
    const duration = probe?.duration ? clock(probe.duration) : "";

    const out: Section[] = [
      {
        id: "file",
        title: t("player.inspector.section.file"),
        rows: [
          row("player.inspector.row.name", clip.name),
          row("player.inspector.row.path", clip.rel),
          row("player.inspector.row.size", clip.size ? bytes(clip.size) : ""),
          row("player.inspector.row.modified", clip.modified),
          row("player.inspector.row.format", probe?.format),
          row("player.duration", duration),
        ],
      },
      {
        id: "breakdown",
        title: t("media.group.breakdown"),
        rows: fieldRows(["sequence", "scene", "shot", "take", "circled", "false_start", "wild_track", "mos", "reel", "clip_name"]),
      },
      {
        id: "settings",
        title: t("player.inspector.section.settings"),
        rows: fieldRows(["iso", "shutter", "white_balance", "tint", "capture_fps"]),
      },
      {
        id: "device",
        title: t("player.inspector.section.device"),
        rows: fieldRows(["camera", "recorder"]),
      },
      {
        id: "lens",
        title: t("player.inspector.section.lens"),
        rows: fieldRows(["lens", "focal_length", "t_stop", "focus_distance", "filters"]),
      },
    ];
    if (mode === "video") {
      out.push({
        id: "video",
        title: t("player.inspector.section.video"),
        rows: [
          row("player.codec", video?.codec),
          row("player.resolution", video ? `${video.width} x ${video.height}` : ""),
          row("player.rate", video ? String(Math.round(fps(video.rate) * 1000) / 1000) : ""),
          row("player.inspector.row.pix_fmt", video?.pix_fmt),
          row("player.inspector.row.frames", video?.frame_count),
          row("player.tc.start", probe?.start_timecode),
        ],
      });
    }
    const sound: Row[] = [];
    if (wav) {
      sound.push(
        row("player.inspector.row.sample_rate", `${wav.sample_rate} Hz`),
        row("player.inspector.row.bits", `${wav.bits} ${wav.format === "Float" ? "float" : "bit"}`),
        row("player.inspector.row.channels", wav.channels),
      );
    } else if (audio.length) {
      const a = audio[0];
      sound.push(
        row("player.codec", a.codec),
        row("player.inspector.row.sample_rate", `${a.sample_rate} Hz`),
        row("player.inspector.row.bits", a.bits),
        row("player.inspector.row.channels", audio.reduce((n, x) => n + x.channels, 0)),
      );
    }
    sound.push(...fieldRows(["tc_rate", "user_bits", "mics"]));
    out.push({ id: "sound", title: t("media.group.sound"), rows: sound });
    out.push({
      id: "notes",
      title: t("media.group.notes"),
      rows: fieldRows(["comment", "rating", "keywords", "color_label"]),
    });
    if (!isBeginner()) {
      const tags = Object.entries(probe?.tags ?? {}).map(([k, v]) => ({ label: k, value: v }));
      const cam = Object.entries(det?.camera ?? {}).map(([k, v]) => ({ label: k, value: v }));
      out.push({
        id: "tags",
        title: t("player.inspector.section.tags"),
        rows: [...tags, ...cam].sort((a, b) => a.label.localeCompare(b.label)),
      });
    }
    return out;
  });

  const q = $derived(filter.trim().toLowerCase());
  const shown = $derived(
    sections
      .map((s) => {
        // Sans filtre : on garde les lignes remplies seulement. Avec filtre : libellé ou valeur.
        const rows = s.rows.filter(
          (r) => r.value !== "" && (!q || r.label.toLowerCase().includes(q) || r.value.toLowerCase().includes(q)),
        );
        return { ...s, rows };
      })
      .filter((s) => s.rows.length > 0),
  );
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.inspector")}>
  <div class="bar">
    <input type="search" bind:value={filter} placeholder={t("player.inspector.filter")} aria-label={t("player.inspector.filter")} />
  </div>
  <div class="body">
    {#if !clip}
      <p class="none">{t("player.inspector.none")}</p>
    {:else}
      {#each shown as s (s.id)}
        {@const open = !!q || !folded[s.id]}
        <section>
          <button type="button" class="head" class:closed={!open} aria-expanded={open} onclick={() => toggle(s.id)}>
            <span class="caret" aria-hidden="true"></span>
            <span class="title">{s.title}</span>
            <span class="n">{s.rows.length}</span>
          </button>
          {#if open}
            <dl>
              {#each s.rows as r (r.label)}
                <dt title={r.label}>{r.label}</dt>
                <dd title={r.value}>{r.value}</dd>
              {/each}
            </dl>
          {/if}
        </section>
      {/each}
      {#if shown.length === 0}
        <p class="none">{t("player.inspector.none")}</p>
      {/if}
    {/if}
  </div>
</div>

<style>
  .panel {
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    background: var(--vf-surface);
    color: var(--vf-text);
  }
  .bar {
    padding: 6px;
    border-bottom: 1px solid var(--vf-border);
  }
  .bar input {
    width: 100%;
    box-sizing: border-box;
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }
  .none {
    margin: 12px 8px;
    color: var(--vf-text-muted);
    font-size: var(--vf-text-sm);
  }
  section {
    border-bottom: 1px solid var(--vf-border);
  }
  .head {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 8px;
    border: 0;
    border-radius: 0;
    background: var(--vf-surface);
    color: var(--vf-text);
    font-size: var(--vf-text-xs);
    font-weight: 600;
    text-align: left;
    cursor: pointer;
  }
  .head:hover {
    background: var(--vf-surface-hover);
  }
  .title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .n {
    color: var(--vf-text-muted);
    font-weight: 400;
  }
  .caret {
    width: 0;
    height: 0;
    border-left: 4px solid transparent;
    border-right: 4px solid transparent;
    border-top: 5px solid currentColor;
  }
  .head.closed .caret {
    border-top: 4px solid transparent;
    border-bottom: 4px solid transparent;
    border-left: 5px solid currentColor;
    border-right: 0;
  }
  dl {
    margin: 0;
    padding: 2px 8px 6px;
    display: grid;
    grid-template-columns: minmax(0, 40%) minmax(0, 1fr);
    column-gap: 8px;
    row-gap: 2px;
    font-size: var(--vf-text-xs);
  }
  dt {
    color: var(--vf-text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  dd {
    margin: 0;
    text-align: right;
    font-family: var(--vf-font-mono);
    font-variant-numeric: tabular-nums;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
