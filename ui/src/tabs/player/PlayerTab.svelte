<!-- Onglet PLAYER : vue VIDEO ou AUDIO selon le mode (charte §3.1 et §7.3).
     Les deux vues restent montées pour conserver leur état lors des bascules. -->
<script lang="ts">
  import { app } from "../../stores/app.svelte";
  import VideoView from "./VideoView.svelte";
  import AudioView from "./AudioView.svelte";
</script>

<div class="view" class:hidden={app.mode !== "video"}><VideoView /></div>
<div class="view" class:hidden={app.mode !== "audio"}><AudioView /></div>

<style>
  .view {
    height: 100%;
  }
  .hidden {
    display: none;
  }
</style>
