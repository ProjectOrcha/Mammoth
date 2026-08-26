<!-- Overview — capacity, throughput, the four fast paths, node health, alerts,
     and whatever the cluster is currently unhappy about. -->
<script lang="ts">
  import { live } from '$lib/live.svelte';
  import { ago, bytes, count, duration, pct, pctValue, rate } from '$lib/format';
  import Panel from '$lib/components/Panel.svelte';
  import Stat from '$lib/components/Stat.svelte';
  import Meter from '$lib/components/Meter.svelte';
  import StateDot from '$lib/components/StateDot.svelte';
  import Sparkline from '$lib/components/Sparkline.svelte';
  import FastPaths from '$lib/components/FastPaths.svelte';

  const report = $derived(live.report);

  const worst = $derived.by(() => {
    if (!report) return [];
    return [...report.nodes]
      .sort(
        (a, b) =>
          Number(b.state === 'dead') - Number(a.state === 'dead') ||
          b.used / b.capacity - a.used / a.capacity,
      )
      .slice(0, 6);
  });

  const healthRows = $derived.by(() => {
    if (!report) return [];
    const h = report.health;
    const total = Object.values(h).reduce((a, b) => a + b, 0);
    return [
      { label: 'healthy', value: h.healthy, tone: 'ok' as const },
      { label: 'degraded', value: h.under_replicated, tone: 'warn' as const },
      { label: 'critical', value: h.critical, tone: 'danger' as const },
      { label: 'over-replicated', value: h.over_replicated, tone: 'accent' as const },
      { label: 'corrupt', value: h.corrupt, tone: 'danger' as const },
      { label: 'missing', value: h.missing, tone: 'danger' as const },
    ].map((r) => ({ ...r, share: total ? (r.value / total) * 100 : 0, total }));
  });
</script>

<header class="page">
  <h1>Overview</h1>
  <p class="eyebrow">
    {report ? `${report.nodes.length} nodes · ${report.nodes.filter((n) => n.state === 'healthy').length} healthy` : 'loading'}
  </p>
</header>

{#if !report}
  <p class="loading mono">reading cluster report…</p>
{:else}
  <div class="stats">
    <Stat
      label="Capacity"
      value={`${bytes(report.used)} / ${bytes(report.capacity)}`}
      note={`${pct(report.used, report.capacity)} used`}
    />
    <Stat label="Read" value={rate(report.throughput.read_bps)} note="across all workers" tone="accent" />
    <Stat label="Write" value={rate(report.throughput.write_bps)} note="client traffic only" tone="accent" />
    <Stat
      label="Block health"
      value={pct(report.health.healthy, report.health.healthy + report.health.under_replicated + report.health.critical, 2)}
      note={`${count(report.health.under_replicated + report.health.critical)} blocks short a fragment`}
      tone={report.health.critical > 0 ? 'danger' : report.health.under_replicated > 0 ? 'warn' : 'ok'}
      href="/distribution"
    />
  </div>

  <section class="section">
    <div class="section-head">
      <h2>The four fast paths</h2>
      <p class="eyebrow">live · click a card for the detail</p>
    </div>
    <FastPaths {report} />
  </section>

  <div class="cols">
    <Panel title="Needs attention" note={`${report.alerts.length} open`}>
      {#if report.alerts.length === 0}
        <p class="quiet">Nothing. Enjoy it.</p>
      {:else}
        <ul class="alerts">
          {#each report.alerts as a (a.id)}
            <li data-level={a.level}>
              <span class="marker" aria-hidden="true">
                {a.level === 'danger' ? '✕' : a.level === 'warn' ? '⚠' : 'ℹ'}
              </span>
              <div>
                <p class="text">{a.text}</p>
                {#if a.fix}<p class="fix mono">{a.fix}</p>{/if}
              </div>
              <span class="when mono">{ago(a.at)}</span>
            </li>
          {/each}
        </ul>
      {/if}
    </Panel>

    <Panel title="Block health" note={`${count(healthRows[0]?.total ?? 0)} blocks`}>
      <ul class="health">
        {#each healthRows as row (row.label)}
          <li>
            <span class="k">{row.label}</span>
            <Meter value={row.share} tone={row.value === 0 ? 'accent' : row.tone} height="0.4rem" />
            <span class="v mono">{count(row.value)}</span>
          </li>
        {/each}
      </ul>
      {#if report.repair.grace_remaining_s > 0}
        <p class="repair mono">
          holding · {duration(report.repair.grace_remaining_s)} of the repair grace
          period left · nothing copied yet
        </p>
      {:else if report.repair.queued > 0}
        <p class="repair mono">
          repairing · {count(report.repair.blocks_per_sec)} blk/s across
          {report.repair.participating} nodes · eta {duration(report.repair.eta_s)}
        </p>
      {/if}
    </Panel>
  </div>

  <Panel title="Workers" note="fullest and unhealthiest first">
    {#snippet actions()}
      <a href="/nodes" class="mono small">all {report.nodes.length} →</a>
    {/snippet}
    <table>
      <thead>
        <tr>
          <th>node</th>
          <th>rack</th>
          <th>state</th>
          <th style="width: 9rem">used</th>
          <th class="num">fragments</th>
          <th class="num">read</th>
          <th>trend</th>
          <th class="num">disk p99</th>
        </tr>
      </thead>
      <tbody>
        {#each worst as n (n.id)}
          <tr>
            <td><a href="/nodes#{n.id}" class="mono">{n.id}</a></td>
            <td class="mono dim">{n.rack}</td>
            <td><StateDot state={n.state} /></td>
            <td>
              <div class="usecell">
                <Meter value={pctValue(n.used, n.capacity)} height="0.35rem" />
                <span class="mono">{pct(n.used, n.capacity)}</span>
              </div>
            </td>
            <td class="num mono">{count(n.fragments)}</td>
            <td class="num mono">{rate(n.read_bps)}</td>
            <td><Sparkline points={n.read_series} /></td>
            <td class="num mono" class:bad={n.disk_p99_ms > 100}>
              {n.disk_p99_ms ? `${n.disk_p99_ms} ms` : '—'}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </Panel>
{/if}

<style>
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }
  .loading {
    color: var(--fg-faint);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(13rem, 1fr));
    gap: var(--gap);
  }
  .section {
    margin-top: 1.5rem;
  }
  .section-head {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 0.7rem;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.35fr) minmax(0, 1fr);
    gap: var(--gap);
    margin: 1.5rem 0;
  }
  @media (max-width: 1000px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }

  .alerts {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .alerts li {
    display: grid;
    grid-template-columns: 1.1rem 1fr auto;
    gap: 0.6rem;
    padding: 0.55rem 0;
    border-bottom: 1px solid var(--rule);
  }
  .alerts li:last-child {
    border-bottom: none;
  }
  .alerts .marker {
    text-align: center;
  }
  .alerts [data-level='danger'] .marker {
    color: var(--danger);
  }
  .alerts [data-level='warn'] .marker {
    color: var(--warn);
  }
  .alerts [data-level='info'] .marker {
    color: var(--info);
  }
  .text {
    margin: 0;
    font-size: 0.8rem;
  }
  .fix {
    margin: 0.2rem 0 0;
    font-size: 0.7rem;
    color: var(--accent);
  }
  .when {
    font-size: 0.68rem;
    color: var(--fg-faint);
    white-space: nowrap;
  }

  .health {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .health li {
    display: grid;
    grid-template-columns: 8rem 1fr 4.2rem;
    align-items: center;
    gap: 0.7rem;
    padding: 0.3rem 0;
  }
  .health .k {
    font-size: 0.74rem;
    color: var(--fg-dim);
  }
  .health .v {
    text-align: right;
    font-size: 0.74rem;
  }
  .repair {
    margin: 0.8rem 0 0;
    padding-top: 0.6rem;
    border-top: 1px solid var(--rule);
    font-size: 0.7rem;
    color: var(--warn);
  }

  .usecell {
    display: grid;
    grid-template-columns: 1fr 2.6rem;
    align-items: center;
    gap: 0.5rem;
  }
  .usecell span {
    font-size: 0.72rem;
    color: var(--fg-dim);
  }
  .dim {
    color: var(--fg-faint);
  }
  .bad {
    color: var(--danger);
  }
  .quiet {
    color: var(--fg-faint);
    margin: 0;
  }
  .small {
    font-size: 0.7rem;
  }
</style>
