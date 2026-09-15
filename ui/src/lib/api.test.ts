import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { clusterReport } from './demo';

beforeEach(() => { vi.resetModules(); vi.useFakeTimers(); });
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); vi.unstubAllEnvs(); });

describe('gateway selection', () => {
  it('does not replace a production outage with a simulated cluster', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', undefined);
    vi.stubEnv('DEV', false);
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
    const { api, currentSource } = await import('./api');
    await expect(api.clusterReport()).rejects.toThrow('offline');
    expect(currentSource()).toBe('unknown');
    expect(vi.getTimerCount()).toBe(0);
  });

  it('retries a failed probe and shares concurrent probing', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'gateway');
    const fetch = vi.fn().mockRejectedValueOnce(new Error('offline'))
      .mockImplementation(async () => new Response(JSON.stringify(clusterReport())));
    vi.stubGlobal('fetch', fetch);
    const { api, currentSource } = await import('./api');
    await expect(api.clusterReport()).rejects.toThrow();
    await Promise.all([api.clusterReport(), api.clusterReport()]);
    expect(currentSource()).toBe('gateway');
    expect(fetch).toHaveBeenCalledTimes(4); // failed probe + shared probe + two reads
    expect(vi.getTimerCount()).toBe(0);
  });

  it.each([401, 403])('surfaces HTTP %s even in auto mode', async (status) => {
    vi.stubEnv('VITE_DATA_SOURCE', 'auto');
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', { status })));
    const { api, currentSource } = await import('./api');
    await expect(api.clusterReport()).rejects.toThrow(String(status));
    expect(currentSource()).toBe('unknown');
  });

  it('rejects the smaller teaching API with an actionable error', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'auto');
    vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response(JSON.stringify({ nodes: [] }))));
    const { api } = await import('./api');
    await expect(api.clusterReport()).rejects.toThrow('ui/src/lib/types.ts');
  });

  it('can run an explicit demo without contacting a gateway', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'demo');
    const fetch = vi.fn();
    vi.stubGlobal('fetch', fetch);
    const { api, currentSource } = await import('./api');
    expect((await api.clusterReport()).nodes).toHaveLength(12);
    expect(currentSource()).toBe('demo');
    expect(fetch).not.toHaveBeenCalled();
  });

  it('reports subscription startup errors without an unhandled rejection', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'gateway');
    vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new Error('offline')));
    const { subscribe } = await import('./api');
    const failed = vi.fn();
    const stop = subscribe(vi.fn(), 2000, failed);
    await vi.runAllTimersAsync();
    expect(failed).toHaveBeenCalledWith(expect.objectContaining({ message: 'offline' }));
    stop();
    expect(vi.getTimerCount()).toBe(0);
  });
});

describe('local gateway contract', () => {
  it('accepts explicit unavailable metrics without replacing real data with demo data', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'gateway');
    const report = { ...clusterReport(), capabilities: { local: true, distributed_metrics: false, history: false, jobs: false }, read_path: null, write_path: null, repair: null, start: null, throughput: null };
    vi.stubGlobal('fetch', vi.fn().mockImplementation(async () => new Response(JSON.stringify(report))));
    const { api, currentSource } = await import('./api');
    const local = await api.clusterReport();
    expect(currentSource()).toBe('gateway');
    expect(local.capabilities?.local).toBe(true);
    expect(Number.isNaN(local.read_path.p99_ms)).toBe(true);
    expect(local.nodes).toEqual(report.nodes);
  });
});

describe('file management requests', () => {
  it('previews bounded UTF-8 data and recognizes binary files', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'gateway');
    const fetch = vi.fn().mockImplementation(async (url: string) => new Response(url.includes('cluster/report') ? JSON.stringify(clusterReport()) : 'hello 雪\n'));
    vi.stubGlobal('fetch', fetch);
    const { api } = await import('./api');
    expect(await api.preview('/a #.txt')).toBe('hello 雪\n');
    expect(fetch.mock.calls.at(-1)?.[0]).toBe('/api/v1/fs/data?path=%2Fa%20%23.txt&start=0&end=65536');
    fetch.mockImplementation(async () => new Response(new Uint8Array([0, 255])));
    expect(await api.preview('/binary')).toBeNull();
    expect(vi.getTimerCount()).toBe(0);
  });
  it('encodes rename, recursive deletion and filter parameters', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'gateway');
    const fetch = vi.fn().mockImplementation(async (url: string) =>
      new Response(JSON.stringify(url.includes('cluster/report') ? clusterReport() : {})));
    vi.stubGlobal('fetch', fetch);
    const { api } = await import('./api');
    await api.rename('/a #?.txt', '/b % 雪.txt');
    await api.remove('/empty folder', true);
    await api.list('/', 201, 200, 'a #');
    expect(fetch.mock.calls.map(call => call[0])).toContain('/api/v1/fs/rename?path=%2Fa%20%23%3F.txt&to=%2Fb%20%25%20%E9%9B%AA.txt');
    expect(fetch.mock.calls.map(call => call[0])).toContain('/api/v1/fs?path=%2Fempty%20folder&recursive=true');
    expect(fetch.mock.calls.map(call => call[0])).toContain('/api/v1/fs?path=%2F&limit=201&offset=200&name=a%20%23');
  });

  it('keeps actionable server errors and their HTTP status', async () => {
    vi.stubEnv('VITE_DATA_SOURCE', 'gateway');
    vi.stubGlobal('fetch', vi.fn().mockImplementation(async (url: string) =>
      url.includes('cluster/report') ? new Response(JSON.stringify(clusterReport())) :
      new Response(JSON.stringify({ code: 'E0101', message: 'No such path: /gone' }), { status: 404 })));
    const { api } = await import('./api');
    await expect(api.stat('/gone')).rejects.toMatchObject({ message: 'No such path: /gone', status: 404, code: 'E0101' });
  });
});
