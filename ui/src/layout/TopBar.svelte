<!-- Barre supérieure : logo, menus (Windows, Linux), onglets, projet, interrupteur de mode. -->
<script lang="ts">
  import Logo from "../components/Logo.svelte";
  import ModeSwitch from "../components/ModeSwitch.svelte";
  import TabBar from "./TabBar.svelte";
  import ProjectMenu from "./ProjectMenu.svelte";
  import MenuBar from "../menu/MenuBar.svelte";
  import { t } from "../i18n/index.svelte";
</script>

<header>
  <div class="brand">
    <Logo size={26} />
    <span>{t("app.name")}</span>
  </div>
  <MenuBar />
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
    min-width: 0;
    display: flex;
    align-items: center;
    gap: var(--vf-space-4);
  }
  .brand,
  header > :global(*) {
    flex-shrink: 0;
  }
  header > .right {
    flex-shrink: 1;
  }
  /* Fenêtre étroite (jusqu'à 1024 px de large, minimum de la fenêtre) : le
     logo seul, des espaces plus serrés ; les onglets, les menus et
     l'interrupteur de mode restent entiers, le nom du projet est abrégé. */
  @media (max-width: 1360px) {
    header {
      gap: var(--vf-space-3);
    }
    .brand span {
      display: none;
    }
    .right {
      gap: var(--vf-space-3);
    }
  }
  @media (max-width: 1150px) {
    header {
      gap: var(--vf-space-2);
      padding: 0 var(--vf-space-3);
    }
    .right {
      gap: var(--vf-space-2);
    }
  }
</style>
