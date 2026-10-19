<!-- En-tête en deux lignes (charte §4, décision d'Hervé du 19/10/2026).
     1. Barre de titre : logo, menus, projet ouvert au centre ; sous Windows,
        elle remplace celle du système (déplacement, double-clic pour
        agrandir, boutons de la fenêtre). Absente sous macOS, où les menus
        sont dans la barre du système.
     2. Onglets centrés, interrupteur VIDEO / AUDIO à droite. -->
<script lang="ts">
  import Logo from "../components/Logo.svelte";
  import ModeSwitch from "../components/ModeSwitch.svelte";
  import TabBar from "./TabBar.svelte";
  import ProjectMenu from "./ProjectMenu.svelte";
  import WindowControls from "./WindowControls.svelte";
  import MenuBar from "../menu/MenuBar.svelte";
  import { isMac, platform } from "../menu/commands.svelte";
  import { inTauri } from "../lib/api";
  import { t } from "../i18n/index.svelte";

  const ownTitleBar = inTauri && platform === "windows";
</script>

{#snippet brand()}
  <div class="brand" data-tauri-drag-region>
    <Logo size={20} />
    <span>{t("app.name")}</span>
  </div>
{/snippet}

<header>
  {#if !isMac}
    <div class="titlebar" data-tauri-drag-region>
      {@render brand()}
      <MenuBar />
      <div class="project" data-tauri-drag-region><ProjectMenu /></div>
      {#if ownTitleBar}<WindowControls />{/if}
    </div>
  {/if}
  <div class="tabsrow">
    <div class="side">
      {#if isMac}{@render brand()}{/if}
    </div>
    <TabBar />
    <div class="side right">
      {#if isMac}<ProjectMenu />{/if}
      <ModeSwitch />
    </div>
  </div>
</header>

<style>
  header {
    display: flex;
    flex-direction: column;
  }
  .titlebar {
    position: relative;
    display: flex;
    align-items: center;
    gap: var(--vf-space-4);
    height: var(--vf-titlebar-height);
    padding-left: var(--vf-space-3);
    background: var(--vf-bg);
    border-bottom: 1px solid var(--vf-border);
    user-select: none;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    font-weight: 700;
    letter-spacing: 0.12em;
    font-size: var(--vf-text-sm);
  }
  .brand :global(svg),
  .brand span {
    pointer-events: none;
  }
  /* Projet au centre de l'espace libre entre les menus et les boutons de la
     fenêtre (jamais par-dessus les menus, même en fenêtre étroite). */
  .project {
    flex: 1;
    min-width: 0;
    display: flex;
    justify-content: center;
    padding: 0 var(--vf-space-4);
  }
  /* Onglets centrés : deux colonnes égales de part et d'autre. */
  .tabsrow {
    display: grid;
    grid-template-columns: 1fr auto 1fr;
    align-items: stretch;
    height: var(--vf-tabbar-height);
    padding: 0 var(--vf-space-4);
    background: var(--vf-surface);
    border-bottom: 1px solid var(--vf-border);
  }
  .side {
    display: flex;
    align-items: center;
    gap: var(--vf-space-4);
    min-width: 0;
  }
  .side.right {
    justify-content: flex-end;
  }
</style>
