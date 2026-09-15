<script lang="ts">
  import { untrack } from 'svelte';
  import { api } from '$lib/api';
  import type { FileStatus } from '$lib/types';
  import Panel from './Panel.svelte';
  let { file }: { file: FileStatus } = $props();
  let text = $state<string | null>(null);
  let loading = $state(true);
  let error = $state('');
  let revision = $state(0);
  let wrap = $state(true);
  const version = $derived(JSON.stringify([file.path, file.checksum, file.modified]));
  $effect(() => {
    void version; void revision;
    const path = untrack(() => file.path);
    let active = true;
    text = null; loading = true; error = '';
    api.preview(path).then(value => { if (active) text = value; }).catch(e => { if (active) error = e instanceof Error ? e.message : String(e); }).finally(() => { if (active) loading = false; });
    return () => { active = false; };
  });
</script>
<Panel title="File preview" note={file.len > 65536 ? 'First 64 KiB · download for the full file' : 'UTF-8 text'}>
  {#snippet actions()}<button aria-pressed={wrap} onclick={() => wrap = !wrap}>Wrap lines</button>{/snippet}
  {#if loading}<p class="quiet" role="status">Loading preview…</p>
  {:else if error}<p class="error" role="alert">{error}</p><button onclick={() => revision++}>Retry preview</button>
  {:else if text === null}<p class="quiet">This file cannot be displayed as UTF-8 text. Download it to open in a compatible application.</p>
  {:else if text === ''}<p class="quiet">This file is empty.</p>
  {:else}<textarea readonly aria-label="File contents" value={text} wrap={wrap ? 'soft' : 'off'} rows={Math.min(18, Math.max(3, text.split('\n').length))}></textarea>{/if}
</Panel>
<style>
  textarea { display: block; width: 100%; font: .8rem/1.7 var(--font-mono); max-height: 28rem; overflow: auto; margin: 0; tab-size: 4; resize: vertical; color: var(--fg); background: transparent; border: 0; padding: .2rem; }
  .quiet { color: var(--fg-dim); }
  .error { color: var(--danger); }
</style>
