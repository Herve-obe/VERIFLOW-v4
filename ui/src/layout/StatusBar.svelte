<!-- Barre d'état : mode, message courant, langue, version. -->
<script lang="ts">
  import { app } from "../stores/app.svelte";
  import { i18n, setLanguage, t, LANGUAGES, type Language } from "../i18n/index.svelte";

  let { version = "" }: { version?: string } = $props();
</script>

<footer>
  <span class="mode">{t("mode." + app.mode)}</span>
  <span class="msg">{app.status || t("status.ready")}</span>
  <label>
    {t("settings.language")}
    <select value={i18n.lang} onchange={(e) => setLanguage(e.currentTarget.value as Language)}>
      {#each Object.keys(LANGUAGES) as lang (lang)}
        <option value={lang}>{lang.toUpperCase()}</option>
      {/each}
    </select>
  </label>
  <span class="mono">v{version}</span>
</footer>

<style>
  footer {
    display: flex;
    align-items: center;
    gap: var(--vf-space-4);
    height: var(--vf-statusbar-height);
    padding: 0 var(--vf-space-4);
    background: var(--vf-surface);
    border-top: 1px solid var(--vf-border);
    font-size: var(--vf-text-xs);
    color: var(--vf-text-muted);
  }
  .mode {
    color: var(--vf-accent);
    font-weight: 700;
    letter-spacing: 0.08em;
  }
  .msg {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  select {
    background: var(--vf-surface-high);
    color: var(--vf-text);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-sm);
    font-size: var(--vf-text-xs);
    margin-left: var(--vf-space-1);
  }
</style>
