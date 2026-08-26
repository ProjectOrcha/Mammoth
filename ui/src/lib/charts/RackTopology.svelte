<!-- RackTopology: a force graph of racks and the nodes inside them. Node size
     is capacity, colour is health, and the rack-to-rack edges carry the
     cross-rack traffic that placement is trying to keep small. -->
<script lang="ts">
  import type { TopologyReport } from '$lib/types';
  import { bytes, pct, rate } from '$lib/format';
  import { chart, palette, tooltipStyle, type ChartOption } from './echarts';

  interface Props {
    topology: TopologyReport;
  }

  let { topology }: Props = $props();

  const option = $derived((): ChartOption => {
    const p = palette();
    const tone = {
      healthy: p.ok,
      warn: p.warn,
      decommissioning: p.info,
      maintenance: p.info,
      dead: p.danger,
    };

    const rackNodes = topology.racks.map((r) => ({
      id: r,
      name: r.split('/').pop() ?? r,
      category: 0,
      symbolSize: 34,
      itemStyle: { color: p.plate, borderColor: p.accent, borderWidth: 1 },
      label: { show: true, color: p.display, fontFamily: p.fontMono, fontSize: 10 },
      value: 0,
    }));

    const workers = topology.nodes.map((n) => ({
      id: n.id,
      name: n.id,
      category: 1,
      // Area, not radius, tracks capacity — a node with twice the disk should
      // not look four times the size.
      symbolSize: 10 + Math.sqrt(n.capacity / 1e12) * 1.9,
      itemStyle: { color: tone[n.state], opacity: n.state === 'dead' ? 0.35 : 0.9 },
      label: { show: true, color: p.dim, fontFamily: p.fontMono, fontSize: 9 },
      value: n.used,
      rack: n.rack,
      state: n.state,
      capacity: n.capacity,
    }));

    const links = [
      ...topology.nodes.map((n) => ({
        source: n.rack,
        target: n.id,
        lineStyle: { color: p.rule, width: 1, opacity: 0.5, curveness: 0 },
      })),
      ...topology.links.map((l) => ({
        source: l.source,
        target: l.target,
        value: l.bps,
        lineStyle: {
          color: p.accent,
          width: 1 + (l.bps / 2e9) * 4,
          opacity: 0.8,
          curveness: 0.18,
        },
        label: {
          show: true,
          formatter: rate(l.bps),
          color: p.faint,
          fontFamily: p.fontMono,
          fontSize: 9,
        },
      })),
    ];

    return {
      backgroundColor: 'transparent',
      tooltip: {
        ...tooltipStyle(p),
        formatter: (params: unknown) => {
          const q = params as {
            dataType: string;
            data: Record<string, unknown>;
            value?: number;
          };
          if (q.dataType === 'edge') {
            return q.data.value ? `cross-rack · ${rate(q.data.value as number)}` : 'rack member';
          }
          const d = q.data;
          if (d.category === 0) return `<b>${d.name}</b>`;
          return [
            `<b>${d.name}</b> · ${d.state}`,
            `${d.rack}`,
            `${bytes(d.value as number)} of ${bytes(d.capacity as number)} · ${pct(
              d.value as number,
              d.capacity as number,
            )}`,
          ].join('<br/>');
        },
      },
      series: [
        {
          type: 'graph',
          layout: 'force',
          roam: true,
          draggable: true,
          categories: [{ name: 'rack' }, { name: 'worker' }],
          force: { repulsion: 190, edgeLength: [40, 130], gravity: 0.12 },
          data: [...rackNodes, ...workers],
          links,
          emphasis: { focus: 'adjacency', scale: 1.1 },
          lineStyle: { color: p.rule },
        },
      ],
    };
  });
</script>

<div class="chart" use:chart={option}></div>
<p class="legend eyebrow">
  size = capacity · colour = health · edge weight = cross-rack traffic · drag to rearrange
</p>

<style>
  .chart {
    width: 100%;
    height: 21rem;
  }
  .legend {
    margin: 0.6rem 0 0;
  }
</style>
