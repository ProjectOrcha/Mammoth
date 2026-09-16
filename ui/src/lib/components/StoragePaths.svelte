<script lang="ts">
  import type { ClusterReport } from '$lib/types';
  import { count } from '$lib/format';
  let { report }: { report: ClusterReport } = $props();
  const blocks = $derived(Object.values(report.health).reduce((a, b) => a + b, 0));
  const copies = $derived(report.nodes.reduce((a, n) => a + n.fragments, 0));
  const cards = $derived([
    { title: 'Verified reads', value: 'Checksummed', note: 'Every returned chunk is checked before delivery.', steps: ['Resolve the file in the namespace', 'Stream the requested bytes and verify each checksum chunk', 'Try another copy if the first is damaged'], href: '/files', action: 'Inspect a file' },
    { title: 'Replicated writes', value: `${count(copies)} copies`, note: `${count(blocks)} blocks across ${report.nodes.length} worker directories.`, steps: ['Split large files into blocks', 'Choose workers with rack-aware placement', 'Write replicas concurrently, then commit the file once they are durable'], href: '/distribution', action: 'Explore placement' },
    { title: 'Replica repair', value: `${count(report.health.healthy)} healthy`, note: 'Restore damaged copies from a verified replica.', steps: ['Check expected copies against stored blocks', 'Find a healthy source for each damaged copy', 'Restore missing or corrupt replicas'], href: '/cluster', action: 'Open maintenance' },
    { title: 'Persistent namespace', value: 'Atomic', note: 'File names and metadata survive a restart.', steps: ['Find names through an indexed metadata database', 'Commit changed records to a durable transaction log', 'Recover committed transactions on startup'], href: '/cluster', action: 'Inspect storage' },
  ]);
</script>

<div class="paths">
  {#each cards as card (card.title)}
    <details>
      <summary><span class="eyebrow">{card.title}</span><strong>{card.value}</strong><span class="note">{card.note}</span><span class="expand">How it works <span aria-hidden="true">+</span></span></summary>
      <ol>{#each card.steps as step}<li>{step}</li>{/each}</ol>
      <a href={card.href}>{card.action} →</a>
    </details>
  {/each}
</div>

<style>
  .paths { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: var(--gap); }
  details { min-width: 0; border: 1px solid var(--rule); background: var(--bg-panel); border-radius: .4rem; overflow: hidden; }
  summary { display: grid; gap: .65rem; padding: 1.1rem; cursor: pointer; list-style: none; }
  summary::-webkit-details-marker { display: none; }
  summary:hover { background: var(--bg-hover); }
  summary:focus-visible { outline: 2px solid var(--accent); outline-offset: -3px; }
  strong { font-size: 1.65rem; font-weight: 500; color: var(--fg); }
  .note { font-size: .8rem; color: var(--fg-dim); min-height: 2.5rem; }
  .expand { display: flex; justify-content: space-between; font-size: .75rem; color: var(--accent); padding-top: .65rem; border-top: 1px solid var(--rule); }
  details[open] .expand span { transform: rotate(45deg); }
  ol { padding: 0 1rem 0 2.2rem; color: var(--fg-dim); font-size: .8rem; }
  li { padding: .3rem 0; }
  a { display: block; padding: .5rem 1.1rem 1rem; font-size: .8rem; }
  @media (max-width: 1350px) { .paths { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
  @media (max-width: 600px) { .paths { grid-template-columns: 1fr; } }
</style>
