<!-- Poignée de redimensionnement entre deux panneaux.
     Props : size (bindable, px) ; min, max (px) ; def (px, double-clic) ;
     orientation "vertical" = poignée verticale qui change une largeur, "horizontal" = poignée
     horizontale qui change une hauteur ; grow "after" (défaut) = le panneau mesuré est avant la
     poignée (tirer vers la droite/le bas l'agrandit), "before" = il est après (tirer vers la
     gauche/le haut l'agrandit). Flèches du clavier : pas de 16 px. Rend le clavier au relâchement. -->
<script lang="ts">
  import { t } from "../i18n/index.svelte";

  let {
    size = $bindable(),
    min,
    max,
    def,
    orientation = "vertical",
    grow = "after",
    label = "",
  }: {
    size: number;
    min: number;
    max: number;
    def: number;
    orientation?: "vertical" | "horizontal";
    grow?: "after" | "before";
    label?: string;
  } = $props();

  let drag: { start: number; size: number } | null = $state(null);
  const clamp = (v: number) => Math.round(Math.min(Math.max(v, min), Math.max(min, max)));
  const vertical = $derived(orientation === "vertical");

  function down(e: PointerEvent) {
    if (e.button !== 0) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    drag = { start: vertical ? e.clientX : e.clientY, size };
  }
  function move(e: PointerEvent) {
    if (!drag) return;
    const delta = (vertical ? e.clientX : e.clientY) - drag.start;
    size = clamp(drag.size + (grow === "after" ? delta : -delta));
  }
  function up(e: PointerEvent) {
    drag = null;
    (e.currentTarget as HTMLElement).blur();
  }
  function key(e: KeyboardEvent) {
    const back = vertical ? "ArrowLeft" : "ArrowUp";
    const fwd = vertical ? "ArrowRight" : "ArrowDown";
    if (e.key !== back && e.key !== fwd) return;
    e.preventDefault();
    const sign = (e.key === fwd ? 1 : -1) * (grow === "after" ? 1 : -1);
    size = clamp(size + sign * 16);
  }
</script>

<div
  class="resizer {orientation}"
  class:drag={drag !== null}
  role="slider"
  aria-orientation={vertical ? "vertical" : "horizontal"}
  aria-label={label || t("player.resize")}
  aria-valuenow={size}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
  ondblclick={() => (size = clamp(def))}
></div>

<style>
  .resizer {
    flex: none;
    background: var(--vf-border);
    touch-action: none;
    outline: none;
    transition: background var(--vf-transition);
  }
  .resizer.vertical {
    width: 5px;
    cursor: col-resize;
  }
  .resizer.horizontal {
    height: 5px;
    cursor: row-resize;
  }
  .resizer:hover,
  .resizer:focus-visible,
  .resizer.drag {
    background: var(--vf-accent);
  }
</style>
