import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Distribution from './distribution/+page.svelte';
import Cluster from './cluster/+page.svelte';
import { live } from '$lib/live.svelte';
import { clusterReport } from '$lib/demo';

const api = vi.hoisted(() => ({ heat: vi.fn(), flow: vi.fn(), topology: vi.fn(), treemap: vi.fn(), skew: vi.fn(), files: vi.fn(), blocks: vi.fn() }));
vi.mock('$lib/api', () => ({ api }));
vi.mock('echarts/core', () => ({ init: () => ({ setOption() {}, resize() {}, dispose() {}, on() {} }), use() {} }));
let component: ReturnType<typeof mount> | undefined;
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  const report = clusterReport();
  report.capabilities = { local: true, distributed_metrics: false, history: false, jobs: true };
  live.report = report; live.source = 'gateway';
  api.heat.mockResolvedValue([]); api.flow.mockResolvedValue({ nodes: [], links: [], window_s: 0 });
  api.topology.mockResolvedValue({ nodes: [], links: [], racks: [], epoch: 1 });
  api.treemap.mockResolvedValue({ value: 0 }); api.skew.mockResolvedValue({ files: 0 }); api.files.mockResolvedValue([]);
});
afterEach(async () => {
  if (component) await unmount(component);
  live.report = null; live.source = 'unknown'; document.body.replaceChildren(); vi.unstubAllGlobals();
});

it('keeps all six distribution panels and the data paths visible for local storage', async () => {
  component = mount(Distribution, { target: document.body });
  await new Promise(resolve => setTimeout(resolve, 0)); flushSync();
  const headings = [...document.querySelectorAll('h2')].map(heading => heading.textContent);
  expect(headings).toEqual(['1 · Node heat grid', '2 · Block placement matrix', '3 · Namespace treemap', '4 · Rack topology', '5 · Skew scatter', '6 · Replica distribution', 'Read, write & recovery']);
  expect(document.querySelector('[aria-label="Storage snapshot"]')).not.toBeNull();
  expect(document.querySelectorAll('details')).toHaveLength(4);
});

it('shows local topology, maintenance, recovery and all four lifecycle cards', () => {
  component = mount(Cluster, { target: document.body }); flushSync();
  expect([...document.querySelectorAll('h2')].map(heading => heading.textContent)).toEqual(['Storage topology', 'Storage maintenance', 'Namespace & recovery', 'Data lifecycle']);
  expect(document.querySelectorAll('details')).toHaveLength(4);
  expect(document.querySelectorAll('a[href^="/nodes#"]')).toHaveLength(live.report!.nodes.length);
});
