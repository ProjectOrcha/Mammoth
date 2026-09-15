import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import FileAction from './FileAction.svelte';
import type { FileStatus } from '../types';
const api = vi.hoisted(() => ({ rename: vi.fn(), remove: vi.fn(), attributes: vi.fn(), setReplication: vi.fn() }));
vi.mock('$lib/api', () => ({ api }));
let component: ReturnType<typeof mount>;
beforeEach(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute('open', ''); };
  vi.resetAllMocks();
});
afterEach(async () => { if (component) await unmount(component); document.body.replaceChildren(); });
const file = { path: '/folder/a.txt', name: 'a.txt', mode: 0o644, owner: 'user', group: 'users', replication: 3 } as FileStatus;
async function settle() { await new Promise(resolve => setTimeout(resolve, 0)); flushSync(); }
it('moves a path and retains the dialog when the server rejects it', async () => {
  const done = vi.fn();
  api.rename.mockRejectedValueOnce(new Error('Destination already exists')).mockResolvedValueOnce(undefined);
  component = mount(FileAction, { target: document.body, props: { file, kind: 'rename', onclose: vi.fn(), oncomplete: done } });
  flushSync();
  const input = document.querySelector('input')!;
  input.value = '/other/b.txt'; input.dispatchEvent(new Event('input', { bubbles: true }));
  document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
  await settle();
  expect(api.rename).toHaveBeenCalledWith('/folder/a.txt', '/other/b.txt');
  expect(document.querySelector('[role="alert"]')?.textContent).toContain('Destination already exists');
  expect(done).not.toHaveBeenCalled();
  document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
  await settle();
  expect(done).toHaveBeenCalledWith('/other/b.txt');
});
it('only sends recursive deletion when the folder checkbox is selected', async () => {
  component = mount(FileAction, { target: document.body, props: { file: { ...file, is_dir: true }, kind: 'delete', onclose: vi.fn(), oncomplete: vi.fn() } });
  flushSync();
  document.querySelector<HTMLInputElement>('input[type="checkbox"]')!.click();
  document.querySelector('form')!.dispatchEvent(new Event('submit', { cancelable: true, bubbles: true }));
  await settle();
  expect(api.remove).toHaveBeenCalledWith('/folder/a.txt', true);
});
