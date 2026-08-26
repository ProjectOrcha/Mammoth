<!-- FlowSankey: bytes moving from each source category to the nodes receiving
     them, over the last minute.
     The shape of the repair band is the point: declustered repair should fan
     out to every healthy node at once. A narrow repair band means placement is
     not spreading the loss, and the rebuild will take hours. -->
<script lang="ts">
  import type { FlowReport } from '$lib/types';
  import { rate } from '$lib/format';
  import { chart, palette, tooltipStyle, type ChartOption } from './echarts';
  import { FLOW_COLOUR } from './colors';

  interface Props {
    flow: FlowReport;
  }

  let { flow }: Props = $props();

  const option = $derived((): ChartOption => {
    const p = palette();
    const sources = [...new Set(flow.links.map((l) => l.source))];
    const targets = [...new Set(flow.links.map((l) => l.target))];

    return {
      backgroundColor: 'transparent',
      tooltip: {
        ...tooltipStyle(p),
        formatter: (params: unknown) => {
          const q = params as { dataType: string; data: { source?: string; target?: string; value?: number; name?: string } };
          if (q.dataType === 'edge') return `${q.data.source} → ${q.data.target}<br/>${rate(q.data.value ?? 0)}`;
          return q.data.name ?? '';
        },
      },
      series: [
        {
          type: 'sankey',
          left: 8,
          right: 60,
          top: 10,
          bottom: 10,
          nodeWidth: 10,
          nodeGap: 7,
          emphasis: { focus: 'adjacency' },
          data: [
            ...sources.map((s) => ({
              name: s,
              itemStyle: { color: FLOW_COLOUR[s] ?? p.accent, borderColor: 'transparent' },
              label: { color: p.fg, fontFamily: p.fontMono, fontSize: 11 },
            })),
            ...targets.map((t) => ({
              name: t,
              itemStyle: { color: p.rule, borderColor: 'transparent' },
              label: { color: p.dim, fontFamily: p.fontMono, fontSize: 10 },
            })),
          ],
          links: flow.links.map((l) => ({
            source: l.source,
            target: l.target,
            value: Math.max(l.bps, 1),
            lineStyle: { color: FLOW_COLOUR[l.source] ?? p.accent, opacity: 0.32 },
          })),
        },
      ],
    };
  });

  const totals = $derived.by(() => {
    const m = new Map<string, number>();
    for (const l of flow.links) m.set(l.source, (m.get(l.source) ?? 0) + l.bps);
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  });
</script>

<div class="chart" use:chart={option}></div>
<ul class="totals">
  {#each totals as [source, bps] (source)}
    <li>
      <span class="dot" style="background: {FLOW_COLOUR[source] ?? 'var(--accent)'}"></span>{source}
      <b class="mono">{rate(bps)}</b>
    </li>
  {/each}
</ul>

<style>
  .chart {
    width: 100%;
    height: 21rem;
  }
  .totals {
    display: flex;
    flex-wrap: wrap;
    gap: 1.1rem;
    list-style: none;
    margin: 0.7rem 0 0;
    padding: 0;
    font-size: 0.72rem;
    color: var(--fg-dim);
  }
  .totals li {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
  }
  .dot {
    width: 0.5rem;
    height: 0.5rem;
    display: inline-block;
  }
</style>
