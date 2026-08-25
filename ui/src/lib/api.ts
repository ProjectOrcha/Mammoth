// Typed client for the gateway API (Part VI §6.4).
//
// The CLI and the UI consume the SAME endpoints, so anything the UI can show,
// a script can also fetch — and the two can never drift apart.

export type NodeState = 'healthy' | 'warn' | 'decommissioning' | 'maintenance' | 'dead';

export interface NodeReport {
  id: string;
  address: string;
  rack: string;
  state: NodeState;
  used: number;
  capacity: number;
  blocks: number;
  volumes: number;
  disk_p99_ms: number;
}

export interface ReplicationHealth {
  healthy: number;
  under_replicated: number;
  critical: number;
  over_replicated: number;
  corrupt: number;
  missing: number;
}

export interface ClusterReport {
  name: string;
  leader: string | null;
  safe_mode: boolean;
  used: number;
  capacity: number;
  nodes: NodeReport[];
  health: ReplicationHealth;
}

const BASE = '/api/v1';

async function get<T>(path: string): Promise<T> {
  const r = await fetch(`${BASE}${path}`);
  if (!r.ok) throw new Error(`${r.status} ${r.statusText} — ${BASE}${path}`);
  return r.json() as Promise<T>;
}

export const api = {
  clusterReport: () => get<ClusterReport>('/cluster/report'),
  nodes: () => get<NodeReport[]>('/nodes'),
  node: (id: string) => get<NodeReport>(`/nodes/${id}`),
  list: (path: string, limit = 200) =>
    get(`/fs?path=${encodeURIComponent(path)}&limit=${limit}`),
  blocks: (path: string) => get(`/fs/blocks?path=${encodeURIComponent(path)}`),
  heat: (metric = 'usage') => get(`/distribution/heat?metric=${metric}`),
  treemap: (path = '/', depth = 3) =>
    get(`/distribution/treemap?path=${encodeURIComponent(path)}&depth=${depth}`),
  skew: (path: string) => get(`/distribution/skew?path=${encodeURIComponent(path)}`),
  topology: () => get('/distribution/topology'),
};

/** Live updates over SSE — simpler than WebSockets and sufficient here.
 *  Event names: node_state, block_health, throughput, job_update, alert. */
export function subscribe(on: (event: string, data: unknown) => void): () => void {
  const es = new EventSource(`${BASE}/events`);
  for (const name of ['node_state', 'block_health', 'throughput', 'job_update', 'alert']) {
    es.addEventListener(name, (e) => on(name, JSON.parse((e as MessageEvent).data)));
  }
  return () => es.close();
}
