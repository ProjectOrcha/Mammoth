<!-- The four fast paths, as live numbers rather than as claims.
     Each tile shows what the cluster is actually doing and, under it, what the
     same operation costs when it is done the HDFS way — because the number only
     means something next to the thing it replaced.
     Design: web/src/content/docs/concepts/fast-paths.md -->
<script lang="ts">
  import type { ClusterReport } from '$lib/types';
  import { count, duration, ms, pct } from '$lib/format';

  interface Props {
    report: ClusterReport;
  }

  let { report }: Props = $props();

  const read = $derived(report.read_path);
  const write = $derived(report.write_path);
  const repair = $derived(report.repair);
  const start = $derived(report.start);

  const readTotal = $derived(read.lease_hits + read.resolve_hits + read.master_hits);

  const cards = $derived([
    {
      key: 'read',
      title: 'One-shot read',
      value: pct(read.lease_hits, readTotal, 1),
      unit: 'of reads cost 0 metadata round trips',
      hadoop: 'HDFS: 2 round trips, every open',
      detail: [
        ['lease hit · 0 RTT', count(read.lease_hits)],
        ['resolved at the worker · 1 RTT', count(read.resolve_hits)],
        ['reached a master', count(read.master_hits)],
        ['short-circuit (local fd)', count(read.short_circuit)],
        ['hedged at a second replica', count(read.hedged)],
        ['p50 · p99', `${ms(read.p50_ms)} · ${ms(read.p99_ms)}`],
      ],
    },
    {
      key: 'write',
      title: 'Fan-out dispersal write',
      value: `depth ${write.depth}`,
      unit: `${write.k}+${write.m} fragments, acked at ${write.quorum_at}`,
      hadoop: 'HDFS: depth 3, chained, serially acked',
      detail: [
        ['policy', write.ec_policy],
        ['mode', write.mode],
        ['fragments still landing after ack', String(write.trailing)],
        ['storage', `${write.storage_ratio.toFixed(2)}× (HDFS 3.00×)`],
        ['client uplink', `${write.uplink_ratio.toFixed(2)}× (HDFS 1.00×)`],
        ['p50 · p99', `${ms(write.p50_ms)} · ${ms(write.p99_ms)}`],
      ],
    },
    {
      key: 'repair',
      title: 'Declustered repair',
      value:
        repair.grace_remaining_s > 0
          ? 'holding'
          : repair.queued > 0
            ? `${repair.participating} of ${repair.node_count}`
            : 'idle',
      unit:
        repair.grace_remaining_s > 0
          ? `grace period — ${duration(repair.grace_remaining_s)} left`
          : repair.queued > 0
            ? 'nodes rebuilding, all at once'
            : 'every block at full redundancy',
      hadoop: 'HDFS: one source, one sink, per block',
      detail:
        repair.queued > 0
          ? ([
              ['blocks left', count(repair.queued)],
              ['of', count(repair.total)],
              ['rate', repair.blocks_per_sec ? `${count(repair.blocks_per_sec)} blk/s` : 'holding'],
              ['eta', repair.eta_s ? duration(repair.eta_s) : '—'],
              ['budget in use', `${repair.budget_pct}% of idle`],
              ['worst block', `${repair.worst_remaining} of ${repair.total_fragments} fragments`],
              ['cause', repair.cause ?? '—'],
            ] as [string, string][])
          : ([['queue', 'empty']] as [string, string][]),
    },
    {
      key: 'start',
      title: 'Warm start',
      value: duration(start.last_start_ms / 1000),
      unit: `to map ${count(start.blocks)} blocks back`,
      hadoop: `HDFS: ~${duration(start.rebuild_equivalent_ms / 1000)} rebuilding from reports`,
      detail: [
        ['block map', start.block_map],
        ['merkle roots matched first try', `${start.roots_matched} of ${start.roots_total}`],
        [
          'buckets streamed',
          `${start.buckets_streamed} of ${(start.merkle_fanout * start.roots_total).toLocaleString()}`,
        ],
        ['shards ready', `${start.shards.filter((s) => s.state === 'ready').length} of ${start.shards.length}`],
      ],
    },
  ]);

  let open = $state<string | null>(null);
</script>

<div class="grid">
  {#each cards as card (card.key)}
    <article class="card" class:open={open === card.key}>
      <button
        class="head"
        onclick={() => (open = open === card.key ? null : card.key)}
        aria-expanded={open === card.key}
      >
        <p class="eyebrow">{card.title}</p>
        <p class="value">{card.value}</p>
        <p class="unit">{card.unit}</p>
        <p class="hadoop">{card.hadoop}</p>
        <span class="chevron" aria-hidden="true">{open === card.key ? '−' : '+'}</span>
      </button>
      {#if open === card.key}
        <dl>
          {#each card.detail as [k, v] (k)}
            <div><dt>{k}</dt><dd class="mono">{v}</dd></div>
          {/each}
        </dl>
      {/if}
    </article>
  {/each}
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(15rem, 1fr));
    gap: var(--gap);
  }
  .card {
    background: var(--bg-panel);
    border: 1px solid var(--rule);
    display: flex;
    flex-direction: column;
  }
  .card.open {
    border-color: var(--rule-strong);
  }
  .head {
    position: relative;
    border: none;
    padding: 0.75rem 0.9rem 0.85rem;
    text-align: left;
    width: 100%;
  }
  .head:hover {
    background: var(--bg-hover);
  }
  .value {
    font-family: var(--font-display);
    font-size: 1.5rem;
    line-height: 1.1;
    margin: 0.35rem 0 0;
    color: var(--fg-display);
  }
  .unit {
    margin: 0.25rem 0 0;
    font-size: 0.74rem;
    color: var(--fg-dim);
    line-height: 1.35;
  }
  .hadoop {
    margin: 0.5rem 0 0;
    padding-top: 0.45rem;
    border-top: 1px dashed var(--rule);
    font-family: var(--font-mono);
    font-size: 0.66rem;
    color: var(--fg-faint);
  }
  .chevron {
    position: absolute;
    top: 0.6rem;
    right: 0.75rem;
    color: var(--fg-faint);
  }
  dl {
    margin: 0;
    padding: 0.25rem 0.9rem 0.85rem;
    border-top: 1px solid var(--rule);
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.28rem 0;
    font-size: 0.74rem;
  }
  dt {
    color: var(--fg-faint);
  }
  dd {
    margin: 0;
    color: var(--fg);
    text-align: right;
  }
</style>
