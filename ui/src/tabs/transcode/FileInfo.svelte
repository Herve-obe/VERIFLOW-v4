<!-- Fiche d'un fichier de la liste : conteneur, image, son, timecode. -->
<script lang="ts">
  import Modal from "../../components/Modal.svelte";
  import { t } from "../../i18n/index.svelte";
  import { describe, type Described } from "../../lib/media";
  import { bytes, duration } from "../../lib/format";

  let { path, onClose }: { path: string; onClose: () => void } = $props();

  let info = $state<Described | null>(null);
  let error = $state("");

  $effect(() => {
    describe([path])
      .then((d) => {
        info = d[0] ?? null;
        if (info?.details.error) error = info.details.error;
      })
      .catch((e) => (error = String(e)));
  });

  const name = $derived(path.split(/[\\/]/).pop() ?? path);
  const p = $derived(info?.details.probe ?? null);
  const fps = (r: { num: number; den: number }) => (r.num / r.den).toFixed(3).replace(/\.?0+$/, "").replace(".", ",");
  // Taille du fichier, fournie par l'analyse (absente du type partagé de MEDIA).
  const size = $derived(((p as { size?: number } | null)?.size ?? 0) as number);
</script>

<Modal title={`${t("transcode.info")} : ${name}`} {onClose} width="min(520px, 94vw)">
  <div class="body">
    {#if error}<p class="tc-warn">{error}</p>{/if}
    {#if !info && !error}<p class="muted">{t("transcode.info.loading")}</p>{/if}
    {#if p}
      <table>
        <tbody>
          <tr><th>{t("transcode.info.path")}</th><td class="mono">{path}</td></tr>
          <tr><th>{t("transcode.info.format")}</th><td>{p.format}</td></tr>
          <tr><th>{t("transcode.info.duration")}</th><td class="mono">{duration(p.duration)}</td></tr>
          {#if size}<tr><th>{t("transcode.info.size")}</th><td>{bytes(size)}</td></tr>{/if}
          {#if p.start_timecode}<tr><th>Timecode</th><td class="mono">{p.start_timecode}</td></tr>{/if}
          {#if p.video}
            <tr><th>{t("transcode.info.video")}</th><td>{p.video.codec}, {p.video.width}×{p.video.height}, {fps(p.video.rate)} i/s, {p.video.pix_fmt}</td></tr>
          {/if}
          {#each p.audio as a, i (i)}
            <tr>
              <th>{t("transcode.info.audio")} {i + 1}</th>
              <td>{a.codec}, {(a.sample_rate / 1000).toString().replace(".", ",")} kHz, {a.channels} {t("transcode.info.channels")}{a.bits ? `, ${a.bits} bits` : ""}</td>
            </tr>
          {/each}
          {#if info?.details.wav?.time_reference != null}
            <tr><th>BWF</th><td class="mono">{info.details.wav.time_reference} {t("transcode.info.samples")}</td></tr>
          {/if}
          {#each Object.entries(info?.details.embedded ?? {}).slice(0, 12) as [k, v] (k)}
            <tr><th>{k}</th><td>{v}</td></tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </div>
</Modal>

<style>
  .body {
    padding: var(--vf-space-3);
    font-size: var(--vf-text-sm);
    max-height: 70vh;
    overflow-y: auto;
  }
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    vertical-align: top;
    padding: 3px 6px;
    border-bottom: 1px solid var(--vf-border);
  }
  th {
    color: var(--vf-text-muted);
    font-weight: 500;
    white-space: nowrap;
    width: 30%;
  }
  td {
    word-break: break-all;
  }
  .muted {
    color: var(--vf-text-muted);
  }
  .tc-warn {
    color: var(--vf-warning);
  }
</style>
