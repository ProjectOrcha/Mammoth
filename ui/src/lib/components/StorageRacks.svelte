<script lang="ts">
  import type { ClusterReport } from '$lib/types';
  import { bytes, count, pctValue } from '$lib/format';
  import Meter from './Meter.svelte';
  import StateDot from './StateDot.svelte';
  let { report }: { report: ClusterReport } = $props();
  const racks = $derived([...new Set(report.nodes.map(node => node.rack))].sort());
</script>
<div class="racks">
  {#each racks as rack (rack)}
    {@const nodes = report.nodes.filter(node => node.rack === rack)}
    <article>
      <header><strong>{rack}</strong><span>{nodes.length} workers</span></header>
      {#each nodes as node (node.id)}
        <a class="worker" href={`/nodes#${encodeURIComponent(node.id)}`}>
          <div><strong>{node.id} ↗</strong><StateDot state={node.state} /></div>
          <Meter value={pctValue(node.used, node.capacity)} height=".35rem" />
          <span>{bytes(node.used)} · {count(node.fragments)} {node.fragments === 1 ? 'copy' : 'copies'}</span>
        </a>
      {/each}
      <footer>{bytes(nodes.reduce((sum, node) => sum + node.used, 0))} stored</footer>
    </article>
  {/each}
</div>
<style>
  .racks { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(15rem, 100%), 1fr)); gap: 1rem; }
  article { border: 1px solid var(--rule); border-radius: .35rem; overflow: hidden; }
  header, footer { padding: .8rem; background: var(--bg-plate); }
  header { display: flex; gap: .5rem; flex-wrap: wrap; justify-content: space-between; }
  header strong { overflow-wrap: anywhere; }
  header span, footer, .worker > span { font-size: .75rem; color: var(--fg-dim); }
  .worker { display: grid; gap: .6rem; padding: .9rem; border-bottom: 1px solid var(--rule); color: var(--fg); }
  .worker:hover { background: var(--bg-hover); text-decoration: none; }
  .worker > div { display: flex; justify-content: space-between; gap: .5rem; }
</style>
