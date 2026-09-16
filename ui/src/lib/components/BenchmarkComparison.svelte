<script lang="ts">
  import comparison from '$lib/data/benchmark-comparison.json';
  let replicas = $state(3);
  const rows = $derived(comparison.rows.filter(row => row.replication === replicas));
  const format = (value: { median: number } | null) => value ? value.median.toLocaleString('en-US', { maximumFractionDigits: 2 }) : '—';
</script>
<div class="comparison">
  <p><strong>{comparison.machine}.</strong> Measured {comparison.date}. This saved comparison is independent of the report selected above.</p>
  <p>8 × 8 MiB files, 4 clients, 3 measured runs after warmup. Mammoth uses direct local calls; HDFS uses loopback RPC. Spark runs local compute on HDFS. Memory policies differ. These small workloads do not establish a distributed winner.</p>
  <label>Replica count <select bind:value={replicas}><option value={3}>3 replicas</option><option value={1}>1 replica</option></select></label>
  <div class="table-scroll"><table>
    <caption>Median rates · {replicas} {replicas === 1 ? 'replica' : 'replicas'} · higher is faster</caption>
    <thead><tr><th scope="col">Operation</th><th scope="col">Unit</th><th scope="col">Mammoth</th><th scope="col">HDFS {comparison.versions.hdfs}</th><th scope="col">Spark {comparison.versions.spark}</th></tr></thead>
    <tbody>{#each rows as row}<tr><th scope="row">{row.label}</th><td>{row.unit}</td><td>{format(row.mammoth)}</td><td>{format(row.hdfs)}</td><td>{format(row.spark)}</td></tr>{/each}</tbody>
  </table></div>
  <p class="hint">* Repeated read reuses Mammoth’s verified memory cache; HDFS performs a second normal read. OS caches remain enabled. Neither read phase measures cold-disk speed. A dash means that operation was not measured for that engine.</p>
  <details><summary>Methodology and limits</summary>{#each comparison.caveats as note}<p>{note}</p>{/each}</details>
  <div class="downloads"><a href="/benchmarks/comparison.json" download>Comparison JSON ↓</a>{#each Object.entries(comparison.artifacts) as [name, filename]}<a href={'/benchmarks/'+filename} download>{name === 'hdfs' ? 'HDFS' : name[0].toUpperCase()+name.slice(1)} evidence ↓</a>{/each}</div>
</div>
<style>
  .comparison { line-height:1.6; }
  p { color:var(--fg-dim); font-size:.85rem; }
  label { display:flex; align-items:center; gap:.75rem; font-size:.85rem; }
  .table-scroll { overflow-x:auto; }
  table { width:100%; border-collapse:collapse; margin:1rem 0; font-size:.8rem; font-variant-numeric:tabular-nums; }
  caption { text-align:left; color:var(--fg-dim); margin-bottom:.5rem; }
  td,th { padding:.6rem; border-bottom:1px solid var(--rule); text-align:right; white-space:nowrap; }
  th:first-child,td:nth-child(2) { text-align:left; }
  tbody th { font-weight:400; }
  summary { cursor:pointer; font-size:.85rem; }
  .downloads { display:flex; flex-wrap:wrap; gap:.5rem 1rem; margin-top:1rem; font-size:.8rem; }
</style>
