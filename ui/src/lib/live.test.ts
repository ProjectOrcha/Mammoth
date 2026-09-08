import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { clusterReport } from './demo';

const mock = vi.hoisted(() => ({ report: vi.fn(), subscribe: vi.fn(), stop: vi.fn() }));
vi.mock('./api', () => ({
  api: { clusterReport: mock.report },
  subscribe: mock.subscribe,
  currentSource: () => 'gateway',
}));
let detach: Array<() => void> = [];
beforeEach(() => {
  vi.resetModules();
  mock.subscribe.mockReturnValue(mock.stop);
});
afterEach(() => { detach.forEach((stop) => stop()); detach = []; vi.resetAllMocks(); });

it('shares in-flight reads and detaches a subscription only once', async () => {
  let finish!: (value: ReturnType<typeof clusterReport>) => void;
  mock.report.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { live } = await import('./live.svelte');
  detach = [live.attach(), live.attach()];
  const reads = [live.refresh(), live.refresh()];
  expect(mock.report).toHaveBeenCalledTimes(1);
  expect(mock.subscribe).toHaveBeenCalledTimes(1);
  finish(clusterReport());
  await Promise.all(reads);
  expect(live.report?.nodes).toHaveLength(12);
  detach[0](); detach[0]();
  expect(mock.stop).not.toHaveBeenCalled();
  detach[1]();
  expect(mock.stop).toHaveBeenCalledTimes(1);
});

it('does not apply a pending report while paused', async () => {
  let finish!: (value: ReturnType<typeof clusterReport>) => void;
  mock.report.mockReturnValue(new Promise((resolve) => { finish = resolve; }));
  const { live } = await import('./live.svelte');
  detach = [live.attach()];
  live.paused = true;
  const pending = live.refresh();
  finish(clusterReport());
  await pending;
  expect(live.report).toBeNull();
  expect(live.updatedAt).toBe(0);
});

it('treats gateway events containing nodes as deltas, not trusted reports', async () => {
  mock.report.mockImplementation(async () => clusterReport());
  const { live } = await import('./live.svelte');
  detach = [live.attach()];
  await live.refresh();
  const onEvent = mock.subscribe.mock.calls[0][0];
  onEvent('node_state', { nodes: [] });
  await live.refresh();
  expect(live.report?.nodes).toHaveLength(12);
  expect(mock.report).toHaveBeenCalledTimes(2);
});
