<!-- Section repliable des réglages ; état ouvert/fermé mémorisé sur le poste. -->
<script lang="ts">
  import { untrack, type Snippet } from "svelte";
  import "./form.css";

  let { id, title, badge = "", children }: { id: string; title: string; badge?: string; children: Snippet } = $props();

  const key = () => `veriflow.transcode.section.${id}`;
  let open = $state(untrack(read));

  function read() {
    try {
      return localStorage.getItem(key()) !== "closed";
    } catch {
      return true;
    }
  }

  function toggle(e: Event) {
    open = (e.currentTarget as HTMLDetailsElement).open;
    try {
      localStorage.setItem(key(), open ? "open" : "closed");
    } catch {
      /* stockage indisponible */
    }
  }
</script>

<details class="tc-section" {open} ontoggle={toggle}>
  <summary>{title}{#if badge}<span class="tc-badge">{badge}</span>{/if}</summary>
  <div class="tc-body">{@render children()}</div>
</details>
