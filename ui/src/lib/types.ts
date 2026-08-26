// The shapes the gateway serves under /api/v1. Kept in one file so the demo
// backend and the real client cannot drift, and re-exported from `api.ts` so
// callers only ever import from there.

export type NodeState = 'healthy' | 'warn' | 'decommissioning' | 'maintenance' | 'dead';

export interface NodeReport {
  id: string;
  address: string;
  rack: string;
  state: NodeState;
  /** Why the node is not `healthy`. Absent when it is. */
  note?: string;
  used: number;
  capacity: number;
  /** Fragments held, not blocks. One block puts one fragment on each of
   *  `k + m` nodes, so this is the node's bytes over the fragment size. */
  fragments: number;
  volumes: number;
  disk_p99_ms: number;
  read_bps: number;
  write_bps: number;
  /** Last 30 samples of read throughput, for the sparkline. */
  read_series: number[];
}

export interface ReplicationHealth {
  healthy: number;
  under_replicated: number;
  critical: number;
  over_replicated: number;
  corrupt: number;
  missing: number;
}

/** Where a read's locations came from. The point of the one-shot read is that
 *  the first two numbers are nearly everything and the third is nearly zero. */
export interface ReadPathStats {
  /** Served from a location lease the client already held: no metadata RTT. */
  lease_hits: number;
  /** Resolved by the worker itself from its namespace learner: one RTT total. */
  resolve_hits: number;
  /** Actually had to ask a master. Stale epoch, or a cold namespace. */
  master_hits: number;
  /** Replica was on the reader's machine — fd passed over a Unix socket. */
  short_circuit: number;
  /** A second replica was raced because the first was slow. */
  hedged: number;
  p50_ms: number;
  p99_ms: number;
}

export type WriteMode = 'disperse' | 'mirror' | 'pipeline';

export interface WritePathStats {
  mode: WriteMode;
  ec_policy: string;
  /** Data fragments. */
  k: number;
  /** Parity fragments — local plus global. */
  m: number;
  /** Network hops from client to durable. 1 for disperse, 3 for a chain. */
  depth: number;
  /** Fragments that must be durable before the write acks. */
  quorum_at: number;
  /** Fragments still landing after the ack. Not on the critical path. */
  trailing: number;
  p50_ms: number;
  p99_ms: number;
  uplink_ratio: number;
  storage_ratio: number;
}

export interface RepairStats {
  queued: number;
  /** Blocks that needed repair when the failure happened. `queued` drains
   *  towards zero from here, so progress is `1 - queued / total`. */
  total: number;
  in_flight: number;
  /** Nodes contributing to the rebuild right now. Declustered repair wants
   *  this to equal the number of healthy nodes. */
  participating: number;
  node_count: number;
  blocks_per_sec: number;
  bytes_per_sec: number;
  /** Share of the rate cap currently in use. Repair yields to client traffic. */
  budget_pct: number;
  eta_s: number;
  /** Seconds left of `repair.delay` before any copying starts. Zero once the
   *  rebuild is under way. */
  grace_remaining_s: number;
  cause: string | null;
  /** Fragments still standing on the worst block in the queue. */
  worst_remaining: number;
  /** Fragments a block starts with. */
  total_fragments: number;
}

export interface ShardStart {
  name: string;
  state: 'ready' | 'reconciling' | 'waiting';
  ready_ms: number | null;
}

export interface StartStats {
  /** How long the last start took, end to end. */
  last_start_ms: number;
  started_at: number;
  block_map: 'mmap' | 'rebuild';
  mapped_bytes: number;
  blocks: number;
  /** Workers whose 32-byte Merkle root matched the snapshot on the first try. */
  roots_matched: number;
  roots_total: number;
  /** Merkle buckets that actually had to be streamed, out of fanout × workers. */
  buckets_streamed: number;
  merkle_fanout: number;
  shards: ShardStart[];
  /** What the same restart costs when the map is rebuilt from block reports. */
  rebuild_equivalent_ms: number;
}

export interface Throughput {
  read_bps: number;
  write_bps: number;
  repair_bps: number;
  balancer_bps: number;
  shuffle_bps: number;
  cross_rack_bps: number;
  cross_rack_capacity: number;
}

export interface Alert {
  id: string;
  level: 'info' | 'warn' | 'danger';
  text: string;
  fix?: string;
  at: number;
}

export interface RaftMember {
  id: string;
  address: string;
  role: 'leader' | 'follower' | 'learner';
  /** Committed index this member has applied. */
  applied: number;
  /** Entries behind the leader. */
  lag: number;
  last_contact_ms: number;
}

export interface ClusterReport {
  name: string;
  leader: string | null;
  safe_mode: boolean;
  used: number;
  capacity: number;
  /** Bumped whenever the node set changes. Every request carries the epoch it
   *  was derived from, so a stale client fails safe instead of reading air. */
  topology_epoch: number;
  placement: 'rendezvous' | 'explicit';
  nodes: NodeReport[];
  health: ReplicationHealth;
  read_path: ReadPathStats;
  write_path: WritePathStats;
  repair: RepairStats;
  start: StartStats;
  throughput: Throughput;
  alerts: Alert[];
  raft: RaftMember[];
  /** Raft log index the leader has committed. */
  raft_index: number;
  snapshot_age_s: number;
}

// ── filesystem ────────────────────────────────────────────────────────────

export interface FileStatus {
  path: string;
  name: string;
  is_dir: boolean;
  len: number;
  block_size: number;
  replication: number | null;
  policy: string;
  blocks: number;
  inlined: boolean;
  mode: number;
  owner: string;
  group: string;
  modified: number;
  checksum: string | null;
}

export type FragmentKind = 'data' | 'local-parity' | 'global-parity' | 'replica';
export type FragmentState = 'ok' | 'pending' | 'repairing' | 'corrupt' | 'missing';

export interface Fragment {
  kind: FragmentKind;
  /** Index within its kind: d0, d1 … p0, p1. */
  idx: number;
  node: string;
  rack: string;
  state: FragmentState;
  /** Set on the fragment a read would actually be served from. */
  preferred?: boolean;
}

export interface BlockPlacement {
  id: number;
  index: number;
  len: number;
  policy: string;
  fragments: Fragment[];
}

export interface BlockLayout {
  path: string;
  len: number;
  block_size: number;
  policy: string;
  inlined: boolean;
  blocks: BlockPlacement[];
  nodes: string[];
  racks: Record<string, string>;
  warnings: string[];
}

// ── distribution ──────────────────────────────────────────────────────────

export type HeatMetric = 'usage' | 'fragments' | 'read_qps' | 'write_qps' | 'disk_p99_ms';

export interface HeatCell {
  node: string;
  rack: string;
  state: NodeState;
  usage: number;
  fragments: number;
  read_qps: number;
  write_qps: number;
  disk_p99_ms: number;
}

export interface TreemapNode {
  name: string;
  path: string;
  value: number;
  age_days: number;
  reads: number;
  children?: TreemapNode[];
}

export interface SkewPoint {
  partition: string;
  size: number;
  reads: number;
  writes: number;
}

export interface SkewReport {
  path: string;
  files: number;
  total: number;
  median: number;
  p99: number;
  max: number;
  points: SkewPoint[];
}

export interface TopologyNode {
  id: string;
  rack: string;
  capacity: number;
  used: number;
  state: NodeState;
}

export interface TopologyLink {
  source: string;
  target: string;
  bps: number;
}

export interface TopologyReport {
  epoch: number;
  nodes: TopologyNode[];
  racks: string[];
  links: TopologyLink[];
}

export type FlowSource = 'clients' | 'repair' | 'balancer' | 'shuffle';

export interface FlowLink {
  source: string;
  target: string;
  bps: number;
}

export interface FlowReport {
  window_s: number;
  nodes: string[];
  links: FlowLink[];
}

// ── jobs ──────────────────────────────────────────────────────────────────

export type TaskState = 'pending' | 'running' | 'done' | 'failed';

export interface Task {
  id: string;
  stage: string;
  node: string;
  state: TaskState;
  start_s: number;
  dur_s: number;
  /** The task read its input from a replica on its own machine. */
  local: boolean;
}

export interface Stage {
  id: string;
  name: string;
  kind: 'map' | 'shuffle' | 'reduce';
  deps: string[];
  tasks: number;
  done: number;
}

export interface Job {
  id: string;
  name: string;
  user: string;
  state: 'running' | 'succeeded' | 'failed';
  submitted: number;
  elapsed_s: number;
  progress: number;
  locality: number;
  stages: Stage[];
  tasks: Task[];
}
