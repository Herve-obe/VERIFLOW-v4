<!-- Liste des clips (dossier ou playlist), sous l'explorateur. Lot 3.
     Lit/écrit : player.clips, player.current, player.source, player.details (stores/player.svelte.ts) ;
     charge via select(i) ; ne touche pas la disposition (la colonne gauche lui donne sa taille).
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";

  let { mode }: { mode: Mode } = $props();
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.clips")}></div>

<style>
  .panel {
    height: 100%;
    min-height: 0;
    background: var(--vf-surface);
  }
</style>
