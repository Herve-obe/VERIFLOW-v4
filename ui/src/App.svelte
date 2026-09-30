<!-- Racine de l'interface : barre supérieure, onglet actif, barre d'état. -->
<script lang="ts">
  import { onMount } from "svelte";
  import TopBar from "./layout/TopBar.svelte";
  import StatusBar from "./layout/StatusBar.svelte";
  import OffloadTab from "./tabs/offload/OffloadTab.svelte";
  import MediaTab from "./tabs/media/MediaTab.svelte";
  import PlayerTab from "./tabs/player/PlayerTab.svelte";
  import SyncTab from "./tabs/sync/SyncTab.svelte";
  import TranscodeTab from "./tabs/transcode/TranscodeTab.svelte";
  import ReportTab from "./tabs/report/ReportTab.svelte";
  import { app, setMode, toggleMode, type Tab } from "./stores/app.svelte";
  import { resolveAction } from "./shortcuts";
  import { appInfo } from "./lib/api";

  const VIEWS = {
    offload: OffloadTab,
    media: MediaTab,
    player: PlayerTab,
    sync: SyncTab,
    transcode: TranscodeTab,
    report: ReportTab,
  };

  let version = $state("");
  const View = $derived(VIEWS[app.tab]);

  onMount(() => {
    setMode(app.mode);
    appInfo().then((info) => (version = info.version));
  });

  function onKeydown(e: KeyboardEvent) {
    const action = resolveAction(e);
    if (!action) return;
    e.preventDefault();
    if (action === "mode.toggle" || action === "mode.toggle.force") toggleMode();
    else app.tab = action.slice("tab.".length) as Tab;
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="shell">
  <TopBar />
  <main>
    <View />
  </main>
  <StatusBar {version} />
</div>

<style>
  .shell {
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100%;
  }
  main {
    overflow: auto;
    background: var(--vf-bg);
  }
</style>
