# Roadmap

Ship at **M5**. By then Mammoth is a fast, easy, S3-compatible distributed
filesystem with the best data-distribution UI in its category. The feedback from
that release decides whether M7 is worth ten weeks.

| Milestone | Weeks | You can demo |
| --- | --- | --- |
| **M1 · CLI + LocalBackend** | 1–4 | `put`, `get`, `ls`, `stat` all work on one machine |
| **M2 · Visualization + `top`** | 5–6 | `viz blocks`, `viz cluster`, `viz skew`, the TUI dashboard |
| **M3 · Web UI** | 7–9 | full GUI with the distribution page, live SSE updates |
| **M4 · Real block storage** | 10–13 | throughput benchmark near raw disk speed |
| **M5 · Distributed + S3 → v0.1 RELEASE** | 14–20 | kill a node and watch re-replication in the UI; DuckDB queries the cluster |
| M6 · HA (Raft) | 21–24 | kill the leader mid-write, it survives |
| M7 · Compute | 25–34 | TeraSort beats Hadoop MapReduce |
| M8 · Migration + EC | 35–42 | migrate a real HDFS cluster |

## Crate status

| Crate | Milestone | Job |
| --- | --- | --- |
| `mammoth-core` | M1 | traits, types, errors, config |
| `mammoth-cli` | M1 | the `mammoth` binary |
| `mammoth-local` | M1 | `LocalBackend` — single-machine simulation |
| `mammoth-viz` | M2 | terminal charts, heatmaps, the `ratatui` dashboard |
| `mammoth-gateway` | M3 | web server, REST/SSE, S3 API, embedded UI |
| `mammoth-storage` | M4 | block files, checksums, volumes, scrubber |
| `mammoth-proto` | M5 | protobuf + tonic codegen |
| `mammoth-rpc` | M5 | transport, connection pool, auth |
| `mammoth-master` | M5 | master role |
| `mammoth-worker` | M5 | worker role |
| `mammoth-client` | M5 | `ClusterBackend` + Rust SDK |
| `mammoth-testkit` | M5 | cluster harness, fault injection, deterministic sim |
| `mammoth-meta` | M6 | inode tree, block map, leases, Raft state machine |
| `mammoth-scheduler` | M7 | queues, placement, locality |
| `mammoth-compute` | M7 | DAG engine, shuffle |
| `mammoth-migrate` | M8 | HDFS/S3 migration |

## Week 1

1. The `Backend` trait in `mammoth-core` — the whole architecture hangs off it.
2. `LocalBackend` faking six workers as six subdirectories on disk.
3. `mammoth ls` and `mammoth put` against it.
4. `mammoth viz blocks` — because seeing your fake blocks land is the moment
   this stops feeling like homework.
