<!-- Logo provisoire : monogramme V + coche + forme d'onde (charte §4). -->
<script lang="ts">
  let { size = 24 }: { size?: number } = $props();
</script>

<svg width={size} height={size} viewBox="0 0 64 64" aria-hidden="true">
  <rect x="2" y="2" width="60" height="60" rx="14" fill="var(--vf-surface-high)" stroke="var(--vf-border)" stroke-width="2" />
  <path d="M14 20 L28 46 L50 16" fill="none" stroke="var(--vf-accent)" stroke-width="6" stroke-linecap="round" stroke-linejoin="round" />
  <path d="M34 50 v-6 M40 50 v-12 M46 50 v-8 M52 50 v-4" stroke="var(--vf-text-muted)" stroke-width="3" stroke-linecap="round" />
</svg>
