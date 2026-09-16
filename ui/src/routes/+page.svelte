<script lang="ts">
  import { onMount } from 'svelte';
  type Memory = { project:string; key:string; title:string; content:string; kind:string; tags:string[]; source:string|null; revision:number; updated_at:number };
  let project=$state(''); let query=$state(''); let memories=$state<Memory[]>([]);
  let error=$state(''); let notice=$state(''); let busy=$state(false); let loaded=$state(false); let truncated=$state(false);
  let key=$state(''); let title=$state(''); let content=$state(''); let kind=$state('note'); let source=$state(''); let revision=$state<number|undefined>(); let tags=$state<string[]>([]);
  onMount(() => { try { project=localStorage.getItem('mammoth:memory-project') ?? ''; } catch { /* optional preference */ } });
  async function request(path:string, init?:RequestInit) {
    const response=await fetch(path,{...init,signal:AbortSignal.timeout(10000)});
    const result=await response.json().catch(()=>null);
    if(!response.ok) throw new Error(result?.message ?? 'Memory service unavailable. Start the AI_coded local service with the same memory root.');
    return result;
  }
  function url(endpoint='memory') { return `/api/v1/${endpoint}?${new URLSearchParams({project,query})}`; }
  function reset() { key='';title='';content='';kind='note';source='';revision=undefined;tags=[]; }
  async function recall() {
    if(!project.trim()) { error='Enter a project name first.';return; }
    busy=true;error='';notice='';
    try { const result=await request(url());memories=result.memories;truncated=result.truncated;loaded=true;try { localStorage.setItem('mammoth:memory-project',project); } catch { /* optional */ } }
    catch(e) { error=e instanceof Error?e.message:String(e);memories=[];loaded=false; }
    finally { busy=false; }
  }
  async function save() {
    if(!project.trim()) { error='Enter a project name first.';return; }
    busy=true;error='';notice='';
    try {
      await request(url(),{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({key,title,content,kind,tags,source:source||null,expected_revision:revision})});
      reset();query='';await recall();notice='Memory committed to disk.';
    } catch(e) { error=e instanceof Error?e.message:String(e); }
    finally { busy=false; }
  }
  async function edit(entry:Memory) {
    busy=true;error='';notice='';
    try { const full:Memory=await request(`/api/v1/memory/get?${new URLSearchParams({project,key:entry.key})}`);key=full.key;title=full.title;content=full.content;kind=full.kind;source=full.source??'';revision=full.revision;tags=full.tags; }
    catch(e) { error=e instanceof Error?e.message:String(e); }
    finally { busy=false; }
  }
</script>
<svelte:head><title>Agent memory · Mammoth</title><meta name="description" content="Durable project decisions, conventions, and handoffs for coding agents." /></svelte:head>
<div class="memory-page">
  <header class="page-heading"><div><p class="kicker">Durable context for coding agents</p><h1>New session. Same memory.</h1><p class="intro">Keep the decisions, conventions, and handoffs your next session needs.</p></div></header>
  <section class="memory-panel"><h2>Recall project context</h2><form onsubmit={(e)=>{e.preventDefault();void recall();}} class="search-form">
    <label>Project<input bind:value={project} oninput={()=>{memories=[];loaded=false;reset();notice='';}} placeholder="team/my-app" required disabled={busy} /></label>
    <label>Search<input bind:value={query} placeholder="Keywords, or leave empty for recent memories" disabled={busy} /></label>
    <button class="action primary" disabled={busy}>{busy?'Working…':'Recall memory'}</button>
  </form><p class="hint">Use the same project name and store as your MCP connection. Memory is saved on the local service, not in this browser.</p></section>
  {#if error}<p class="memory-error" role="alert">{error}</p>{/if}
  {#if notice}<p role="status">{notice}</p>{/if}
  <div class="memory-grid"><section aria-label="Recalled memories">
    <h2>Project memory</h2>
    {#if !loaded}<p class="empty">Choose a project and recall its context. Connect your coding agent with <code>mammoth mcp --project my-app</code> to use memory across sessions.</p>
    {:else if memories.length===0}<p class="empty">No matching memories. Save a useful decision or handoff to get started.</p>{/if}
    {#if truncated}<p class="hint">The recall budget omitted some context. Refine your search or open an entry to read its full text.</p>{/if}
    {#each memories as entry (entry.key)}<article class="memory-panel"><p class="kicker">{entry.kind} · revision {entry.revision}</p><h3>{entry.title}</h3><p class="memory-content">{entry.content}</p><p class="hint">{entry.key} · {new Date(entry.updated_at).toLocaleString()}{entry.source ? ` · ${entry.source}` : ''}</p><button class="action secondary" disabled={busy} onclick={()=>edit(entry)}>Read full entry / edit</button></article>{/each}
  </section><section class="memory-panel"><h2>{revision ? 'Revise memory' : 'Remember something useful'}</h2><form onsubmit={(e)=>{e.preventDefault();void save();}} class="entry-form">
    <label>Stable key<input bind:value={key} required maxlength="200" placeholder="current-handoff" disabled={busy || revision!==undefined} /></label>
    <label>Title<input bind:value={title} required maxlength="300" placeholder="Where to pick up next session" disabled={busy} /></label>
    <label>Kind<select bind:value={kind} disabled={busy}><option value="note">Note</option><option value="decision">Decision</option><option value="convention">Convention</option><option value="handoff">Handoff</option></select></label>
    <label>Context<textarea bind:value={content} rows="7" required placeholder="What changed, why it matters, and the next step." disabled={busy}></textarea></label>
    <label>Source (optional)<input bind:value={source} maxlength="1000" placeholder="File, commit, or issue URL" disabled={busy} /></label>
    <p class="hint">Save concise, verified context. Never store secrets. {revision ? `Updating revision ${revision}; conflicts will be reported.` : 'Existing keys require reading the entry before updating.'}</p>
    <button class="action primary" disabled={busy}>{revision ? 'Save revision' : 'Save memory'}</button>
    {#if revision}<button type="button" class="action secondary" onclick={reset} disabled={busy}>Cancel edit</button>{/if}
  </form></section></div>
</div>
<style>
.memory-page { max-width:1200px;margin:auto; } .memory-panel { border:1px solid var(--rule);border-radius:12px;padding:1.3rem;margin:1rem 0;background:var(--bg-panel); } .memory-grid { display:grid;grid-template-columns:1.2fr 1fr;gap:1.5rem;align-items:start; } .search-form { display:grid;grid-template-columns:1fr 2fr auto;gap:1rem;align-items:end; } label { display:grid;gap:.45rem;font-size:.85rem; } input,select,textarea { width:100%;box-sizing:border-box;padding:.7rem;border:1px solid var(--rule);border-radius:6px;background:var(--bg, #0b1420);color:inherit;font:inherit; } .entry-form { display:grid;gap:1rem; } .memory-content { white-space:pre-wrap;overflow-wrap:anywhere;line-height:1.6; } .hint,.empty { opacity:.75;font-size:.85rem;line-height:1.6;overflow-wrap:anywhere; } .memory-error { color:var(--danger,#ffaba8); } h2 { font-size:1.15rem; } h3 { font-size:1rem; } @media(max-width:800px) { .memory-grid,.search-form { grid-template-columns:1fr; } }
</style>
