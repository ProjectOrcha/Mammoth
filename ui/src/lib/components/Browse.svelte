<!-- The file browser. One component for both routes: given a path it decides
     whether to list a directory or open a file, so /files and
     /files/[...path] cannot drift apart. -->
<script lang="ts">
  import { api } from '$lib/api';
  import type { BlockLayout, FileStatus } from '$lib/types';
  import { ago, bibytes, bytes, count, joinPath, segments } from '$lib/format';
  import Panel from '$lib/components/Panel.svelte';
  import BlockMatrix from '$lib/charts/BlockMatrix.svelte';

  interface Props {
    path: string;
  }

  let { path }: Props = $props();

  let status = $state<FileStatus | null>(null);
  let entries = $state<FileStatus[] | null>(null);
  let layout = $state<BlockLayout | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(true);

  // Re-runs whenever `path` changes, which is what makes deep links work.
  $effect(() => {
    const wanted = path;
    loading = true;
    error = null;
    entries = null;
    layout = null;

    (async () => {
      try {
        const s = await api.stat(wanted);
        if (wanted !== path) return;
        status = s;
        if (!s) {
          error = `no such path: ${wanted}`;
        } else if (s.is_dir) {
          entries = await api.list(wanted);
        } else {
          layout = await api.blocks(wanted);
        }
      } catch (e) {
        error = e instanceof Error ? e.message : String(e);
      } finally {
        if (wanted === path) loading = false;
      }
    })();
  });

  const crumbs = $derived.by(() => {
    const parts = segments(path);
    let acc = '';
    return [
      { name: '/', href: '/files' },
      ...parts.map((p) => {
        acc = joinPath(acc || '/', p);
        return { name: p, href: `/files${acc}` };
      }),
    ];
  });

  /** The fragments a read would actually touch — the point being that the
   *  client worked this out itself, with no round trip to a master. Degraded
   *  counts are file-wide; the preferred node is for the first block, which is
   *  where a sequential read starts. */
  const readPlan = $derived.by(() => {
    const first = layout?.blocks[0];
    if (!first || !layout) return null;
    const preferred = first.fragments.find((f) => f.preferred) ?? first.fragments[0];
    const degradedFragments = layout.blocks.reduce(
      (a, b) => a + b.fragments.filter((f) => f.state !== 'ok').length,
      0,
    );
    const degradedBlocks = layout.blocks.filter((b) =>
      b.fragments.some((f) => f.state !== 'ok'),
    ).length;
    return { preferred, degradedFragments, degradedBlocks, fragments: first.fragments.length };
  });

  const kindCounts = $derived.by(() => {
    const first = layout?.blocks[0];
    if (!first) return null;
    const c = { data: 0, 'local-parity': 0, 'global-parity': 0, replica: 0 };
    for (const f of first.fragments) c[f.kind]++;
    return c;
  });
</script>

<nav class="crumbs" aria-label="Path">
  {#each crumbs as c, i (c.href)}
    {#if i > 1}<span class="sep" aria-hidden="true">/</span>{/if}
    {#if i === crumbs.length - 1}
      <span class="mono here">{c.name}</span>
    {:else}
      <a class="mono" href={c.href}>{c.name}</a>
    {/if}
  {/each}
</nav>

{#if loading}
  <p class="quiet mono">reading {path}…</p>
{:else if error}
  <Panel title="Not found">
    <p class="err mono">{error}</p>
    <p class="quiet">
      The demo namespace has <code class="mono">/warehouse</code>,
      <code class="mono">/logs</code>, <code class="mono">/data</code>,
      <code class="mono">/tmp</code> and <code class="mono">/user</code>.
    </p>
  </Panel>
{:else if entries}
  <Panel title={path} note={`${entries.length} entries`}>
    {#if entries.length === 0}
      <p class="quiet">Empty.</p>
    {:else}
      <table>
        <thead>
          <tr>
            <th>name</th>
            <th class="num">size</th>
            <th>policy</th>
            <th class="num">blocks</th>
            <th>owner</th>
            <th class="num">modified</th>
          </tr>
        </thead>
        <tbody>
          {#each entries as e (e.path)}
            <tr>
              <td>
                <a href={`/files${e.path}`} class="mono">
                  <span class="icon" aria-hidden="true">{e.is_dir ? '▸' : '·'}</span>{e.name}{e.is_dir
                    ? '/'
                    : ''}
                </a>
              </td>
              <td class="num mono">{bytes(e.len)}</td>
              <td>
                <span class="policy" data-inline={e.inlined}>{e.policy}</span>
              </td>
              <td class="num mono">{e.inlined ? 'inlined' : e.blocks ? count(e.blocks) : '—'}</td>
              <td class="mono dim">{e.owner}</td>
              <td class="num mono dim">{ago(e.modified)}</td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/if}
  </Panel>
{:else if layout && status}
  <div class="filehead">
    <h2>{status.name}</h2>
    <p class="mono dim">
      {bytes(status.len)} · {status.policy} ·
      {status.inlined ? 'inlined, no blocks allocated' : `${count(status.blocks)} blocks`} ·
      {status.checksum}
    </p>
  </div>

  {#each layout.warnings as w (w)}
    <p class="warning">⚠ {w}</p>
  {/each}

  {#if status.inlined}
    <Panel title="Inlined">
      <p class="quiet">
        This file is under the inline threshold, so it never became blocks at all — its
        bytes live directly in the metadata store, and its durability comes from Raft
        replication of that metadata. No block id, no fragment bookkeeping, and a read
        of it costs exactly one round trip because resolving it <em>is</em> reading it.
      </p>
    </Panel>
  {:else}
    <div class="cols">
      <Panel title="Read plan" note="derived on the client">
        {#if readPlan}
          <dl>
            <div><dt>fragments per block</dt><dd class="mono">{readPlan.fragments}</dd></div>
            <div>
              <dt>block 1 reads from</dt>
              <dd class="mono">{readPlan.preferred.node} · {readPlan.preferred.rack.split('/').pop()}</dd>
            </div>
            <div>
              <dt>metadata round trips</dt>
              <dd class="mono ok">0 — placement is computed</dd>
            </div>
            <div>
              <dt>degraded fragments</dt>
              <dd class="mono" class:warn={readPlan.degradedFragments > 0}>
                {readPlan.degradedFragments}
                {#if readPlan.degradedBlocks}
                  across {readPlan.degradedBlocks} block{readPlan.degradedBlocks === 1 ? '' : 's'}
                {/if}
              </dd>
            </div>
          </dl>
          <p class="hint">
            The client derived this replica set itself from the block id and the topology
            epoch, so it can hedge at a second node without asking anyone. If a fragment is
            rebuilding, the read reconstructs from the rest of its local group.
          </p>
        {/if}
      </Panel>

      <Panel title="Layout" note={layout.policy}>
        {#if kindCounts}
          <dl>
            <div><dt>block size</dt><dd class="mono">{bibytes(layout.block_size)}</dd></div>
            {#if kindCounts.replica}
              <div><dt>replicas</dt><dd class="mono">{kindCounts.replica} whole copies</dd></div>
              <div><dt>storage</dt><dd class="mono">{kindCounts.replica.toFixed(2)}×</dd></div>
            {:else}
              <div><dt>data fragments</dt><dd class="mono">{kindCounts.data}</dd></div>
              <div><dt>local parity</dt><dd class="mono">{kindCounts['local-parity']}</dd></div>
              <div><dt>global parity</dt><dd class="mono">{kindCounts['global-parity']}</dd></div>
              <div>
                <dt>storage</dt>
                <dd class="mono">
                  {(
                    (kindCounts.data + kindCounts['local-parity'] + kindCounts['global-parity']) /
                    kindCounts.data
                  ).toFixed(2)}×
                </dd>
              </div>
              <div>
                <dt>to repair one loss</dt>
                <dd class="mono">{kindCounts['local-parity'] ? 3 : kindCounts.data} fragments</dd>
              </div>
            {/if}
          </dl>
        {/if}
      </Panel>
    </div>

    <Panel
      title="Block placement"
      note={`${layout.blocks.length} of ${count(status.blocks)} blocks shown`}
    >
      <BlockMatrix {layout} />
    </Panel>
  {/if}
{/if}

<style>
  .crumbs {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 0.3rem;
    margin-bottom: 1rem;
    font-size: 0.85rem;
  }
  .sep {
    color: var(--fg-faint);
  }
  .here {
    color: var(--fg-display);
  }
  .filehead {
    margin-bottom: 1rem;
  }
  .filehead p {
    margin: 0.35rem 0 0;
    font-size: 0.75rem;
  }
  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(18rem, 1fr));
    gap: var(--gap);
    margin-bottom: var(--gap);
  }
  .icon {
    display: inline-block;
    width: 1rem;
    color: var(--fg-faint);
  }
  .policy {
    font-family: var(--font-mono);
    font-size: 0.68rem;
    padding: 0.05rem 0.35rem;
    border: 1px solid var(--rule);
    color: var(--fg-dim);
  }
  .policy[data-inline='true'] {
    color: var(--ok);
    border-color: var(--ok);
  }
  .warning {
    margin: 0 0 var(--gap);
    padding: 0.55rem 0.75rem;
    border: 1px solid var(--warn);
    color: var(--warn);
    font-size: 0.78rem;
    background: var(--bg-panel);
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
  dd.ok {
    color: var(--ok);
  }
  dd.warn {
    color: var(--warn);
  }
  .hint {
    margin: 0.8rem 0 0;
    font-size: 0.74rem;
    color: var(--fg-faint);
    line-height: 1.5;
  }
  .quiet {
    color: var(--fg-faint);
    margin: 0;
    line-height: 1.6;
  }
  .err {
    color: var(--danger);
    margin: 0 0 0.5rem;
  }
  .dim {
    color: var(--fg-faint);
  }
</style>
