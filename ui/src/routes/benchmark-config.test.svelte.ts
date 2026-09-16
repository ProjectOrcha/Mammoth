import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Benchmarks from './benchmarks/+page.svelte';
import Configure from './configure/+page.svelte';
import BenchmarkResults from '$lib/components/BenchmarkResults.svelte';
import type { BenchmarkReport } from '$lib/benchmarks';
import { live } from '$lib/live.svelte';
import { clusterReport } from '$lib/demo';
import { benchmarkDefaults } from '$lib/benchmarks';

const api = vi.hoisted(() => ({ benchmarks: vi.fn(), runBenchmark: vi.fn(), configuration: vi.fn(), validateConfiguration: vi.fn() }));
vi.mock('$lib/api', () => ({ api }));
let component: ReturnType<typeof mount> | undefined;
const config = { settings: { read_cache_size: '256MiB', compute_memory_budget: '128MiB', spill_directory: '', block_size: '128MiB', inline_threshold: '1MiB', replication: 3, ui_listen: '127.0.0.1:8080', s3_listen: '127.0.0.1:9000' }, toml: '[storage]\nreplication = 3\n', restart_required: true, notes: [] };
const settle = async () => { await new Promise(resolve => setTimeout(resolve, 0)); flushSync(); };
beforeEach(() => {
  vi.clearAllMocks();
  const report = clusterReport();
  report.capabilities = { local: true, distributed_metrics: false, history: false, jobs: true, benchmarks: true, configuration: true };
  live.report = report; live.source = 'gateway';
  api.benchmarks.mockResolvedValue({ reports: [], active: null, defaults: benchmarkDefaults });
  api.configuration.mockResolvedValue(config); api.validateConfiguration.mockResolvedValue(config);
});
afterEach(async () => { if (component) await unmount(component); component = undefined; live.report = null; live.source = 'unknown'; document.body.replaceChildren(); });

it('submits real options and does not manufacture demo measurements', async () => {
  component = mount(Benchmarks, { target: document.body }); await settle();
  expect(document.body.textContent).toContain('No measured results yet');
  document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await settle();
  expect(api.runBenchmark).toHaveBeenCalledWith(benchmarkDefaults);
  live.source = 'demo'; flushSync();
  const button = [...document.querySelectorAll('button')].find(b => b.textContent === 'Run benchmark');
  expect(button?.disabled).toBe(true);
  expect(document.body.textContent).toContain('does not contain measured results');
});

it('shows failed runs and disables submission while a run is active', async () => {
  api.benchmarks.mockResolvedValue({ reports: [], active: { state: 'running' }, defaults: benchmarkDefaults });
  component = mount(Benchmarks, { target: document.body }); await settle();
  expect(document.querySelector('fieldset')?.disabled).toBe(true);
  api.benchmarks.mockResolvedValue({ reports: [], active: { state: 'failed', error: 'Disk full' }, defaults: benchmarkDefaults });
  [...document.querySelectorAll('button')].find(b => b.textContent === 'Refresh')!.click(); await settle();
  expect(document.querySelector('[role="alert"]')?.textContent).toContain('Disk full');
});

it('validates a draft without changing active settings and invalidates stale downloads', async () => {
  component = mount(Configure, { target: document.body }); await settle();
  document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await settle();
  expect(api.validateConfiguration).toHaveBeenCalledWith(config.settings);
  expect(document.body.textContent).toContain('Download mammoth.toml');
  const input = document.querySelector('input')!;
  input.value = '4MiB'; input.dispatchEvent(new Event('input', { bubbles: true })); flushSync();
  expect(document.body.textContent).toContain('Validate it again');
  expect(document.body.textContent).not.toContain('Download mammoth.toml');
  expect([...document.querySelectorAll('dd')].map(dd => dd.textContent)).toContain('128MiB');
});

it('submits compute and memory settings in bytes and omits metadata controls', async () => {
  component = mount(Benchmarks, { target: document.body }); await settle();
  const workload = document.querySelector('select')!;
  workload.value = 'compute'; workload.dispatchEvent(new Event('change', { bubbles: true })); flushSync();
  const cache = [...document.querySelectorAll('label')].find(label => label.textContent?.includes('Read cache'))!.querySelector('input')!;
  cache.value = '0'; cache.dispatchEvent(new Event('input', { bubbles: true })); flushSync();
  expect(document.body.textContent).not.toContain('Files per metadata phase');
  document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await settle();
  expect(api.runBenchmark).toHaveBeenCalledWith({ ...benchmarkDefaults, workload: 'compute', read_cache_size: 0 });
});

it('exports edited cache and compute memory settings only after validation', async () => {
  component = mount(Configure, { target: document.body }); await settle();
  const input = [...document.querySelectorAll('label')].find(label => label.textContent?.includes('Memory target per job'))!.querySelector('input')!;
  input.value = '32MiB'; input.dispatchEvent(new Event('input', { bubbles: true })); flushSync();
  document.querySelector('form')!.dispatchEvent(new Event('submit', { bubbles: true, cancelable: true })); await settle();
  expect(api.validateConfiguration).toHaveBeenCalledWith({ ...config.settings, compute_memory_budget: '32MiB' });
});

const legacyReport: BenchmarkReport = {
  schema_version: 1, id: 'legacy', timestamp_ms: 0, scope: 'local',
  environment: { engine: 'local-wal-stream-v2', os: 'macos', arch: 'aarch64', profile: 'release', logical_cpus: 1, data_directory: '/test' },
  options: benchmarkDefaults, summary: [{ phase: 'read', replication: 1, unit: 'MiB/s', median: 1, min: 1, max: 1, median_p99_ms: 1 }],
  samples: [], verified: true, cleanup_complete: true, notes: [],
};
it('keeps historical results out of the default current-engine view', async () => {
  api.benchmarks.mockResolvedValue({ reports: [legacyReport], active: null, defaults: benchmarkDefaults });
  component = mount(Benchmarks, { target: document.body }); await settle();
  expect(document.body.textContent).toContain('No measured results yet for the current engine');
  const measured = [...document.querySelectorAll('h2')].find(h => h.textContent === 'Measured results')!.closest('section');
  expect(measured?.querySelector('table')).toBeNull();
  expect(document.body.textContent).toContain('This saved comparison is independent');
});
it('does not attach current cache settings or read semantics to older results', () => {
  component = mount(BenchmarkResults, { target: document.body, props: { report: legacyReport } }); flushSync();
  expect(document.body.textContent).toContain('Historical engine');
  expect(document.body.textContent).not.toContain('Read · fresh cache');
  expect(document.body.textContent).not.toContain('Read cache:');
});
