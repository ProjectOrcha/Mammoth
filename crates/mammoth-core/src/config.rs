//! One config file, sane defaults (Part XII).
//!
//! Resolution order, lowest precedence first:
//!   1. the defaults in this module
//!   2. `/etc/mammoth/mammoth.toml`, then `~/.mammoth/mammoth.toml`
//!   3. the path given by `--config` / `MAMMOTH_CONFIG`
//!   4. environment overrides, `MAMMOTH_` prefixed, `__` for nesting:
//!      `MAMMOTH_STORAGE__REPLICATION=2`
//!
//! `mammoth config show` prints the resolved value *and* which layer set it.

use serde::{Deserialize, Serialize};

/// The whole of `mammoth.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Cluster identity and master addresses.
    pub cluster: Cluster,
    /// This node's role and failure domain.
    pub node: Node,
    /// Block storage settings.
    pub storage: Storage,
    /// Write pipeline settings.
    pub write: Write,
    /// Read path settings.
    pub read: Read,
    /// Re-replication and repair settings.
    pub repair: Repair,
    /// Master-only settings.
    pub master: Master,
    /// Gateway listeners.
    pub gateway: Gateway,
    /// TLS and authentication.
    pub security: Security,
    /// Metrics and logging.
    pub telemetry: Telemetry,
}

/// `[cluster]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Cluster {
    /// Human-readable cluster name, shown in the CLI and UI headers.
    pub name: String,
    /// Every master, so clients can retry elsewhere on `NotLeader`.
    pub masters: Vec<String>,
}

impl Default for Cluster {
    fn default() -> Self {
        Self { name: "mammoth".into(), masters: vec!["127.0.0.1:7000".into()] }
    }
}

/// Which subsystems this process runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// Namespace, block map, leases, scheduler.
    Master,
    /// Block storage and task execution.
    #[default]
    Worker,
    /// S3 API and web UI.
    Gateway,
    /// All of the above in one process — the quickstart layout.
    All,
}

/// `[node]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Node {
    /// Role this process runs as.
    pub role: Role,
    /// Failure domain path, e.g. `/dc1/rack-a`. Drives rack-aware placement.
    pub rack: String,
}

impl Default for Node {
    fn default() -> Self {
        Self { role: Role::Worker, rack: "/default-rack".into() }
    }
}

/// `[storage]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Storage {
    /// One directory per physical disk. Do not put two volumes on one spindle.
    pub volumes: Vec<String>,
    /// Default block size. See Part IX §9.1 before changing it.
    pub block_size: String,
    /// Default replica count.
    pub replication: u8,
    /// Files at or below this size skip the block layer entirely (§9.3).
    pub inline_threshold: String,
    /// Space kept free on every volume for the OS.
    pub reserved_space: String,
    /// How the replica set for a block is decided.
    pub placement: Placement,
    /// Background bit-rot scrubber.
    pub scrub: Scrub,
}

/// How the replica set for a block is decided.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Placement {
    /// Rendezvous (Highest Random Weight) hashing: a pure function of the block
    /// ID and the topology, so every party derives the same answer without
    /// asking. Prerequisite for one-shot reads, declustered repair and warm
    /// start (guide ch. 12 §0).
    #[default]
    Rendezvous,
    /// Placement is stored in the master and looked up, HDFS-style. Everything
    /// on the fast paths gets slower; kept for migration comparisons.
    Explicit,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            volumes: vec![],
            block_size: "128MiB".into(),
            replication: 3,
            inline_threshold: "1MiB".into(),
            reserved_space: "10GiB".into(),
            placement: Placement::Rendezvous,
            scrub: Scrub::default(),
        }
    }
}

/// `[storage.scrub]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Scrub {
    /// Whether to continuously re-verify block checksums.
    pub enabled: bool,
    /// Throttle, so scrubbing never competes with real reads.
    pub bytes_per_sec: String,
}

impl Default for Scrub {
    fn default() -> Self {
        Self { enabled: true, bytes_per_sec: "50MiB".into() }
    }
}

/// How many replicas must be durable before a write is acknowledged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AckPolicy {
    /// Wait for every replica. Safest, worst p99 — one slow disk stalls the client.
    All,
    /// Ack at a majority and repair the rest asynchronously (Part VIII §6).
    #[default]
    Quorum,
}

/// How a block's copies reach the workers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WriteMode {
    /// Erasure-coded fragments, all sent at once. Network depth 1, storage
    /// 1.5-1.67x, client uplink (k+m)/k. The default (guide ch. 12 §2).
    #[default]
    Disperse,
    /// Whole copies down a two-level fan-out tree: client -> one worker ->
    /// the rest in parallel. Depth 2, 1x uplink, 3x storage. For thin clients
    /// and blocks small enough that one hop is all of it.
    Mirror,
    /// The HDFS chain: client -> w1 -> w2 -> w3, acks back down. Depth 3.
    /// Kept so migrations can compare like with like.
    Pipeline,
}

/// `[write]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Write {
    /// Dispersal, mirroring, or the HDFS chain.
    pub mode: WriteMode,
    /// Erasure-coding policy, e.g. `lrc-6-2-2` or `rs-6-3`. Ignored unless
    /// `mode = "disperse"`.
    pub ec_policy: String,
    /// Durability/latency tradeoff.
    pub ack_policy: AckPolicy,
    /// Unit of transfer on a fragment stream.
    pub packet_size: String,
    /// Per-fragment sliding window. Each stream flows independently, so there
    /// is no shared packet barrier and jitter stays on one socket.
    pub window: String,
}

impl Default for Write {
    fn default() -> Self {
        Self {
            mode: WriteMode::Disperse,
            ec_policy: "lrc-6-2-2".into(),
            ack_policy: AckPolicy::Quorum,
            packet_size: "64KiB".into(),
            window: "8MiB".into(),
        }
    }
}

/// `[read]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Read {
    /// Pass the file descriptor over a Unix socket when the replica is local.
    pub short_circuit: bool,
    /// Fire a duplicate request at another replica after this delay.
    pub hedged_after: String,
    /// Size of the hybrid memory + SSD read cache.
    pub cache_size: String,
    /// How long a location lease stays usable. While one is valid the client
    /// reads any range of that file with no metadata round trip at all.
    pub lease_ttl: String,
    /// Let workers resolve `path + range` from their own read-only replica of
    /// the namespace, so a client with no lease still costs one round trip.
    pub inline_resolve: bool,
}

impl Default for Read {
    fn default() -> Self {
        Self {
            short_circuit: true,
            hedged_after: "50ms".into(),
            cache_size: "4GiB".into(),
            lease_ttl: "60s".into(),
            inline_resolve: true,
        }
    }
}

/// Which blocks the repair queue works on first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepairPriority {
    /// Least-redundant blocks first: a block down to its last fragment is
    /// repaired before one that has only lost its first.
    #[default]
    Redundancy,
    /// Oldest damage first. Simpler, and wrong when it matters.
    Age,
}

/// `[repair]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Repair {
    /// Grace period before touching anything for a worker that is merely
    /// absent. Confirmed disk loss skips the window and starts immediately.
    pub delay: String,
    /// `auto` = every healthy worker participates. Repair is declustered, so
    /// rebuild time scales with the cluster and not with one disk.
    pub parallelism: String,
    /// Token-bucket cap. `auto` measures idle bandwidth. Not optional: repair
    /// that takes an outage with it is worse than repair that takes longer.
    pub bytes_per_sec: String,
    /// Queue ordering.
    pub priority: RepairPriority,
}

impl Default for Repair {
    fn default() -> Self {
        Self {
            delay: "10m".into(),
            parallelism: "auto".into(),
            bytes_per_sec: "auto".into(),
            priority: RepairPriority::Redundancy,
        }
    }
}

/// `[master]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Master {
    /// gRPC listen address.
    pub listen: String,
    /// Raft log and snapshot directory.
    pub data_dir: String,
    /// Worker heartbeat interval.
    pub heartbeat_ms: u64,
    /// How long a worker may be silent before its blocks are re-replicated.
    pub dead_after: String,
    /// Whether the block map is memory-mapped back or rebuilt from reports.
    pub block_map: BlockMapMode,
    /// Leaves in each worker's block-ID Merkle tree. A matching root confirms
    /// millions of blocks in 32 bytes; a mismatch narrows to one bucket.
    pub merkle_fanout: u32,
    /// Whether safe mode is one cluster-wide gate or per namespace shard.
    pub safemode: SafeMode,
    /// Fraction of blocks that must report in before leaving safe mode.
    /// Only consulted when `safemode = "global"`.
    pub safemode_threshold: f64,
}

/// How the master gets its block map back at startup.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockMapMode {
    /// The map is archived next to the Raft snapshot and memory-mapped on
    /// start: O(1) in the number of blocks (guide ch. 12 §4).
    #[default]
    Mmap,
    /// Re-derive the map from full block reports, HDFS-style — the 30-minute
    /// boot. Kept as a tested fallback for a corrupt archive.
    Rebuild,
}

/// How much of the cluster one un-reconciled shard holds up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SafeMode {
    /// Each namespace shard leaves safe mode as soon as its own ranges
    /// reconcile. Reads are served from the mapped snapshot immediately.
    #[default]
    PerRange,
    /// One cluster-wide gate on `safemode_threshold`, HDFS-style.
    Global,
}

impl Default for Master {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:7000".into(),
            data_dir: "/var/lib/mammoth/meta".into(),
            heartbeat_ms: 3000,
            dead_after: "10m".into(),
            block_map: BlockMapMode::Mmap,
            merkle_fanout: 1024,
            safemode: SafeMode::PerRange,
            safemode_threshold: 0.999,
        }
    }
}

/// `[gateway]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Gateway {
    /// S3-compatible API listener.
    pub s3_listen: String,
    /// Web UI and REST/SSE listener.
    pub ui_listen: String,
}

impl Default for Gateway {
    fn default() -> Self {
        Self { s3_listen: "0.0.0.0:9000".into(), ui_listen: "0.0.0.0:8080".into() }
    }
}

/// `[security]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Security {
    /// `auto` | `required` | `off`. `off` is for local development only.
    pub tls: String,
    /// `token` | `mtls` | `kerberos` | `none`.
    pub auth: String,
}

impl Default for Security {
    fn default() -> Self {
        Self { tls: "auto".into(), auth: "token".into() }
    }
}

/// `[telemetry]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Telemetry {
    /// Prometheus scrape endpoint.
    pub metrics_listen: String,
    /// `json` for production, `pretty` for a terminal.
    pub log_format: String,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self { metrics_listen: "0.0.0.0:9100".into(), log_format: "json".into() }
    }
}
