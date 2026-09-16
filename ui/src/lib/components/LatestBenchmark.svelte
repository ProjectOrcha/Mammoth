<script lang="ts">
  import { api } from '$lib/api';
  import { live } from '$lib/live.svelte';
  import type { BenchmarkReport } from '$lib/benchmarks';
  import Panel from './Panel.svelte';
  import BenchmarkResults from './BenchmarkResults.svelte';
  let report = $state<BenchmarkReport | null>(null);
  let error = $state('');
  const available = $derived(live.source === 'gateway' && live.report?.capabilities?.benchmarks === true);
  $effect(() => {
    if (!available) return;
    let stopped = false; let timer: ReturnType<typeof setTimeout>;
    async function refresh() {
      try { const state = await api.benchmarks(); if (!stopped) { report = state.reports.find(item => item.environment.engine === "local-memory-parallel-v3") ?? null; error = ''; } }
      catch (e) { if (!stopped) error = e instanceof Error ? e.message : String(e); }
      if (!stopped) timer = setTimeout(refresh, 10000);
    }
    void refresh(); return () => { stopped = true; clearTimeout(timer); };
  });
</script>
{#if available}
  <Panel title="Latest benchmark" note="Measured on this machine">
    {#snippet actions()}<a href="/benchmarks">Run & compare →</a>{/snippet}
    {#if error}<p role="alert">Unable to load benchmark results: {error}</p>{/if}
    {#if report}<BenchmarkResults {report} compact />{:else if !error}<p>No measurements for the updated engine yet. <a href="/benchmarks">Run the benchmark suite</a> to measure your storage.</p>{/if}
  </Panel>
{/if}
