<script lang="ts">
  import type { Snippet } from 'svelte';

  let {
    label,
    value,
    note,
    href,
    children,
  }: {
    label: string;
    value: string;
    note: string;
    href?: string;
    children?: Snippet;
  } = $props();
</script>

<svelte:element this={href ? 'a' : 'div'} {href} class="metric">
  <p class="label">
    {label}{#if href}<span aria-hidden="true">↗</span>{/if}
  </p>
  <p class="value">{value}</p>
  {#if children}<div class="visual">{@render children()}</div>{/if}
  <p class="note">{note}</p>
</svelte:element>

<style>
  .metric {
    min-width: 0;
    padding: 1.2rem;
    border: 1px solid var(--rule);
    border-radius: 6px;
    background: var(--bg-panel);
    color: var(--fg);
    display: flex;
    flex-direction: column;
    text-decoration: none;
  }
  a.metric:hover {
    border-color: var(--accent);
    background: var(--bg-hover);
  }
  .label {
    display: flex;
    justify-content: space-between;
    margin: 0;
    color: var(--fg-dim);
    font-size: 0.8rem;
  }
  .label span {
    color: var(--accent);
  }
  .value {
    margin: 0.6rem 0;
    font-size: clamp(1.55rem, 2.6vw, 2.2rem);
    line-height: 1.15;
    letter-spacing: -0.045em;
    font-weight: 550;
    overflow-wrap: anywhere;
  }
  .visual {
    margin: 0.15rem 0 0.55rem;
  }
  .note {
    margin: auto 0 0;
    color: var(--fg-dim);
    font-size: 0.75rem;
  }
  @media (max-width: 500px) {
    .metric {
      padding: 0.9rem;
    }
    .value {
      font-size: 1.65rem;
    }
  }
</style>
