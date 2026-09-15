<!-- The file browser. One component for both routes: given a path it decides
     whether to list a directory or open a file, so /files and
     /files/[...path] cannot drift apart. -->
<script lang="ts">
  import { goto } from '$app/navigation';
  import { untrack } from 'svelte';
  import { live } from '$lib/live.svelte';
  import { api, currentSource } from '$lib/api';
  import type { BlockLayout, FileStatus } from '$lib/types';
  import { ago, bibytes, bytes, count, fileHref, joinPath, segments } from '$lib/format';
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
  let revision = $state(0);
  let offset = $state(0);
  let busy = $state(false);
  let actionError = $state<string | null>(null);
  let folderName = $state('');
  let refreshView = () => {};
  $effect(() => { void live.updatedAt; untrack(() => refreshView()); });

  $effect(() => { void path; offset = 0; });

  async function change(action: () => Promise<void>) {
    const selected = path;
    busy = true; actionError = null;
    try { await action(); if (path === selected) revision++; await live.refresh(); }
    catch (e) { if (path === selected) actionError = e instanceof Error ? e.message : String(e); }
    finally { busy = false; }
  }

  async function upload(event: Event) {
    const input = event.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    const selected = path;
    if (!file) return;
    const name = joinPath(selected, file.name);
    await change(async () => {
      let exists = false;
      try { exists = (await api.stat(name)) !== null; }
      catch (e) { if (!(e instanceof Error) || !e.message.startsWith('404 ')) throw e; }
      if (exists && !window.confirm(`Replace ${file.name}?`)) return;
      await api.upload(name, file);
    });
    input.value = '';
  }

  async function makeFolder(event: SubmitEvent) {
    event.preventDefault();
    if (!folderName.trim() || /[\/\\]/.test(folderName) || ['.', '..'].includes(folderName)) { actionError = 'Enter a single folder name.'; return; }
    const destination = joinPath(path, folderName);
    await change(() => api.mkdir(destination));
    if (!actionError) folderName = '';
  }


  // Cleanup invalidates every result, including a second request and A → B → A navigation.
  $effect(() => {
    const wanted = path;
    const pageOffset = offset;
    void revision;
    let active = true;
    loading = true;
    error = null;
    status = null;
    entries = null;
    layout = null;

    let fetching = false;
    const load = async () => {
      if (!active || fetching) return;
      fetching = true;
      try {
        const nextStatus = await api.stat(wanted);
        if (!active) return;
        if (!nextStatus) throw new Error(`No such path: ${wanted}`);
        const result = nextStatus.is_dir
          ? { entries: await api.list(wanted, 200, pageOffset), layout: null }
          : { entries: null, layout: await api.blocks(wanted) };
        if (!active) return;
        if (!nextStatus.is_dir && !result.layout) throw new Error(`No block layout: ${wanted}`);
        status = nextStatus;
        entries = result.entries;
        layout = result.layout;
        error = null;
      } catch (e) {
        if (active) error = e instanceof Error ? e.message : String(e);
      } finally {
        fetching = false;
        if (active) loading = false;
      }
    };
    refreshView = () => { void load(); };
    refreshView();

    return () => { active = false; };
  });

  const crumbs = $derived.by(() => {
    const parts = segments(path);
    let acc = '';
    return [
      { name: '/', href: '/files' },
      ...parts.map((p) => {
        acc = joinPath(acc || '/', p);
        return { name: p, href: fileHref(acc) };
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

<svelte:head><title>{path} · Files · Mammoth</title></svelte:head>

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
  <Panel title="Unable to load path">
    <p class="err mono" role="alert">{error}</p>
    {#if currentSource() === 'demo'}
    <p class="quiet">
      The demo namespace has <code class="mono">/warehouse</code>,
      <code class="mono">/logs</code>, <code class="mono">/data</code>,
      <code class="mono">/tmp</code> and <code class="mono">/user</code>.
    </p>
    {/if}
  </Panel>
{:else if entries}
  {#if currentSource() === 'gateway'}
    <div class="file-actions">
      <label>Upload file <input type="file" onchange={upload} disabled={busy} /></label>
      <form onsubmit={makeFolder}><input aria-label="New folder name" placeholder="New folder name" bind:value={folderName} disabled={busy} /><button disabled={busy}>Create folder</button></form>
    </div>
    {#if actionError}<p role="alert" class="err">{actionError}</p>{/if}
  {/if}
  <Panel title={path} note={`${entries.length} entries · page ${offset / 200 + 1}`}>
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
                <a href={fileHref(e.path)} class="mono">
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
    <div class="pagination"><button disabled={offset === 0 || loading} onclick={() => offset = Math.max(0, offset - 200)}>Previous</button><button disabled={entries.length < 200 || loading} onclick={() => offset += 200}>Next</button></div>
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

  {#if currentSource() === 'gateway'}
    <div class="file-actions"><a href={api.downloadUrl(status.path)} download={status.name}>Download</a><button disabled={busy} onclick={() => {
      const selected = path;
      if (window.confirm(`Delete ${status?.name}?`)) void change(async () => { await api.remove(selected); await goto(fileHref(selected.slice(0, selected.lastIndexOf('/')) || '/')); });
    }}>Delete file</button></div>
    {#if actionError}<p role="alert" class="err">{actionError}</p>{/if}
  {/if}

  {#each layout.warnings as w (w)}
    <p class="warning">⚠ {w}</p>
  {/each}

  {#if status.inlined}
    <Panel title="Inlined">
      <p class="quiet">
        This file is under the inline threshold, so it never became blocks at all — its
        bytes live directly in the metadata store. In local mode, the namespace is committed atomically on this machine; it has no separate block replicas.
      </p>
    </Panel>
  {:else}
    <div class="cols">
      <Panel title="Read plan" note="reported replica layout">
        {#if readPlan}
          <dl>
            <div><dt>fragments per block</dt><dd class="mono">{readPlan.fragments}</dd></div>
            <div>
              <dt>block 1 reads from</dt>
              <dd class="mono">{#if readPlan.preferred}{readPlan.preferred.node} · {readPlan.preferred.rack.split('/').pop()}{:else}No fragments available{/if}</dd>
            </div>
            <div>
              <dt>metadata round trips</dt>
              <dd class="mono ok">{live.report?.capabilities?.local ? 'Not measured in local mode' : '0 — placement is computed'}</dd>
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
            The gateway reports these copies. The local backend verifies checksums and tries another replica if a copy is damaged or missing.
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
  .file-actions, .file-actions form, .pagination { display: flex; flex-wrap: wrap; gap: .75rem; align-items: center; margin: .75rem 0; }

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
    grid-template-columns: repeat(auto-fit, minmax(min(18rem, 100%), 1fr));
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
