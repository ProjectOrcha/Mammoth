<!-- The state glyph, matching `mammoth viz`: filled for healthy, hollow for
     degraded, a cross for dead. Colour AND shape, so it survives a colour-blind
     reader and a black-and-white screenshot. -->
<script lang="ts">
  import type { NodeState } from '$lib/types';

  interface Props {
    state: NodeState;
    label?: boolean;
  }

  let { state, label = true }: Props = $props();

  const GLYPH: Record<NodeState, string> = {
    healthy: '●',
    warn: '◐',
    decommissioning: '◔',
    maintenance: '◌',
    dead: '✕',
  };
</script>

<span class="state" data-state={state}>
  <span class="glyph" aria-hidden="true">{GLYPH[state]}</span>
  {#if label}{state}{/if}
</span>

<style>
  .state {
    display: inline-flex;
    align-items: baseline;
    gap: 0.35rem;
    white-space: nowrap;
  }
  .glyph {
    font-size: 0.8em;
  }
  [data-state='healthy'] {
    color: var(--ok);
  }
  [data-state='warn'] {
    color: var(--warn);
  }
  [data-state='decommissioning'],
  [data-state='maintenance'] {
    color: var(--info);
  }
  [data-state='dead'] {
    color: var(--danger);
  }
</style>
