<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { live } from '$lib/live.svelte';
  import { benchmarkDefaults, type BenchmarkOptions, type BenchmarkState } from '$lib/benchmarks';
  import Panel from '$lib/components/Panel.svelte';
  import BenchmarkResults from '$lib/components/BenchmarkResults.svelte';
  import BenchmarkComparison from '$lib/components/BenchmarkComparison.svelte';

  let options = $state<BenchmarkOptions>({ ...benchmarkDefaults });
  let cacheMiB = $state(256); let computeMiB = $state(32);
  let fileMiB = $state(8); let blockMiB = $state(4); let replicas = $state('1,3');
  let results = $state<BenchmarkState | null>(null);
  let error = $state(''); let loadError = $state(''); let submitting = $state(false); let selected = $state('');
  const running = $derived(submitting || results?.active?.state === 'running');
  const available = $derived(live.source === 'gateway' && live.report?.capabilities?.benchmarks === true);
  const report = $derived(selected ? results?.reports.find(report => report.id === selected) : results?.reports.find(report => report.environment.engine === 'local-memory-parallel-v3'));

  async function load() {
    try { results = await api.benchmarks(); loadError = ''; }
    catch (e) { loadError = e instanceof Error ? e.message : String(e); }
  }
  onMount(() => {
    let stopped = false; let timer: ReturnType<typeof setTimeout>;
    async function poll() { await load(); if (!stopped) timer = setTimeout(poll, 2500); }
    void poll(); return () => { stopped = true; clearTimeout(timer); };
  });
  async function run(event: SubmitEvent) {
    event.preventDefault(); submitting = true; error = '';
    try {
      const request = { ...options, read_cache_size: cacheMiB * 1048576, compute_memory_budget: computeMiB * 1048576, file_size: fileMiB * 1048576, block_size: blockMiB * 1048576, replications: replicas.split(',').map(Number) };
      await api.runBenchmark(request); selected = ''; await load();
    } catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { submitting = false; }
  }
</script>

<svelte:head><title>Benchmarks · Mammoth</title></svelte:head>
<div class="benchmarks">
<header class="page"><h1>Benchmarks</h1><p class="eyebrow">Measure. Verify. Repeat.</p></header>
<p class="intro">Measure storage I/O, metadata, and parallel sort and word count. Compare replica counts and memory settings on this machine, with every measured run kept in the report.</p>
<div class="scope-note"><strong>One host, six simulated workers.</strong> Temporary stores keep benchmark data separate from your files. Reads retain OS caches. Network saturation and distributed TeraSort are not available yet.</div>

<Panel title="Run a benchmark" note="I/O + metadata + verified local compute">
  {#if !available}<p class="hint">Use My storage with an updated running gateway to execute benchmarks. The example workspace does not contain measured results.</p>{/if}
  <form onsubmit={run}>
    <fieldset disabled={!available || running}>
      <label>Workload<select bind:value={options.workload}><option value="suite">Full suite (recommended)</option><option value="dfsio">Storage I/O</option><option value="metadata">Metadata</option><option value="compute">Sort + word count</option></select></label>
      <label>Concurrent clients<input type="number" min="1" max="32" bind:value={options.concurrency} required /></label>
      <label>Replicas<select bind:value={replicas}><option value="1,3">Compare 1 and 3</option><option value="1">1 replica</option><option value="2">2 replicas</option><option value="3">3 replicas</option><option value="6">6 replicas</option></select></label>
      <label>Measured runs<input type="number" min="1" max="10" bind:value={options.iterations} required /></label>
      {#if options.workload !== 'metadata'}
        <label>Files<input type="number" min="1" max="1024" bind:value={options.files} required /></label>
        <label>MiB per file<input type="number" min="1" max="1024" bind:value={fileMiB} required /></label>
        <label>Block size (MiB)<input type="number" min="1" max="256" bind:value={blockMiB} required /></label>
      {/if}
      {#if options.workload === 'suite' || options.workload === 'metadata'}<label>Files per metadata phase<input type="number" min="1" max="100000" bind:value={options.operations} required /></label>{/if}
      <label>Read cache (MiB)<input type="number" min="0" max="1024" bind:value={cacheMiB} required /></label>
      {#if options.workload === 'suite' || options.workload === 'compute'}<label>Memory per job (MiB)<input type="number" min="1" max="256" bind:value={computeMiB} required /></label>{/if}
      <label>Warmup runs<input type="number" min="0" max="3" bind:value={options.warmups} required /></label>
      <label>Random seed<input type="number" min="1" max="4294967295" bind:value={options.seed} required /></label>
    </fieldset>
    <div class="form-actions"><button disabled={!available || running}>{running ? 'Benchmark running…' : 'Run benchmark'}</button><span class="hint">Reads report fresh and reused cache phases separately. Larger jobs spill to disk.</span></div>
  </form>
  {#if running}<p role="status">Running on this machine. You can leave this page and return for the results.</p>{/if}
  {#if results?.active?.state === 'failed'}<p class="load-error" role="alert">Run failed: {results.active.error}</p>{/if}
  {#if error}<p class="load-error" role="alert">{error}</p>{/if}
  {#if loadError}<p class="load-error" role="alert">{loadError}</p>{/if}
</Panel>
</div>

<Panel title="Measured results" note="Latest 20 reports · full JSON export">
  {#snippet actions()}
    {#if results?.reports.length}<select aria-label="Benchmark report" bind:value={selected}><option value="">Latest current-engine report</option>{#each results.reports as result}<option value={result.id}>{new Date(result.timestamp_ms).toLocaleString()} · {result.options.workload}</option>{/each}</select>{/if}
    <button onclick={load}>Refresh</button>
  {/snippet}
  {#if report}<BenchmarkResults {report} />{:else}<p class="empty">No measured results yet for the current engine. Older reports remain in the history selector. Run the suite here, or use <code>mammoth bench suite</code> with this service’s local root.</p>{/if}
</Panel>

<div id="mac-comparison"><Panel title="Mac · Mammoth, Hadoop & Spark" note="Published snapshot · verified outputs"><BenchmarkComparison /></Panel></div>

<style>
  .intro { max-width:75ch; color:var(--fg-dim); }
  .scope-note { border-left:3px solid var(--accent); background:var(--bg-panel); padding:1rem; margin:1.2rem 0; line-height:1.6; }
  .benchmarks :global(.panel) { margin-bottom:1rem; }
  fieldset { border:0; padding:0; margin:0; display:grid; grid-template-columns:repeat(auto-fit,minmax(180px,1fr)); gap:1rem; }
  label { display:grid; gap:.4rem; font-size:.8rem; color:var(--fg-dim); }
  input,select { width:100%; min-width:0; }
  .form-actions { display:flex; gap:1rem; align-items:center; flex-wrap:wrap; margin-top:1.2rem; }
  .hint,.empty { color:var(--fg-dim); font-size:.85rem; }
</style>
