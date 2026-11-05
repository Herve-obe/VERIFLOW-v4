<!-- Inspecteur de métadonnées en lecture seule, colonne de droite. Lot 6.
     Lit : currentClip() et currentDetails(). Aucun champ éditable. Largeur et défilement : 100 % du parent,
     overflow-y auto à gérer par le composant.
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";

  let { mode }: { mode: Mode } = $props();
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.inspector")}></div>

<style>
  .panel {
    height: 100%;
    min-height: 0;
    background: var(--vf-surface);
  }
</style>
