<!-- Aide > Guide d'utilisation : un chapitre par onglet (ui/src/guide/*.md),
     sommaire à gauche. -->
<script lang="ts">
  import { marked } from "marked";
  import fr from "../guide/fr.md?raw";
  import en from "../guide/en.md?raw";
  import { i18n } from "../i18n/index.svelte";

  interface Chapter {
    title: string;
    html: string;
  }

  /** Chapitres : un par titre de premier niveau. */
  function chapters(md: string): Chapter[] {
    return md
      .split(/^# /m)
      .filter((part) => part.trim())
      .map((part) => {
        const nl = part.indexOf("\n");
        const title = part.slice(0, nl).trim();
        return { title, html: marked.parse(part.slice(nl + 1), { async: false }) as string };
      });
  }

  const list = $derived(chapters(i18n.lang === "fr" ? fr : en));
  let current = $state(0);
  let pane = $state<HTMLElement | null>(null);

  function show(i: number) {
    current = i;
    pane?.scrollTo({ top: 0 });
  }
</script>

<div class="guide">
  <nav class="toc">
    {#each list as c, i (c.title)}
      <button class:on={current === i} onclick={() => show(i)}>{c.title}</button>
    {/each}
  </nav>
  <article class="content" bind:this={pane}>
    {#if list[current]}
      <h2>{list[current].title}</h2>
      <!-- Contenu fourni avec l'application (fichiers du guide), pas de saisie extérieure. -->
      {@html list[current].html}
    {/if}
  </article>
</div>

<style>
  .guide {
    display: grid;
    grid-template-columns: 210px 1fr;
    height: 100%;
    min-height: 0;
  }
  .toc {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: var(--vf-space-2);
    border-right: 1px solid var(--vf-border);
    overflow: auto;
  }
  .toc button {
    background: none;
    border: 0;
    border-radius: var(--vf-radius-sm);
    padding: var(--vf-space-2) var(--vf-space-3);
    color: var(--vf-text-muted);
    text-align: left;
    font-size: var(--vf-text-sm);
    cursor: pointer;
  }
  .toc button:hover {
    background: var(--vf-surface-hover);
    color: var(--vf-text);
  }
  .toc button.on {
    background: var(--vf-selection);
    color: var(--vf-accent);
    font-weight: 600;
  }
  .content {
    padding: var(--vf-space-4) var(--vf-space-6);
    overflow: auto;
    min-height: 0;
    font-size: var(--vf-text-md);
    line-height: 1.55;
  }
  .content h2 {
    margin: 0 0 var(--vf-space-3);
    font-size: var(--vf-text-lg);
    letter-spacing: 0.04em;
  }
  .content :global(h2:not(:first-child)) {
    margin: var(--vf-space-6) 0 var(--vf-space-2);
    font-size: var(--vf-text-md);
    color: var(--vf-accent);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  .content :global(p),
  .content :global(li) {
    color: var(--vf-text);
  }
  .content :global(ul),
  .content :global(ol) {
    padding-left: var(--vf-space-6);
  }
  .content :global(li) {
    margin: 3px 0;
  }
  .content :global(code) {
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-sm);
    background: var(--vf-surface-high);
    border-radius: var(--vf-radius-sm);
    padding: 0 4px;
  }
  .content :global(strong) {
    color: var(--vf-text);
  }
</style>
