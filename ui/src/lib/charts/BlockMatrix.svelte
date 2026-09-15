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

  const CELL = 30;
  const LEFT = 62;
  const TOP = 48;

  let pageIndex = $state(0);
  $effect(() => { void layout.path; pageIndex = 0; });
  const pageCount = $derived(Math.max(1, Math.ceil(layout.blocks.length / maxRows)));
  const currentPage = $derived(Math.min(pageIndex, pageCount - 1));
  const blocks = $derived(layout.blocks.slice(currentPage * maxRows, (currentPage + 1) * maxRows));
  const nodes = $derived([...layout.nodes].sort((a, b) =>
    (layout.racks[a] ?? '').localeCompare(layout.racks[b] ?? '') || a.localeCompare(b, undefined, { numeric: true })));

  const columns = $derived.by(() => {
    const groups = new Map<string, string[]>();
    for (const node of nodes) {
      const rack = layout.racks[node] ?? '?';
      groups.set(rack, [...(groups.get(rack) ?? []), node]);
    }
    const positions = new Map<string, number>();
    const bands: { rack: string; x0: number; x1: number }[] = [];
    let offset = 0;
    for (const [rack, members] of groups) {
      const size = Math.max(76, members.length * CELL);
      const inset = (size - members.length * CELL) / 2;
      members.forEach((node, index) => positions.set(node, offset + inset + index * CELL + 2));
      bands.push({ rack, x0: offset, x1: offset + size });
      offset += size + 12;
    }
    return { positions, bands, width: offset };
  });
  const x = (node: string) => columns.positions.get(node) ?? 0;
  const y = $derived(
    scaleBand<number>()
      .domain(blocks.map((b) => b.index))
      .range([0, blocks.length * CELL])
      .padding(0.14),
  );

  const width = $derived(LEFT + columns.width + 12);
  const height = $derived(TOP + blocks.length * CELL + 26);

  const bands = $derived(columns.bands);

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
    <svg {width} {height} role="group" aria-label="Block placement matrix">
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
          text-anchor="middle"><title>{band.rack}</title>{(band.rack.split('/').pop() ?? '').slice(0, 12)}</text
        >
      {/each}

      {#each nodes as n (n)}
        <text x={LEFT + (x(n) ?? 0) + (CELL - 4) / 2} y={TOP - 8} class="col" text-anchor="middle">
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
              width={(CELL - 4)}
              height={y.bandwidth()}
              fill={fill(f)}
              stroke={stroke(f)}
              stroke-width="1.5"
              stroke-dasharray={f.state === 'repairing' ? '3 2' : undefined}
              role="button"
              tabindex="0"
              aria-label={`${n}, block ${b.index + 1}, ${f.kind}, ${f.state}`}
              onfocus={() => (hover = { node: n, block: b.index, f, len: b.len })}
              onclick={() => (hover = { node: n, block: b.index, f, len: b.len })}
              onkeydown={(event) => { if (event.key === 'Enter' || event.key === ' ') { event.preventDefault(); hover = { node: n, block: b.index, f, len: b.len }; } }}
              onmouseenter={() => (hover = { node: n, block: b.index, f, len: b.len })}
              onmouseleave={() => (hover = null)}
            />
          {:else}
            <rect
              x={LEFT + (x(n) ?? 0) + (CELL - 4) / 2 - 1}
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

  {#if pageCount > 1}
    <div class="pagination" aria-label="Block pages">
      <button disabled={currentPage === 0} onclick={() => pageIndex = currentPage - 1}>Previous blocks</button>
      <span class="mono">{currentPage * maxRows + 1}–{Math.min((currentPage + 1) * maxRows, layout.blocks.length)} of {layout.blocks.length}</span>
      <button disabled={currentPage === pageCount - 1} onclick={() => pageIndex = currentPage + 1}>Next blocks</button>
    </div>
  {/if}
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
      {hover.f.state} · {bibytes(hover.len / (hover.f.kind === 'replica' ? 1 : Math.max(1, layout.blocks.find(b => b.index === hover?.block)?.fragments.filter(f => f.kind === 'data').length ?? 1)))}
      {#if hover.f.preferred}· read from here{/if}
    </p>
  {:else}
    <p class="hovered mono dim">Hover, tap, or focus a cell to inspect its fragment.</p>
  {/if}
</div>

<style>
  .pagination { display: flex; flex-wrap: wrap; align-items: center; gap: .6rem; margin-top: .6rem; }
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
  rect[role='button'] {
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
