<!-- ★ The visualization hub — six views, all live, plus a time machine.
     Hadoop's UI shows you tables of numbers. This shows you where your data
     actually is, and what moved it there. (Part VII §7.2) -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { live } from '$lib/live.svelte';
  import type {
    ClusterReport,
    FlowReport,
    HeatCell,
    HeatMetric,
    SkewReport,
    TopologyReport,
    TreemapNode,
  } from '$lib/types';
  import { bytes, count, duration, ms, pct, rate } from '$lib/format';
  import Panel from '$lib/components/Panel.svelte';
  import Meter from '$lib/components/Meter.svelte';
  import HeatGrid from '$lib/charts/HeatGrid.svelte';
  import Treemap from '$lib/charts/Treemap.svelte';
  import RackTopology from '$lib/charts/RackTopology.svelte';
  import SkewScatter from '$lib/charts/SkewScatter.svelte';
  import FlowSankey from '$lib/charts/FlowSankey.svelte';
  import BlockMatrix from '$lib/charts/BlockMatrix.svelte';

  const METRICS: { key: HeatMetric; label: string }[] = [
    { key: 'usage', label: 'usage' },
    { key: 'fragments', label: 'fragments' },
    { key: 'read_qps', label: 'read qps' },
    { key: 'write_qps', label: 'write qps' },
    { key: 'disk_p99_ms', label: 'disk p99' },
  ];

  const FILES = [
    '/data/nyc-taxi.parquet',
    '/data/sales-2026.csv',
    '/data/events.csv.gz',
    '/data/config.json',
  ];

  let metric = $state<HeatMetric>('usage');
  let minutesAgo = $state(0);
  let colourBy = $state<'age' | 'reads'>('age');
  let matrixPath = $state(FILES[0]);

  let heat = $state<HeatCell[] | null>(null);
  let flow = $state<FlowReport | null>(null);
  let topology = $state<TopologyReport | null>(null);
  let tree = $state<TreemapNode | null>(null);
  let skew = $state<SkewReport | null>(null);
  let past = $state<ClusterReport | null>(null);
  let matrix = $state<Awaited<ReturnType<typeof api.blocks>>>(null);

  const report = $derived(minutesAgo > 0 ? past : live.report);
  const travelling = $derived(minutesAgo > 0);

  // The three report-shaped views replay with the slider; the namespace views
  // (treemap, skew) do not, because the namespace is not what the incident
  // changed. The slider label says so rather than pretending otherwise.
  $effect(() => {
    const m = minutesAgo;
    const met = metric;
    void live.updatedAt;
    (async () => {
      const [h, f, t, p] = await Promise.all([
        api.heat(met, m),
        api.flow(m),
        api.topology(m),
        m > 0 ? api.reportAt(m) : Promise.resolve(null),
      ]);
      heat = h;
      flow = f;
      topology = t;
      past = p;
    })();
  });

  $effect(() => {
    const path = matrixPath;
    api.blocks(path).then((m) => (matrix = m));
  });

  // `onMount` can return a cleanup OR be async, never both — so attach
  // synchronously and kick the one-shot loads off beside it.
  onMount(() => {
    void Promise.all([api.treemap('/', 3), api.skew('/warehouse/events')]).then(([t, s]) => {
      tree = t;
      skew = s;
    });
    return live.attach();
  });

  const repair = $derived(report?.repair ?? null);
  const skewRatio = $derived(skew ? skew.max / skew.median : 0);
</script>

<header class="page">
  <h1>Distribution</h1>
  <p class="eyebrow">six views · where every byte actually is</p>
</header>

<section class="timemachine">
  <div class="tm-head">
    <p class="eyebrow">Time machine</p>
    <p class="tm-state mono" class:travelling>
      {travelling ? `viewing T−${duration(minutesAgo * 60)}` : 'now · live'}
    </p>
  </div>
  <input
    type="range"
    min="0"
    max="1440"
    step="1"
    bind:value={minutesAgo}
    aria-label="Minutes ago"
  />
  <div class="tm-foot">
    <span class="eyebrow">24h ago</span>
    <span class="eyebrow tm-note">
      replays the heat grid, topology, flow and repair — the namespace views stay live
    </span>
    <span class="eyebrow">now</span>
  </div>
  {#if travelling}
    <button class="reset" onclick={() => (minutesAgo = 0)}>back to now</button>
  {/if}
</section>

{#if repair}
  <section class="repair" data-active={repair.queued > 0}>
    <div class="repair-head">
      <p class="eyebrow">Declustered repair</p>
      <p class="mono">
        {#if repair.grace_remaining_s > 0}
          holding — {duration(repair.grace_remaining_s)} of the grace period left
        {:else if repair.queued > 0}
          {count(repair.queued)} of {count(repair.total)} blocks left ·
          {repair.participating} of {repair.node_count} nodes rebuilding ·
          {count(repair.blocks_per_sec)} blk/s · {rate(repair.bytes_per_sec)} · eta
          {duration(repair.eta_s)}
        {:else}
          every block at full redundancy
        {/if}
      </p>
    </div>
    {#if repair.queued > 0}
      <div class="fan">
        {#each Array(repair.node_count) as _, i (i)}
          <span class="node" class:on={i < repair.participating}></span>
        {/each}
      </div>
      {#if repair.total > 0}
        <div class="progress">
          <Meter
            value={repair.total - repair.queued}
            max={repair.total}
            tone="accent"
            height="0.3rem"
          />
          <span class="mono">{pct(repair.total - repair.queued, repair.total, 1)} rebuilt</span>
        </div>
      {/if}
      <p class="repair-note">
        {#if repair.grace_remaining_s > 0}
          Nothing is being copied yet. A node that is merely absent gets
          <code class="mono">repair.delay</code> — ten minutes — before the cluster
          spends a hundred terabytes of network on a machine that may just be
          rebooting. Confirmed disk loss skips the window.
        {:else}
          Every surviving node is both a source and a sink, so the rebuild scales with
          the cluster instead of with one disk. Rate is capped at {repair.budget_pct}% of
          measured idle bandwidth and yields to client traffic. Worst block is
          {repair.worst_remaining} of {repair.total_fragments} fragments.
          {#if repair.cause}Cause: {repair.cause}.{/if}
        {/if}
      </p>
    {/if}
  </section>
{/if}

<div class="grid">
  <Panel title="1 · Node heat grid" note={travelling ? `T−${minutesAgo}m` : 'live'} span={2}>
    {#snippet actions()}
      <div class="seg">
        {#each METRICS as m (m.key)}
          <button aria-pressed={metric === m.key} onclick={() => (metric = m.key)}>{m.label}</button>
        {/each}
      </div>
    {/snippet}
    {#if heat}
      <HeatGrid cells={heat} {metric} onselect={(n) => (window.location.href = `/nodes#${n}`)} />
    {:else}
      <p class="quiet mono">reading…</p>
    {/if}
  </Panel>

  <Panel title="2 · Block placement matrix" note={matrix?.policy ?? ''} span={2}>
    {#snippet actions()}
      <select bind:value={matrixPath} aria-label="File">
        {#each FILES as f (f)}<option value={f}>{f}</option>{/each}
      </select>
    {/snippet}
    {#if matrix?.inlined}
      <p class="quiet">
        Inlined — under the threshold, so it never became blocks. Nothing to place.
      </p>
    {:else if matrix}
      <BlockMatrix layout={matrix} maxRows={10} />
    {:else}
      <p class="quiet mono">reading…</p>
    {/if}
  </Panel>

  <Panel title="3 · Namespace treemap">
    {#snippet actions()}
      <div class="seg">
        <button aria-pressed={colourBy === 'age'} onclick={() => (colourBy = 'age')}>age</button>
        <button aria-pressed={colourBy === 'reads'} onclick={() => (colourBy = 'reads')}>reads</button>
      </div>
    {/snippet}
    {#if tree}
      <Treemap root={tree} {colourBy} />
    {:else}
      <p class="quiet mono">reading…</p>
    {/if}
  </Panel>

  <Panel title="4 · Rack topology" note={topology ? `epoch ${topology.epoch}` : ''}>
    {#if topology}
      <RackTopology {topology} />
    {:else}
      <p class="quiet mono">reading…</p>
    {/if}
  </Panel>

  <Panel
    title="5 · Skew scatter"
    note={skew ? `${skew.files.toLocaleString()} files · ${bytes(skew.total)}` : ''}
  >
    {#if skew}
      <SkewScatter report={skew} />
      <p class="finding" class:bad={skewRatio > 10}>
        median {bytes(skew.median)} · p99 {bytes(skew.p99)} · max {bytes(skew.max)}
        {#if skewRatio > 10}
          — severe skew, {skewRatio.toFixed(0)}× the median. One task processes
          {bytes(skew.max)} while the rest process about {bytes(skew.median)}, and that one
          task is your job's runtime.
        {/if}
      </p>
    {:else}
      <p class="quiet mono">reading…</p>
    {/if}
  </Panel>

  <Panel title="6 · Flow" note={flow ? `last ${flow.window_s}s` : ''}>
    {#if flow}
      <FlowSankey {flow} />
      {#if report}
        <p class="finding">
          cross-rack {rate(report.throughput.cross_rack_bps)} of
          {rate(report.throughput.cross_rack_capacity)} ·
          {pct(report.throughput.cross_rack_bps, report.throughput.cross_rack_capacity)} of the link
        </p>
        <Meter
          value={report.throughput.cross_rack_bps}
          max={report.throughput.cross_rack_capacity}
          height="0.35rem"
        />
      {/if}
    {:else}
      <p class="quiet mono">reading…</p>
    {/if}
  </Panel>
</div>

{#if report}
  <Panel title="Read path" note="what the one-shot read is actually doing">
    <div class="readbar">
      {#each [{ k: 'lease hit · 0 RTT', v: report.read_path.lease_hits, tone: 'ok' }, { k: 'worker resolve · 1 RTT', v: report.read_path.resolve_hits, tone: 'accent' }, { k: 'reached a master', v: report.read_path.master_hits, tone: 'warn' }] as seg (seg.k)}
        {@const total =
          report.read_path.lease_hits + report.read_path.resolve_hits + report.read_path.master_hits}
        <div class="seg-row">
          <span class="k">{seg.k}</span>
          <Meter value={(seg.v / total) * 100} tone={seg.tone as 'ok'} height="0.4rem" />
          <span class="v mono">{count(seg.v)} · {pct(seg.v, total, 1)}</span>
        </div>
      {/each}
    </div>
    <p class="finding">
      p50 {ms(report.read_path.p50_ms)} · p99 {ms(report.read_path.p99_ms)} ·
      {count(report.read_path.short_circuit)} served over a passed file descriptor ·
      {count(report.read_path.hedged)} raced a second replica. HDFS would have spent two
      round trips on every one of these.
    </p>
  </Panel>
{/if}

<style>
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }

  .timemachine {
    background: var(--bg-panel);
    border: 1px solid var(--rule);
    padding: 0.7rem 0.9rem 0.8rem;
    margin-bottom: var(--gap);
    position: relative;
  }
  .tm-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    margin-bottom: 0.4rem;
  }
  .tm-state {
    margin: 0;
    font-size: 0.72rem;
    color: var(--fg-faint);
  }
  .tm-state.travelling {
    color: var(--warn);
  }
  .timemachine input[type='range'] {
    width: 100%;
    direction: rtl;
    accent-color: var(--accent);
  }
  .tm-foot {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    margin-top: 0.2rem;
  }
  .tm-note {
    text-align: center;
    letter-spacing: 0.1em;
  }
  .reset {
    position: absolute;
    top: 0.55rem;
    right: 0.9rem;
    font-size: 0.68rem;
  }

  .repair {
    border: 1px solid var(--rule);
    background: var(--bg-panel);
    padding: 0.7rem 0.9rem 0.8rem;
    margin-bottom: var(--gap);
  }
  .repair[data-active='true'] {
    border-color: var(--warn);
  }
  .repair-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .repair-head p.mono {
    margin: 0;
    font-size: 0.72rem;
    color: var(--warn);
  }
  .fan {
    display: flex;
    gap: 0.25rem;
    margin: 0.6rem 0 0;
  }
  .progress {
    display: grid;
    grid-template-columns: 1fr auto;
    align-items: center;
    gap: 0.7rem;
    margin-top: 0.5rem;
  }
  .progress span {
    font-size: 0.68rem;
    color: var(--fg-faint);
  }
  .fan .node {
    height: 0.5rem;
    flex: 1;
    background: var(--track);
  }
  .fan .node.on {
    background: var(--warn);
    animation: flicker 1.6s ease-in-out infinite;
  }
  .fan .node.on:nth-child(2n) {
    animation-delay: 0.4s;
  }
  .fan .node.on:nth-child(3n) {
    animation-delay: 0.8s;
  }
  @keyframes flicker {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.45; }
  }
  .repair-note {
    margin: 0.6rem 0 0;
    font-size: 0.73rem;
    color: var(--fg-faint);
    line-height: 1.55;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: var(--gap);
    margin-bottom: var(--gap);
  }
  @media (max-width: 1100px) {
    .grid {
      grid-template-columns: 1fr;
    }
    .grid :global(section) {
      grid-column: span 1 !important;
    }
  }

  .seg {
    display: flex;
    gap: 0;
  }
  .seg button {
    font-size: 0.66rem;
    letter-spacing: 0.06em;
    padding: 0.18rem 0.45rem;
    border-right-width: 0;
  }
  .seg button:last-child {
    border-right-width: 1px;
  }

  .readbar {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
  }
  .seg-row {
    display: grid;
    grid-template-columns: 11rem 1fr 9rem;
    align-items: center;
    gap: 0.7rem;
    font-size: 0.74rem;
  }
  .seg-row .k {
    color: var(--fg-dim);
  }
  .seg-row .v {
    text-align: right;
    color: var(--fg-faint);
    font-size: 0.72rem;
  }

  .finding {
    margin: 0.7rem 0 0;
    font-size: 0.73rem;
    color: var(--fg-faint);
    line-height: 1.55;
  }
  .finding.bad {
    color: var(--warn);
  }
  .quiet {
    color: var(--fg-faint);
    margin: 0;
  }
</style>
