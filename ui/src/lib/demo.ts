// A simulated cluster, so the UI is useful with nothing behind it.
//
// `npm run dev` with no `mammoth serve --role gateway` running would otherwise
// be seven empty pages. This module generates a complete, internally consistent
// cluster — twelve workers, three racks, one dead node, a repair in flight, a
// namespace with real skew — from a seeded PRNG, so the charts hold still
// between renders and a screenshot is reproducible.
//
// Everything here is derived from a handful of primitives (block size, EC
// policy, per-node bandwidth) rather than typed in, so the numbers agree with
// each other: the repair ETA really is the queue divided by the rate, and the
// rate really is what eleven nodes can write.

import type {
  Alert,
  BlockLayout,
  BlockPlacement,
  ClusterReport,
  FileStatus,
  FlowLink,
  FlowReport,
  Fragment,
  HeatCell,
  Job,
  NodeReport,
  NodeState,
  RaftMember,
  SkewReport,
  Stage,
  Task,
  TopologyLink,
  TopologyReport,
  TreemapNode,
} from './types';

// ── primitives ────────────────────────────────────────────────────────────

const BLOCK = 128 * 1024 * 1024;
/** LRC(6,2,2): six data, two local parities, two global. */
export const EC = { k: 6, local: 2, global: 2, policy: 'lrc-6-2-2' };
const FRAGMENTS = EC.k + EC.local + EC.global;
const FRAG = BLOCK / EC.k;
/** Bytes a healthy node spends on repair per second: 40% of what an idle
 *  NVMe + 25 GbE node can move. `repair.bytes_per_sec = "auto"` measures this
 *  rather than taking it on faith. */
const REPAIR_BUDGET_PER_NODE = 600e6;
/** `repair.delay` — a node that is merely absent gets ten minutes before we
 *  copy a hundred terabytes that may turn out to be unnecessary. */
const REPAIR_DELAY_S = 600;

const RACKS = ['/dc1/rack-a', '/dc1/rack-b', '/dc1/rack-c'];

interface Spec {
  id: string;
  rack: number;
  capacity: number;
  fill: number;
  state: NodeState;
  note?: string;
  p99: number;
  /** For the dead node: how full it was before it stopped answering. The whole
   *  incident is derived from this one number. */
  fillBeforeDeath?: number;
}

// One dead, one near-full, one slow. A cluster with nothing wrong teaches
// nothing, and every screen here exists to show something wrong.
const SPECS: Spec[] = [
  { id: 'w1', rack: 0, capacity: 160e12, fill: 0.71, state: 'healthy', p99: 8 },
  { id: 'w2', rack: 0, capacity: 160e12, fill: 0.58, state: 'healthy', p99: 7 },
  { id: 'w3', rack: 0, capacity: 160e12, fill: 0.94, state: 'warn', note: 'near full — new writes skip it', p99: 12 },
  { id: 'w4', rack: 0, capacity: 160e12, fill: 0.38, state: 'healthy', p99: 9 },
  { id: 'w5', rack: 1, capacity: 160e12, fill: 0.63, state: 'healthy', p99: 8 },
  { id: 'w6', rack: 1, capacity: 160e12, fill: 0.64, state: 'healthy', p99: 7 },
  { id: 'w7', rack: 1, capacity: 160e12, fill: 0.69, state: 'warn', note: 'disk p99 340 ms — demote candidate', p99: 340 },
  { id: 'w8', rack: 1, capacity: 160e12, fill: 0.24, state: 'healthy', p99: 6 },
  { id: 'w9', rack: 2, capacity: 180e12, fill: 0.61, state: 'healthy', p99: 8 },
  { id: 'w10', rack: 2, capacity: 180e12, fill: 0.68, state: 'healthy', p99: 9 },
  { id: 'w11', rack: 2, capacity: 180e12, fill: 0.62, state: 'healthy', p99: 8 },
  {
    id: 'w12',
    rack: 2,
    capacity: 180e12,
    fill: 0.0,
    state: 'dead',
    note: 'no heartbeat',
    p99: 0,
    fillBeforeDeath: 0.59,
  },
];

/** Deterministic PRNG. Same seed, same cluster, every render. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** A stable hash, so `place()` below is a pure function of the block id. */
function mix(x: number): number {
  let h = x >>> 0;
  h ^= h >>> 16;
  h = Math.imul(h, 0x7feb352d);
  h ^= h >>> 15;
  h = Math.imul(h, 0x846ca68b);
  h ^= h >>> 16;
  return h >>> 0;
}

/**
 * Rendezvous (Highest Random Weight) placement — the same function the docs
 * describe, in miniature. Score every live node for this block, sort, and walk
 * the ranking taking one node per rack first so replicas never share a failure
 * domain. Deterministic, and it is why nothing here has to store placement.
 */
export function place(block: number, n: number, live = liveNodeIds()): string[] {
  const ranked = live
    .map((id) => ({ id, score: mix(block ^ mix(idSeed(id))) }))
    .sort((a, b) => b.score - a.score);

  const out: string[] = [];
  const usedRacks = new Set<string>();
  for (const r of ranked) {
    if (out.length === n) break;
    const rack = rackOf(r.id);
    if (!usedRacks.has(rack)) {
      usedRacks.add(rack);
      out.push(r.id);
    }
  }
  for (const r of ranked) {
    if (out.length === n) break;
    if (!out.includes(r.id)) out.push(r.id);
  }
  return out;
}

function idSeed(id: string): number {
  return Number(id.slice(1)) * 0x9e3779b1;
}
function rackOf(id: string): string {
  return RACKS[SPECS.find((s) => s.id === id)!.rack];
}
function liveNodeIds(): string[] {
  return SPECS.filter((s) => s.state !== 'dead').map((s) => s.id);
}

// ── live state ────────────────────────────────────────────────────────────
//
// Small and explicit. `advance()` moves it on; everything else is derived.

/** Bytes a node holds, from its fill. */
function usedOn(s: Spec, fill = s.fill): number {
  return s.capacity * fill;
}

/** Every count below is derived from capacity, fill and the EC policy, so the
 *  block total, the health breakdown, the repair queue and the ETA cannot
 *  disagree with each other — which is exactly how the numbers in a dashboard
 *  usually go wrong. */
const BYTES_EVER_STORED =
  SPECS.reduce((a, s) => a + usedOn(s), 0) +
  SPECS.reduce((a, s) => a + usedOn(s, s.fillBeforeDeath ?? 0), 0);

const TOTAL_FRAGMENTS = Math.round(BYTES_EVER_STORED / FRAG);
const TOTAL_BLOCKS = Math.round(TOTAL_FRAGMENTS / FRAGMENTS);

/**
 * Blocks that lost a fragment when w12 stopped answering.
 *
 * One fragment per block, so this is simply the fragment count w12 held — and
 * it is most of the cluster. `lrc-6-2-2` spreads ten fragments across twelve
 * nodes, so the chance a given block has one on any particular node is 10/12.
 * Erasure coding wants its width to be much smaller than the node count; this
 * cluster is a demonstration of what happens when it is not.
 */
const DEGRADED_AT_DEATH = Math.round(
  usedOn(SPECS.find((s) => s.id === 'w12')!, 0.59) / FRAG,
);

/** Blocks that were already two fragments down from an earlier disk failure,
 *  and lost a third to w12. These go to the front of the repair queue. */
const CRITICAL_BLOCKS = 12;

const state = {
  t0: Date.now(),
  /** Seconds since w12 stopped answering. */
  deadFor: 42 * 60,
  /** Blocks still a fragment short. Drains at the repair rate. */
  queued: 0,
  critical: CRITICAL_BLOCKS,
  epoch: 41,
  raftIndex: 8_412_990,
  jitter: rng(0xc0ffee),
};

/** How long repair has actually been running: time since the failure, minus
 *  the grace period it waits out first. */
function repairingFor(): number {
  return Math.max(0, state.deadFor - REPAIR_DELAY_S);
}

/** The queue is a function of the clock, not an accumulator — so the time
 *  machine and the live view are the same calculation run at two times. */
function queuedAt(seconds: number): number {
  const rate = (healthyNodes().length * REPAIR_BUDGET_PER_NODE) / FRAG;
  return Math.max(0, DEGRADED_AT_DEATH - rate * Math.max(0, seconds - REPAIR_DELAY_S));
}

state.queued = queuedAt(state.deadFor);

export function advance(dtSeconds: number): void {
  state.deadFor += dtSeconds;
  state.queued = queuedAt(state.deadFor);
  state.raftIndex += Math.round(180 * dtSeconds);
}

function healthyNodes(): Spec[] {
  return SPECS.filter((s) => s.state === 'healthy' || s.state === 'warn');
}

/** Declustered: every healthy node reads and writes, so the rate is the
 *  cluster's, not one disk's. This is the whole argument, as a number.
 *  Zero while the grace period is still running — nothing has been copied yet. */
function repairBlocksPerSec(): number {
  if (state.queued <= 0 || repairingFor() <= 0) return 0;
  return (healthyNodes().length * REPAIR_BUDGET_PER_NODE) / FRAG;
}

function jit(base: number, spread = 0.12): number {
  return base * (1 + (state.jitter() - 0.5) * 2 * spread);
}

// ── the cluster report ────────────────────────────────────────────────────

function nodes(): NodeReport[] {
  return SPECS.map((s, i) => {
    const r = rng(0x51ede + i);
    const dead = s.state === 'dead';
    const used = s.capacity * s.fill;
    const readBps = dead ? 0 : jit(s.p99 > 100 ? 140e6 : 380e6, 0.2);
    return {
      id: s.id,
      address: `192.168.1.${11 + i}:7001`,
      rack: RACKS[s.rack],
      state: s.state,
      note: s.note,
      used,
      capacity: s.capacity,
      // What a node stores is fragments, not blocks: one block puts one
      // fragment on each of `k + m` nodes, so a node's count is simply its
      // bytes over the fragment size.
      fragments: dead ? 0 : Math.round(used / FRAG),
      volumes: 4,
      disk_p99_ms: s.p99,
      read_bps: readBps,
      write_bps: dead ? 0 : jit(s.fill > 0.9 ? 0 : 120e6, 0.3),
      read_series: Array.from({ length: 30 }, () => (dead ? 0 : readBps * (0.6 + r() * 0.8))),
    };
  });
}

function readPath(): ClusterReport['read_path'] {
  // The shape the one-shot read is supposed to produce: nearly everything
  // served from a lease the client already had, a slice resolved at the worker,
  // and a rounding error that actually reached a master.
  const total = 1_284_002;
  return {
    lease_hits: Math.round(total * 0.938),
    resolve_hits: Math.round(total * 0.058),
    master_hits: Math.round(total * 0.004),
    short_circuit: Math.round(total * 0.211),
    hedged: Math.round(total * 0.006),
    p50_ms: 0.9,
    p99_ms: 6.4,
  };
}

function writePath(): ClusterReport['write_path'] {
  return {
    mode: 'disperse',
    ec_policy: EC.policy,
    k: EC.k,
    m: EC.local + EC.global,
    depth: 1,
    quorum_at: EC.k + 1,
    trailing: FRAGMENTS - (EC.k + 1),
    p50_ms: 14.2,
    p99_ms: 41.8,
    uplink_ratio: FRAGMENTS / EC.k,
    storage_ratio: FRAGMENTS / EC.k,
  };
}

function repair(): ClusterReport['repair'] {
  const rate = repairBlocksPerSec();
  const waiting = state.queued > 0 && repairingFor() <= 0;
  const participating = rate > 0 ? healthyNodes().length : 0;
  return {
    queued: Math.round(state.queued),
    total: state.queued > 0 ? DEGRADED_AT_DEATH : 0,
    in_flight: Math.round(Math.min(state.queued, participating * 64)),
    participating,
    node_count: healthyNodes().length,
    blocks_per_sec: rate,
    bytes_per_sec: rate * FRAG,
    budget_pct: rate > 0 ? 40 : 0,
    eta_s: rate > 0 ? state.queued / rate : 0,
    // A node that is merely absent gets `repair.delay` before anything is
    // copied — a reboot should not cost a hundred terabytes of network.
    grace_remaining_s: waiting ? REPAIR_DELAY_S - state.deadFor : 0,
    cause: state.queued > 0 ? 'w12 stopped answering' : null,
    worst_remaining: state.critical > 0 ? FRAGMENTS - 3 : FRAGMENTS - 1,
    total_fragments: FRAGMENTS,
  };
}

function start(): ClusterReport['start'] {
  return {
    last_start_ms: 6_240,
    started_at: state.t0 - 3 * 3600e3,
    block_map: 'mmap',
    mapped_bytes: 512 * 1024 * 1024,
    blocks: TOTAL_BLOCKS,
    roots_matched: 11,
    roots_total: 12,
    buckets_streamed: 3,
    merkle_fanout: 1024,
    shards: [
      { name: 'shard-0  /warehouse', state: 'ready', ready_ms: 2_180 },
      { name: 'shard-1  /logs', state: 'ready', ready_ms: 3_010 },
      { name: 'shard-2  /data', state: 'ready', ready_ms: 4_440 },
      { name: 'shard-3  /tmp · /user', state: 'ready', ready_ms: 6_240 },
    ],
    // What the same restart costs when the map is rebuilt from block reports
    // instead of mapped back — the HDFS number, kept next to ours on purpose.
    rebuild_equivalent_ms: 32 * 60_000,
  };
}

function alerts(): Alert[] {
  const out: Alert[] = [];
  if (state.queued > 0) {
    const share = Math.round((DEGRADED_AT_DEATH / TOTAL_BLOCKS) * 100);
    out.push({
      id: 'repair',
      level: state.critical > 0 ? 'danger' : 'warn',
      text: `${Math.round(state.queued).toLocaleString()} blocks are a fragment short after w12 stopped answering`,
      fix: 'mammoth admin repair status --live',
      at: Date.now() - state.deadFor * 1000,
    });
    // The interesting alert is not that a node died — it is *why* one node
    // dying touched almost everything.
    out.push({
      id: 'ec-width',
      level: 'danger',
      text:
        `one node failure degraded ${share}% of blocks — lrc-6-2-2 spreads ` +
        `${FRAGMENTS} fragments over ${SPECS.length} nodes, so nearly every block ` +
        `has one on every node`,
      fix: 'mammoth admin ec convert / --policy replication-3   # until the cluster is bigger',
      at: Date.now() - state.deadFor * 1000,
    });
  }
  out.push({
    id: 'w3',
    level: 'warn',
    text: 'w3 is 94% full — new writes are skipping it',
    fix: 'mammoth admin balancer start --threshold 10',
    at: Date.now() - 41 * 60e3,
  });
  out.push({
    id: 'w7',
    level: 'warn',
    text: 'w7 disk p99 is 340 ms — a slow disk is worse than a dead one',
    fix: 'mammoth doctor --node w7',
    at: Date.now() - 12 * 60e3,
  });
  out.push({
    id: 'tmp',
    level: 'info',
    text: '/tmp holds 71 TB, 94% of it older than 30 days',
    fix: 'mammoth rm /tmp --older-than 30d --dry-run',
    at: Date.now() - 6 * 3600e3,
  });
  return out;
}

function raft(): RaftMember[] {
  return [
    { id: 'm1', address: '192.168.1.5:7000', role: 'leader', applied: state.raftIndex, lag: 0, last_contact_ms: 0 },
    { id: 'm2', address: '192.168.1.6:7000', role: 'follower', applied: state.raftIndex - 3, lag: 3, last_contact_ms: 41 },
    { id: 'm3', address: '192.168.1.7:7000', role: 'follower', applied: state.raftIndex - 1, lag: 1, last_contact_ms: 38 },
    ...liveNodeIds().slice(0, 3).map((id, i) => ({
      id: `${id} (learner)`,
      address: `192.168.1.${11 + i}:7001`,
      role: 'learner' as const,
      applied: state.raftIndex - 40 - i * 12,
      lag: 40 + i * 12,
      last_contact_ms: 120 + i * 30,
    })),
  ];
}

export function clusterReport(): ClusterReport {
  const ns = nodes();
  const used = ns.reduce((a, n) => a + n.used, 0);
  const capacity = ns.reduce((a, n) => a + n.capacity, 0);
  const degraded = Math.round(state.queued);
  const critical = Math.round(state.critical);

  return {
    name: 'prod-01',
    leader: 'm1',
    safe_mode: false,
    used,
    capacity,
    topology_epoch: state.epoch,
    placement: 'rendezvous',
    nodes: ns,
    health: {
      healthy: TOTAL_BLOCKS - degraded - critical,
      under_replicated: degraded,
      critical,
      over_replicated: 88,
      corrupt: 0,
      missing: 0,
    },
    read_path: readPath(),
    write_path: writePath(),
    repair: repair(),
    start: start(),
    throughput: {
      read_bps: ns.reduce((a, n) => a + n.read_bps, 0),
      write_bps: ns.reduce((a, n) => a + n.write_bps, 0),
      repair_bps: repair().bytes_per_sec,
      balancer_bps: jit(180e6, 0.25),
      shuffle_bps: jit(890e6, 0.3),
      cross_rack_bps: jit(4.0e9, 0.15),
      cross_rack_capacity: 10e9,
    },
    alerts: alerts(),
    raft: raft(),
    raft_index: state.raftIndex,
    snapshot_age_s: 412,
  };
}

// ── the namespace ─────────────────────────────────────────────────────────

interface Entry {
  name: string;
  dir?: Entry[];
  len?: number;
  policy?: string;
  ageDays?: number;
  reads?: number;
}

const TREE: Entry = {
  name: '',
  dir: [
    {
      name: 'warehouse',
      dir: [
        { name: 'events', len: 612e12, policy: EC.policy, ageDays: 40, reads: 82_000 },
        { name: 'sales', len: 148e12, policy: EC.policy, ageDays: 90, reads: 21_000 },
        { name: 'dim', len: 82e12, policy: 'replication-3', ageDays: 12, reads: 44_000 },
      ],
    },
    {
      name: 'logs',
      dir: [
        { name: '2026-06', len: 96e12, policy: EC.policy, ageDays: 70, reads: 400 },
        { name: '2026-07', len: 104e12, policy: EC.policy, ageDays: 40, reads: 1_200 },
        { name: '2026-08', len: 110e12, policy: EC.policy, ageDays: 10, reads: 9_400 },
      ],
    },
    { name: 'tmp', len: 71e12, policy: 'replication-2', ageDays: 120, reads: 12 },
    { name: 'user', len: 17e12, policy: 'replication-3', ageDays: 20, reads: 3_100 },
    {
      name: 'data',
      dir: [
        { name: 'sales-2026.csv', len: 350e6, policy: 'replication-3', ageDays: 2, reads: 610 },
        { name: 'nyc-taxi.parquet', len: 1.2e9, policy: EC.policy, ageDays: 5, reads: 4_820 },
        { name: 'events.csv.gz', len: 8.2e9, policy: EC.policy, ageDays: 9, reads: 140 },
        { name: 'config.json', len: 4_210, policy: 'inline', ageDays: 1, reads: 90 },
      ],
    },
  ],
};

function lookup(path: string): Entry | null {
  const parts = path.split('/').filter(Boolean);
  let node: Entry | undefined = TREE;
  for (const p of parts) {
    node = node?.dir?.find((c) => c.name === p);
    if (!node) return null;
  }
  return node ?? null;
}

function sizeOf(e: Entry): number {
  return e.dir ? e.dir.reduce((a, c) => a + sizeOf(c), 0) : (e.len ?? 0);
}

function statusOf(e: Entry, path: string): FileStatus {
  const isDir = !!e.dir;
  const len = sizeOf(e);
  const inlined = e.policy === 'inline';
  return {
    path,
    name: e.name,
    is_dir: isDir,
    len,
    block_size: BLOCK,
    replication: e.policy?.startsWith('replication') ? Number(e.policy.split('-')[1]) : null,
    policy: e.policy ?? (isDir ? '—' : EC.policy),
    blocks: isDir || inlined ? 0 : Math.max(1, Math.ceil(len / BLOCK)),
    inlined,
    mode: isDir ? 0o755 : 0o644,
    owner: 'analytics',
    group: 'data',
    modified: Date.now() - (e.ageDays ?? 1) * 86400e3,
    checksum: isDir ? null : `crc32c:${mix(len).toString(16)}`,
  };
}

export function list(path: string): FileStatus[] {
  const node = lookup(path);
  if (!node?.dir) return [];
  const base = path === '/' ? '' : path.replace(/\/$/, '');
  return node.dir
    .map((c) => statusOf(c, `${base}/${c.name}`))
    .sort((a, b) => Number(b.is_dir) - Number(a.is_dir) || a.name.localeCompare(b.name));
}

export function stat(path: string): FileStatus | null {
  if (path === '/') return statusOf(TREE, '/');
  const node = lookup(path);
  return node ? statusOf(node, path) : null;
}

export function blockLayout(path: string): BlockLayout | null {
  const s = stat(path);
  if (!s || s.is_dir) return null;

  const mirrored = s.policy.startsWith('replication');
  const width = mirrored ? (s.replication ?? 3) : FRAGMENTS;
  const count = Math.min(s.blocks, 24);
  const seed = mix(path.length * 7919);

  const blocks: BlockPlacement[] = Array.from({ length: count }, (_, i) => {
    const id = 1001 + ((seed + i * 37) % 90000);
    const targets = place(id, width);
    const fragments: Fragment[] = targets.map((node, j) => {
      const kind: Fragment['kind'] = mirrored
        ? 'replica'
        : j < EC.k
          ? 'data'
          : j < EC.k + EC.local
            ? 'local-parity'
            : 'global-parity';
      const idx = mirrored ? j : j < EC.k ? j : j < EC.k + EC.local ? j - EC.k : j - EC.k - EC.local;
      // The fragment that used to live on w12 is what repair is rebuilding.
      const lost = i % 5 === 2 && j === width - 2 && state.queued > 0;
      return {
        kind,
        idx,
        node,
        rack: rackOf(node),
        state: lost ? 'repairing' : 'ok',
        preferred: j === 0,
      };
    });
    return {
      id,
      index: i,
      len: i === count - 1 ? s.len % BLOCK || BLOCK : BLOCK,
      policy: s.policy,
      fragments,
    };
  });

  const warnings: string[] = [];
  if (path.endsWith('.gz')) {
    warnings.push(
      'gzip is not splittable — this file is processed by a single task even though it spans every node it touches.',
    );
  }
  if (blocks.some((b) => b.fragments.some((f) => f.state === 'repairing'))) {
    warnings.push('Some fragments are being rebuilt after w12 stopped answering. Reads reconstruct in the meantime.');
  }

  return {
    path,
    len: s.len,
    block_size: BLOCK,
    policy: s.policy,
    inlined: s.inlined,
    blocks,
    nodes: SPECS.map((n) => n.id),
    racks: Object.fromEntries(SPECS.map((n) => [n.id, RACKS[n.rack]])),
    warnings,
  };
}

// ── distribution ──────────────────────────────────────────────────────────

export function heat(): HeatCell[] {
  return nodes().map((n) => ({
    node: n.id,
    rack: n.rack,
    state: n.state,
    usage: (n.used / n.capacity) * 100,
    fragments: n.fragments,
    read_qps: Math.round(n.read_bps / 4e6),
    write_qps: Math.round(n.write_bps / 4e6),
    disk_p99_ms: n.disk_p99_ms,
  }));
}

/** A directory's age is its children's, weighted by how much disk each holds —
 *  otherwise every directory is the same colour and the treemap only says
 *  something at the leaves, which is not where you look first. */
function ageOf(e: Entry): number {
  if (!e.dir?.length) return e.ageDays ?? 30;
  const total = sizeOf(e) || 1;
  return e.dir.reduce((a, c) => a + ageOf(c) * (sizeOf(c) / total), 0);
}

function readsOf(e: Entry): number {
  return e.dir?.length ? e.dir.reduce((a, c) => a + readsOf(c), 0) : (e.reads ?? 0);
}

export function treemap(path = '/', depth = 3): TreemapNode {
  function build(e: Entry, p: string, d: number): TreemapNode {
    return {
      name: e.name || '/',
      path: p || '/',
      value: sizeOf(e),
      age_days: Math.round(ageOf(e)),
      reads: readsOf(e),
      children:
        e.dir && d > 0 ? e.dir.map((c) => build(c, `${p}/${c.name}`, d - 1)) : undefined,
    };
  }
  const node = lookup(path) ?? TREE;
  return build(node, path === '/' ? '' : path, depth);
}

export function skew(path = '/warehouse/events'): SkewReport {
  const r = rng(0xbadc0de);
  const points = Array.from({ length: 240 }, (_, i) => {
    const day = String(i % 31).padStart(2, '0');
    const partition = `dt=2026-${String(8 - Math.floor(i / 31)).padStart(2, '0')}-${day}`;
    // One partition is 68x the median. That is the whole point of the chart:
    // your job's runtime is set by whichever task draws this one.
    const hot = i === 2;
    const size = hot ? 89e9 : 0.7e9 + r() * 1.4e9;
    return {
      partition,
      size,
      reads: hot ? 8_200 : Math.round(20 + r() * 260),
      writes: hot ? 12 : Math.round(r() * 900),
    };
  });
  const sizes = points.map((p) => p.size).sort((a, b) => a - b);
  return {
    path,
    files: 1_024,
    total: points.reduce((a, p) => a + p.size, 0),
    median: sizes[Math.floor(sizes.length / 2)],
    p99: sizes[Math.floor(sizes.length * 0.99)],
    max: sizes[sizes.length - 1],
    points,
  };
}

export function topology(): TopologyReport {
  const ns = nodes();
  const links: TopologyLink[] = [];
  for (let i = 0; i < RACKS.length; i++) {
    for (let j = i + 1; j < RACKS.length; j++) {
      links.push({ source: RACKS[i], target: RACKS[j], bps: jit(1.3e9, 0.4) });
    }
  }
  return {
    epoch: state.epoch,
    nodes: ns.map((n) => ({
      id: n.id,
      rack: n.rack,
      capacity: n.capacity,
      used: n.used,
      state: n.state,
    })),
    racks: RACKS,
    links,
  };
}

export function flow(): FlowReport {
  const live = liveNodeIds();
  const rep = repair();
  const links: FlowLink[] = [];

  const push = (source: string, targets: string[], total: number) => {
    if (total <= 0 || targets.length === 0) return;
    for (const t of targets) links.push({ source, target: t, bps: total / targets.length });
  };

  push('clients', live.filter((_, i) => i % 2 === 0), jit(2.1e9, 0.15));
  // Declustered repair: every healthy node is a target, which is exactly what
  // this chart is for — the fan should be wide, not narrow.
  push('repair', live, rep.bytes_per_sec);
  push('balancer', ['w8'], jit(180e6, 0.3));
  push('shuffle', live.filter((_, i) => i % 3 === 0), jit(890e6, 0.25));

  return { window_s: 60, nodes: live, links };
}

// ── jobs ──────────────────────────────────────────────────────────────────

export function jobs(): Job[] {
  const stages: Stage[] = [
    { id: 'scan', name: 'scan /warehouse/events', kind: 'map', deps: [], tasks: 240, done: 240 },
    { id: 'filter', name: 'filter dt >= 2026-08', kind: 'map', deps: ['scan'], tasks: 240, done: 218 },
    { id: 'shuffle', name: 'shuffle by user_id', kind: 'shuffle', deps: ['filter'], tasks: 64, done: 22 },
    { id: 'agg', name: 'aggregate', kind: 'reduce', deps: ['shuffle'], tasks: 64, done: 0 },
  ];

  const r = rng(0x10b5);
  const live = liveNodeIds();
  const tasks: Task[] = [];
  let id = 0;
  const complete = (id: string) => {
    const s = stages.find((x) => x.id === id);
    return !!s && s.done === s.tasks;
  };
  for (const [si, stage] of stages.entries()) {
    // A stage cannot have started until everything it depends on has finished,
    // so a reduce does not show running tasks while its shuffle is at 22/64.
    const started = stage.deps.every(complete);
    for (let i = 0; i < Math.min(stage.tasks, 26); i++) {
      const done = i / 26 < stage.done / stage.tasks;
      const running = started && !done && i / 26 < stage.done / stage.tasks + 0.14;
      // One straggler, on the slow disk, because that is what a Gantt is for.
      const straggler = si === 2 && i === 7;
      tasks.push({
        id: `t${id++}`,
        stage: stage.id,
        node: straggler ? 'w7' : live[Math.floor(r() * live.length)],
        state: done ? 'done' : running ? 'running' : 'pending',
        start_s: si * 46 + i * 1.4 + r() * 3,
        dur_s: straggler ? 88 : 12 + r() * 18,
        local: r() > 0.12,
      });
    }
  }

  return [
    {
      id: 'job-2026-0826-0041',
      name: 'daily-active-users',
      user: 'analytics',
      state: 'running',
      submitted: Date.now() - 214e3,
      elapsed_s: 214,
      progress: 0.62,
      locality: 0.88,
      stages,
      tasks,
    },
    {
      id: 'job-2026-0826-0038',
      name: 'sessionize',
      user: 'analytics',
      state: 'succeeded',
      submitted: Date.now() - 3_100e3,
      elapsed_s: 742,
      progress: 1,
      locality: 0.91,
      stages: [],
      tasks: [],
    },
    {
      id: 'job-2026-0826-0033',
      name: 'export-parquet',
      user: 'etl',
      state: 'failed',
      submitted: Date.now() - 7_400e3,
      elapsed_s: 118,
      progress: 0.31,
      locality: 0.64,
      stages: [],
      tasks: [],
    },
  ];
}

// ── the time machine ──────────────────────────────────────────────────────

/**
 * Run `fn` against a rewound clock.
 *
 * Not a recording — it is the same generator with the incident's clock moved
 * back, which is enough to make the slider on the distribution page tell the
 * truth about the shape of a failure: w12 alive, then dead, then the repair fan
 * opening across every surviving node and closing again.
 */
function rewound<T>(minutesAgo: number, fn: () => T): T {
  if (minutesAgo <= 0) return fn();

  const saved = { deadFor: state.deadFor, queued: state.queued, critical: state.critical };
  const deadAtMinutes = state.deadFor / 60;
  const w12 = SPECS.find((s) => s.id === 'w12')!;

  if (minutesAgo > deadAtMinutes) {
    w12.state = 'healthy';
    w12.fill = w12.fillBeforeDeath ?? 0;
    state.deadFor = 0;
    state.queued = 0;
    state.critical = 0;
  } else {
    // Same function as the live view, run at an earlier clock — so dragging the
    // slider forward reproduces exactly what the page showed at that moment.
    state.deadFor = (deadAtMinutes - minutesAgo) * 60;
    state.queued = queuedAt(state.deadFor);
    state.critical = CRITICAL_BLOCKS;
  }

  try {
    return fn();
  } finally {
    w12.state = 'dead';
    w12.fill = 0;
    state.deadFor = saved.deadFor;
    state.queued = saved.queued;
    state.critical = saved.critical;
  }
}

/** The cluster as it was `minutesAgo` minutes ago. */
export function reportAt(minutesAgo: number): ClusterReport {
  return rewound(minutesAgo, clusterReport);
}

export function heatAt(minutesAgo: number): HeatCell[] {
  return rewound(minutesAgo, heat);
}

export function flowAt(minutesAgo: number): FlowReport {
  return rewound(minutesAgo, flow);
}

export function topologyAt(minutesAgo: number): TopologyReport {
  return rewound(minutesAgo, topology);
}
