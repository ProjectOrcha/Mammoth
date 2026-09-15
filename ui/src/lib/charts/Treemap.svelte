<!-- Treemap: area is bytes, colour is age or read heat. Click to zoom into a
     directory. Answers "what is eating my disk" in one glance, which is the
     question a table of sizes never quite manages. -->
<script lang="ts">
  import { escapeHtml } from '$lib/html';
  import type { TreemapNode } from '$lib/types';
  import { bytes, count, fileHref } from '$lib/format';
  import { chart, palette, tooltipStyle, type ChartOption } from './echarts.svelte';

  interface Props {
    root: TreemapNode;
    colourBy?: 'age' | 'reads';
  }

  let { root, colourBy = 'age' }: Props = $props();

  // Keep navigation in Svelte: ECharts discards its internal zoom whenever
  // live data replaces the tree, even if the same directories are still there.
  let selectedPath = $state('');
  function trail(node: TreemapNode, path: string): TreemapNode[] | null {
    if (node.path === path) return [node];
    for (const child of node.children ?? []) {
      const found = trail(child, path);
      if (found) return [node, ...found];
    }
    return null;
  }
  const ancestors = $derived(trail(root, selectedPath) ?? [root]);
  const current = $derived(ancestors[ancestors.length - 1]);
  const entries = $derived(current.children ?? [current]);
  function open(params: unknown) {
    const path = (params as { data?: { path?: string } }).data?.path;
    const node = entries.find(entry => entry.path === path);
    if (node?.children?.length) selectedPath = node.path;
  }

  interface Datum {
    name: string;
    path: string;
    value: number;
    heat: number;
    age: number;
    reads: number | null;
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
    const heat = colourBy === 'age' ? Math.min(1, n.age_days / 120) : Math.min(1, (n.reads ?? 0) / 90_000);
    const rgb = ramp(heat);
    const fg = ink(rgb);
    return {
      name: n.name || '/',
      path: n.path,
      value: n.value,
      heat,
      age: n.age_days,
      reads: n.reads,
      itemStyle: { color: `rgb(${rgb.join(', ')})` },
      label: { color: fg },
    };
  }

  const option = $derived((): ChartOption => {
    const p = palette();
    const data = entries.map(build);
    return {
      backgroundColor: 'transparent',
      tooltip: {
        ...tooltipStyle(p),
        formatter: (params: unknown) => {
          const d = (params as { data: Datum }).data;
          return [
            `<b>${escapeHtml(d.path || '/')}</b>`,
            `${bytes(d.value)}`,
            `${d.age.toFixed(1)} days old${d.reads === null ? '' : ` · ${count(d.reads)} reads`}`,
          ].join('<br/>');
        },
      },
      series: [
        {
          type: 'treemap',
          id: 'namespace',
          data,
          roam: false,
          animation: false,
          nodeClick: false,
          left: 0, right: 0, top: 0, bottom: 0,
          breadcrumb: {
            show: false,
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

<nav class="crumbs" aria-label="Treemap location">
  {#each ancestors as node, index (node.path)}
    {#if index > 0}<span aria-hidden="true">/</span>{/if}
    <button onclick={() => selectedPath = node.path} aria-current={node.path === current.path ? 'location' : undefined}>{node.name || '/'}</button>
  {/each}
</nav>
<div class="chart" role="img" aria-label={`Namespace sizes in ${current.path}: ${bytes(current.value)} total`} use:chart={{ option, onClick: open }}></div>
<p class="legend eyebrow">
  area = bytes · colour = {colourBy === 'age' ? 'age (blue new → red old)' : 'read heat'}
</p>
<p class="hint quiet">Select a folder to see its contents. Use the path above to go back.</p>
<details>
  <summary>View all entries ({entries.length})</summary>
  <ul class="entries">
    {#each entries as entry (entry.path)}
      <li>
        {#if entry.children?.length}
          <button onclick={() => selectedPath = entry.path}>{entry.name || '/'}</button>
        {:else}
          <a href={fileHref(entry.path)}>{entry.name || '/'}</a>
        {/if}
        <span class="mono">{bytes(entry.value)}</span>
      </li>
    {/each}
  </ul>
</details>

<style>
  .chart {
    width: 100%;
    height: 20rem;
  }
  .legend {
    margin: 0.6rem 0 0;
  }
  .crumbs { display: flex; flex-wrap: wrap; align-items: center; gap: .4rem; margin-bottom: .6rem; }
  .crumbs button[aria-current] { color: var(--fg); font-weight: bold; }
  .hint { margin: .5rem 0; font-size: .75rem; }
  summary { cursor: pointer; font-size: .8rem; }
  .entries { list-style: none; padding: 0; max-height: 12rem; overflow: auto; }
  .entries li { display: flex; justify-content: space-between; align-items: center; gap: .5rem; padding: .3rem 0; }
  .entries a, .entries button { overflow-wrap: anywhere; min-width: 0; text-align: left; }
  .entries span { white-space: nowrap; }
</style>
