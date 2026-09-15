import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Treemap from './Treemap.svelte';
import SkewScatter from './SkewScatter.svelte';
import BlockMatrix from './BlockMatrix.svelte';
import type { BlockLayout, TreemapNode } from '../types';

const engine = vi.hoisted(() => ({ setOption: vi.fn(), resize: vi.fn(), dispose: vi.fn(), on: vi.fn() }));
vi.mock('echarts/core', () => ({ init: () => engine, use: vi.fn() }));
let component: ReturnType<typeof mount> | undefined;
beforeEach(() => {
  vi.stubGlobal('ResizeObserver', class { observe() {} disconnect() {} });
  vi.clearAllMocks();
});
afterEach(async () => {
  if (component) await unmount(component);
  document.body.replaceChildren();
  vi.unstubAllGlobals();
});

it('redraws charts when live data and colour controls change, then disposes', async () => {
  let root = $state({ name: '/', path: '/', value: 100, age_days: 60, reads: 0 });
  let colourBy = $state<'age' | 'reads'>('age');
  component = mount(Treemap, { target: document.body, props: { get root() { return root; }, get colourBy() { return colourBy; } } });
  flushSync();
  const initial = engine.setOption.mock.lastCall![0].series[0].data[0];
  colourBy = 'reads';
  flushSync();
  const changed = engine.setOption.mock.lastCall![0].series[0].data[0];
  expect(changed.itemStyle.color).not.toBe(initial.itemStyle.color);
  root.value = 250;
  flushSync();
  expect(engine.setOption.mock.lastCall![0].series[0].data[0].value).toBe(250);
  await unmount(component);
  component = undefined;
  expect(engine.dispose).toHaveBeenCalledOnce();
});

it('keeps the open folder across live refreshes and allows navigation to tiny entries', () => {
  const tree = (size: number): TreemapNode => ({ name: '', path: '/', value: size + 1, age_days: 0, reads: null, children: [
    { name: 'large', path: '/large', value: size, age_days: 0, reads: null, children: [
      { name: 'data', path: '/large/data', value: size, age_days: 0, reads: null },
    ] },
    { name: 'tiny', path: '/tiny', value: 1, age_days: 0, reads: null, children: [
      { name: 'data', path: '/tiny/data', value: 1, age_days: 0, reads: null },
    ] },
  ] });
  let root = $state(tree(100_000_000));
  component = mount(Treemap, { target: document.body, props: { get root() { return root; } } });
  flushSync();
  const click = engine.on.mock.calls.find(([name]) => name === 'click')![1];
  click({ data: { path: '/large' } }); flushSync();
  root = tree(200_000_000); flushSync();
  const data = () => engine.setOption.mock.lastCall![0].series[0].data;
  expect(data().map((entry: { path: string }) => entry.path)).toEqual(['/large/data']);
  expect(data()[0].value).toBe(200_000_000);
  expect(document.querySelector('[aria-current="location"]')?.textContent).toBe('large');
  (document.querySelector('nav button') as HTMLButtonElement).click(); flushSync();
  [...document.querySelectorAll('button')].find(button => button.textContent === 'tiny')!.click(); flushSync();
  expect(data()[0].path).toBe('/tiny/data');
  const remaining = tree(200_000_000);
  remaining.children = remaining.children!.filter(entry => entry.path !== '/tiny');
  root = remaining; flushSync();
  expect(document.querySelector('[aria-current="location"]')?.textContent).toBe('/');
  expect(data()[0].path).toBe('/large');
});

it('plots files with zero bytes and missing read metrics without invalid coordinates', () => {
  component = mount(SkewScatter, { target: document.body, props: { report: {
    path: '/', files: 2, total: 100, median: 0, max: 100, p99: 100,
    points: [{ partition: '/empty', size: 0, reads: null, writes: null }, { partition: '/data', size: 100, reads: null, writes: null }],
  } } });
  flushSync();
  const option = engine.setOption.mock.lastCall![0];
  expect(option.yAxis.name).toBe('file number');
  expect(option.series[0].data).toHaveLength(2);
  expect(option.series[0].data.map((point: number[]) => point.slice(0, 2))).toEqual([[1, 1], [100, 2]]);
  expect(option.series[0].markLine.data).toEqual([]);
  expect(document.body.textContent).not.toMatch(/NaN|Infinity/);
});

it('provides every block through pagination and supports keyboard inspection', () => {
  const layout: BlockLayout = {
    path: '/large', len: 30, block_size: 1, policy: 'replication-1', inlined: false,
    nodes: ['w1'], racks: { w1: 'rack-a' }, warnings: [],
    blocks: Array.from({ length: 30 }, (_, index) => ({ id: index, index, len: 1, policy: 'replication-1', fragments: [{ node: 'w1', rack: 'rack-a', idx: 0, kind: 'replica', state: 'ok' }] })),
  };
  component = mount(BlockMatrix, { target: document.body, props: { layout, maxRows: 10 } });
  flushSync();
  const next = [...document.querySelectorAll('button')].find(button => button.textContent === 'Next blocks')!;
  next.click(); flushSync();
  next.click(); flushSync();
  const cell = document.querySelector('[aria-label="w1, block 30, replica, ok"]')!;
  cell.dispatchEvent(new KeyboardEvent('keydown', { key: 'Enter', bubbles: true }));
  flushSync();
  expect(document.body.textContent).toContain('w1 · blk 30');
  expect(next.disabled).toBe(true);
});
