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

## Planned crate responsibilities

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

## Implemented in this checkout

The CLI parses help/version and reports unsupported commands without a panic.
Core types/errors and 16 teaching examples exist. The dashboard runs with demo
data; the docs site builds. A separate executable GFS model now lives in
`mammoth-local`: it demonstrates in-memory chunk replicas, heartbeat repair,
primary-ordered mutations and standby takeover with stale-lease rejection.
Run `cargo run -p mammoth-local --example gfs-demo`. It does not implement
`LocalBackend`, durable storage, network services or Raft.

`cargo xtask` builds the UI, generates the CLI
reference, copies logos and delegates release builds. Storage, gateway serving
and the distributed harnesses remain placeholders. See the detailed
[current-status table](guide/START-HERE.md#1-know-what-works-today).

The week estimates above are planning targets, not a promise for a beginner team.
Use the [four-person plan](guide/TEAM-PLAN.md) to progress by tested handoffs.

## Reliability gates from the GFS review

The full [GFS coverage audit](guide/GFS-COVERAGE.md) maps each video item to
code and tests. Keep these gates open until the real service passes them:

- **M4:** checksummed chunk files, durable acknowledgements, crash/restart tests.
- **M5:** typed location and mutation RPCs, direct worker streaming, heartbeat
  deadlines, source-verified repair, explicit concurrent-write semantics and
  worker-enforced leases/version ordering. Bytes sent in parallel still need an
  ordering and retry protocol.
- **M6:** persisted replicated metadata, quorum election, old-leader fencing,
  client leader discovery/retry, recovery during writes and lost-quorum tests.
  Changing DNS alone does not establish safe leadership.
- **M5–M6:** multi-process fault tests and benchmarks under repair load. The
  local model's atomic operations do not test partitions or partial disk writes.

M5 remains a single-master release target. **HA is not delivered before M6**;
the three-master architecture diagrams show the later target.

## Building it

Step-by-step, with worked code: **[docs/guide/](guide/)**. Chapters 5–8 cover
M1–M2 in full.

## Week 1

1. The `Backend` trait in `mammoth-core` — the whole architecture hangs off it.
2. `LocalBackend` faking six workers as six subdirectories on disk.
3. `mammoth ls` and `mammoth put` against it.
4. `mammoth viz blocks` — because seeing your fake blocks land is the moment
   this stops feeling like homework.
