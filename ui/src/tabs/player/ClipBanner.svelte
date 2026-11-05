<!-- Bandeau d'infos du clip courant (grands caractères). Lot 4.
     Lit : currentClip() et currentDetails() (stores/player.svelte.ts). Champ absent = « - ».
     Hauteur libre (auto) : PlayerTab ne la contraint pas.
     Props définitives (ne pas les changer) : mode ("video" | "audio"), le mode de l'onglet PLAYER
     auquel appartient ce panneau (les deux modes restent montés, donc ne pas lire app.mode pour
     décider du contenu). -->
<script lang="ts">
  import type { Mode } from "../../stores/app.svelte";
  import { t } from "../../i18n/index.svelte";

  let { mode }: { mode: Mode } = $props();
</script>

<div class="panel" data-mode={mode} role="region" aria-label={t("player.panel.banner")}></div>

<style>
  .panel {
    height: 100%;
    min-height: 0;
    background: var(--vf-surface);
  }
</style>
