import { afterEach, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount, untrack } from 'svelte';
import FilePreview from './FilePreview.svelte';
import type { FileStatus } from '$lib/types';
const api = vi.hoisted(() => ({ preview: vi.fn() }));
vi.mock('$lib/api', () => ({ api }));
let component: ReturnType<typeof mount> | undefined;
afterEach(async () => { if (component) await unmount(component); document.body.replaceChildren(); vi.resetAllMocks(); });

it('keeps the preview stable during polling and refreshes when content changes', async () => {
  api.preview.mockResolvedValue('first');
  let file = $state({ path: '/text', checksum: 'one', modified: 1, len: 5 } as FileStatus);
  component = mount(FilePreview, { target: document.body, props: { get file() { return file; } } });
  await new Promise(resolve => setTimeout(resolve, 0)); flushSync();
  const preview = document.querySelector('textarea');
  expect(preview?.value).toBe('first');
  file = untrack(() => ({ ...file }));
  flushSync();
  expect(api.preview).toHaveBeenCalledTimes(1);
  expect(document.querySelector('textarea')).toBe(preview);
  api.preview.mockResolvedValue('second');
  file = untrack(() => ({ ...file, checksum: 'two' }));
  await new Promise(resolve => setTimeout(resolve, 0)); flushSync();
  expect(document.querySelector('textarea')?.value).toBe('second');
  expect(api.preview).toHaveBeenCalledTimes(2);
});
