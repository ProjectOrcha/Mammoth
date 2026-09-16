<script lang="ts">
  import type { BenchmarkReport } from '$lib/benchmarks';
  import { downloadText } from '$lib/benchmarks';
  import { bytes } from '$lib/format';
  let { report, compact = false }: { report: BenchmarkReport; compact?: boolean } = $props();
  const currentEngine = $derived(report.environment.engine === "local-memory-parallel-v3");
  const rows = $derived(compact && report.summary.some(row => row.phase === 'write') ? report.summary.filter(row => row.phase === 'write' || row.phase === 'read' || row.phase === 'read_cached') : report.summary);
  const fmt = (value: number) => value.toLocaleString(undefined, { maximumFractionDigits: 2 });
  const ratio = (phase: string, value: number) => 100 * value / Math.max(...report.summary.filter(row => row.phase === phase).map(row => row.median), 0.001);
</script>

<div class="result-heading">
  <div><span class="badge" class:ok={report.verified && report.cleanup_complete}>{report.verified && report.cleanup_complete ? 'Verified & cleaned' : 'Incomplete'}</span>
    <span class="dim">{new Date(report.timestamp_ms).toLocaleString()}</span></div>
  <button onclick={() => downloadText(`${report.id}.json`, JSON.stringify(report, null, 2))}>Download JSON</button>
</div>
{#if !currentEngine}<p class="scope"><strong>Historical engine:</strong> this report predates the memory and parallel compute update. It does not measure the current engine.</p>{/if}
<p class="scope">Engine: {report.environment.engine ?? "legacy / unrecorded"}.</p>
<p class="scope">{report.scope}. {report.environment.profile} build · {report.environment.os}/{report.environment.arch} · {report.environment.logical_cpus} logical CPUs.</p>
<p class="dim">{report.options.files} × {bytes(report.options.file_size)} · {report.options.concurrency} clients · {report.options.iterations} measured runs · {report.options.warmups} excluded warmups · {report.options.operations} files per metadata phase</p>
<div class="table-wrap"><table>
  <caption>Median of measured iterations; range includes every measured run. Bars compare replica counts within each operation.</caption>
  <thead><tr><th>Operation</th><th>Replicas</th><th>Median rate</th><th>Min–max</th><th>Median p99</th></tr></thead>
  <tbody>{#each rows as row (`${row.phase}-${row.replication}`)}
    <tr><th scope="row">{row.phase === 'read_cached' ? 'Read · repeated cache' : row.phase === 'read' && currentEngine ? 'Read · fresh cache' : row.phase}</th><td>{row.replication}</td><td class="rate"><strong>{fmt(row.median)} {row.unit}</strong><span class="track"><span style={`width:${ratio(row.phase, row.median)}%`}></span></span></td><td>{fmt(row.min)}–{fmt(row.max)}</td><td>{fmt(row.median_p99_ms)} ms</td></tr>
  {/each}</tbody>
</table></div>
<p class="scope">This report measures Mammoth with OS caches retained. See the <a href="/benchmarks#mac-comparison">separate Mac comparison with HDFS and Spark</a>. Physical network and distributed shuffle are unmeasured.</p>
{#if !compact}
  {#if currentEngine && report.options.read_cache_size !== undefined}<p class="scope">Read cache: {bytes(report.options.read_cache_size)} · Memory target per job: {bytes(report.options.compute_memory_budget)}</p>{/if}
  {#if report.samples.some(sample => sample.cache || sample.compute)}
    <div class="table-wrap"><table><caption>Cache and compute evidence for each measured run. Cached bytes include entry overhead; spill bytes include intermediate merges.</caption>
      <thead><tr><th>Operation / run</th><th>Replicas</th><th>Cache hits / misses</th><th>Cache used</th><th>Spilled bytes</th></tr></thead>
      <tbody>{#each report.samples.filter(sample => sample.cache || sample.compute) as sample}
        <tr><th>{sample.phase} / {sample.iteration}</th><td>{sample.replication}</td><td>{sample.cache ? `${fmt(sample.cache.hits)} / ${fmt(sample.cache.misses)}` : '—'}</td><td>{sample.cache ? bytes(sample.cache.resident_bytes) : '—'}</td><td>{sample.compute ? bytes(sample.compute.jobs.reduce((total, job) => total + job.spilled_bytes, 0)) : '—'}</td></tr>
      {/each}</tbody>
    </table></div>
  {/if}
  <details><summary>Methodology & raw measurements</summary>
    <ul>{#each report.notes as note}<li>{note}</li>{/each}</ul>
    <p>Block size: {bytes(report.options.block_size)} · Seed: {report.options.seed} · Inline threshold: 0</p>
    <pre>{JSON.stringify(report.samples, null, 2)}</pre>
  </details>
{/if}

<style>
  .result-heading { display:flex; align-items:center; justify-content:space-between; gap:1rem; flex-wrap:wrap; }
  .result-heading .dim { margin-left:.6rem; font-size:.8rem; }
  .scope { color:var(--fg-dim); font-size:.85rem; }
  .dim { color:var(--fg-dim); }
  .table-wrap { overflow-x:auto; }
  table { width:100%; border-collapse:collapse; font-variant-numeric:tabular-nums; white-space:nowrap; }
  caption { text-align:left; padding:.6rem 0; font-size:.78rem; color:var(--fg-dim); white-space:normal; }
  th,td { padding:.75rem .6rem; text-align:left; border-bottom:1px solid var(--rule); }
  thead th { font-size:.72rem; text-transform:uppercase; color:var(--fg-dim); }
  tbody th { text-transform:capitalize; }
  .rate { min-width:11rem; }
  .track { display:block; background:var(--track); height:4px; margin-top:.4rem; }
  .track span { display:block; height:100%; background:var(--accent); }
  details { margin-top:1rem; } li { margin-bottom:.5rem; color:var(--fg-dim); }
  pre { max-height:24rem; overflow:auto; font-size:.75rem; }
</style>
