// Typed dashboard client. Development can fall back to clearly labelled demo
// data; production requires a gateway unless VITE_DATA_SOURCE=demo is set.

import * as demo from './demo';
import { chosenWorkspace } from './workspace';
import { benchmarkDefaults, type BenchmarkOptions, type BenchmarkState, type Configuration, type StorageSettings } from './benchmarks';
import type {
  BlockLayout,
  ClusterReport,
  FileStatus,
  FlowReport,
  HeatCell,
  Job,
  NodeReport,
  SkewReport,
  TopologyReport,
  TreemapNode,
} from './types';

export * from './types';

const BASE = '/api/v1';
const REQUEST_TIMEOUT_MS = 10_000;
const PROBE_TIMEOUT_MS = 1500;

export class ApiError extends Error {
  constructor(message: string, public status: number, public code?: string) {
    super(message);
    this.name = 'ApiError';
  }
}

async function responseError(response: Response): Promise<ApiError> {
  const detail = await response.json().catch(() => null);
  return new ApiError(detail?.message ?? `${response.status} ${response.statusText}`, response.status, detail?.code);
}

export type Source = 'unknown' | 'gateway' | 'demo';
let source: Source = 'unknown';
let probing: Promise<Source> | null = null;

export function currentSource(): Source {
  return source;
}

/** Reject an incompatible API before any page dereferences its nested fields.
 * The Rust teaching report is smaller than this dashboard's contract. */
function checkReport(value: unknown): asserts value is ClusterReport {
  // Wire-level nulls mean unavailable. Normalize them to non-finite display
  // values for the existing chart model; capability checks hide these panels.
  // These values are never presented as measured zeroes.
  const local = value as ClusterReport | null;
  if (local?.capabilities?.local === true && local.capabilities.distributed_metrics === false) {
    const missing = (keys: string[]) => Object.fromEntries(keys.map(k => [k, Number.NaN]));
    const target = local as unknown as Record<string, unknown>;
    target.read_path ??= missing(['lease_hits', 'resolve_hits', 'master_hits', 'short_circuit', 'hedged', 'p50_ms', 'p99_ms']);
    target.write_path ??= { ...missing(['k', 'm', 'depth', 'quorum_at', 'trailing', 'p50_ms', 'p99_ms', 'uplink_ratio', 'storage_ratio']), mode: 'mirror', ec_policy: 'unavailable' };
    target.repair ??= { ...missing(['queued', 'total', 'in_flight', 'participating', 'node_count', 'blocks_per_sec', 'bytes_per_sec', 'budget_pct', 'eta_s', 'grace_remaining_s', 'worst_remaining', 'total_fragments']), cause: null };
    target.start ??= { ...missing(['last_start_ms', 'started_at', 'mapped_bytes', 'blocks', 'roots_matched', 'roots_total', 'buckets_streamed', 'merkle_fanout', 'rebuild_equivalent_ms']), block_map: 'rebuild', shards: [] };
    target.throughput ??= missing(['read_bps', 'write_bps', 'repair_bps', 'balancer_bps', 'shuffle_bps', 'cross_rack_bps', 'cross_rack_capacity']);
  }
  const r = value as ClusterReport | null;
  if (!r || typeof r.name !== 'string' || !Array.isArray(r.nodes) ||
      !Array.isArray(r.alerts) || !Array.isArray(r.raft) || !r.health ||
      !r.throughput || !r.read_path || !r.write_path || !r.repair || !r.start ||
      !Array.isArray(r.start.shards)) {
    throw new Error('Incompatible cluster report. The gateway must implement ui/src/lib/types.ts.');
  }
}

async function fetchJson<T>(path: string, timeout = REQUEST_TIMEOUT_MS): Promise<T> {
  const ctl = new AbortController();
  const timer = setTimeout(() => ctl.abort(), timeout);
  try {
    const r = await fetch(`${BASE}${path}`, { signal: ctl.signal, headers: { Accept: 'application/json' } });
    if (!r.ok) throw await responseError(r);
    return await r.json() as T;
  } catch (error) {
    if (ctl.signal.aborted) throw new Error('The server took too long to respond. Please try again.');
    throw error;
  } finally {
    // Also release timers on HTTP failures, bad JSON and network errors.
    clearTimeout(timer);
  }
}

async function probe(): Promise<Source> {
  if (source !== 'unknown') return source;
  if (probing) return probing;
  probing = (async (): Promise<Source> => {
    const mode = chosenWorkspace() ?? import.meta.env.VITE_DATA_SOURCE ?? (import.meta.env.DEV ? 'auto' : 'gateway');
    if (!['auto', 'demo', 'gateway'].includes(mode)) {
      throw new Error('VITE_DATA_SOURCE must be auto, demo or gateway.');
    }
    if (mode === 'demo') return source = 'demo';
    let report: unknown;
    try {
      report = await fetchJson('/cluster/report', PROBE_TIMEOUT_MS);
    } catch (error) {
      // An auth failure is never a reason to replace real data with fake data.
      const message = error instanceof Error ? error.message : String(error);
      if (mode !== 'auto' || (error instanceof ApiError && [401, 403].includes(error.status)) || /^(401|403)\b/.test(message)) throw error;
      return source = 'demo';
    }
    checkReport(report);
    return source = 'gateway';
  })();
  try {
    return await probing;
  } finally {
    // A failed probe must be retryable after the gateway is fixed.
    probing = null;
  }
}

async function get<T>(path: string, fallback: () => T): Promise<T> {
  if ((await probe()) === 'demo') return fallback();
  return fetchJson<T>(path);
}

const q = encodeURIComponent;

async function mutate<T = void>(path: string, method: string, body?: BodyInit): Promise<T> {
  if ((await probe()) !== 'gateway') throw new Error('File changes require a live gateway.');
  const response = await fetch(`${BASE}${path}`, { method, body });
  if (!response.ok) {
    throw await responseError(response);
  }
  return response.status === 204 ? undefined as T : response.json();
}

export const api = {
  benchmarks: () => get<BenchmarkState>('/benchmarks', () => ({ active: null, reports: [], defaults: benchmarkDefaults })),
  runBenchmark: (options: BenchmarkOptions) => mutate<BenchmarkState['active']>('/benchmarks', 'POST', JSON.stringify(options)),
  configuration: () => get<Configuration | null>('/configuration', () => null),
  validateConfiguration: (settings: StorageSettings) => mutate<Configuration>('/configuration/validate', 'POST', JSON.stringify(settings)),
  upload: (path: string, file: File) => mutate(`/fs/data?path=${q(path)}`, 'PUT', file),
  mkdir: (path: string) => mutate(`/fs/directory?path=${q(path)}`, 'PUT'),
  remove: (path: string, recursive = false) => mutate(`/fs?path=${q(path)}&recursive=${recursive}`, 'DELETE'),
  rename: (path: string, to: string) => mutate(`/fs/rename?path=${q(path)}&to=${q(to)}`, 'POST'),
  attributes: (path: string, mode: number, owner: string, group: string) => mutate(`/fs/attributes?path=${q(path)}&mode=${mode}&owner=${q(owner)}&group=${q(group)}`, 'POST'),
  setReplication: (path: string, replication: number) => mutate(`/fs/replication?path=${q(path)}&replication=${replication}`, 'POST'),
  repair: () => mutate<{ repaired: number }>('/admin/repair', 'POST'),
  downloadUrl: (path: string) => `${BASE}/fs/data?path=${q(path)}`,
  preview: async (path: string) => {
    if ((await probe()) !== 'gateway') return null;
    const ctl = new AbortController();
    const timer = setTimeout(() => ctl.abort(), REQUEST_TIMEOUT_MS);
    try {
      const response = await fetch(`${BASE}/fs/data?path=${q(path)}&start=0&end=65536`, { signal: ctl.signal });
      if (!response.ok) throw await responseError(response);
      const data = new Uint8Array(await response.arrayBuffer());
      if (data.includes(0)) return null;
      try { return new TextDecoder('utf-8', { fatal: true }).decode(data, { stream: data.length === 65536 }); }
      catch { return null; }
    } finally { clearTimeout(timer); }
  },

  clusterReport: async () => {
    const report = await get<ClusterReport>('/cluster/report', demo.clusterReport);
    checkReport(report);
    return report;
  },

  nodes: () => get<NodeReport[]>('/nodes', () => demo.clusterReport().nodes),

  node: (id: string) =>
    get<NodeReport | undefined>(`/nodes/${q(id)}`, () =>
      demo.clusterReport().nodes.find((n) => n.id === id),
    ),

  list: (path: string, limit = 200, offset = 0, name = '') =>
    get<FileStatus[]>(`/fs?path=${q(path)}&limit=${limit}&offset=${offset}&name=${q(name)}`, () => demo.list(path).filter(file => file.name.toLowerCase().includes(name.toLowerCase())).slice(offset, offset + limit)),

  files: () => get<FileStatus[]>('/fs/search', () => []),

  stat: (path: string) => get<FileStatus | null>(`/fs/stat?path=${q(path)}`, () => demo.stat(path)),

  blocks: (path: string) =>
    get<BlockLayout | null>(`/fs/blocks?path=${q(path)}`, () => demo.blockLayout(path)),

  heat: (metric = 'usage', minutesAgo = 0) =>
    get<HeatCell[]>(`/distribution/heat?metric=${q(metric)}&minutes_ago=${minutesAgo}`, () =>
      demo.heatAt(minutesAgo),
    ),

  treemap: (path = '/', depth = 3) =>
    get<TreemapNode>(`/distribution/treemap?path=${q(path)}&depth=${depth}`, () =>
      demo.treemap(path, depth),
    ),

  skew: (path: string) =>
    get<SkewReport>(`/distribution/skew?path=${q(path)}`, () => demo.skew(path)),

  topology: (minutesAgo = 0) =>
    get<TopologyReport>(`/distribution/topology?minutes_ago=${minutesAgo}`, () =>
      demo.topologyAt(minutesAgo),
    ),

  flow: (minutesAgo = 0) =>
    get<FlowReport>(`/distribution/flow?minutes_ago=${minutesAgo}`, () => demo.flowAt(minutesAgo)),

  jobs: () => get<Job[]>('/jobs', demo.jobs),
  submitJob: (kind: 'wordcount' | 'sort', input: string, output: string, overwrite = false) =>
    mutate<Job>(`/jobs?kind=${kind}&input=${q(input)}&output=${q(output)}&overwrite=${overwrite}`, 'POST'),

  /** The cluster as it was N minutes ago, for the distribution page's slider. */
  reportAt: async (minutesAgo: number) => {
    const report = await get<ClusterReport>(`/cluster/report?minutes_ago=${minutesAgo}`, () =>
      demo.reportAt(minutesAgo),
    );
    checkReport(report);
    return report;
  },
};

/** Live updates over SSE — simpler than WebSockets and sufficient here.
 *  Event names: node_state, block_health, throughput, job_update, alert.
 *
 *  Against the demo backend the same callback is driven by a local ticker, so
 *  every page that subscribes behaves identically either way. */
export function subscribe(
  on: (event: string, data: unknown) => void,
  intervalMs = 2000,
  onError: (error: Error) => void = () => {},
): () => void {
  let stopped = false;
  let cleanup = () => {};

  probe().then((s) => {
    if (stopped) return;

    if (s === 'gateway') {
      const es = new EventSource(`${BASE}/events`);
      for (const name of ['node_state', 'block_health', 'throughput', 'job_update', 'alert']) {
        es.addEventListener(name, (e) => {
          try { on(name, JSON.parse((e as MessageEvent).data)); }
          catch { onError(new Error(`Invalid ${name} event from gateway.`)); }
        });
      }
      es.onopen = () => on('connected', null);
      es.onerror = () => onError(new Error('Live connection lost. Reconnecting; displayed data may be stale.'));
      cleanup = () => es.close();
      return;
    }

    const timer = setInterval(() => {
      demo.advance(intervalMs / 1000);
      on('throughput', demo.clusterReport());
    }, intervalMs);
    cleanup = () => clearInterval(timer);
  }).catch((e) => {
    if (!stopped) onError(e instanceof Error ? e : new Error(String(e)));
  });

  return () => {
    stopped = true;
    cleanup();
  };
}
