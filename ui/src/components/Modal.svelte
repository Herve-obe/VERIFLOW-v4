<!-- Fenêtre au premier plan : déplaçable par sa barre de titre, fermée par
     Échap, la croix ou un clic à côté. Sert au lecteur rapide et aux alertes. -->
<script lang="ts">
  import type { Snippet } from "svelte";

  let {
    title,
    onClose,
    width = "auto",
    height = "auto",
    actions,
    children,
  }: {
    title: string;
    onClose: () => void;
    width?: string;
    height?: string;
    actions?: Snippet;
    children: Snippet;
  } = $props();

  let dx = $state(0);
  let dy = $state(0);

  function startMove(e: PointerEvent) {
    // Les boutons de la barre de titre restent cliquables.
    if (e.button !== 0 || (e.target as HTMLElement).closest("button, select, input")) return;
    e.preventDefault();
    const x0 = e.clientX - dx;
    const y0 = e.clientY - dy;
    const move = (m: PointerEvent) => {
      // La barre de titre reste toujours atteignable à l'écran.
      const limitX = window.innerWidth / 2 - 80;
      const limitY = window.innerHeight / 2 - 40;
      dx = Math.max(-limitX, Math.min(limitX, m.clientX - x0));
      dy = Math.max(-limitY, Math.min(limitY, m.clientY - y0));
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
      document.body.classList.remove("vf-moving");
    };
    document.body.classList.add("vf-moving");
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
  }
</script>

<svelte:window onkeydown={(e) => e.key === "Escape" && onClose()} />

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onClose()}>
  <div
    class="dialog"
    role="dialog"
    aria-modal="true"
    aria-label={title}
    style:width
    style:height
    style:transform={`translate(${dx}px, ${dy}px)`}
  >
    <header role="toolbar" tabindex="-1" onpointerdown={startMove} ondblclick={() => ((dx = 0), (dy = 0))}>
      <span class="title">{title}</span>
      <div class="actions">
        {@render actions?.()}
        <button class="close" onclick={onClose} aria-label="Fermer">
          <svg viewBox="0 0 16 16"><path d="M4 4 L12 12 M12 4 L4 12" stroke="currentColor" stroke-width="1.6" /></svg>
        </button>
      </div>
    </header>
    <div class="body">{@render children()}</div>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    left: 0;
    background: var(--vf-backdrop);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
  }
  .dialog {
    max-width: 96vw;
    max-height: 94vh;
    display: flex;
    flex-direction: column;
    background: var(--vf-bg);
    border: 1px solid var(--vf-border);
    border-radius: var(--vf-radius-lg);
    box-shadow: 0 12px 40px var(--vf-shadow);
    overflow: hidden;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--vf-space-3);
    padding: var(--vf-space-2) var(--vf-space-3);
    background: var(--vf-surface);
    border-bottom: 1px solid var(--vf-border);
    cursor: grab;
    user-select: none;
  }
  .title {
    font-family: var(--vf-font-mono);
    font-size: var(--vf-text-sm);
    color: var(--vf-text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: var(--vf-space-2);
    cursor: default;
  }
  .close {
    width: 24px;
    height: 24px;
    padding: 4px;
    background: none;
    border: 1px solid transparent;
    border-radius: var(--vf-radius-sm);
    color: var(--vf-text-muted);
    cursor: pointer;
  }
  .close:hover {
    border-color: var(--vf-border);
    color: var(--vf-text);
  }
  .body {
    flex: 1;
    min-height: 0;
    overflow: auto;
  }
  :global(body.vf-moving),
  :global(body.vf-moving *) {
    cursor: grabbing;
    user-select: none;
  }
</style>
