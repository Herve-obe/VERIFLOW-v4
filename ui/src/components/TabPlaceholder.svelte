<!-- Contenu provisoire d'un onglet non encore développé. -->
<script lang="ts">
  import { app, type Tab } from "../stores/app.svelte";
  import { t } from "../i18n/index.svelte";

  let { tab }: { tab: Tab } = $props();
</script>

<section>
  <h1>{t("tab." + tab)} <span class="mode">{t("mode." + app.mode)}</span></h1>
  <p>{t("tab." + tab + ".desc")}</p>
  <p class="todo">{t("tab.placeholder")}</p>
</section>

<style>
  section {
    height: 100%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--vf-space-2);
    text-align: center;
    padding: var(--vf-space-8);
  }
  h1 {
    margin: 0;
    font-size: var(--vf-text-xl);
    letter-spacing: 0.1em;
  }
  .mode {
    color: var(--vf-accent);
  }
  p {
    margin: 0;
    max-width: 520px;
    color: var(--vf-text-muted);
  }
  .todo {
    margin-top: var(--vf-space-4);
    font-size: var(--vf-text-sm);
    color: var(--vf-text-disabled);
  }
</style>
