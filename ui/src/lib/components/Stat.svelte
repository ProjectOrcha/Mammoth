<!-- One number, said once, large. The label is the micro-label; the note under
     it is where the "compared to what" goes. -->
<script lang="ts">
  interface Props {
    label: string;
    value: string;
    note?: string;
    tone?: 'default' | 'ok' | 'warn' | 'danger' | 'accent';
    href?: string;
  }

  let { label, value, note, tone = 'default', href }: Props = $props();
</script>

<svelte:element this={href ? 'a' : 'div'} {href} class="stat" data-tone={tone}>
  <p class="eyebrow">{label}</p>
  <p class="value">{value}</p>
  {#if note}<p class="note">{note}</p>{/if}
</svelte:element>

<style>
  .stat {
    display: block;
    background: var(--bg-panel);
    border: 1px solid var(--rule);
    padding: 0.75rem 0.9rem 0.85rem;
    min-width: 0;
    color: inherit;
    text-decoration: none;
  }
  a.stat:hover {
    background: var(--bg-hover);
    border-color: var(--rule-strong);
    text-decoration: none;
  }
  .value {
    font-family: var(--font-display);
    font-size: 1.6rem;
    line-height: 1.1;
    letter-spacing: 0.02em;
    margin: 0.35rem 0 0;
    color: var(--fg);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  [data-tone='ok'] .value {
    color: var(--ok);
  }
  [data-tone='warn'] .value {
    color: var(--warn);
  }
  [data-tone='danger'] .value {
    color: var(--danger);
  }
  [data-tone='accent'] .value {
    color: var(--fg-display);
  }
  .note {
    margin: 0.3rem 0 0;
    font-size: 0.72rem;
    color: var(--fg-faint);
    line-height: 1.35;
  }
</style>
