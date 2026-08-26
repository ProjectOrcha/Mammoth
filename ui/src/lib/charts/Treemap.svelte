<!-- Treemap: area is bytes, colour is age or read heat. Click to zoom into a
     directory. Answers "what is eating my disk" in one glance, which is the
     question a table of sizes never quite manages. -->
<script lang="ts">
  import type { TreemapNode } from '$lib/types';
  import { bytes, count } from '$lib/format';
  import { chart, palette, tooltipStyle, type ChartOption } from './echarts';

  interface Props {
    root: TreemapNode;
    colourBy?: 'age' | 'reads';
  }

  let { root, colourBy = 'age' }: Props = $props();

  interface Datum {
    name: string;
    path: string;
    value: number;
    heat: number;
    age: number;
    reads: number;
    children?: Datum[];
    itemStyle?: { color: string };
    label?: { color: string };
  }

  /** Same steel → gold → red ramp as the heat grid, so the two pages mean the
   *  same thing by the same colour. */
  function ramp(t: number): [number, number, number] {
    const a: [number, number, number] = [22, 73, 130];
    const b: [number, number, number] = [219, 197, 96];
    const c: [number, number, number] = [226, 86, 77];
    const lo = t < 0.5 ? a : b;
    const hi = t < 0.5 ? b : c;
    const f = t < 0.5 ? t * 2 : (t - 0.5) * 2;
    return lo.map((v, i) => Math.round(v + (hi[i] - v) * f)) as [number, number, number];
  }

  /** White is illegible on the gold middle of the ramp, so the label colour has
   *  to follow each tile's luminance. WCAG relative luminance. */
  function ink(rgb: [number, number, number]): string {
    const lin = rgb.map((v) => {
      const u = v / 255;
      return u <= 0.04045 ? u / 12.92 : ((u + 0.055) / 1.055) ** 2.4;
    });
    const L = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
    return L > 0.42 ? '#05111d' : '#fefefe';
  }

  function build(n: TreemapNode): Datum {
    // Old and cold both read as "you could move this"; hot and new read as
    // "leave it alone".
    const heat = colourBy === 'age' ? Math.min(1, n.age_days / 120) : Math.min(1, n.reads / 90_000);
    const rgb = ramp(heat);
    const fg = ink(rgb);
    return {
      name: n.name,
      path: n.path,
      value: n.value,
      heat,
      age: n.age_days,
      reads: n.reads,
      itemStyle: { color: `rgb(${rgb.join(', ')})` },
      label: { color: fg },
      children: n.children?.map(build),
    };
  }

  const option = $derived((): ChartOption => {
    const p = palette();
    const built = build(root);
    const data = built.children ?? [built];
    return {
      backgroundColor: 'transparent',
      tooltip: {
        ...tooltipStyle(p),
        formatter: (params: unknown) => {
          const d = (params as { data: Datum }).data;
          return [
            `<b>${d.path || '/'}</b>`,
            `${bytes(d.value)}`,
            `${d.age} days old · ${count(d.reads)} reads`,
          ].join('<br/>');
        },
      },
      series: [
        {
          type: 'treemap',
          data,
          roam: false,
          // One level at a time, click to descend. Drawing the whole tree at
          // once in a panel this size stacks a parent's header label on top of
          // its children's, and neither ends up readable.
          leafDepth: 1,
          nodeClick: 'zoomToNode',
          breadcrumb: {
            show: true,
            height: 20,
            itemStyle: {
              color: p.plate,
              borderColor: p.rule,
              textStyle: { color: p.dim, fontFamily: p.fontMono, fontSize: 10 },
            },
          },
          label: {
            show: true,
            fontFamily: p.fontMono,
            fontSize: 11,
            formatter: (params: unknown) => {
              const d = (params as { data: Datum }).data;
              return `${d.name}\n${bytes(d.value)}`;
            },
          },
          itemStyle: { borderColor: p.panel, borderWidth: 2, gapWidth: 2 },
          levels: [
            { itemStyle: { borderWidth: 3, borderColor: p.panel, gapWidth: 3 } },
            { itemStyle: { borderWidth: 1, borderColor: p.panel, gapWidth: 1 } },
          ],
        },
      ],
    };
  });
</script>

<div class="chart" use:chart={option}></div>
<p class="legend eyebrow">
  area = bytes · colour = {colourBy === 'age' ? 'age (blue new → red old)' : 'read heat'}
</p>

<style>
  .chart {
    width: 100%;
    height: 20rem;
  }
  .legend {
    margin: 0.6rem 0 0;
  }
</style>
