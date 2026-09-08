import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Browse from './Browse.svelte';
import type { FileStatus } from '../types';

const api = vi.hoisted(() => ({ stat: vi.fn(), list: vi.fn(), blocks: vi.fn() }));
vi.mock('$lib/api', () => ({ api, currentSource: () => 'gateway' }));

let component: ReturnType<typeof mount> | undefined;
afterEach(async () => {
  if (component) await unmount(component);
  document.body.replaceChildren();
  vi.resetAllMocks();
});

const directory = (path: string) => ({ path, name: path, is_dir: true }) as FileStatus;
const entry = (path: string) => ({ ...directory(path), len: 0, modified: 0, owner: 'test' });
const pending = <T,>() => {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
};
async function settle() {
  await new Promise((resolve) => setTimeout(resolve, 0));
  flushSync();
}

describe('file navigation', () => {
  it.each(['resolve', 'reject'] as const)('ignores a stale directory %s after navigation', async (outcome) => {
    const old = pending<FileStatus[]>();
    api.stat.mockImplementation(async (path) => directory(path));
    api.list.mockImplementation((path) => path === '/old' ? old.promise : Promise.resolve([entry('/new/current')]));
    let path = $state('/old');
    component = mount(Browse, { target: document.body, props: { get path() { return path; } } });
    await settle();
    path = '/new';
    await settle();
    if (outcome === 'resolve') old.resolve([entry('/old/stale')]);
    else old.reject(new Error('stale failure'));
    await settle();
    expect(document.body.textContent).toContain('/new/current');
    expect(document.body.textContent).not.toContain('stale');
  });

  it('reports missing layouts instead of leaving a blank file page', async () => {
    api.stat.mockResolvedValue({ ...directory('/file'), is_dir: false });
    api.blocks.mockResolvedValue(null);
    component = mount(Browse, { target: document.body, props: { path: '/file' } });
    await settle();
    expect(document.querySelector('[role="alert"]')?.textContent).toContain('No block layout');
    expect(document.body.textContent).not.toContain('demo namespace');
  });
});
