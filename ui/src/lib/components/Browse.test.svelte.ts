import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Browse from './Browse.svelte';
import type { FileStatus } from '../types';

const api = vi.hoisted(() => ({ stat: vi.fn(), list: vi.fn(), blocks: vi.fn(), upload: vi.fn() }));
vi.mock('$lib/api', () => ({ api, currentSource: () => 'gateway' }));
vi.mock('$lib/live.svelte', () => ({ live: { source: 'gateway', refresh: vi.fn() } }));

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

describe('uploads', () => {
  async function select(file: File) {
    api.stat.mockImplementation(async (path) => path === '/data' ? directory(path) : null);
    api.list.mockResolvedValue([]);
    component = mount(Browse, { target: document.body, props: { path: '/data' } });
    await settle();
    const input = document.querySelector<HTMLInputElement>('input[type="file"]')!;
    Object.defineProperty(input, 'files', { value: [file] });
    input.dispatchEvent(new Event('change', { bubbles: true }));
    await settle();
  }

  it.each([false, true])('requires explicit confirmation for an empty source (confirmed: %s)', async (confirmed) => {
    const confirm = vi.spyOn(window, 'confirm').mockReturnValue(confirmed);
    const file = new File([], 'raptor.pt');
    await select(file);
    expect(confirm).toHaveBeenCalledWith(expect.stringContaining('already empty (0 bytes) on your computer'));
    if (confirmed) expect(api.upload).toHaveBeenCalledWith('/data/raptor.pt', file);
    else {
      expect(api.upload).not.toHaveBeenCalled();
      expect(document.body.textContent).toContain('was not uploaded');
    }
  });

  it('passes a nonempty binary file through without an empty-file prompt', async () => {
    const confirm = vi.spyOn(window, 'confirm');
    const file = new File([new Uint8Array([0, 255, 1, 128])], 'binary.pt');
    await select(file);
    expect(confirm).not.toHaveBeenCalled();
    expect(api.upload).toHaveBeenCalledWith('/data/binary.pt', file);
  });
});

it('keeps the search field mounted while filtering and disables Next on an exact full page', async () => {
  api.stat.mockResolvedValue(directory('/data'));
  const files = Array.from({ length: 200 }, (_, index) => entry(`/data/file-${index}`));
  api.list.mockResolvedValue(files);
  component = mount(Browse, { target: document.body, props: { path: '/data' } });
  await settle();
  expect(document.querySelector<HTMLSelectElement>('.row-action')?.value).toBe('');
  const next = [...document.querySelectorAll('button')].find(button => button.textContent === 'Next')!;
  expect(next.disabled).toBe(true);
  const search = document.querySelector<HTMLInputElement>('[aria-label="Filter files"]')!;
  search.focus();
  search.value = 'match';
  const filtered = pending<FileStatus[]>();
  api.list.mockReturnValue(filtered.promise);
  search.dispatchEvent(new Event('input', { bubbles: true }));
  flushSync();
  expect(document.activeElement).toBe(search);
  await settle();
  expect(api.list).toHaveBeenLastCalledWith('/data', 201, 0, 'match');
  filtered.resolve([]);
  await settle();
  expect(document.body.textContent).toContain('No entries match');
  expect(document.activeElement).toBe(search);
});
