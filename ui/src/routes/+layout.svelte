<!-- The shell: a fixed rail, a status header, and the page. Every page reads
     the same cluster report from `live`, so the header and the content can
     never disagree about what the cluster is doing. -->
<script lang="ts">
  import { onMount } from 'svelte';
  import { page } from '$app/state';
  import { live } from '$lib/live.svelte';
  import { bytes, clock, pctValue } from '$lib/format';
  import Meter from '$lib/components/Meter.svelte';
  import { switchWorkspace, type Workspace } from '$lib/workspace';
  import '../app.css';

  let { children } = $props();

  const NAV = [
    { href: '/', label: 'Overview', glyph: '◈' },
    { href: '/nodes', label: 'Nodes', glyph: '▦' },
    { href: '/files', label: 'Files', glyph: '▤' },
    { href: '/distribution', label: 'Distribution', glyph: '◉' },
    { href: '/jobs', label: 'Jobs', glyph: '▶' },
    { href: '/benchmarks', label: 'Benchmarks', glyph: '↗' },
    { href: '/cluster', label: 'Cluster', glyph: '◇' },
    { href: '/configure', label: 'Configure', glyph: '⚙' },
  ];

  let theme = $state<'dark' | 'light'>('dark');
  let dismissedBanner = $state(false);

  onMount(() => {
    theme =
      (document.documentElement.dataset.theme as 'dark' | 'light') ?? 'dark';
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
    href === '/'
      ? page.url.pathname === '/'
      : page.url.pathname.startsWith(href),
  );
</script>

<a class="skip-link" href="#main-content">Skip to content</a>
<div class="shell">
  <nav class="rail" aria-label="Sections">
    <a class="brand" href="/">
      <img src="/logo.svg" alt="" width="26" height="26" />
      <span>Mammoth</span>
    </a>

    <ul>
      {#each NAV as item (item.href)}
        <li>
          <a
            href={item.href}
            aria-current={active(item.href) ? 'page' : undefined}
          >
            <span class="glyph" aria-hidden="true">{item.glyph}</span
            >{item.label}
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
        <p class="mono foot-line">
          {report.placement} · epoch {report.topology_epoch}
        </p>
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
        <select class="workspace" aria-label="Workspace" value={live.source === 'demo' ? 'demo' : 'gateway'} onchange={(event) => switchWorkspace(event.currentTarget.value as Workspace)}>
          <option value="gateway">My storage</option>
          <option value="demo">Example cluster</option>
        </select>
        <span class="name">{report?.name ?? 'mammoth'}</span>
        {#if report}
          {#if !report.capabilities?.local}<span class="mono dim">leader {report.leader ?? '—'}</span>{/if}
          {#if report.safe_mode}
            <span class="badge danger">safe mode</span>
          {/if}
        {/if}
      </div>

      <div class="right">
        <button
          class="mobile-theme"
          onclick={toggleTheme}
          aria-label="Toggle theme"
        >
          {theme === 'dark' ? '☾' : '☀'}
        </button>
        {#if live.updatedAt}
          <span class="mono dim updated">Updated {clock(live.updatedAt)}</span>
        {/if}
        <span class="source-label" data-source={live.source}>
          <span
            class="dot"
            class:paused={live.paused ||
              !!live.error ||
              live.source === 'unknown'}
          ></span>
          {live.error
            ? 'Disconnected'
            : live.source === 'unknown'
              ? 'Connecting'
              : live.source === 'gateway'
                ? 'Live'
                : 'Demo'}
        </span>
        <button
          class="pill"
          data-source={live.source}
          onclick={() => {
            live.paused = !live.paused;
            if (!live.paused) void live.refresh();
          }}
          aria-pressed={live.paused}
          title={live.paused ? 'Resume live updates' : 'Pause live updates'}
        >
          {live.paused ? 'Resume updates' : 'Pause updates'}
        </button>
      </div>
    </header>

    {#if live.source === 'demo' && !dismissedBanner}
      <div class="banner" role="status">
        <div>
          <strong>Example cluster.</strong>
          Explore a simulated cluster, including an offline worker and a repair in
          progress. All values are example data.
        </div>
        <button
          onclick={() => (dismissedBanner = true)}
          aria-label="Dismiss demo notice">✕</button
        >
      </div>
    {/if}

    {#if report?.capabilities?.local}
      <div class="banner" role="status"><div><strong>My storage.</strong> Your files are persistent. Worker directories and reference capacities model three racks on this machine.</div><button onclick={() => switchWorkspace('demo')}>Explore example cluster →</button></div>
    {/if}

    {#if live.error}
      <div class="banner danger" role="alert">
        <div>
          <strong>API error.</strong>
          {live.error} Displayed values may be stale.
        </div>
        <button onclick={() => window.location.reload()}>Reconnect</button>
      </div>
    {/if}

    <main id="main-content">
      {@render children()}
    </main>
  </div>
</div>

<style>
  .workspace { font-size: .75rem; max-width: 10rem; }
  .banner button { flex-shrink: 0; }
  @media (max-width: 900px) { .cluster .name { display: none; } .updated { display: none; } }
  @media (max-width: 600px) { .banner { flex-wrap: wrap; } .workspace { max-width: 8.5rem; } }
  .mobile-theme {
    display: none;
  }
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
    padding: 0.75rem 1.2rem;
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
    font-size: 0.72rem;
    min-height: 34px;
    border-radius: 4px;
    color: var(--fg-dim);
  }
  .source-label {
    display: inline-flex;
    gap: 0.45rem;
    align-items: center;
    color: var(--fg-dim);
    font-size: 0.72rem;
  }
  .dot {
    width: 0.45rem;
    height: 0.45rem;
    border-radius: 50%;
    background: var(--ok);
    animation: pulse 2s ease-in-out infinite;
  }
  .source-label[data-source='demo'] .dot {
    background: var(--warn);
  }
  .dot.paused {
    background: var(--fg-faint);
    animation: none;
  }
  @keyframes pulse {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
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
  .banner button {
    border: none;
    min-width: 30px;
    min-height: 30px;
    color: var(--fg-faint);
  }

  main {
    padding: 1.6rem;
    min-width: 0;
  }

  .skip-link {
    position: fixed;
    top: -5rem;
    left: 1rem;
    z-index: 20;
    padding: 0.75rem;
    background: var(--bg-panel);
  }
  .skip-link:focus {
    top: 0.5rem;
  }
  @media (max-width: 600px) {
    .rail ul { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); }
    .rail li a { justify-content: center; padding: .65rem .35rem; gap: .35rem; font-size: .8rem; }
    .right { gap: .5rem; }
    .updated {
      display: none;
    }
    .topbar {
      height: auto;
      min-height: var(--header);
      flex-wrap: wrap;
      gap: 0.4rem;
      padding: 0.6rem;
    }
    .cluster {
      flex-wrap: wrap;
      gap: 0.4rem;
    }
    main {
      padding: 1rem;
    }
  }
  @media (max-width: 900px) {
    .shell {
      grid-template-columns: 1fr;
    }
    .mobile-theme {
      display: inline-block;
    }
    .rail {
      min-width: 0;
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
  @media (max-width: 600px) {
    .rail ul { display: grid; }
  }
</style>
