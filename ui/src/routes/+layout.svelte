<!-- The shell: a fixed rail, a status header, and the page. Every page reads
     the same cluster report from `live`, so the header and the content can
     never disagree about what the cluster is doing. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { live } from '$lib/live.svelte';
  import { bytes, clock, pctValue } from '$lib/format';
  import Meter from '$lib/components/Meter.svelte';
  import '../app.css';

  let { children } = $props();

  const NAV = [
    { href: '/', label: 'Overview', glyph: '◈' },
    { href: '/nodes', label: 'Nodes', glyph: '▦' },
    { href: '/files', label: 'Files', glyph: '▤' },
    { href: '/distribution', label: 'Distribution', glyph: '◉' },
    { href: '/jobs', label: 'Jobs', glyph: '▶' },
    { href: '/cluster', label: 'Cluster', glyph: '◇' },
  ];

  let theme = $state<'dark' | 'light'>('dark');
  let dismissedBanner = $state(false);

  onMount(() => {
    theme = (document.documentElement.dataset.theme as 'dark' | 'light') ?? 'dark';
    return live.attach();
  });

  function toggleTheme() {
    theme = theme === 'dark' ? 'light' : 'dark';
    document.documentElement.dataset.theme = theme;
    try {
      localStorage.setItem('mammoth:theme', theme);
    } catch {
      /* private mode — the choice just does not persist */
    }
  }

  const report = $derived(live.report);
  const active = $derived((href: string) =>
    href === '/' ? page.url.pathname === '/' : page.url.pathname.startsWith(href),
  );
</script>

<div class="shell">
  <nav class="rail" aria-label="Sections">
    <a class="brand" href="/">
      <img src="/logo.svg" alt="" width="26" height="26" />
      <span>Mammoth</span>
    </a>

    <ul>
      {#each NAV as item (item.href)}
        <li>
          <a href={item.href} aria-current={active(item.href) ? 'page' : undefined}>
            <span class="glyph" aria-hidden="true">{item.glyph}</span>{item.label}
          </a>
        </li>
      {/each}
    </ul>

    <div class="rail-foot">
      <p class="eyebrow">Capacity</p>
      {#if report}
        <Meter value={pctValue(report.used, report.capacity)} />
        <p class="mono foot-line">
          {bytes(report.used)} / {bytes(report.capacity)}
        </p>
        <p class="eyebrow" style="margin-top: 0.9rem">Placement</p>
        <p class="mono foot-line">{report.placement} · epoch {report.topology_epoch}</p>
      {:else}
        <Meter value={0} />
        <p class="mono foot-line">—</p>
      {/if}
      <button class="theme" onclick={toggleTheme}>
        {theme === 'dark' ? '☾ dark' : '☀ light'}
      </button>
    </div>
  </nav>

  <div class="main">
    <header class="topbar">
      <div class="cluster">
        <span class="name">{report?.name ?? 'mammoth'}</span>
        {#if report}
          <span class="mono dim">leader {report.leader ?? '—'}</span>
          {#if report.safe_mode}
            <span class="badge danger">safe mode</span>
          {/if}
        {/if}
      </div>

      <div class="right">
        {#if live.updatedAt}
          <span class="mono dim">{clock(live.updatedAt)}</span>
        {/if}
        <button
          class="pill"
          data-source={live.source}
          onclick={() => (live.paused = !live.paused)}
          title={live.paused ? 'Resume live updates' : 'Pause live updates'}
        >
          <span class="dot" class:paused={live.paused}></span>
          {live.paused ? 'paused' : live.source === 'gateway' ? 'live' : 'simulated'}
        </button>
      </div>
    </header>

    {#if live.source === 'demo' && !dismissedBanner}
      <div class="banner" role="status">
        <div>
          <strong>No gateway answered on <code class="mono">/api/v1</code>.</strong>
          Everything below is a simulated cluster from
          <code class="mono">src/lib/demo.ts</code> — twelve workers, one of them dead, a
          repair in flight. Start the real thing with
          <code class="mono">mammoth serve --role gateway</code> and reload.
        </div>
        <button onclick={() => (dismissedBanner = true)} aria-label="Dismiss">✕</button>
      </div>
    {/if}

    {#if live.error}
      <div class="banner danger" role="alert">
        <div><strong>API error.</strong> {live.error}</div>
      </div>
    {/if}

    <main>
      {@render children()}
    </main>
  </div>
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: var(--rail) minmax(0, 1fr);
    min-height: 100vh;
  }

  /* ── rail ─────────────────────────────────────────────────────────────── */
  .rail {
    background: var(--bg-rail);
    border-right: 1px solid var(--rule);
    display: flex;
    flex-direction: column;
    position: sticky;
    top: 0;
    height: 100vh;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 0.55rem;
    padding: 0 1rem;
    height: var(--header);
    border-bottom: 1px solid var(--rule);
    font-family: var(--font-display);
    font-size: 1.05rem;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--fg-display);
  }
  .brand:hover {
    text-decoration: none;
  }
  .rail ul {
    list-style: none;
    margin: 0.75rem 0 0;
    padding: 0;
    flex: 1;
  }
  .rail li a {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    padding: 0.5rem 1rem;
    color: var(--fg-dim);
    border-left: 2px solid transparent;
  }
  .rail li a:hover {
    background: var(--bg-hover);
    color: var(--fg);
    text-decoration: none;
  }
  .rail li a[aria-current='page'] {
    color: var(--fg-display);
    border-left-color: var(--accent);
    background: var(--bg-hover);
  }
  .glyph {
    width: 1rem;
    text-align: center;
    opacity: 0.75;
  }
  .rail-foot {
    padding: 1rem;
    border-top: 1px solid var(--rule);
  }
  .foot-line {
    margin: 0.35rem 0 0;
    color: var(--fg-faint);
    font-size: 0.72rem;
  }
  .theme {
    margin-top: 1rem;
    width: 100%;
    font-size: 0.72rem;
    color: var(--fg-dim);
  }

  /* ── main ─────────────────────────────────────────────────────────────── */
  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    height: var(--header);
    padding: 0 1.25rem;
    border-bottom: 1px solid var(--rule);
    background: var(--bg-rail);
    position: sticky;
    top: 0;
    z-index: 5;
  }
  .cluster {
    display: flex;
    align-items: baseline;
    gap: 0.75rem;
    min-width: 0;
  }
  .name {
    font-family: var(--font-display);
    letter-spacing: 0.12em;
    text-transform: uppercase;
    color: var(--fg);
  }
  .dim {
    color: var(--fg-faint);
    font-size: 0.75rem;
  }
  .right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  .badge {
    font-family: var(--font-mono);
    font-size: 0.65rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    padding: 0.1rem 0.4rem;
    border: 1px solid currentColor;
  }
  .badge.danger {
    color: var(--danger);
  }
  .pill {
    display: inline-flex;
    align-items: center;
    gap: 0.45rem;
    font-family: var(--font-mono);
    font-size: 0.68rem;
    letter-spacing: 0.14em;
    text-transform: uppercase;
    color: var(--fg-dim);
  }
  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: var(--ok);
    animation: pulse 2s ease-in-out infinite;
  }
  .pill[data-source='demo'] .dot {
    background: var(--warn);
  }
  .dot.paused {
    background: var(--fg-faint);
    animation: none;
  }
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.35; }
  }

  .banner {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 1rem;
    padding: 0.6rem 1.25rem;
    background: var(--bg-plate);
    border-bottom: 1px solid var(--rule);
    font-size: 0.8rem;
    color: var(--fg-dim);
  }
  .banner.danger {
    color: var(--danger);
  }
  .banner code {
    color: var(--accent);
  }
  .banner button {
    border: none;
    padding: 0 0.3rem;
    color: var(--fg-faint);
  }

  main {
    padding: 1.25rem;
    min-width: 0;
  }

  @media (max-width: 900px) {
    .shell {
      grid-template-columns: 1fr;
    }
    .rail {
      position: static;
      height: auto;
    }
    .rail ul {
      display: flex;
      overflow-x: auto;
      margin: 0;
    }
    .rail li a {
      border-left: none;
      border-bottom: 2px solid transparent;
      white-space: nowrap;
    }
    .rail li a[aria-current='page'] {
      border-left-color: transparent;
      border-bottom-color: var(--accent);
    }
    .rail-foot {
      display: none;
    }
  }
</style>
