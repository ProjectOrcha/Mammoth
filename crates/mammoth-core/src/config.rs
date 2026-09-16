//! One config file, sane defaults (Part XII).
//!
//! Resolution order, lowest precedence first:
//!   1. the defaults in this module
//!   2. `/etc/mammoth/mammoth.toml`, then `~/.mammoth/mammoth.toml`
//!   3. the path given by `--config` / `MAMMOTH_CONFIG`
//!   4. environment overrides, `MAMMOTH_` prefixed, `__` for nesting:
//!      `MAMMOTH_STORAGE__REPLICATION=2`
//!
//! `mammoth config show` prints the resolved values; source-layer provenance is not implemented.

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
    /// Local parallel text-job memory settings.
    pub compute: Compute,
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
    /// Budget for the process-local verified read cache; zero disables it.
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
            cache_size: "256MiB".into(),
            lease_ttl: "60s".into(),
            inline_resolve: true,
        }
    }
}

/// `[compute]` settings for the implemented local parallel text engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Compute {
    /// Working-set target per job; an accounting target rather than process RSS.
    pub memory_budget: String,
    /// Optional existing directory for recomputable spill files; empty uses OS temp.
    pub spill_directory: String,
}
impl Default for Compute {
    fn default() -> Self {
        Self { memory_budget: "128MiB".into(), spill_directory: String::new() }
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
        Self { s3_listen: "127.0.0.1:9000".into(), ui_listen: "127.0.0.1:8080".into() }
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
        Self { tls: "off".into(), auth: "none".into() }
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

impl Config {
    /// Load defaults, system/user config, explicit config, then environment.
    pub fn load(path: Option<&std::path::Path>) -> crate::Result<Self> {
        use figment::{
            providers::{Env, Format, Serialized, Toml},
            Figment,
        };
        let mut f = Figment::from(Serialized::defaults(Self::default()))
            .merge(Toml::file("/etc/mammoth/mammoth.toml"));
        if let Some(home) = std::env::var_os("HOME") {
            f = f.merge(Toml::file(std::path::PathBuf::from(home).join(".mammoth/mammoth.toml")));
        }
        if let Some(path) = path {
            if !path.is_file() {
                return Err(crate::Error::Config(format!(
                    "config file not found: {}",
                    path.display()
                )));
            }
            f = f.merge(Toml::file(path));
        }
        let config: Self = f
            .merge(
                Env::prefixed("MAMMOTH_").ignore(&["CONFIG", "MASTERS", "LOCAL_ROOT"]).split("__"),
            )
            .extract()
            .map_err(|e| crate::Error::Config(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }
    /// Fail closed when a local service is asked to enable unimplemented security.
    pub fn validate_local_service(&self) -> crate::Result<()> {
        self.validate()?;
        if self.security.tls != "off" || self.security.auth != "none" {
            return Err(crate::Error::Config("TLS and authentication are not implemented by the local service. Use security.tls=off and security.auth=none only on a trusted local machine; do not expose it to untrusted networks.".into()));
        }
        if self.storage.replication > 6 {
            return Err(crate::Error::Config("local storage supports at most six replicas".into()));
        }
        Ok(())
    }

    /// Check bounds before any I/O or memory allocation.
    pub fn validate(&self) -> crate::Result<()> {
        if parse_size(&self.read.cache_size)? > 4 * 1024 * 1024 * 1024 {
            return Err(crate::Error::Config(
                "read cache must be at most 4 GiB; zero disables it".into(),
            ));
        }
        if !(64 * 1024..=4 * 1024 * 1024 * 1024).contains(&parse_size(&self.compute.memory_budget)?)
        {
            return Err(crate::Error::Config("compute memory budget must be 64 KiB–4 GiB".into()));
        }
        let block = parse_size(&self.storage.block_size)?;
        let inline = parse_size(&self.storage.inline_threshold)?;
        if block == 0 || block > 256 * 1024 * 1024 || inline > 16 * 1024 * 1024 {
            return Err(crate::Error::Config(
                "block_size must be 1 B–256 MiB and inline_threshold at most 16 MiB".into(),
            ));
        }
        if self.storage.replication == 0 {
            return Err(crate::Error::Config("replication must be positive".into()));
        }
        if !(0.0..=1.0).contains(&self.master.safemode_threshold) || self.master.heartbeat_ms == 0 {
            return Err(crate::Error::Config(
                "invalid safe-mode threshold or heartbeat interval".into(),
            ));
        }
        for address in [&self.master.listen, &self.gateway.ui_listen, &self.gateway.s3_listen] {
            address
                .parse::<std::net::SocketAddr>()
                .map_err(|e| crate::Error::Config(format!("{address}: {e}")))?;
        }
        Ok(())
    }
}

/// Parse integer byte sizes with SI or IEC units, rejecting overflow.
pub fn parse_size(value: &str) -> crate::Result<u64> {
    let value = value.trim();
    let split = value.find(|c: char| !c.is_ascii_digit()).unwrap_or(value.len());
    let n: u64 = value[..split]
        .parse()
        .map_err(|_| crate::Error::Config(format!("invalid byte size: {value}")))?;
    let factor = match value[split..].trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1,
        "kb" => 1000,
        "mb" => 1000_u64.pow(2),
        "gb" => 1000_u64.pow(3),
        "kib" => 1024,
        "mib" => 1024_u64.pow(2),
        "gib" => 1024_u64.pow(3),
        _ => return Err(crate::Error::Config(format!("unknown size unit: {value}"))),
    };
    n.checked_mul(factor)
        .ok_or_else(|| crate::Error::Config(format!("byte size overflow: {value}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_sizes_and_settings_are_rejected_before_allocation() {
        assert_eq!(parse_size("128MiB").unwrap(), 134217728);
        assert_eq!(parse_size("2 MB").unwrap(), 2_000_000);
        for value in ["", "-1", "1.5MiB", "10watts", "18446744073709551615GiB"] {
            assert!(parse_size(value).is_err(), "{value}");
        }
        let mut config = Config::default();
        config.storage.block_size = "0".into();
        assert!(config.validate().is_err());
        config.storage.block_size = "1MiB".into();
        config.storage.replication = 0;
        assert!(config.validate().is_err());
    }
}
