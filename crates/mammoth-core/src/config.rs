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
    /// Background bit-rot scrubber.
    pub scrub: Scrub,
}

impl Default for Storage {
    fn default() -> Self {
        Self {
            volumes: vec![],
            block_size: "128MiB".into(),
            replication: 3,
            inline_threshold: "1MiB".into(),
            reserved_space: "10GiB".into(),
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

/// `[write]`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Write {
    /// Unit of the chain-replication pipeline.
    pub packet_size: String,
    /// Durability/latency tradeoff.
    pub ack_policy: AckPolicy,
}

impl Default for Write {
    fn default() -> Self {
        Self { packet_size: "64KiB".into(), ack_policy: AckPolicy::Quorum }
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
}

impl Default for Read {
    fn default() -> Self {
        Self { short_circuit: true, hedged_after: "50ms".into(), cache_size: "4GiB".into() }
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
    /// Fraction of blocks that must report in before leaving safe mode.
    pub safemode_threshold: f64,
}

impl Default for Master {
    fn default() -> Self {
        Self {
            listen: "0.0.0.0:7000".into(),
            data_dir: "/var/lib/mammoth/meta".into(),
            heartbeat_ms: 3000,
            dead_after: "10m".into(),
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
