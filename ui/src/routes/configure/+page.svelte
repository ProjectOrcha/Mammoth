<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import { live } from '$lib/live.svelte';
  import { downloadText, type Configuration, type StorageSettings } from '$lib/benchmarks';
  import Panel from '$lib/components/Panel.svelte';
  let active = $state<Configuration | null>(null);
  let draft = $state<StorageSettings | null>(null);
  let validated = $state<Configuration | null>(null);
  let validatedKey = $state(''); let error = $state(''); let busy = $state(false);
  const currentKey = $derived(JSON.stringify(draft));
  const canDownload = $derived(validated !== null && currentKey === validatedKey);
  async function load() {
    error = ''; busy = true;
    try { active = await api.configuration(); draft = active ? { ...active.settings } : null; validated = null; }
    catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { busy = false; }
  }
  onMount(() => { void load(); });
  async function validate(event: SubmitEvent) {
    event.preventDefault(); if (!draft) return;
    busy = true; error = ''; validated = null;
    const settings = { ...draft }; const key = JSON.stringify(settings);
    try { validated = await api.validateConfiguration(settings); validatedKey = key; }
    catch (e) { error = e instanceof Error ? e.message : String(e); }
    finally { busy = false; }
  }
</script>

<svelte:head><title>Configure · Mammoth</title></svelte:head>
<header class="page"><h1>Configure</h1><p class="eyebrow">Storage, memory & listeners</p></header>
<p class="intro">Review this service’s active settings, prepare a configuration, and download it for your next restart. Storage changes apply to new writes.</p>
{#if error}<p class="load-error" role="alert">{error}</p><button onclick={load}>Reload settings</button>{/if}
{#if live.source === 'demo'}<Panel title="Connect to your storage"><p>The example workspace has no active configuration. Switch to My storage to configure your local service.</p></Panel>
{:else if draft && active}
  <div class="columns">
    <Panel title="Configuration draft" note="Validated by the running service">
      <form onsubmit={validate}><fieldset disabled={busy}>
        <label>Block size<input type="text" bind:value={draft.block_size} placeholder="128MiB" required /><small>1 B to 256 MiB. Smaller blocks create more metadata.</small></label>
        <label>Inline threshold<input type="text" bind:value={draft.inline_threshold} placeholder="1MiB" required /><small>0 disables inlining; maximum 16 MiB.</small></label>
        <label>Default replicas<input type="number" min="1" max="6" bind:value={draft.replication} required /><small>Copies on simulated worker directories on this machine.</small></label>
        <label>Read cache size<input type="text" bind:value={draft.read_cache_size} placeholder="256MiB" required /><small>Verified reads held in this service’s memory. 0 disables caching; maximum 4 GiB.</small></label>
        <label>Memory target per job<input type="text" bind:value={draft.compute_memory_budget} placeholder="128MiB" required /><small>64 KiB to 4 GiB. Larger jobs spill to disk. This is a working-set target, not a total memory limit.</small></label>
        <label>Spill directory<input type="text" bind:value={draft.spill_directory} placeholder="System temporary directory" /><small>Optional existing directory for temporary job files. Files are removed after completion or failure.</small></label>
        <label>Dashboard address<input type="text" bind:value={draft.ui_listen} placeholder="127.0.0.1:8080" required /></label>
        <label>S3 address<input type="text" bind:value={draft.s3_listen} placeholder="127.0.0.1:9000" required /></label>
      </fieldset><div class="actions"><button disabled={busy}>{busy ? 'Checking…' : 'Validate configuration'}</button><button type="button" disabled={busy} onclick={() => { draft = { ...active!.settings }; validated = null; }}>Reset draft</button></div></form>
    </Panel>
    <Panel title="Active settings" note="Loaded at service startup">
      <dl>{#each Object.entries(active.settings) as [key,value]}<dt>{key.replaceAll('_',' ')}</dt><dd>{value}</dd>{/each}</dl>
      <p>Benchmarks use their own settings and temporary stores. Set workload sizes and replica comparisons on the <a href="/benchmarks">Benchmarks page</a>.</p>
      <p class="hint">The local service has no TLS, authentication, physical worker network or distributed shuffle.</p>
    </Panel>
  </div>
  {#if canDownload && validated}
    <Panel title="Ready to download" note="Restart required to apply">
      {#snippet actions()}<button onclick={() => downloadText('mammoth.toml', validated!.toml, 'application/toml')}>Download mammoth.toml</button>{/snippet}
      <pre>{validated.toml}</pre>
      <p role="status">Configuration is valid. Download it, stop the service, then start it using your file and the same storage directory:</p>
      <pre>mammoth --local-root /path/to/store stop
mammoth --config /path/to/mammoth.toml --local-root /path/to/store serve --role all</pre>
      <p class="hint">MAMMOTH_ environment overrides take precedence over the file. Existing files retain their current layout.</p>
    </Panel>
  {:else if validated}<p class="hint">Your draft changed. Validate it again to enable download.</p>{/if}
{:else if !error}<p class="hint">Loading active settings…</p>{/if}

<style>
  .intro { max-width:75ch; color:var(--fg-dim); margin-bottom:1.5rem; }
  .columns { display:grid; grid-template-columns:1.2fr 1fr; gap:1rem; margin-bottom:1rem; }
  fieldset { padding:0; margin:0; border:0; display:grid; gap:1rem; }
  label { display:grid; gap:.35rem; color:var(--fg-dim); font-size:.85rem; }
  small,.hint { color:var(--fg-dim); font-size:.78rem; }
  input { width:100%; }
  .actions { display:flex; gap:.6rem; flex-wrap:wrap; margin-top:1rem; }
  dl { display:grid; grid-template-columns:1fr 1fr; gap:.8rem; }
  dt { text-transform:capitalize; color:var(--fg-dim); } dd { margin:0; overflow-wrap:anywhere; }
  pre { padding:1rem; background:var(--bg-plate); overflow:auto; }
  @media(max-width:800px) { .columns { grid-template-columns:1fr; } }
</style>
