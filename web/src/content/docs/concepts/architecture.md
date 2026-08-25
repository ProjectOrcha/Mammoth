---
title: Architecture
description: Masters, workers, gateway — and the one trait everything hangs off.
---

```
┌──────────────────────── mammoth cluster ─────────────────────────┐
│                                                                   │
│   masters (3, Raft consensus)          the index — HA by default  │
│   ┌───────┐  ┌───────┐  ┌───────┐                                 │
│   │leader │  │follow │  │follow │      namespace, block map,      │
│   └───┬───┘  └───────┘  └───────┘      leases, scheduler          │
│       │                                                            │
│       │ heartbeats (3s) + block digests                            │
│       ▼                                                            │
│   workers (N)                          the shelves + the muscle    │
│   ┌───────┐  ┌───────┐  ┌───────┐                                 │
│   │blocks │  │blocks │  │blocks │      block storage,             │
│   │tasks  │  │tasks  │  │tasks  │      task execution, shuffle    │
│   └───────┘  └───────┘  └───────┘                                 │
│       ▲                                                            │
│       │ data (never touches the master)                            │
│   ┌───┴────────────────────────────────────┐                      │
│   │ gateway  ·  S3 :9000  ·  Web UI :8080  │                      │
│   └────────────────────────────────────────┘                      │
└───────────────────────────────────────────────────────────────────┘
```

One binary: `mammoth serve --role master|worker|gateway|all`.

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
