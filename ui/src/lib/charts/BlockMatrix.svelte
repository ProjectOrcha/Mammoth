<!-- BlockMatrix: blocks (rows) × nodes (columns), one cell per fragment,
     coloured by what the fragment IS and shaped by how it is DOING. Bad
     placement is visible instantly: a row whose cells cluster inside one rack
     band is a block that does not survive that rack.
     Scales come from d3; the drawing is plain SVG, which is less code than
     asking a chart library for a grid. -->
<script lang="ts">
  import { scaleBand } from 'd3-scale';
  import type { BlockLayout, Fragment } from '$lib/types';
  import { bibytes } from '$lib/format';
  import { FRAGMENT_COLOUR } from './colors';

  interface Props {
    layout: BlockLayout;
    /** Rows to draw before scrolling. */
    maxRows?: number;
  }

  let { layout, maxRows = 24 }: Props = $props();

  const CELL = 22;
  const LEFT = 62;
  const TOP = 40;

  const blocks = $derived(layout.blocks.slice(0, maxRows));
  const nodes = $derived(layout.nodes);

  const x = $derived(scaleBand<string>().domain(nodes).range([0, nodes.length * CELL]).padding(0.14));
  const y = $derived(
    scaleBand<number>()
      .domain(blocks.map((b) => b.index))
      .range([0, blocks.length * CELL])
      .padding(0.14),
  );

  const width = $derived(LEFT + nodes.length * CELL + 12);
  const height = $derived(TOP + blocks.length * CELL + 26);

  /** Rack bands, so the eye can group columns without reading labels. */
  const bands = $derived.by(() => {
    const out: { rack: string; x0: number; x1: number }[] = [];
    for (const n of nodes) {
      const rack = layout.racks[n] ?? '?';
      const left = x(n) ?? 0;
      const last = out[out.length - 1];
      if (last && last.rack === rack) last.x1 = left + x.bandwidth();
      else out.push({ rack, x0: left, x1: left + x.bandwidth() });
    }
    return out;
  });

  function fill(f: Fragment): string {
    if (f.state === 'missing') return 'transparent';
    if (f.state === 'corrupt') return 'var(--danger)';
    if (f.state === 'repairing' || f.state === 'pending') return 'transparent';
    return FRAGMENT_COLOUR[f.kind];
  }

  function stroke(f: Fragment): string {
    if (f.state === 'repairing' || f.state === 'pending') return 'var(--warn)';
    if (f.state === 'missing') return 'var(--danger)';
    return 'transparent';
  }

  const byNode = $derived.by(() => {
    const m = new Map<string, Map<number, Fragment>>();
    for (const b of layout.blocks) {
      for (const f of b.fragments) {
        if (!m.has(f.node)) m.set(f.node, new Map());
        m.get(f.node)!.set(b.index, f);
      }
    }
    return m;
  });

  let hover = $state<{ node: string; block: number; f: Fragment; len: number } | null>(null);

  function label(f: Fragment): string {
    const prefix = { data: 'd', 'local-parity': 'l', 'global-parity': 'p', replica: 'r' }[f.kind];
    return `${prefix}${f.idx}`;
  }
</script>

<div class="wrap">
  <div class="scroll">
    <svg {width} {height} role="img" aria-label="Block placement matrix">
      {#each bands as band (band.rack + band.x0)}
        <rect
          x={LEFT + band.x0 - 3}
          y={TOP - 20}
          width={band.x1 - band.x0 + 6}
          height={blocks.length * CELL + 22}
          fill="var(--bg-plate)"
          opacity="0.55"
        />
        <text
          x={LEFT + (band.x0 + band.x1) / 2}
          y={TOP - 26}
          class="rack"
          text-anchor="middle">{band.rack.split('/').pop()}</text
        >
      {/each}

      {#each nodes as n (n)}
        <text x={LEFT + (x(n) ?? 0) + x.bandwidth() / 2} y={TOP - 8} class="col" text-anchor="middle">
          {n}
        </text>
      {/each}

      {#each blocks as b (b.id)}
        <text x={LEFT - 10} y={TOP + (y(b.index) ?? 0) + CELL / 2} class="row" text-anchor="end">
          blk {b.index + 1}
        </text>
      {/each}

      {#each nodes as n (n)}
        {#each blocks as b (b.id)}
          {@const f = byNode.get(n)?.get(b.index)}
          {#if f}
            <rect
              x={LEFT + (x(n) ?? 0)}
              y={TOP + (y(b.index) ?? 0)}
              width={x.bandwidth()}
              height={y.bandwidth()}
              fill={fill(f)}
              stroke={stroke(f)}
              stroke-width="1.5"
              stroke-dasharray={f.state === 'repairing' ? '3 2' : undefined}
              role="presentation"
              onmouseenter={() => (hover = { node: n, block: b.index, f, len: b.len })}
              onmouseleave={() => (hover = null)}
            />
          {:else}
            <rect
              x={LEFT + (x(n) ?? 0) + x.bandwidth() / 2 - 1}
              y={TOP + (y(b.index) ?? 0) + y.bandwidth() / 2 - 1}
              width="2"
              height="2"
              fill="var(--rule-strong)"
            />
          {/if}
        {/each}
      {/each}
    </svg>
  </div>

  <div class="legend">
    <span><i style="background: {FRAGMENT_COLOUR.data}"></i>data</span>
    <span><i style="background: {FRAGMENT_COLOUR['local-parity']}"></i>local parity</span>
    <span><i style="background: {FRAGMENT_COLOUR['global-parity']}"></i>global parity</span>
    <span><i style="background: {FRAGMENT_COLOUR.replica}"></i>replica</span>
    <span><i class="repairing"></i>rebuilding</span>
    <span><i class="absent"></i>absent</span>
  </div>

  {#if hover}
    <p class="hovered mono">
      {hover.node} · blk {hover.block + 1} · {label(hover.f)} · {hover.f.kind} ·
      {hover.f.state} · {bibytes(hover.len / (hover.f.kind === 'replica' ? 1 : 6))}
      {#if hover.f.preferred}· read from here{/if}
    </p>
  {:else}
    <p class="hovered mono dim">hover a cell for the fragment</p>
  {/if}
</div>

<style>
  .wrap {
    min-width: 0;
  }
  .scroll {
    overflow-x: auto;
    max-width: 100%;
  }
  svg {
    display: block;
  }
  .col,
  .row,
  .rack {
    font-family: var(--font-mono);
    fill: var(--fg-faint);
    font-size: 9px;
  }
  .rack {
    fill: var(--fg-dim);
    letter-spacing: 0.12em;
    text-transform: uppercase;
  }
  rect[role='presentation'] {
    cursor: crosshair;
  }
  .legend {
    display: flex;
    flex-wrap: wrap;
    gap: 0.9rem;
    margin-top: 0.7rem;
    font-size: 0.68rem;
    color: var(--fg-faint);
  }
  .legend span {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
  }
  .legend i {
    width: 0.55rem;
    height: 0.55rem;
    display: inline-block;
  }
  .legend i.repairing {
    border: 1.5px dashed var(--warn);
  }
  .legend i.absent {
    border: 1px solid var(--rule-strong);
    background: transparent;
  }
  .hovered {
    margin: 0.5rem 0 0;
    font-size: 0.7rem;
    color: var(--fg-dim);
    min-height: 1rem;
  }
  .hovered.dim {
    color: var(--fg-faint);
  }
</style>
