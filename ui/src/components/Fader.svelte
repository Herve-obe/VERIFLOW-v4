<!-- Fader vertical (gain en dB) : glisser, molette, flèches du clavier, double-clic = 0 dB.
     Composant maison pour un rendu identique sur Windows, macOS et Linux. -->
<script lang="ts">
  let {
    value = $bindable(0),
    min = -60,
    max = 12,
    step = 0.5,
    label = "",
    onchange,
  }: {
    value?: number;
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    onchange?: (v: number) => void;
  } = $props();

  let track = $state<HTMLDivElement | null>(null);
  const ratio = $derived((value - min) / (max - min));

  function set(v: number) {
    const snapped = Math.round(Math.min(max, Math.max(min, v)) / step) * step;
    if (snapped !== value) {
      value = snapped;
      onchange?.(snapped);
    }
  }

  function fromPointer(e: PointerEvent) {
    if (!track) return;
    const r = track.getBoundingClientRect();
    set(min + (1 - (e.clientY - r.top) / r.height) * (max - min));
  }

  function onKeydown(e: KeyboardEvent) {
    const delta = { ArrowUp: step, ArrowDown: -step, PageUp: 6, PageDown: -6 }[e.key];
    if (delta === undefined) return;
    e.preventDefault();
    e.stopPropagation(); // ne pas déclencher les raccourcis de lecture
    set(value + delta);
  }
</script>

<div
  class="fader"
  role="slider"
  tabindex="0"
  aria-label={label}
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  bind:this={track}
  onpointerdown={(e) => {
    e.currentTarget.setPointerCapture(e.pointerId);
    fromPointer(e);
  }}
  onpointermove={(e) => e.buttons === 1 && fromPointer(e)}
  onwheel={(e) => {
    e.preventDefault();
    set(value + (e.deltaY < 0 ? step : -step) * 2);
  }}
  ondblclick={() => set(0)}
  onkeydown={onKeydown}
>
  <div class="rail"></div>
  <div class="zero" style:bottom={`${((0 - min) / (max - min)) * 100}%`}></div>
  <div class="thumb" style:bottom={`calc(${ratio * 100}% - 7px)`}></div>
</div>

<style>
  .fader {
    position: relative;
    width: 20px;
    height: var(--vf-meter-height);
    cursor: ns-resize;
    touch-action: none;
  }
  .rail {
    position: absolute;
    left: 50%;
    top: 0;
    bottom: 0;
    width: 4px;
    transform: translateX(-50%);
    background: var(--vf-meter-bg);
    border-radius: 2px;
  }
  .zero {
    position: absolute;
    left: 2px;
    right: 2px;
    height: 1px;
    background: var(--vf-text-disabled);
  }
  .thumb {
    position: absolute;
    left: 0;
    width: 20px;
    height: 14px;
    border-radius: var(--vf-radius-sm);
    background: var(--vf-text);
    border: 1px solid var(--vf-border);
    box-shadow: inset 0 -2px 0 var(--vf-accent);
  }
  .fader:focus-visible .thumb {
    outline: 2px solid var(--vf-accent);
  }
</style>
