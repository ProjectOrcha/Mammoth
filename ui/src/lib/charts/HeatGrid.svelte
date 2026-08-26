<!-- HeatGrid: one tile per node, arranged by rack. Colour is the chosen metric.
     Hand-drawn rather than an ECharts heatmap because the tiles have to group
     by rack, carry a state glyph and stay clickable — a categorical heatmap
     fights all three. This is the one people screenshot. -->
<script lang="ts">
  import type { HeatCell, HeatMetric } from '$lib/types';
  import { count, ms, pct } from '$lib/format';

  interface Props {
    cells: HeatCell[];
    metric?: HeatMetric;
    onselect?: (node: string) => void;
  }

  let { cells, metric = 'usage', onselect }: Props = $props();

  const racks = $derived.by(() => {
    const m = new Map<string, HeatCell[]>();
    for (const c of cells) {
      if (!m.has(c.rack)) m.set(c.rack, []);
      m.get(c.rack)!.push(c);
    }
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]));
  });

  function raw(c: HeatCell): number {
    return c[metric];
  }

  const max = $derived(Math.max(...cells.map(raw), 1));

  /** 0..1 within the metric's own range, so "blocks" and "usage" both use the
   *  whole ramp instead of usage always looking hot and QPS always looking cold. */
  function intensity(c: HeatCell): number {
    if (c.state === 'dead') return 0;
    return metric === 'usage' ? Math.min(1, c.usage / 100) : Math.min(1, raw(c) / max);
  }

  /** steel → gold → red. Low is calm, high is loud, and the midpoint is the
   *  colour the rest of the UI already uses for "look at this". The ramp is the
   *  same in both themes — a heat scale that changes meaning with the theme is
   *  not a heat scale. */
  function ramp(t: number): [number, number, number] {
    const stops: [number, [number, number, number]][] = [
      [0, [22, 73, 130]],
      [0.55, [219, 197, 96]],
      [1, [226, 86, 77]],
    ];
    let a = stops[0];
    let b = stops[stops.length - 1];
    for (let i = 0; i < stops.length - 1; i++) {
      if (t >= stops[i][0] && t <= stops[i + 1][0]) {
        a = stops[i];
        b = stops[i + 1];
        break;
      }
    }
    const f = b[0] === a[0] ? 0 : (t - a[0]) / (b[0] - a[0]);
    return a[1].map((v, i) => Math.round(v + (b[1][i] - v) * f)) as [number, number, number];
  }

  /**
   * Ink that survives the whole ramp.
   *
   * White reads fine on the deep blue at the cold end and is close to illegible
   * on the gold in the middle, so the label colour has to follow the tile's
   * luminance rather than being picked once. WCAG relative luminance, 0.55 as
   * the crossover.
   */
  function ink(rgb: [number, number, number]): string {
    const lin = rgb.map((c) => {
      const v = c / 255;
      return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
    });
    const L = 0.2126 * lin[0] + 0.7152 * lin[1] + 0.0722 * lin[2];
    return L > 0.42 ? '#05111d' : '#fefefe';
  }

  function tileStyle(c: HeatCell): string {
    const rgb = ramp(intensity(c));
    return `--heat: rgb(${rgb.join(', ')}); --ink: ${ink(rgb)}`;
  }

  function display(c: HeatCell): string {
    if (c.state === 'dead') return '—';
    switch (metric) {
      case 'usage':
        return `${Math.round(c.usage)}%`;
      case 'fragments':
        return count(c.fragments);
      case 'read_qps':
        return count(c.read_qps);
      case 'write_qps':
        return count(c.write_qps);
      case 'disk_p99_ms':
        return ms(c.disk_p99_ms);
    }
  }

  const GLYPH = { healthy: '', warn: '⚠', decommissioning: '◔', maintenance: '◌', dead: '✕' };
</script>

<div class="racks">
  {#each racks as [rack, nodes] (rack)}
    {@const used = nodes.reduce((a, n) => a + n.usage, 0) / nodes.length}
    <div class="rack">
      <p class="eyebrow">{rack} · {pct(used, 100)} avg</p>
      <div class="tiles">
        {#each nodes as c (c.node)}
          <button
            class="tile"
            data-state={c.state}
            style={tileStyle(c)}
            onclick={() => onselect?.(c.node)}
            title={`${c.node} · ${c.state} · ${Math.round(c.usage)}% used · ${count(c.fragments)} blocks · ${ms(c.disk_p99_ms)} p99`}
          >
            <span class="id mono">{c.node}</span>
            <span class="metric">{display(c)}</span>
            {#if GLYPH[c.state]}<span class="flag" aria-hidden="true">{GLYPH[c.state]}</span>{/if}
          </button>
        {/each}
      </div>
    </div>
  {/each}
</div>

<div class="ramp">
  <span class="eyebrow">low</span>
  <div class="bar"></div>
  <span class="eyebrow">high</span>
</div>

<style>
  .racks {
    display: flex;
    flex-wrap: wrap;
    gap: 1.25rem;
  }
  .rack {
    min-width: 0;
  }
  .tiles {
    display: flex;
    gap: 0.35rem;
    margin-top: 0.45rem;
  }
  .tile {
    position: relative;
    width: 4.1rem;
    height: 3.4rem;
    border: 1px solid var(--rule);
    background: var(--heat);
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    justify-content: space-between;
    padding: 0.3rem 0.35rem;
    color: var(--ink);
  }
  .tile:hover {
    border-color: var(--accent);
  }
  .tile[data-state='dead'] {
    background: repeating-linear-gradient(
      45deg,
      var(--bg-plate),
      var(--bg-plate) 4px,
      transparent 4px,
      transparent 8px
    );
    color: var(--fg-faint);
    border-color: var(--danger);
  }
  .id {
    font-size: 0.66rem;
    opacity: 0.92;
  }
  .metric {
    font-family: var(--font-display);
    font-size: 0.95rem;
    letter-spacing: 0.02em;
  }
  .flag {
    position: absolute;
    top: 0.25rem;
    right: 0.3rem;
    font-size: 0.6rem;
  }
  .ramp {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 1rem;
  }
  .ramp .bar {
    height: 0.35rem;
    width: 9rem;
    background: linear-gradient(90deg, rgb(22, 73, 130), rgb(219, 197, 96), rgb(226, 86, 77));
  }
</style>
