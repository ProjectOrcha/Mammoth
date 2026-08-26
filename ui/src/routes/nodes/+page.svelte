<!-- Node grid — the table, plus per-node volumes, disk latency and block count.
     Sortable, filterable, and grouped by rack when you ask for it, because
     "which rack is the problem" is the question this page exists to answer. -->
<script lang="ts">
  import { live } from '$lib/live.svelte';
  import { bytes, count, pct, pctValue, rate } from '$lib/format';
  import type { NodeReport } from '$lib/types';
  import Panel from '$lib/components/Panel.svelte';
  import Meter from '$lib/components/Meter.svelte';
  import StateDot from '$lib/components/StateDot.svelte';
  import Sparkline from '$lib/components/Sparkline.svelte';

  type Key = 'id' | 'rack' | 'state' | 'usage' | 'fragments' | 'read_bps' | 'write_bps' | 'disk_p99_ms';

  let sort = $state<Key>('usage');
  let desc = $state(true);
  let query = $state('');
  let groupByRack = $state(true);
  let selected = $state<string | null>(null);

  const report = $derived(live.report);

  function value(n: NodeReport, k: Key): number | string {
    return k === 'usage' ? n.used / n.capacity : n[k];
  }

  const rows = $derived.by(() => {
    const ns = report?.nodes ?? [];
    const q = query.trim().toLowerCase();
    const filtered = q
      ? ns.filter(
          (n) =>
            n.id.includes(q) ||
            n.rack.toLowerCase().includes(q) ||
            n.state.includes(q) ||
            n.address.includes(q),
        )
      : ns;
    return [...filtered].sort((a, b) => {
      const x = value(a, sort);
      const y = value(b, sort);
      const cmp = typeof x === 'number' && typeof y === 'number' ? x - y : String(x).localeCompare(String(y));
      return desc ? -cmp : cmp;
    });
  });

  const grouped = $derived.by(() => {
    if (!groupByRack) return [{ rack: null as string | null, nodes: rows }];
    const map = new Map<string, NodeReport[]>();
    for (const n of rows) {
      if (!map.has(n.rack)) map.set(n.rack, []);
      map.get(n.rack)!.push(n);
    }
    return [...map.entries()]
      .sort((a, b) => a[0].localeCompare(b[0]))
      .map(([rack, nodes]) => ({ rack, nodes }));
  });

  const detail = $derived(report?.nodes.find((n) => n.id === selected) ?? null);

  function head(k: Key) {
    if (sort === k) desc = !desc;
    else {
      sort = k;
      desc = k !== 'id' && k !== 'rack';
    }
  }

  const COLUMNS: { key: Key; label: string; num?: boolean }[] = [
    { key: 'id', label: 'node' },
    { key: 'rack', label: 'rack' },
    { key: 'state', label: 'state' },
    { key: 'usage', label: 'used' },
    { key: 'fragments', label: 'fragments', num: true },
    { key: 'read_bps', label: 'read', num: true },
    { key: 'write_bps', label: 'write', num: true },
    { key: 'disk_p99_ms', label: 'disk p99', num: true },
  ];
</script>

<header class="page">
  <h1>Nodes</h1>
  <p class="eyebrow">
    {#if report}
      {report.nodes.length} workers · {new Set(report.nodes.map((n) => n.rack)).size} racks · epoch
      {report.topology_epoch}
    {:else}loading{/if}
  </p>
</header>

<Panel title="Workers">
  {#snippet actions()}
    <input type="search" placeholder="filter…" bind:value={query} aria-label="Filter nodes" />
    <button aria-pressed={groupByRack} onclick={() => (groupByRack = !groupByRack)}>by rack</button>
  {/snippet}

  {#if !report}
    <p class="quiet mono">reading…</p>
  {:else}
    <table>
      <thead>
        <tr>
          {#each COLUMNS as c (c.key)}
            <th class:num={c.num}>
              <button class="sorter" onclick={() => head(c.key)}>
                {c.label}{#if sort === c.key}<span aria-hidden="true">{desc ? ' ↓' : ' ↑'}</span>{/if}
              </button>
            </th>
          {/each}
          <th>trend</th>
        </tr>
      </thead>
      {#each grouped as group (group.rack ?? 'all')}
        <tbody>
          {#if group.rack}
            {@const used = group.nodes.reduce((a, n) => a + n.used, 0)}
            {@const cap = group.nodes.reduce((a, n) => a + n.capacity, 0)}
            <tr class="rackrow">
              <td colspan="9">
                <span class="mono rack">{group.rack}</span>
                <span class="mono dim">
                  {bytes(used)} / {bytes(cap)} · {pct(used, cap)} · {group.nodes.length} nodes
                </span>
              </td>
            </tr>
          {/if}
          {#each group.nodes as n (n.id)}
            <tr
              id={n.id}
              class:selected={selected === n.id}
              onclick={() => (selected = selected === n.id ? null : n.id)}
            >
              <td><span class="mono link">{n.id}</span></td>
              <td class="mono dim">{n.rack}</td>
              <td><StateDot state={n.state} /></td>
              <td style="width: 10rem">
                <div class="usecell">
                  <Meter value={pctValue(n.used, n.capacity)} height="0.35rem" />
                  <span class="mono">{pct(n.used, n.capacity)}</span>
                </div>
              </td>
              <td class="num mono">{count(n.fragments)}</td>
              <td class="num mono">{rate(n.read_bps)}</td>
              <td class="num mono">{rate(n.write_bps)}</td>
              <td class="num mono" class:bad={n.disk_p99_ms > 100}>
                {n.disk_p99_ms ? `${n.disk_p99_ms} ms` : '—'}
              </td>
              <td><Sparkline points={n.read_series} /></td>
            </tr>
          {/each}
        </tbody>
      {/each}
    </table>
  {/if}
</Panel>

{#if detail}
  <div class="detail">
    <Panel title={`Node ${detail.id}`} note={detail.address}>
      <dl>
        <div><dt>state</dt><dd><StateDot state={detail.state} /></dd></div>
        {#if detail.note}<div><dt>note</dt><dd class="warnnote">{detail.note}</dd></div>{/if}
        <div><dt>rack</dt><dd class="mono">{detail.rack}</dd></div>
        <div><dt>used</dt><dd class="mono">{bytes(detail.used)} of {bytes(detail.capacity)}</dd></div>
        <div>
          <dt>fragments</dt>
          <dd class="mono">{detail.fragments.toLocaleString()}</dd>
        </div>
        <div><dt>volumes</dt><dd class="mono">{detail.volumes}</dd></div>
        <div><dt>disk p99</dt><dd class="mono">{detail.disk_p99_ms} ms</dd></div>
        <div><dt>read · write</dt><dd class="mono">{rate(detail.read_bps)} · {rate(detail.write_bps)}</dd></div>
      </dl>

      <p class="hint">
        Every fragment this node holds is also derivable without it: placement is a
        function of the block id, so a decommission is a diff, not a lookup.
      </p>

      <div class="cmds">
        <p class="eyebrow">commands</p>
        <code class="mono">mammoth doctor --node {detail.id}</code>
        <code class="mono">mammoth admin decommission {detail.id}</code>
        <code class="mono">mammoth viz cluster</code>
      </div>
    </Panel>
  </div>
{/if}

<style>
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }
  .sorter {
    border: none;
    padding: 0;
    font: inherit;
    letter-spacing: inherit;
    text-transform: inherit;
    color: inherit;
  }
  .sorter:hover {
    color: var(--accent);
    background: none;
  }
  th.num .sorter {
    float: right;
  }
  .rackrow td {
    background: var(--bg-plate);
    padding: 0.35rem 0.6rem;
  }
  .rack {
    color: var(--fg-display);
    margin-right: 0.75rem;
    font-size: 0.72rem;
  }
  tbody tr:not(.rackrow) {
    cursor: pointer;
  }
  tr.selected td {
    background: var(--bg-hover);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .link {
    color: var(--accent);
  }
  .usecell {
    display: grid;
    grid-template-columns: 1fr 2.6rem;
    align-items: center;
    gap: 0.5rem;
  }
  .usecell span {
    font-size: 0.72rem;
    color: var(--fg-dim);
  }
  .dim {
    color: var(--fg-faint);
    font-size: 0.72rem;
  }
  .bad {
    color: var(--danger);
  }
  .quiet {
    color: var(--fg-faint);
    margin: 0;
  }
  .detail {
    margin-top: var(--gap);
    max-width: 34rem;
  }
  dl {
    margin: 0;
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.3rem 0;
    border-bottom: 1px solid var(--rule);
    font-size: 0.78rem;
  }
  dt {
    color: var(--fg-faint);
  }
  dd {
    margin: 0;
    text-align: right;
  }
  .warnnote {
    color: var(--warn);
  }
  .hint {
    font-size: 0.74rem;
    color: var(--fg-faint);
    line-height: 1.5;
    margin: 0.9rem 0 0;
  }
  .cmds {
    margin-top: 1rem;
    padding-top: 0.8rem;
    border-top: 1px solid var(--rule);
  }
  .cmds code {
    display: block;
    color: var(--accent);
    font-size: 0.72rem;
    padding: 0.15rem 0;
  }
</style>
