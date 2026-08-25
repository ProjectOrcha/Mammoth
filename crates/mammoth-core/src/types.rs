//! Wire and display types shared by every crate.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// A globally unique block identifier. Rendered as `blk_0000000000012345`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BlockId(pub u64);

/// A worker identifier, e.g. `w3`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NodeId(pub String);

/// One entry in a directory listing, or the result of `stat`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileStatus {
    /// Absolute path inside the Mammoth namespace.
    pub path: PathBuf,
    /// `true` for directories, which have no blocks and no replication factor.
    pub is_dir: bool,
    /// Length in bytes. Zero for directories.
    pub len: u64,
    /// Block size this file was written with, in bytes.
    pub block_size: u64,
    /// Target replica count. `None` for directories.
    pub replication: Option<u8>,
    /// Number of blocks. Zero for inlined small files (Part IX §9.3).
    pub blocks: u32,
    /// `true` when the bytes live in the metadata store rather than in blocks.
    pub inlined: bool,
    /// Unix mode bits.
    pub mode: u32,
    /// Owning user and group.
    pub owner: String,
    /// Owning group.
    pub group: String,
    /// Last modification, as a Unix timestamp in seconds.
    pub modified: i64,
    /// `crc32c:8a3f21e0` style composite checksum, when computed.
    pub checksum: Option<String>,
}

/// Health of one replica of one block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplicaState {
    /// First replica written; served preferentially for reads.
    Primary,
    /// A healthy secondary copy.
    Replica,
    /// Checksum mismatch — scheduled for re-replication from a good copy.
    Corrupt,
}

/// One replica of one block, on one node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Replica {
    /// Node holding this replica.
    pub node: NodeId,
    /// Failure domain of that node, e.g. `/dc1/rack-a`.
    pub rack: String,
    /// Health of this copy.
    pub state: ReplicaState,
}

/// Where the replicas of one block live. The input to the block placement matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockPlacement {
    /// Which block this is.
    pub id: BlockId,
    /// Zero-based position of this block within its file.
    pub index: u32,
    /// Bytes in this block. The last block of a file is usually partial.
    pub len: u64,
    /// Every known copy of this block.
    pub replicas: Vec<Replica>,
}

/// Liveness of a worker, as judged by the master's heartbeat tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeState {
    /// Heartbeating, with capacity to spare.
    Healthy,
    /// Heartbeating, but near-full or serving slow reads.
    Warn,
    /// Draining before removal; no new writes are placed here.
    Decommissioning,
    /// Intentionally offline; the master will not re-replicate its blocks yet.
    Maintenance,
    /// Missed heartbeats past `master.dead_after`; its blocks are being rebuilt.
    Dead,
}

/// A worker as the master sees it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeReport {
    /// Worker identifier.
    pub id: NodeId,
    /// Advertised address.
    pub address: String,
    /// Failure domain, e.g. `/dc1/rack-a`.
    pub rack: String,
    /// Liveness.
    pub state: NodeState,
    /// Bytes in use across all volumes.
    pub used: u64,
    /// Total bytes across all volumes.
    pub capacity: u64,
    /// Number of block replicas stored here.
    pub blocks: u64,
    /// Configured storage volumes.
    pub volumes: u16,
    /// 99th percentile disk service time, in milliseconds.
    pub disk_p99_ms: f64,
}

/// Cluster-wide replication accounting — the numbers behind `mammoth viz health`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicationHealth {
    /// Blocks at their target replica count.
    pub healthy: u64,
    /// Below target but with more than one copy left.
    pub under_replicated: u64,
    /// Down to a single copy — urgent.
    pub critical: u64,
    /// Above target; the excess will be reclaimed.
    pub over_replicated: u64,
    /// Failed checksum verification.
    pub corrupt: u64,
    /// No copies remain anywhere.
    pub missing: u64,
}

/// A whole-cluster snapshot: the payload behind `GET /api/v1/cluster/report`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterReport {
    /// Cluster name from `[cluster] name`.
    pub name: String,
    /// Current Raft leader, if one is elected.
    pub leader: Option<NodeId>,
    /// `true` while the master is still collecting block reports.
    pub safe_mode: bool,
    /// Bytes used cluster-wide.
    pub used: u64,
    /// Bytes of raw capacity cluster-wide.
    pub capacity: u64,
    /// Every known worker.
    pub nodes: Vec<NodeReport>,
    /// Replication accounting.
    pub health: ReplicationHealth,
}
