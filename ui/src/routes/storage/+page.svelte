<script lang="ts">
  import { live } from '$lib/live.svelte';
  import {
    ago,
    bytes,
    count,
    duration,
    pct,
    pctValue,
    rate,
  } from '$lib/format';
  import { priorityNodes, summarizeCluster } from '$lib/overview';
  import OverviewMetric from '$lib/components/OverviewMetric.svelte';
  import Meter from '$lib/components/Meter.svelte';
  import StateDot from '$lib/components/StateDot.svelte';
  import Sparkline from '$lib/components/Sparkline.svelte';
  import FastPaths from '$lib/components/FastPaths.svelte';
  import StoragePaths from '$lib/components/StoragePaths.svelte';
  import Panel from '$lib/components/Panel.svelte';
  import LatestBenchmark from '$lib/components/LatestBenchmark.svelte';

  const report = $derived(live.report);
  const summary = $derived(report ? summarizeCluster(report) : null);
  const workers = $derived(priorityNodes(report?.nodes ?? []));
  let allAlerts = $state(false);
  let refreshing = $state(false);
  const sortedAlerts = $derived(
    [...(report?.alerts ?? [])].sort(
      (a, b) =>
        ({ danger: 2, warn: 1, info: 0 })[b.level] -
        { danger: 2, warn: 1, info: 0 }[a.level],
    ),
  );
  const visibleAlerts = $derived(
    allAlerts ? sortedAlerts : sortedAlerts.slice(0, 3),
  );
  const healthRows = $derived(
    report
      ? [
          { label: 'Healthy', value: report.health.healthy, tone: 'ok' },
          {
            label: 'Under-replicated',
            value: report.health.under_replicated,
            tone: 'warn',
          },
          { label: 'Critical', value: report.health.critical, tone: 'danger' },
          {
            label: 'Over-replicated',
            value: report.health.over_replicated,
            tone: 'info',
          },
          { label: 'Corrupt', value: report.health.corrupt, tone: 'danger' },
          { label: 'Missing', value: report.health.missing, tone: 'danger' },
        ]
      : [],
  );

  async function refresh() {
    refreshing = true;
    live.paused = false;
    try {
      await live.refresh();
    } finally {
      refreshing = false;
    }
  }
</script>

<svelte:head><title>Overview · Mammoth</title></svelte:head>

<div class="overview">
  <header class="page-heading">
    <div>
      <p class="kicker">Your cluster, at a glance</p>
      <h1>Overview</h1>
      <p class="intro">
        Storage, workers, and the things that need your attention.
      </p>
    </div>
    <div class="page-actions">
      <button class="action secondary" onclick={refresh} disabled={refreshing}>
        <span aria-hidden="true">↻</span>
        {refreshing ? 'Refreshing…' : 'Refresh'}
      </button>
      <a class="action primary" href="/files"
        >Browse files <span aria-hidden="true">→</span></a
      >
    </div>
  </header>

  <LatestBenchmark />
  {#if live.source === 'gateway' && report?.memory_cache}
    <Panel title="Read cache" note="Verified bytes held in memory">
      {#snippet actions()}<a href="/configure">Configure memory →</a>{/snippet}
      <p>{bytes(report.memory_cache.resident_bytes)} used of {bytes(report.memory_cache.capacity_bytes)} · {count(report.memory_cache.entries)} cached ranges</p>
      <p>{count(report.memory_cache.hits)} hits · {count(report.memory_cache.misses)} misses · {count(report.memory_cache.evictions)} evictions since this service started.</p>
      <p>Repeated reads can reuse verified bytes. File changes select the new version; health checks still inspect storage.</p>
    </Panel>
  {/if}
  {#if !report || !summary}
    <section
      class="empty-state"
      aria-live="polite"
      aria-busy={refreshing || !live.error}
    >
      <span class="empty-icon" aria-hidden="true">◈</span>
      <h2>
        {live.error
          ? 'The cluster is out of reach'
          : 'Connecting to your cluster'}
      </h2>
      <p>
        {live.error
          ? 'No report is available yet. Check the gateway connection, then try again.'
          : 'Capacity, worker health, and activity will appear here when the report arrives.'}
      </p>
      {#if live.error}<button
          class="action secondary"
          onclick={refresh}
          disabled={refreshing}
          >{refreshing ? 'Trying again…' : 'Try again'}</button
        >{/if}
    </section>
  {:else}
    <section
      class="cluster-summary"
      class:attention={summary.needsAttention}
      aria-label="Cluster status"
    >
      <span class="status-icon" aria-hidden="true"
        >{summary.needsAttention ? '!' : '✓'}</span
      >
      <div>
        <h2>
          {report.safe_mode
            ? 'Cluster is in safe mode'
            : report.nodes.length === 0
              ? 'No workers connected'
              : summary.needsAttention
                ? 'Your cluster needs attention'
                : 'Your cluster is healthy'}
        </h2>
        <p>
          {summary.healthyNodes} of {report.nodes.length} workers healthy
          {#if summary.deadNodes}
            · {summary.deadNodes} offline{/if}
          {#if summary.atRisk}
            · {count(summary.atRisk)} critical, corrupt, or missing blocks{/if}
        </p>
      </div>
      <a
        href={report.safe_mode
          ? '/cluster'
          : summary.needsAttention
            ? '#attention'
            : '/nodes'}
      >
        {report.safe_mode
          ? 'Inspect cluster'
          : summary.needsAttention
            ? 'Review issues'
            : 'View workers'} <span aria-hidden="true">→</span>
      </a>
    </section>

    <div class="metrics">
      <OverviewMetric
        label="Storage used"
        value={bytes(report.used)}
        note={`${bytes(Math.max(0, report.capacity - report.used))} available of ${bytes(report.capacity)}`}
        href="/distribution"
      >
        <Meter
          value={pctValue(report.used, report.capacity)}
          height="0.3rem"
          label="Storage used"
        />
      </OverviewMetric>
      <OverviewMetric
        label="Healthy workers"
        value={`${summary.healthyNodes} / ${report.nodes.length}`}
        note={`${new Set(report.nodes.map((n) => n.rack)).size} racks · ${report.nodes.length - summary.healthyNodes} need attention`}
        href="/nodes"
      >
        <div class="worker-dots" aria-hidden="true">
          {#each report.nodes.slice(0, 24) as node (node.id)}<span
              data-state={node.state}
            ></span>{/each}
        </div>
      </OverviewMetric>
      {#if report.capabilities?.local}
        <OverviewMetric label="Stored blocks" value={count(summary.totalBlocks)} note="Files below the inline threshold stay in metadata" href="/files">
          <div class="metric-caption">Browse your files and their block layouts</div>
        </OverviewMetric>
        <OverviewMetric label="Replica copies" value={count(report.nodes.reduce((total, node) => total + node.fragments, 0))} note="Stored across the worker directories" href="/distribution">
          <div class="metric-caption">Inspect placement and redundancy</div>
        </OverviewMetric>
      {:else}
      <OverviewMetric
        label="Read throughput"
        value={report.throughput.read_bps === 0
          ? '0 B/s'
          : rate(report.throughput.read_bps)}
        note="Across all workers"
      >
        <div class="metric-caption">
          <span class="direction" aria-hidden="true">↓</span> From storage to clients
        </div>
      </OverviewMetric>
      <OverviewMetric
        label="Write throughput"
        value={report.throughput.write_bps === 0
          ? '0 B/s'
          : rate(report.throughput.write_bps)}
        note="Client traffic only"
      >
        <div class="metric-caption">
          <span class="direction" aria-hidden="true">↑</span> From clients to storage
        </div>
      </OverviewMetric>
      {/if}
    </div>

    <div class="health-layout">
      <section
        class="surface incidents"
        id="attention"
        aria-labelledby="attention-title"
      >
        <header class="section-heading">
          <div>
            <h2 id="attention-title">
              Needs attention <span class="count-badge"
                >{report.alerts.length}</span
              >
            </h2>
            <p>
              Highest severity first. Inspect an issue before making changes.
            </p>
          </div>
        </header>
        {#if visibleAlerts.length === 0}
          <p class="quiet">
            No open alerts. Worker and block health are shown separately.
          </p>
        {:else}
          <ul class="alerts" id="alert-list">
            {#each visibleAlerts as alert (alert.id)}
              <li data-level={alert.level}>
                <div class="alert-meta">
                  <span class="severity"
                    >{alert.level === 'danger'
                      ? 'Critical'
                      : alert.level === 'warn'
                        ? 'Warning'
                        : 'Info'}</span
                  ><time datetime={new Date(alert.at).toISOString()}
                    >{ago(alert.at)}</time
                  >
                </div>
                <p class="alert-text">{alert.text}</p>
                <div class="alert-actions">
                  <a
                    href={report.nodes.some((n) => n.id === alert.id)
                      ? `/nodes#${encodeURIComponent(alert.id)}`
                      : '/distribution'}
                    >Inspect {report.nodes.some((n) => n.id === alert.id)
                      ? 'worker'
                      : 'distribution'} <span aria-hidden="true">↗</span></a
                  >
                  {#if alert.fix}
                    <details>
                      <summary>Suggested command</summary><code
                        >{alert.fix}</code
                      >{#if live.source === 'demo'}<p>
                          Example only; this command is not implemented in the
                          Rust scaffold.
                        </p>{/if}
                    </details>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
          {#if sortedAlerts.length > 3}
            <button
              class="show-alerts"
              aria-expanded={allAlerts}
              aria-controls="alert-list"
              onclick={() => (allAlerts = !allAlerts)}
              >{allAlerts
                ? 'Show fewer alerts'
                : `Show all ${sortedAlerts.length} alerts`}
              <span aria-hidden="true">{allAlerts ? '−' : '+'}</span></button
            >
          {/if}
        {/if}
      </section>

      <section class="surface block-health" aria-labelledby="health-title">
        <header class="section-heading">
          <h2 id="health-title">Block health</h2>
          <a href="/distribution" aria-label="Explore block distribution"
            >Explore <span aria-hidden="true">↗</span></a
          >
        </header>
        <p class="health-number">
          {summary.totalBlocks
            ? pct(report.health.healthy, summary.totalBlocks, 1)
            : '—'} <span>healthy</span>
        </p>
        <p class="health-note">
          {summary.totalBlocks
            ? `${count(summary.totalBlocks)} blocks across your cluster`
            : 'No blocks reported yet'}
        </p>
        <div class="health-bar" aria-hidden="true">
          {#each healthRows as row (row.label)}<span
              style:width={`${pctValue(row.value, summary.totalBlocks)}%`}
              style:background={`var(--${row.tone})`}
            ></span>{/each}
        </div>
        <dl class="health-legend">
          {#each healthRows as row (row.label)}<div>
              <dt>
                <span style:background={`var(--${row.tone})`} aria-hidden="true"
                ></span>{row.label}
              </dt>
              <dd>{count(row.value)}</dd>
            </div>{/each}
        </dl>
        {#if report.capabilities?.distributed_metrics !== false}
        <div class="repair">
          <div class="repair-heading">
            <h3>
              {report.repair.grace_remaining_s > 0
                ? 'Repair waiting'
                : report.repair.queued > 0
                  ? 'Repair in progress'
                  : 'Repair queue clear'}
            </h3>
            <span
              >{report.repair.queued > 0 && report.repair.total > 0
                ? pct(
                    Math.max(0, report.repair.total - report.repair.queued),
                    report.repair.total,
                  )
                : ''}</span
            >
          </div>
          {#if report.repair.queued > 0}
            <Meter
              value={Math.max(0, report.repair.total - report.repair.queued)}
              max={report.repair.total}
              tone="accent"
              height="0.3rem"
              label="Repair progress"
            />
            <p>
              {report.repair.grace_remaining_s > 0
                ? `${duration(report.repair.grace_remaining_s)} grace period remaining before copying starts.`
                : `${count(report.repair.queued)} blocks left · ${report.repair.participating} workers rebuilding`}
            </p>
            {#if report.repair.grace_remaining_s === 0}<p class="repair-eta">
                {report.repair.blocks_per_sec > 0
                  ? `Estimated completion in ${duration(report.repair.eta_s)}`
                  : 'Waiting for repair activity'}
              </p>{/if}
          {:else}<p>No blocks are currently waiting to be rebuilt.</p>{/if}
        </div>
        {:else}<div class="repair"><h3>Replica protection</h3><p>Checksums protect each stored copy. Inspect placement or run a repair check from Cluster.</p><a href="/cluster">Check and repair replicas →</a></div>{/if}
      </section>
    </div>

    <section class="surface workers" aria-labelledby="workers-title">
      <header class="section-heading">
        <div>
          <h2 id="workers-title">Workers</h2>
          <p>Unhealthy workers first, then highest storage use.</p>
        </div>
        <a href="/nodes"
          >View all {report.nodes.length} <span aria-hidden="true">→</span></a
        >
      </header>
      {#if workers.length === 0}<p class="quiet">
          Workers will appear here after they join the cluster.
        </p>{:else}
        <!-- svelte-ignore a11y_no_noninteractive_tabindex (The horizontal scroll region needs keyboard focus for arrow-key scrolling.) -->
        <div
          class="table-scroll"
          role="region"
          aria-label="Worker health table"
          tabindex="0"
        >
          <table>
            <thead
              ><tr
                ><th scope="col">Worker / rack</th><th scope="col">Status</th
                ><th scope="col">Storage used</th><th scope="col" class="num"
                  >Read rate</th
                ><th scope="col">Recent reads</th><th scope="col" class="num"
                  ><abbr
                    title="99th percentile disk latency: 99% of operations finish within this time"
                    >Disk p99</abbr
                  ></th
                ></tr
              ></thead
            >
            <tbody>
              {#each workers as node (node.id)}
                <tr>
                  <td
                    ><a
                      class="worker-name"
                      href={`/nodes#${encodeURIComponent(node.id)}`}
                      >{node.id} <span aria-hidden="true">↗</span></a
                    ><span class="rack">{node.rack}</span></td
                  >
                  <td><StateDot state={node.state} /></td>
                  <td
                    ><div class="usecell">
                      <Meter
                        value={pctValue(node.used, node.capacity)}
                        height="0.3rem"
                        label={`${node.id} storage used`}
                      /><span
                        >{node.capacity
                          ? pct(node.used, node.capacity)
                          : '—'}</span
                      >
                    </div></td
                  >
                  <td class="num">{rate(node.read_bps)}</td>
                  <td><Sparkline points={node.read_series} /></td>
                  <td class="num" class:bad={node.disk_p99_ms > 100}
                    >{node.disk_p99_ms ? `${node.disk_p99_ms} ms` : '—'}</td
                  >
                </tr>
              {/each}
            </tbody>
          </table>
        </div>
      {/if}
    </section>

    {#if report.capabilities?.local}
      <section class="performance" aria-labelledby="tools-title">
        <header class="section-heading"><div><h2 id="tools-title">Inside your storage</h2><p>Follow a file from upload to verified reads and recovery.</p></div><a href="/cluster">Cluster details ↗</a></header>
        <StoragePaths {report} />
      </section>
    {:else}
    <section class="performance" aria-labelledby="performance-title">
      <header class="section-heading">
        <div>
          <p class="kicker">A closer look</p>
          <h2 id="performance-title">The four fast paths</h2>
          <p>
            {live.source === 'demo'
              ? 'Simulated performance'
              : 'Reported performance'} · Expand a card to explore how it works.
          </p>
        </div>
        <a href="/cluster">Cluster details <span aria-hidden="true">→</span></a>
      </header>
      <FastPaths {report} />
    </section>
    {/if}
    <footer class="overview-footer">
      <span
        >{live.source === 'demo'
          ? 'Demo workspace · Simulated data'
          : 'Cluster workspace'}</span
      ><a href="/distribution"
        >See where your data lives <span aria-hidden="true">→</span></a
      >
    </footer>
  {/if}
</div>

<style>
  .overview {
    max-width: 1440px;
    margin: 0 auto;
  }
  .page-heading {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1.5rem;
    margin: 0.5rem 0 1.7rem;
    flex-wrap: wrap;
  }
  .kicker {
    color: var(--fg-dim);
    font-size: 0.68rem;
    font-family: var(--font-mono);
    letter-spacing: 0.15em;
    text-transform: uppercase;
    margin: 0 0 0.5rem;
  }
  h1 {
    font-size: 2.2rem;
    letter-spacing: 0.01em;
    text-transform: none;
  }
  .intro {
    color: var(--fg-dim);
    margin: 0.5rem 0 0;
    font-size: 0.87rem;
  }
  .page-actions {
    display: flex;
    gap: 0.6rem;
    flex-wrap: wrap;
  }
  .action {
    padding: 0.65rem 1rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.7rem;
    font-size: 0.8rem;
    border: 1px solid var(--rule-strong);
    border-radius: 4px;
    text-decoration: none;
    min-height: 42px;
  }
  .primary {
    background: var(--accent);
    color: var(--bg);
    border-color: var(--accent);
    font-weight: 600;
  }
  .primary:hover {
    filter: brightness(1.12);
  }
  .secondary {
    background: var(--bg-panel);
    color: var(--fg);
  }
  button:disabled {
    cursor: wait;
    opacity: 0.65;
  }
  .cluster-summary {
    display: flex;
    align-items: center;
    gap: 1rem;
    padding: 1rem 1.2rem;
    border: 1px solid var(--rule);
    border-left: 3px solid var(--ok);
    border-radius: 4px;
    background: var(--bg-panel);
    margin-bottom: 1rem;
  }
  .cluster-summary.attention {
    border-left-color: var(--warn);
  }
  .cluster-summary h2 {
    font: 550 0.9rem var(--font-ui);
    text-transform: none;
    letter-spacing: 0;
    color: var(--fg);
  }
  .cluster-summary p {
    color: var(--fg-dim);
    margin: 0.3rem 0 0;
    font-size: 0.78rem;
  }
  .cluster-summary > a {
    margin-left: auto;
    flex-shrink: 0;
    font-size: 0.8rem;
  }
  .status-icon {
    display: grid;
    place-items: center;
    width: 2rem;
    height: 2rem;
    flex-shrink: 0;
    border: 1px solid var(--rule-strong);
    border-radius: 50%;
    color: var(--ok);
    font-weight: 600;
  }
  .attention .status-icon {
    color: var(--warn);
  }
  .metrics {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 1rem;
  }
  .worker-dots {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-height: 5px;
  }
  .worker-dots span {
    height: 5px;
    flex: 1;
    max-width: 24px;
    min-width: 3px;
    background: var(--ok);
    border-radius: 1px;
  }
  .worker-dots [data-state='warn'],
  .worker-dots [data-state='decommissioning'],
  .worker-dots [data-state='maintenance'] {
    background: var(--warn);
  }
  .worker-dots [data-state='dead'] {
    background: var(--danger);
  }
  .metric-caption {
    font-size: 0.72rem;
    color: var(--fg-dim);
  }
  .direction {
    color: var(--accent);
    margin-right: 0.2rem;
  }
  .health-layout {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    gap: 1rem;
    margin: 1.4rem 0;
  }
  .surface {
    background: var(--bg-panel);
    border: 1px solid var(--rule);
    border-radius: 6px;
    min-width: 0;
  }
  .section-heading {
    padding: 1.2rem;
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 1rem;
    flex-wrap: wrap;
  }
  .section-heading h2 {
    font: 550 1rem var(--font-ui);
    letter-spacing: 0;
    text-transform: none;
    color: var(--fg);
    display: flex;
    gap: 0.6rem;
    align-items: center;
  }
  .section-heading p {
    margin: 0.4rem 0 0;
    font-size: 0.75rem;
    color: var(--fg-dim);
  }
  .section-heading a {
    font-size: 0.78rem;
  }
  .count-badge {
    font: 500 0.72rem var(--font-ui);
    padding: 0.1rem 0.45rem;
    border-radius: 4px;
    background: var(--bg-raised);
    color: var(--fg-dim);
  }
  #attention {
    scroll-margin-top: 5rem;
  }
  .alerts {
    list-style: none;
    margin: 0;
    padding: 0 1.2rem;
  }
  .alerts li {
    padding: 1rem 0;
    border-top: 1px solid var(--rule);
  }
  .alert-meta {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    font-size: 0.67rem;
  }
  .severity {
    border: 1px solid currentColor;
    border-radius: 3px;
    padding: 0.05rem 0.4rem;
    font-weight: 550;
  }
  [data-level='danger'] .severity {
    color: var(--danger);
  }
  [data-level='warn'] .severity {
    color: var(--warn);
  }
  [data-level='info'] .severity {
    color: var(--info);
  }
  time {
    color: var(--fg-dim);
  }
  .alert-text {
    font-size: 0.83rem;
    margin: 0.5rem 0;
    line-height: 1.6;
  }
  .alert-actions {
    display: flex;
    align-items: baseline;
    gap: 0.5rem 1rem;
    flex-wrap: wrap;
    font-size: 0.73rem;
  }
  .alert-actions details {
    min-width: 0;
    color: var(--fg-dim);
  }
  .alert-actions summary {
    cursor: pointer;
    padding: 0.25rem 0;
  }
  .alert-actions details[open] {
    flex-basis: 100%;
  }
  .alert-actions code {
    display: block;
    margin-top: 0.5rem;
    padding: 0.65rem;
    background: var(--bg);
    border: 1px solid var(--rule);
    color: var(--accent);
    overflow-wrap: anywhere;
    font: 0.75rem/1.6 var(--font-mono);
  }
  .alert-actions details p {
    margin: 0.4rem 0 0;
  }
  .show-alerts {
    border: none;
    border-top: 1px solid var(--rule);
    width: 100%;
    padding: 0.85rem 1.2rem;
    color: var(--accent);
    font-size: 0.78rem;
    display: flex;
    justify-content: space-between;
  }
  .health-number {
    font-size: 2.6rem;
    letter-spacing: -0.045em;
    margin: 0 1.2rem;
    line-height: 1.1;
  }
  .health-number span {
    font-size: 0.85rem;
    letter-spacing: 0;
    color: var(--fg-dim);
  }
  .health-note {
    color: var(--fg-dim);
    font-size: 0.75rem;
    margin: 0.4rem 1.2rem 1.2rem;
  }
  .health-bar {
    display: flex;
    margin: 0 1.2rem 1.1rem;
    height: 9px;
    background: var(--track);
    border-radius: 2px;
    overflow: hidden;
  }
  .health-legend {
    margin: 0 1.2rem 1.2rem;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.7rem 1.2rem;
  }
  .health-legend div {
    display: flex;
    justify-content: space-between;
    gap: 0.4rem;
    font-size: 0.73rem;
  }
  dt {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    color: var(--fg-dim);
  }
  dt span {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    flex-shrink: 0;
  }
  dd {
    margin: 0;
  }
  .repair {
    background: var(--bg);
    margin: 0 1.2rem 1.2rem;
    padding: 1rem;
    border: 1px solid var(--rule);
    border-radius: 4px;
  }
  .repair-heading {
    display: flex;
    justify-content: space-between;
    margin-bottom: 0.7rem;
    gap: 0.5rem;
    font-size: 0.78rem;
    color: var(--accent);
  }
  .repair h3 {
    font: 500 0.8rem var(--font-ui);
    letter-spacing: 0;
    text-transform: none;
    color: var(--fg);
  }
  .repair p {
    color: var(--fg-dim);
    font-size: 0.73rem;
    margin: 0.7rem 0 0;
  }
  .repair .repair-eta {
    margin-top: 0.25rem;
    color: var(--accent);
  }
  .table-scroll {
    overflow-x: auto;
    padding: 0 1.2rem;
  }
  .table-scroll:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: -2px;
  }
  table {
    min-width: 610px;
  }
  th {
    font: 0.7rem var(--font-ui);
    letter-spacing: 0;
    text-transform: none;
    color: var(--fg-dim);
    padding-block: 0.6rem;
  }
  td {
    padding-block: 0.8rem;
  }
  tbody tr:last-child td {
    border-bottom: none;
  }
  .worker-name {
    font-weight: 550;
    font-size: 0.83rem;
  }
  .worker-name span {
    color: var(--fg-dim);
    margin-left: 0.3rem;
    font-size: 0.7rem;
  }
  .rack {
    display: block;
    color: var(--fg-dim);
    font: 0.65rem var(--font-mono);
    margin-top: 0.2rem;
  }
  .usecell {
    display: grid;
    grid-template-columns: minmax(4rem, 1fr) 2.4rem;
    align-items: center;
    gap: 0.65rem;
    font-size: 0.75rem;
  }
  .bad {
    color: var(--danger);
  }
  abbr {
    text-underline-offset: 3px;
    cursor: help;
  }
  .performance {
    margin-top: 1.8rem;
  }
  .performance .section-heading {
    padding: 0 0 1rem;
  }
  .performance .kicker {
    margin: 0 0 0.5rem;
  }
  .overview-footer {
    display: flex;
    justify-content: space-between;
    gap: 0.7rem;
    flex-wrap: wrap;
    color: var(--fg-dim);
    font-size: 0.7rem;
    padding: 1.5rem 0 0.3rem;
  }
  .quiet {
    color: var(--fg-dim);
    padding: 0 1.2rem 1.2rem;
  }
  .empty-state {
    text-align: center;
    border: 1px dashed var(--rule-strong);
    border-radius: 6px;
    padding: 4rem 1.5rem;
    background: var(--bg-panel);
  }
  .empty-state h2 {
    font-size: 1.2rem;
    text-transform: none;
    letter-spacing: 0;
  }
  .empty-state p {
    color: var(--fg-dim);
    max-width: 48ch;
    margin: 1rem auto;
  }
  .empty-icon {
    color: var(--accent);
    display: block;
    font-size: 2.5rem;
    margin-bottom: 1rem;
  }
  @media (max-width: 1200px) {
    .health-layout {
      grid-template-columns: 1.15fr 1fr;
    }
    .health-legend {
      grid-template-columns: 1fr;
    }
  }
  @media (max-width: 1050px) {
    .metrics {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
    .health-layout {
      grid-template-columns: 1fr;
    }
    .health-legend {
      grid-template-columns: 1fr 1fr;
    }
  }
  @media (max-width: 600px) {
    .page-heading {
      margin-top: 0.5rem;
      gap: 1rem;
    }
    .page-actions {
      width: 100%;
    }
    .page-actions a {
      flex: 1;
    }
    .cluster-summary {
      flex-wrap: wrap;
      gap: 0.7rem;
      padding: 1rem;
    }
    .cluster-summary > div {
      flex: 1;
      min-width: 180px;
    }
    .cluster-summary > a {
      margin-left: 2.7rem;
    }
    .metrics {
      gap: 0.65rem;
    }
    .health-legend {
      gap: 0.7rem;
    }
    .section-heading {
      gap: 0.7rem;
    }
    .intro {
      font-size: 0.8rem;
    }
  }
  @media (max-width: 360px) {
    .health-legend {
      grid-template-columns: 1fr;
    }
  }
</style>
