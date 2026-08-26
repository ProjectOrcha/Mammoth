---
title: Architecture
description: Masters, workers, gateway — and the one trait everything hangs off.
---

```mermaid
flowchart TB
    clients["clients<br/>CLI · SDK · any S3 tool"]
    gw["gateway<br/>S3 :9000 · Web UI :8080"]
    masters["masters ×3 · Raft — the index<br/>1 leader, 2 followers<br/>namespace · block map<br/>leases · scheduler<br/>HA by default"]
    workers["workers ×N — the shelves,<br/>and the muscle<br/>block storage<br/>task execution · shuffle"]

    clients --> gw
    gw -->|"open — once per lease,<br/>not once per read"| masters
    masters -->|"namespace, streamed to<br/>read-only learners"| workers
    gw ==>|"data — never touches the master<br/>reads resolve at the worker<br/>writes disperse in parallel"| workers
    workers -.->|"heartbeats every 3s<br/>+ 32-byte Merkle root"| masters
```

One binary: `mammoth serve --role master|worker|gateway|all`.

Note what the master is *not* on: the read path. Placement is computed from the
block ID rather than looked up, `open` returns a lease covering the whole file,
and workers carry a read-only replica of the namespace — so a warm read never
speaks to a master at all, and a cold one can resolve at the first worker it
reaches. That, the parallel write, the declustered repair and the memory-mapped
restart are the four mechanisms that separate this design from HDFS's:
**[The four fast paths](/Mammoth/concepts/fast-paths/)**.

## The Backend trait

Everything above hides behind one trait. The CLI, the gateway and the SDK are
written against it, so swapping a single-machine simulation for a real cluster
changes no caller.

```rust
#[async_trait]
pub trait Backend: Send + Sync {
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>>;
    async fn stat(&self, path: &Path) -> Result<FileStatus>;
    async fn read(&self, path: &Path, range: Range<u64>) -> Result<ByteStream>;
    async fn write(&self, path: &Path, data: ByteStream) -> Result<()>;
    async fn remove(&self, path: &Path, recursive: bool) -> Result<()>;
    async fn block_layout(&self, path: &Path) -> Result<Vec<BlockPlacement>>;
    async fn cluster_report(&self) -> Result<ClusterReport>;
}
```

Two implementations: `LocalBackend` (one machine, simulated workers) and
`ClusterBackend` (real masters and workers over gRPC).
