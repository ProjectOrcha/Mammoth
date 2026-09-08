import { afterEach, describe, expect, it, vi } from 'vitest';
import { flushSync, mount, unmount } from 'svelte';
import Overview from './+page.svelte';
import { live } from '$lib/live.svelte';
import { clusterReport } from '$lib/demo';

let component: ReturnType<typeof mount> | undefined;
afterEach(async () => {
  if (component) await unmount(component);
  live.report = null;
  live.error = null;
  live.paused = false;
  live.source = 'unknown';
  document.body.replaceChildren();
  vi.restoreAllMocks();
});

function render() {
  component = mount(Overview, { target: document.body });
  flushSync();
}

function button(label: string) {
  return [...document.querySelectorAll('button')].find((button) =>
    button.textContent?.includes(label),
  )!;
}

describe('overview interactions', () => {
  it('expands and collapses alerts without losing severity order', () => {
    live.report = clusterReport();
    render();
    expect(document.querySelectorAll('#alert-list > li')).toHaveLength(3);
    expect(
      document.querySelector('#alert-list > li')?.getAttribute('data-level'),
    ).toBe('danger');
    button('Show all').click();
    flushSync();
    expect(document.querySelectorAll('#alert-list > li')).toHaveLength(
      live.report.alerts.length,
    );
    expect(button('Show fewer').getAttribute('aria-expanded')).toBe('true');
    button('Show fewer').click();
    flushSync();
    expect(document.querySelectorAll('#alert-list > li')).toHaveLength(3);
  });

  it('shows a recoverable error instead of a permanent loading message', () => {
    live.error = 'Gateway is unavailable';
    render();
    expect(document.body.textContent).toContain('The cluster is out of reach');
    expect(document.body.textContent).not.toContain(
      'Connecting to your cluster',
    );
    expect(button('Try again')).toBeDefined();
  });

  it('resumes updates on refresh and prevents duplicate clicks while refreshing', async () => {
    live.report = clusterReport();
    live.paused = true;
    let finish!: () => void;
    const refresh = vi.spyOn(live, 'refresh').mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    render();
    button('Refresh').click();
    flushSync();
    expect(live.paused).toBe(false);
    expect(button('Refreshing').disabled).toBe(true);
    button('Refreshing').click();
    expect(refresh).toHaveBeenCalledTimes(1);
    finish();
    await Promise.resolve();
    flushSync();
    expect(button('Refresh').disabled).toBe(false);
  });

  it('handles a cluster with no blocks, workers, or repairs', () => {
    live.report = {
      ...clusterReport(),
      nodes: [],
      alerts: [],
      health: {
        healthy: 0,
        under_replicated: 0,
        critical: 0,
        over_replicated: 0,
        corrupt: 0,
        missing: 0,
      },
    };
    live.report.repair = {
      ...live.report.repair,
      total: 0,
      queued: 0,
      grace_remaining_s: 0,
    };
    render();
    expect(document.body.textContent).toContain('No workers connected');
    expect(document.body.textContent).toContain('No blocks reported yet');
    expect(document.body.textContent).not.toMatch(/NaN|Infinity/);
  });
});
