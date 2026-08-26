<!-- A bordered plate with a letterspaced eyebrow. The unit of layout on every
     page, so spacing and rules are decided once here. -->
<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    note?: string;
    /** Right-aligned controls in the header rule. */
    actions?: Snippet;
    children: Snippet;
    /** Let the body scroll instead of stretching the page. */
    scroll?: boolean;
    span?: number;
  }

  let { title, note, actions, children, scroll = false, span }: Props = $props();
</script>

<section class="panel" style={span ? `grid-column: span ${span}` : undefined}>
  <header>
    <p class="eyebrow">{title}</p>
    {#if actions}
      <div class="actions">{@render actions()}</div>
    {:else if note}
      <p class="note mono">{note}</p>
    {/if}
  </header>
  <div class="body" class:scroll>
    {@render children()}
  </div>
</section>

<style>
  .panel {
    background: var(--bg-panel);
    border: 1px solid var(--rule);
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--rule);
    min-height: 2.35rem;
  }
  .note {
    margin: 0;
    color: var(--fg-faint);
    font-size: 0.7rem;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
  }
  .body {
    padding: 0.9rem;
    min-width: 0;
    flex: 1;
  }
  .body.scroll {
    overflow: auto;
    max-height: 24rem;
  }
</style>
