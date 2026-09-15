<script lang="ts">
  import { api } from '$lib/api';
  import type { FileStatus } from '$lib/types';
  import { parentOf } from '$lib/format';

  let { file, kind, onclose, oncomplete }: {
    file: FileStatus;
    kind: 'rename' | 'delete' | 'properties';
    onclose: () => void;
    oncomplete: (destination?: string) => void;
  } = $props();
  let dialog: HTMLDialogElement;
  let destination = $state('');
  let mode = $state('');
  let owner = $state('');
  let group = $state('');
  let replication = $state(3);
  let busy = $state(false);
  let error = $state('');
  let recursive = $state(false);

  $effect(() => {
    destination = file.path;
    mode = (file.mode ?? 0o644).toString(8).padStart(3, '0');
    owner = file.owner;
    group = file.group;
    replication = file.replication ?? 3;
    dialog.showModal();
  });

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (busy) return;
    error = '';
    busy = true;
    try {
      if (kind === 'rename') {
        if (!destination.startsWith('/') || destination === file.path) throw new Error('Enter a different absolute path, starting with /.');
        await api.rename(file.path, destination);
        oncomplete(destination);
      } else if (kind === 'delete') {
        await api.remove(file.path, recursive);
        oncomplete(parentOf(file.path));
      } else {
        if (!/^[0-7]{3,4}$/.test(mode)) throw new Error('Permissions must be three or four octal digits, such as 644.');
        if (!owner.trim() || !group.trim()) throw new Error('Owner and group are required.');
        await api.attributes(file.path, parseInt(mode, 8), owner.trim(), group.trim());
        oncomplete();
      }
    } catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { busy = false; }
  }

  async function setReplication() {
    busy = true; error = '';
    try { await api.setReplication(file.path, replication); oncomplete(); }
    catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { busy = false; }
  }
</script>

<dialog bind:this={dialog} onclose={onclose} oncancel={(event) => { if (busy) event.preventDefault(); }} aria-labelledby="file-action-title">
  <form onsubmit={submit}>
    <header><h2 id="file-action-title">{kind === 'rename' ? 'Rename or move' : kind === 'delete' ? `Delete ${file.is_dir ? 'folder' : 'file'}` : 'Properties'}</h2><button type="button" disabled={busy} onclick={onclose} aria-label="Close file action">✕</button></header>
    <p class="path mono">{file.path}</p>
    {#if kind === 'rename'}
      <label>Destination path<input type="text" bind:value={destination} required disabled={busy} /></label>
      <p class="hint">Keep the parent path to rename. Change it to move into another folder.</p>
    {:else if kind === 'delete'}
      <p>This permanently removes {file.name || file.path} from Mammoth.</p>
      {#if file.is_dir}<label class="check"><input type="checkbox" bind:checked={recursive} disabled={busy} /> Also delete all files and folders inside</label><p class="hint">Without this option, only an empty folder can be deleted.</p>{/if}
    {:else}
      <div class="fields">
        <label>Permissions<input type="text" bind:value={mode} pattern={'[0-7]{3,4}'} required disabled={busy} /></label>
        <label>Owner<input type="text" bind:value={owner} required disabled={busy} /></label>
        <label>Group<input type="text" bind:value={group} required disabled={busy} /></label>
      </div>
      <p class="hint">Ownership and permissions are descriptive in local storage.</p>
      {#if !file.is_dir && !file.inlined}
        <div class="replication"><label>Replica copies<input type="number" min="1" max="255" bind:value={replication} disabled={busy} /></label><button type="button" disabled={busy || !Number.isInteger(replication) || replication < 1 || replication > 255} onclick={setReplication}>Apply replication</button></div>
      {/if}
    {/if}
    {#if error}<p class="error" role="alert">{error}</p>{/if}
    <footer><button type="button" disabled={busy} onclick={onclose}>Cancel</button><button class:danger={kind === 'delete'} class:primary={kind !== 'delete'} disabled={busy}>{busy ? 'Working…' : kind === 'delete' ? 'Delete permanently' : kind === 'rename' ? 'Save path' : 'Save properties'}</button></footer>
  </form>
</dialog>

<style>
  dialog { width: min(34rem, calc(100vw - 2rem)); max-height: calc(100dvh - 2rem); padding: 1.25rem; border: 1px solid var(--rule-strong); border-radius: .5rem; background: var(--bg-panel); color: var(--fg); }
  dialog::backdrop { background: rgb(0 0 0 / .65); }
  header, footer, .replication { display: flex; align-items: center; gap: .75rem; flex-wrap: wrap; }
  header { justify-content: space-between; }
  footer { justify-content: flex-end; margin-top: 1.5rem; }
  .path { overflow-wrap: anywhere; color: var(--fg-dim); }
  label { display: grid; gap: .4rem; margin-top: .75rem; min-width: 0; }
  input { width: 100%; }
  .check { display: flex; align-items: flex-start; }
  .check input { width: auto; margin-top: .25rem; }
  .fields { display: grid; gap: .5rem; grid-template-columns: repeat(auto-fit, minmax(min(8rem, 100%), 1fr)); }
  .hint { color: var(--fg-faint); font-size: .8rem; }
  .error, .danger { color: var(--danger); }
  .danger { border-color: var(--danger); }
  .primary { background: var(--accent); color: var(--bg); }
  .replication { border-top: 1px solid var(--rule); padding-top: .5rem; }
</style>
