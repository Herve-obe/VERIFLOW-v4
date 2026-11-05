<!-- Bande de clips horizontale sous l'image. Lot 5.
     Lit : player.clips, player.current ; charge via select(i). Hauteur imposée par PlayerTab (100 % du parent).
     Carte VIDEO = Thumb hover={false}, carte AUDIO = nom + Wave ; clés player.strip.prev / .next.
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";

  let { mode }: { mode: Mode } = $props();
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.strip")}></div>

<style>
  .panel {
    height: 100%;
    min-height: 0;
    background: var(--vf-surface);
  }
</style>
