<!-- Jobs — stage DAG, task Gantt, and the two numbers that explain a slow job:
     data locality, and the one task that is taking eight times as long as the
     rest. -->
<script lang="ts">
  import { onMount, untrack } from 'svelte';
  import { live } from '$lib/live.svelte';
  import { page } from '$app/state';
  import { switchWorkspace } from '$lib/workspace';
  import Stat from '$lib/components/Stat.svelte';
  import { api } from '$lib/api';
  import type { Job, Task } from '$lib/types';
  import { ago, duration, pct, fileHref, bytes } from '$lib/format';
  import Panel from '$lib/components/Panel.svelte';
  import Meter from '$lib/components/Meter.svelte';

  let jobs = $state<Job[] | null>(null);
  let selectedId = $state<string | null>(null);

  let error = $state<string | null>(null);
  let jobKind = $state<'wordcount' | 'sort'>('wordcount');
  let inputPath = $state('');
  let availableFiles = $state<string[]>([]);
  let jobFilter = $state('all');
  const filteredJobs = $derived(jobs?.filter(job => jobFilter === 'all' || job.state === jobFilter));
  $effect(() => {
    if (filteredJobs && !filteredJobs.some(job => job.id === selectedId)) selectedId = filteredJobs[0]?.id ?? null;
  });
  let outputPath = $state('');
  let overwrite = $state(false);
  let submitting = $state(false);
  let submitError = $state('');

  async function submit(event: SubmitEvent) {
    event.preventDefault();
    if (submitting) return;
    submitting = true; submitError = '';
    try {
      const created = await api.submitJob(jobKind, inputPath, outputPath, overwrite);
      jobFilter = 'all';
      selectedId = created.id;
      jobs = [created, ...(jobs ?? []).filter(job => job.id !== created.id)];
      overwrite = false;
      live.paused = false;
      await live.refresh();
    } catch (e) { submitError = e instanceof Error ? e.message : String(e); }
    finally { submitting = false; }
  }
  let active = false;
  let busy = false;

  async function loadJobs() {
    if (!active || busy) return;
    busy = true;
    try {
      const j = await api.jobs();
      if (!active) return;
      jobs = j;
      if (!j.some((item) => item.id === selectedId)) {
        selectedId = j.find((item) => item.state === 'running')?.id ?? j[0]?.id ?? null;
      }
      error = null;
    } catch (e) {
      if (active) error = e instanceof Error ? e.message : String(e);
    } finally { busy = false; }
  }

  onMount(() => {
    active = true;
    inputPath = page.url.searchParams.get('input') ?? '';
    if (inputPath) outputPath = `${inputPath}.result.txt`;
    void api.files().then(files => { if (active) availableFiles = files.map(file => file.path); }).catch(() => {});
    void loadJobs();
    return () => { active = false; };
  });
  $effect(() => {
    void live.updatedAt;
    untrack(() => { void loadJobs(); });
  });

  const job = $derived(jobs?.find((j) => j.id === selectedId) ?? null);

  /** Depth of each stage in the DAG, so a chain lays out left to right and a
   *  fan-in lands in the same column. */
  const layers = $derived.by(() => {
    if (!job) return [];
    const depth = new Map<string, number>();
    const visiting = new Set<string>();
    const of = (id: string): number => {
      if (visiting.has(id)) return 0;
      visiting.add(id);
      if (depth.has(id)) return depth.get(id)!;
      const stage = job.stages.find((s) => s.id === id);
      const d = stage && stage.deps.length ? Math.max(...stage.deps.map(of)) + 1 : 0;
      visiting.delete(id);
      depth.set(id, d);
      return d;
    };
    for (const s of job.stages) of(s.id);
    const max = Math.max(0, ...depth.values());
    return Array.from({ length: max + 1 }, (_, i) =>
      job.stages.filter((s) => depth.get(s.id) === i),
    );
  });

  const span = $derived.by(() => {
    if (!job?.tasks.length) return 1;
    return Math.max(.001, ...job.tasks.map((t) => t.start_s + t.dur_s));
  });

  const straggler = $derived.by(() => {
    if (!job?.tasks.length) return null;
    const durations = [...job.tasks].sort((a, b) => a.dur_s - b.dur_s);
    const median = durations[Math.floor(durations.length / 2)].dur_s;
    const worst = durations[durations.length - 1];
    return median > 0 && worst.dur_s > median * 3 ? { task: worst, ratio: worst.dur_s / median } : null;
  });

  const byStage = $derived.by(() => {
    if (!job) return [];
    return job.stages.map((s) => ({
      stage: s,
      tasks: job.tasks.filter((t) => t.stage === s.id),
    }));
  });

  function tone(t: Task): string {
    if (t.id === straggler?.task.id) return 'var(--danger)';
    return { done: 'var(--ok)', running: 'var(--accent)', pending: 'var(--track)', failed: 'var(--danger)' }[
      t.state
    ];
  }
</script>

<svelte:head><title>Jobs · Mammoth</title></svelte:head>

<header class="page">
  <h1>Jobs</h1>
  <p class="eyebrow">{jobs ? `${jobs.length} recent` : 'loading'}</p>
</header>

{#if live.source === 'gateway' && live.report?.capabilities?.jobs}
  <Panel title="Run a text job" note="Parallel UTF-8 processing · automatic disk spilling">
    <datalist id="job-input-files">{#each availableFiles as path}<option value={path}></option>{/each}</datalist>
    <form class="job-form" onsubmit={submit}>
      <label>Operation<select bind:value={jobKind} disabled={submitting}><option value="wordcount">Word count</option><option value="sort">Sort lines</option></select></label>
      <label>Input file<input type="text" list="job-input-files" bind:value={inputPath} placeholder="/sample/words.txt" required disabled={submitting} /></label>
      <label>Output file<input type="text" bind:value={outputPath} placeholder="/sample/counts.txt" required disabled={submitting} /></label>
      <button disabled={submitting}>{submitting ? 'Starting…' : 'Run job'}</button>
      <label class="overwrite"><input type="checkbox" bind:checked={overwrite} disabled={submitting} /> Replace the output file if it already exists</label>
    </form>
    <p class="hint">Uses multiple CPU threads on this machine, keeping batches in memory and spilling larger inputs. Adjust the memory target in <a href="/configure">Configure</a>. Browse <a href="/files">Files</a> to find an input. Recent jobs are kept for this service session; CLI jobs are not included.</p>
    {#if submitError}<p class="load-error" role="alert">{submitError}</p>{/if}
  </Panel>
{:else if live.source === 'gateway' && live.report?.capabilities?.jobs === false}
  <Panel title="Local text jobs"><p>The running service does not support dashboard jobs. Restart it with the latest Mammoth build to enable this page.</p><p><code>mammoth job wordcount /sample/words.txt /sample/counts.txt</code></p></Panel>
{/if}

{#if error}<p class="load-error" role="alert">Unable to load jobs: {error}</p><button onclick={loadJobs}>Try again</button>{/if}

{#if jobs}
  <div class="job-stats">
    <Stat label="Running" value={String(jobs.filter(job => job.state === 'running').length)} note="In progress" />
    <Stat label="Completed" value={String(jobs.filter(job => job.state === 'succeeded').length)} note="Output ready" tone="ok" />
    <Stat label="Failed" value={String(jobs.filter(job => job.state === 'failed').length)} note="Open a job for details" />
  </div>
{/if}
<div class="cols">
  <Panel title="Recent jobs" scroll>
    {#snippet actions()}<select aria-label="Filter jobs" bind:value={jobFilter}><option value="all">All jobs</option><option value="running">Running</option><option value="succeeded">Completed</option><option value="failed">Failed</option></select><button onclick={loadJobs}>Refresh jobs</button>{/snippet}
    {#if !jobs && error}
      <p class="quiet">Job data unavailable.</p>
    {:else if !jobs}
      <p class="quiet mono">reading…</p>
    {:else if jobs.length === 0}
      <p class="quiet">No jobs have been submitted in this service session.</p>
    {:else if filteredJobs?.length === 0}<p class="quiet">No jobs match this filter.</p>
    {:else}
      <ul class="joblist">
        {#each filteredJobs ?? [] as j (j.id)}
          <li>
            <button class:on={j.id === selectedId} onclick={() => (selectedId = j.id)}>
              <span class="dot" data-state={j.state}></span>
              <span class="jname">{j.name}</span>
              <span class="mono jmeta">{j.user} · {ago(j.submitted)}</span>
              <span class="mono jpct">{pct(j.progress, 1)}</span>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </Panel>

  {#if job?.execution === 'local'}
    <Panel title={job.name} note={job.id}>
      <dl>
        <div><dt>State</dt><dd data-state={job.state}>{job.state}</dd></div>
        <div><dt>Elapsed</dt><dd>{duration(job.elapsed_s)}</dd></div>
        <div><dt>Input</dt><dd><a href={fileHref(job.input ?? '/')}>{job.input}</a></dd></div>
        <div><dt>Output</dt><dd>{#if job.state === 'succeeded'}<a href={fileHref(job.output ?? '/')}>{job.output}</a>{:else}{job.output}{/if}</dd></div>
      </dl>
      {#if job.metrics}<dl>
        <div><dt>Processing</dt><dd>{job.metrics.mode} · {job.metrics.worker_threads} CPU threads</dd></div>
        <div><dt>Memory target</dt><dd>{bytes(job.metrics.memory_budget)}</dd></div>
        <div><dt>Input / output</dt><dd>{bytes(job.metrics.input_bytes)} / {bytes(job.metrics.output_bytes)}</dd></div>
        <div><dt>Temporary disk writes</dt><dd>{bytes(job.metrics.spilled_bytes)} · {job.metrics.spill_runs} runs · {job.metrics.merge_passes} merge passes</dd></div>
      </dl>{/if}
      <Meter value={job.progress * 100} tone="accent" />
      {#if job.error}<p class="load-error" role="alert">{job.error}</p>{/if}
      {#if job.state === 'running'}<p class="hint" role="status">Processing the input and writing the result…</p>{/if}
    </Panel>
  {:else if job}
    <Panel title={job.name} note={job.id}>
      <dl>
        <div><dt>state</dt><dd class="mono" data-state={job.state}>{job.state}</dd></div>
        <div><dt>elapsed</dt><dd class="mono">{duration(job.elapsed_s)}</dd></div>
        <div><dt>progress</dt><dd class="mono">{pct(job.progress, 1)}</dd></div>
        <div>
          <dt>data locality</dt>
          <dd class="mono" class:good={job.locality > 0.85}>{pct(job.locality, 1)}</dd>
        </div>
      </dl>
      <Meter value={job.progress * 100} tone="accent" />
      <p class="hint">
        Locality is the share of tasks that read their input from a replica on their own
        machine — a short-circuit read over a passed file descriptor, with no network hop
        at all. It is the whole reason the scheduler cares where blocks are.
      </p>
    </Panel>
  {:else}
    <Panel title="Your next job" note="Choose a file, an operation and an output path">
      <div class="empty-workflow"><span>1<br /><strong>Input file</strong></span><span aria-hidden="true">→</span><span>2<br /><strong>Process text</strong></span><span aria-hidden="true">→</span><span>3<br /><strong>Open result</strong></span></div>
      <p class="hint">Word count groups words with their counts. Sort puts lines in order. Completed jobs show their execution stage and timeline here.</p>
      <a href="/files">Browse input files →</a>
      {#if live.source === 'gateway'}<p class="hint"><button onclick={() => switchWorkspace('demo')}>Explore a distributed job example →</button></p>{/if}
    </Panel>
  {/if}
</div>

{#if job && job.stages.length}
  <Panel title="Stages" note={`${job.stages.length} ${job.stages.length === 1 ? 'stage' : 'stages'}`}>
    <div class="dag">
      {#each layers as layer, i (i)}
        {#if i > 0}<span class="arrow" aria-hidden="true">→</span>{/if}
        <div class="layer">
          {#each layer as s (s.id)}
            <article class="stage" data-kind={s.kind}>
              <p class="eyebrow">{s.kind}</p>
              <p class="sname">{s.name}</p>
              <Meter value={s.tasks ? (s.done / s.tasks) * 100 : 0} tone="accent" height="0.3rem" />
              <p class="mono stasks">{s.done} / {s.tasks} tasks</p>
            </article>
          {/each}
        </div>
      {/each}
    </div>
    <p class="hint">
      {#if job.execution === 'local'}This job runs as one local task, covering input reads, text processing and output writes. The timeline measures that complete execution.{:else}The shuffle is where a job spends most of its time — it is an all-to-all network
      transfer plus a disk sort. Every performance conversation about a job like this
      eventually becomes a conversation about that stage.{/if}
    </p>
  </Panel>

  <Panel title="Task timeline" note={`${job.tasks.length} of ${job.stages.reduce((a, s) => a + s.tasks, 0)} tasks shown`}>
    <div class="gantt">
      {#each byStage as group (group.stage.id)}
        <p class="glabel eyebrow">{group.stage.name}</p>
        {#each group.tasks as t (t.id)}
          <div class="grow">
            <span class="gid mono">{t.id}</span>
            <div class="gtrack">
              <div
                class="gbar"
                style="left: {(t.start_s / span) * 100}%; width: {Math.max(
                  0.6,
                  (t.dur_s / span) * 100,
                )}%; background: {tone(t)}"
                title={`${t.id} · ${t.node} · ${t.state} · ${duration(t.dur_s)}${t.local ? ' · local read' : ''}`}
              ></div>
            </div>
            <span class="gnode mono" class:remote={!t.local}>{t.node}</span>
            <span class="gdur mono">{duration(t.dur_s)}</span>
          </div>
        {/each}
      {/each}
    </div>

    {#if straggler}
      <p class="straggler">
        ⚠ <code class="mono">{straggler.task.id}</code> on
        <code class="mono">{straggler.task.node}</code> is running
        {straggler.ratio.toFixed(0)}× the median at {duration(straggler.task.dur_s)}. That one
        task sets this job's runtime. Inspect that worker and its input size to investigate the delay.
        <br />
        <span class="mono fix">mammoth doctor --node {straggler.task.node}</span>
      </p>
    {/if}
  </Panel>
{:else if job && job.execution !== 'local'}
  <Panel title="Stages">
    <p class="quiet">This job has finished; its per-task detail has aged out.</p>
  </Panel>
{/if}

<style>
  .job-stats { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: var(--gap); margin: var(--gap) 0; }
  .empty-workflow { display: flex; align-items: center; justify-content: space-between; gap: .5rem; padding: 1.2rem 0; color: var(--fg-dim); font-size: .75rem; }
  .empty-workflow strong { color: var(--fg); }
  .job-form { display: grid; gap: .8rem; align-items: end; grid-template-columns: minmax(8rem, .7fr) minmax(0, 1fr) minmax(0, 1fr) auto; }
  .job-form label { display: grid; gap: .4rem; min-width: 0; }
  .job-form .overwrite { grid-column: 1 / -1; display: flex; align-items: center; }
  .job-form input[type='checkbox'] { width: auto; }
  :global(main > .panel + .cols), :global(main > .panel + .panel) { margin-top: var(--gap); }
  dd { overflow-wrap: anywhere; min-width: 0; }
  @media (max-width: 700px) {
    .job-form { grid-template-columns: 1fr; }
    .joblist button { grid-template-columns: .6rem minmax(0, 1fr) 3rem; }
    .jmeta { grid-column: 2; }
    .jpct { grid-column: 3; grid-row: 1 / 3; }
  }
  .load-error { color: var(--danger); overflow-wrap: anywhere; }
  .page {
    display: flex;
    align-items: baseline;
    gap: 1rem;
    margin-bottom: 1.1rem;
  }
  .cols {
    display: grid;
    grid-template-columns: minmax(0, 1.1fr) minmax(0, 1fr);
    gap: var(--gap);
    margin-bottom: var(--gap);
  }
  @media (max-width: 1000px) {
    .cols {
      grid-template-columns: 1fr;
    }
  }

  .joblist {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .joblist button {
    display: grid;
    grid-template-columns: 0.6rem 1fr auto 3rem;
    align-items: center;
    gap: 0.6rem;
    width: 100%;
    border: none;
    border-bottom: 1px solid var(--rule);
    padding: 0.5rem 0.3rem;
    text-align: left;
  }
  .joblist button.on {
    background: var(--bg-hover);
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
  }
  .dot[data-state='running'] {
    background: var(--accent);
  }
  .dot[data-state='succeeded'] {
    background: var(--ok);
  }
  .dot[data-state='failed'] {
    background: var(--danger);
  }
  .jname {
    font-size: 0.8rem;
  }
  .jmeta,
  .jpct {
    font-size: 0.68rem;
    color: var(--fg-faint);
  }
  .jpct {
    text-align: right;
  }

  dl {
    margin: 0 0 0.8rem;
  }
  dl div {
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.28rem 0;
    border-bottom: 1px solid var(--rule);
    font-size: 0.78rem;
  }
  dt {
    color: var(--fg-faint);
  }
  dd {
    margin: 0;
    text-align: right;
  }
  dd[data-state='succeeded'],
  dd.good {
    color: var(--ok);
  }
  dd[data-state='failed'] {
    color: var(--danger);
  }
  dd[data-state='running'] {
    color: var(--accent);
  }

  .dag {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    overflow-x: auto;
    padding-bottom: 0.3rem;
  }
  .layer {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }
  .stage {
    min-width: 12rem;
    border: 1px solid var(--rule);
    background: var(--bg-plate);
    padding: 0.5rem 0.6rem;
  }
  .stage[data-kind='shuffle'] {
    border-color: var(--warn);
  }
  .sname {
    margin: 0.25rem 0 0.45rem;
    font-size: 0.78rem;
  }
  .stasks {
    margin: 0.3rem 0 0;
    font-size: 0.68rem;
    color: var(--fg-faint);
  }
  .arrow {
    color: var(--fg-faint);
  }

  .gantt {
    display: grid;
    gap: 0.12rem;
  }
  .glabel {
    margin: 0.7rem 0 0.25rem;
  }
  .grow {
    display: grid;
    grid-template-columns: 2.6rem 1fr 3rem 3.2rem;
    align-items: center;
    gap: 0.5rem;
  }
  .gid,
  .gnode,
  .gdur {
    font-size: 0.64rem;
    color: var(--fg-faint);
  }
  .gnode.remote {
    color: var(--warn);
  }
  .gdur {
    text-align: right;
  }
  .gtrack {
    position: relative;
    height: 0.55rem;
    background: var(--track);
  }
  .gbar {
    position: absolute;
    top: 0;
    height: 100%;
  }
  .straggler {
    margin: 1rem 0 0;
    padding: 0.6rem 0.75rem;
    border: 1px solid var(--danger);
    color: var(--danger);
    font-size: 0.76rem;
    line-height: 1.55;
  }
  .straggler code {
    color: inherit;
  }
  .fix {
    color: var(--accent);
    font-size: 0.7rem;
  }
  .hint {
    margin: 0.8rem 0 0;
    font-size: 0.74rem;
    color: var(--fg-faint);
    line-height: 1.55;
  }
  .quiet {
    color: var(--fg-faint);
    margin: 0;
  }
</style>
