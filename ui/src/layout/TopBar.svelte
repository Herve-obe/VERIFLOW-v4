<!-- Barre supérieure : logo, onglets, projet, interrupteur de mode. -->
<script lang="ts">
  import Logo from "../components/Logo.svelte";
  import ModeSwitch from "../components/ModeSwitch.svelte";
  import TabBar from "./TabBar.svelte";
  import ProjectMenu from "./ProjectMenu.svelte";
  import { t } from "../i18n/index.svelte";
</script>

<header>
  <div class="brand">
    <Logo size={26} />
    <span>{t("app.name")}</span>
  </div>
  <TabBar />
  <div class="right">
    <ProjectMenu />
    <ModeSwitch />
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: stretch;
    gap: var(--vf-space-4);
    height: var(--vf-topbar-height);
    padding: 0 var(--vf-space-4);
    background: var(--vf-surface);
    border-bottom: 1px solid var(--vf-border);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    font-weight: 700;
    letter-spacing: 0.12em;
    font-size: var(--vf-text-md);
  }
  .right {
    margin-left: auto;
    display: flex;
    align-items: center;
    gap: var(--vf-space-4);
  }
</style>
