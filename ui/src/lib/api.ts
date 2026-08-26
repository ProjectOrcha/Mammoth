// Typed client for the gateway API (Part VI §6.4).
//
// The CLI and the UI consume the SAME endpoints, so anything the UI can show,
// a script can also fetch — and the two can never drift apart.
//
// One addition the scaffold did not have: if no gateway answers, the client
// falls back to the simulated cluster in `demo.ts` and says so, once, in the
// header. `npm run dev` is then useful on its own, and the fallback is loud
// rather than silent — a dashboard that invents numbers without telling you is
// worse than one that shows nothing.

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
/** A gateway on the same host answers in single-digit milliseconds. If it has
 *  not answered in a second and a half it is not there. */
const PROBE_TIMEOUT_MS = 1500;

export type Source = 'unknown' | 'gateway' | 'demo';

let source: Source = 'unknown';
let probing: Promise<Source> | null = null;

export function currentSource(): Source {
  return source;
}

async function probe(): Promise<Source> {
  if (source !== 'unknown') return source;
  if (probing) return probing;

  probing = (async () => {
    try {
      const ctl = new AbortController();
      const timer = setTimeout(() => ctl.abort(), PROBE_TIMEOUT_MS);
      const r = await fetch(`${BASE}/cluster/report`, { signal: ctl.signal });
      clearTimeout(timer);
      source = r.ok ? 'gateway' : 'demo';
    } catch {
      source = 'demo';
    }
    return source;
  })();

  return probing;
}

async function get<T>(path: string, fallback: () => T): Promise<T> {
  if ((await probe()) === 'demo') return fallback();
  const r = await fetch(`${BASE}${path}`);
  if (!r.ok) throw new Error(`${r.status} ${r.statusText} — ${BASE}${path}`);
  return (await r.json()) as T;
}

const q = encodeURIComponent;

export const api = {
  clusterReport: () => get<ClusterReport>('/cluster/report', demo.clusterReport),

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
): () => void {
  let stopped = false;
  let cleanup = () => {};

  probe().then((s) => {
    if (stopped) return;

    if (s === 'gateway') {
      const es = new EventSource(`${BASE}/events`);
      for (const name of ['node_state', 'block_health', 'throughput', 'job_update', 'alert']) {
        es.addEventListener(name, (e) => on(name, JSON.parse((e as MessageEvent).data)));
      }
      cleanup = () => es.close();
      return;
    }

    const timer = setInterval(() => {
      demo.advance(intervalMs / 1000);
      on('throughput', demo.clusterReport());
    }, intervalMs);
    cleanup = () => clearInterval(timer);
  });

  return () => {
    stopped = true;
    cleanup();
  };
}
