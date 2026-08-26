<!-- Jobs — stage DAG, task Gantt, and the two numbers that explain a slow job:
     data locality, and the one task that is taking eight times as long as the
     rest. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { api } from '$lib/api';
  import type { Job, Task } from '$lib/types';
  import { ago, duration, pct } from '$lib/format';
  import Panel from '$lib/components/Panel.svelte';
  import Meter from '$lib/components/Meter.svelte';

  let jobs = $state<Job[] | null>(null);
  let selectedId = $state<string | null>(null);

  onMount(() => {
    void api.jobs().then((j) => {
      jobs = j;
      selectedId ??= j.find((x) => x.state === 'running')?.id ?? j[0]?.id ?? null;
    });
  });

  const job = $derived(jobs?.find((j) => j.id === selectedId) ?? null);

  /** Depth of each stage in the DAG, so a chain lays out left to right and a
   *  fan-in lands in the same column. */
  const layers = $derived.by(() => {
    if (!job) return [];
    const depth = new Map<string, number>();
    const of = (id: string): number => {
      if (depth.has(id)) return depth.get(id)!;
      const stage = job.stages.find((s) => s.id === id);
      const d = stage && stage.deps.length ? Math.max(...stage.deps.map(of)) + 1 : 0;
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
    return Math.max(...job.tasks.map((t) => t.start_s + t.dur_s));
  });

  const straggler = $derived.by(() => {
    if (!job?.tasks.length) return null;
    const durations = [...job.tasks].sort((a, b) => a.dur_s - b.dur_s);
    const median = durations[Math.floor(durations.length / 2)].dur_s;
    const worst = durations[durations.length - 1];
    return worst.dur_s > median * 3 ? { task: worst, ratio: worst.dur_s / median } : null;
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

<header class="page">
  <h1>Jobs</h1>
  <p class="eyebrow">{jobs ? `${jobs.length} recent` : 'loading'}</p>
</header>

<div class="cols">
  <Panel title="Recent" scroll>
    {#if !jobs}
      <p class="quiet mono">reading…</p>
    {:else}
      <ul class="joblist">
        {#each jobs as j (j.id)}
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

  {#if job}
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
  {/if}
</div>

{#if job && job.stages.length}
  <Panel title="Stages" note={`${job.stages.length} stages`}>
    <div class="dag">
      {#each layers as layer, i (i)}
        {#if i > 0}<span class="arrow" aria-hidden="true">→</span>{/if}
        <div class="layer">
          {#each layer as s (s.id)}
            <article class="stage" data-kind={s.kind}>
              <p class="eyebrow">{s.kind}</p>
              <p class="sname">{s.name}</p>
              <Meter value={(s.done / s.tasks) * 100} tone="accent" height="0.3rem" />
              <p class="mono stasks">{s.done} / {s.tasks} tasks</p>
            </article>
          {/each}
        </div>
      {/each}
    </div>
    <p class="hint">
      The shuffle is where a job spends most of its time — it is an all-to-all network
      transfer plus a disk sort. Every performance conversation about a job like this
      eventually becomes a conversation about that stage.
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
        task sets this job's runtime. w7's disk p99 is 340 ms — a slow disk is worse than a
        dead one, because nothing routes around it.
        <br />
        <span class="mono fix">mammoth doctor --node {straggler.task.node}</span>
      </p>
    {/if}
  </Panel>
{:else if job}
  <Panel title="Stages">
    <p class="quiet">This job has finished; its per-task detail has aged out.</p>
  </Panel>
{/if}

<style>
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
