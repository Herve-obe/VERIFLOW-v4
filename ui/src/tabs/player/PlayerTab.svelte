<!-- Onglet PLAYER : vue VIDEO ou AUDIO selon le mode (charte §3.1 et §7.3).
     Les deux vues restent montées pour conserver leur état lors des bascules. -->
<script lang="ts">
  import { app } from "../../stores/app.svelte";
  import VideoView from "./VideoView.svelte";
  import AudioView from "./AudioView.svelte";
  import { isBeginner } from "../../stores/level.svelte";
</script>

<div class="view" class:hidden={app.mode !== "video"}><VideoView path={app.playerVideo} logs={!isBeginner()} /></div>
<div class="view" class:hidden={app.mode !== "audio"}><AudioView paths={app.playerAudio} logs={!isBeginner()} /></div>

<style>
  .view {
    height: 100%;
  }
  .hidden {
    display: none;
  }
</style>
