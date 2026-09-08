// Typed dashboard client. Development can fall back to clearly labelled demo
// data; production requires a gateway unless VITE_DATA_SOURCE=demo is set.

import * as demo from './demo';
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

export type Source = 'unknown' | 'gateway' | 'demo';
let source: Source = 'unknown';
let probing: Promise<Source> | null = null;

export function currentSource(): Source {
  return source;
}

/** Reject an incompatible API before any page dereferences its nested fields.
 * The Rust teaching report is smaller than this dashboard's contract. */
function checkReport(value: unknown): asserts value is ClusterReport {
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
    if (!r.ok) throw new Error(`${r.status} ${r.statusText} — ${BASE}${path}`);
    return await r.json() as T;
  } finally {
    // Also release timers on HTTP failures, bad JSON and network errors.
    clearTimeout(timer);
  }
}

async function probe(): Promise<Source> {
  if (source !== 'unknown') return source;
  if (probing) return probing;
  probing = (async (): Promise<Source> => {
    const mode = import.meta.env.VITE_DATA_SOURCE ?? (import.meta.env.DEV ? 'auto' : 'gateway');
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
      if (mode !== 'auto' || /^(401|403)\b/.test(message)) throw error;
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

export const api = {
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

  list: (path: string, limit = 200) =>
    get<FileStatus[]>(`/fs?path=${q(path)}&limit=${limit}`, () => demo.list(path).slice(0, limit)),

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

  /** The cluster as it was N minutes ago, for the distribution page's slider. */
  reportAt: (minutesAgo: number) =>
    get<ClusterReport>(`/cluster/report?minutes_ago=${minutesAgo}`, () =>
      demo.reportAt(minutesAgo),
    ),
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
