<!-- Cluster — Raft members, the leader, log index and snapshot age, and the
     panel this page exists for: what the last start actually cost.
     A master restart is HDFS's worst operational number (30+ minutes of
     read-only cluster, during the incident that caused the restart), so it gets
     a whole panel rather than a line in a table. -->
<script lang="ts">
  import { live } from '$lib/live.svelte';
  import { ago, bibytes, count, duration, ms } from '$lib/format';
  import Panel from '$lib/components/Panel.svelte';
  import Meter from '$lib/components/Meter.svelte';
  import Stat from '$lib/components/Stat.svelte';

  const report = $derived(live.report);
  const start = $derived(report?.start ?? null);

  const speedup = $derived(start ? start.rebuild_equivalent_ms / start.last_start_ms : 0);
  const slowestShard = $derived(
    start ? Math.max(...start.shards.map((s) => s.ready_ms ?? 0)) : 0,
  );
</script>

<header class="page">
  <h1>Cluster</h1>
  <p class="eyebrow">
    {report ? `raft · ${report.raft.filter((m) => m.role !== 'learner').length} voters · leader ${report.leader}` : 'loading'}
  </p>
</header>

{#if !report || !start}
  <p class="quiet mono">reading cluster report…</p>
{:else}
  <div class="stats">
    <Stat label="Last start" value={duration(start.last_start_ms / 1000)} note={ago(start.started_at)} tone="ok" />
    <Stat
      label="Rebuilt instead"
      value={duration(start.rebuild_equivalent_ms / 1000)}
      note={`what HDFS pays for the same map · ${speedup.toFixed(0)}× slower`}
      tone="danger"
    />
    <Stat label="Raft index" value={count(report.raft_index)} note={`snapshot ${duration(report.snapshot_age_s)} old`} />
    <Stat
      label="Safe mode"
      value={report.safe_mode ? 'on' : 'off'}
      note={`${start.shards.filter((s) => s.state === 'ready').length} of ${start.shards.length} shards ready`}
      tone={report.safe_mode ? 'warn' : 'ok'}
    />
  </div>

  <Panel title="Warm start" note={`block map: ${start.block_map}`}>
    <div class="warm">
      <div>
        <p class="lead">
          The block map was <strong>mapped back</strong>, not rebuilt.
          {bibytes(start.mapped_bytes)} of <code class="mono">rkyv</code>-archived state
          <code class="mono">mmap</code>ed next to the Raft snapshot, covering
          {count(start.blocks)} blocks — usable the moment it was mapped, because the
          archived form is the in-memory form. Loading it is O(1) in the number of blocks.
        </p>

        <dl>
          <div>
            <dt>Merkle roots matched first try</dt>
            <dd class="mono ok">{start.roots_matched} of {start.roots_total}</dd>
          </div>
          <div>
            <dt>bytes to confirm a matching worker</dt>
            <dd class="mono ok">32</dd>
          </div>
          <div>
            <dt>buckets that had to stream</dt>
            <dd class="mono">
              {start.buckets_streamed} of {(start.merkle_fanout * start.roots_total).toLocaleString()}
            </dd>
          </div>
          <div>
            <dt>block reports requested</dt>
            <dd class="mono ok">0</dd>
          </div>
        </dl>

        <p class="note">
          A worker with four million blocks confirms all of them with one 32-byte root.
          Only the one whose root disagreed had to descend its tree, and only the three
          buckets that actually differed were sent. HDFS streams every block id from every
          node, every time.
        </p>
      </div>

      <div>
        <p class="eyebrow">Shards leaving safe mode</p>
        <ul class="shards">
          {#each start.shards as s (s.name)}
            <li>
              <span class="sname mono">{s.name}</span>
              <div class="strack">
                <div
                  class="sbar"
                  style="width: {((s.ready_ms ?? 0) / slowestShard) * 100}%"
                  data-state={s.state}
                ></div>
              </div>
              <span class="sms mono">{s.ready_ms !== null ? ms(s.ready_ms) : '—'}</span>
            </li>
          {/each}
        </ul>
        <p class="note">
          Safe mode is per shard, not one cluster-wide gate on 99.9% of all blocks. Reads
          were served from the mapped snapshot immediately — it is committed, durable state
          — and writes opened shard by shard as the roots came in.
        </p>

        <p class="eyebrow" style="margin-top: 1rem">Against a rebuild</p>
        <div class="compare">
          <span class="clabel mono">mammoth</span>
          <Meter value={start.last_start_ms} max={start.rebuild_equivalent_ms} tone="ok" height="0.5rem" />
          <span class="cval mono">{duration(start.last_start_ms / 1000)}</span>
        </div>
        <div class="compare">
          <span class="clabel mono">rebuild</span>
          <Meter value={1} max={1} tone="danger" height="0.5rem" />
          <span class="cval mono">{duration(start.rebuild_equivalent_ms / 1000)}</span>
        </div>
      </div>
    </div>
  </Panel>

  <Panel title="Raft" note={`epoch ${report.topology_epoch}`}>
    <table>
      <thead>
        <tr>
          <th>member</th>
          <th>address</th>
          <th>role</th>
          <th class="num">applied</th>
          <th class="num">lag</th>
          <th class="num">last contact</th>
        </tr>
      </thead>
      <tbody>
        {#each report.raft as m (m.id)}
          <tr>
            <td class="mono">{m.id}</td>
            <td class="mono dim">{m.address}</td>
            <td><span class="role" data-role={m.role}>{m.role}</span></td>
            <td class="num mono">{m.applied.toLocaleString()}</td>
            <td class="num mono" class:warnv={m.lag > 100}>{m.lag}</td>
            <td class="num mono dim">{m.last_contact_ms ? ms(m.last_contact_ms) : '—'}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    <p class="note">
      The learners are workers. They carry a read-only, non-voting replica of the
      namespace so a client with no location lease can send <code class="mono">path + range</code>
      straight to the nearest worker and get bytes back in one round trip. They cost the
      quorum nothing — they never vote, and they are allowed to lag.
    </p>
  </Panel>

  <Panel title="Placement" note={report.placement}>
    <p class="lead">
      Placement is <strong>computed, not remembered</strong>. Given a block id and the
      topology at epoch {report.topology_epoch}, every party — client, gateway, master,
      worker — derives the same replica set independently, in about 200 ns, with no lookup
      and no message.
    </p>
    <ul class="consequences">
      <li>A client can work out where a block is → it does not have to ask.</li>
      <li>A writer can work out where fragments go → no allocation round trip.</li>
      <li>Every node's blocks are spread across every other node → repair is N-way parallel.</li>
      <li>The master can work out what it should have → reports are a diff, not a rebuild.</li>
    </ul>
    <p class="note">
      The topology epoch is stamped on every request. A client holding a stale one is told
      to re-resolve and handed the current topology — about 4 KB — rather than reading a
      node that no longer holds the block.
    </p>
  </Panel>
{/if}

<style>
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: var(--gap);
    margin-bottom: var(--gap);
  }
  .warm {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(19rem, 1fr));
    gap: 1.5rem;
  }
  .lead {
    margin: 0 0 0.9rem;
    font-size: 0.82rem;
    line-height: 1.6;
    color: var(--fg-dim);
  }
  .lead code,
  .note code {
    color: var(--accent);
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
    font-size: 0.76rem;
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
  .note {
    margin: 0.85rem 0 0;
    font-size: 0.73rem;
    color: var(--fg-faint);
    line-height: 1.55;
  }
  .shards {
    list-style: none;
    margin: 0.5rem 0 0;
    padding: 0;
  }
  .shards li {
    display: grid;
    grid-template-columns: 10rem 1fr 3.4rem;
    align-items: center;
    gap: 0.6rem;
    padding: 0.22rem 0;
  }
  .sname,
  .sms {
    font-size: 0.68rem;
    color: var(--fg-faint);
  }
  .sms {
    text-align: right;
  }
  .strack {
    height: 0.45rem;
    background: var(--track);
  }
  .sbar {
    height: 100%;
    background: var(--ok);
  }
  .sbar[data-state='reconciling'] {
    background: var(--warn);
  }
  .sbar[data-state='waiting'] {
    background: var(--track);
  }
  .compare {
    display: grid;
    grid-template-columns: 4.5rem 1fr 4rem;
    align-items: center;
    gap: 0.6rem;
    padding: 0.18rem 0;
  }
  .clabel,
  .cval {
    font-size: 0.68rem;
    color: var(--fg-faint);
  }
  .cval {
    text-align: right;
  }
  .role {
    font-family: var(--font-mono);
    font-size: 0.66rem;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    padding: 0.05rem 0.35rem;
    border: 1px solid var(--rule);
    color: var(--fg-dim);
  }
  .role[data-role='leader'] {
    color: var(--fg-display);
    border-color: var(--accent);
  }
  .role[data-role='learner'] {
    color: var(--info);
    border-color: var(--info);
  }
  .consequences {
    margin: 0;
    padding-left: 1.1rem;
    font-size: 0.78rem;
    color: var(--fg-dim);
    line-height: 1.7;
  }
  .dim {
    color: var(--fg-faint);
  }
  .warnv {
    color: var(--warn);
  }
  .quiet {
    color: var(--fg-faint);
    margin: 0;
  }
</style>
