# Implementation status on AI_coded

## Working application

| Area | Implemented behavior |
| --- | --- |
| Local filesystem | Persistent namespace; automatic parent creation on upload; list/stat/read/write/remove; mkdir; atomic rename; recursive copy; POSIX metadata; replica changes |
| Block storage | Immutable block directories; per-4-KiB CRC32C; file sync, atomic publication and directory sync on Unix; rack-aware rendezvous placement; corruption fallback; explicit repair and garbage collection |
| Concurrency | OS file lock serializes processes sharing a store; interrupted streams preserve committed files; read snapshots retain one generation across overwrites |
| CLI | Filesystem commands, human/JSON/YAML/CSV output, terminal logo/help, local service status and graceful stop/shutdown, stable errors, health checks, configuration validation, completion scripts, basic HDFS command translation |
| Visualization | Block matrix, capacity tables, topology, skew, namespace sizes, replication health and an interactive `top` dashboard |
| Dashboard | Embedded production assets, live filesystem and node views, upload/download, folder creation, filtering, rename/move, properties, recursive deletion, paginated layouts, replica repair, local job submission, SSE refresh and errors |
| HTTP client | Backend implementation over the versioned filesystem API; encoded paths and streamed request/response bodies |
| S3 subset | Bucket/object CRUD, copy, list v1/v2, prefix/delimiter/pagination, HEAD, ranges, ETags, conditional GET and upload checksum verification |
| Compute | Local UTF-8 word count and line sort, up to 64 MiB input |
| Migration | Local file/tree import/export with staged downloads; no symlink traversal |
| Tooling | UI build, generated CLI docs, Docker local-service configuration, CI and draft-release workflow |

## Exact boundaries

The full Mammoth project is **not complete**. The guide's runnable local
M1–M3 application and a durable block-storage foundation are implemented. Passing
these tests is not evidence that the production distributed milestones are done.

- **M4:** production throughput and crash/power-loss qualification are still open.
  Input and read snapshots use temporary local files, adding disk I/O. A namespace
  transaction uses one JSON snapshot and one global process lock. Cluster health
  currently verifies every block; this is unsuitable for a very large deployment.
- **M5:** separate master/worker services, gRPC, direct worker streaming,
  heartbeats, versioned topology, worker-enforced leases and automatic repair are
  not implemented. `serve --role master|worker` reports this explicitly. The
  HTTP client is a gateway client; it does not discover leaders. The S3 subset
  has no multipart uploads, AWS chunked uploads, IAM, versioning, ACLs or encryption.
  S3 range reads currently capture the whole file before selecting the response range.
- **M6:** no Raft log, quorum election, fencing, snapshots or distributed HA.
  The GFS model's master takeover remains an independent teaching simulation.
- **M7:** no distributed DAG scheduler, worker task isolation, shuffle or TeraSort
  benchmark. Dashboard-submitted local text jobs have a bounded session history (100 jobs, two concurrent). Restarting clears this history; CLI jobs are not tracked in it. Output files remain persistent.
- **M8:** no native HDFS import/cutover, resumable transfer journal or erasure coding.
- **Operations:** capacities are simulated reference values for worker directories
  on one physical disk, not independently available disk capacity or enforced quotas.
  Authentication/TLS configuration describes future capabilities. This development
  service runs without authentication; network exposure requires `--allow-remote`.
  Ownership/modes are descriptive, not enforced ACLs.
- **Deployment:** Docker is a single-container development layout. Multi-node Helm
  installation remains a future deployment target. No GitHub Pages deployment,
  release publication, remote push or cloud infrastructure change is performed
  by this implementation.

No software change can establish that every possible execution is error-free.
The commands below reproduce the checks; the remaining gates above require their
own implementation and failure testing.

## Local on-disk format

```
<root>/
  store.lock
  ns/namespace.json
  staging/
  workers/w1/blocks/<shard>/<shard>/blk_<id>/data
  workers/w1/blocks/<shard>/<shard>/blk_<id>/meta.json
  workers/w1/tmp/
  ... w2 through w6
```

The namespace stores file metadata, MD5 entity tags, inline bytes and block
placements. Each block header records layout version, length and CRC32C values.
Data becomes durable before its namespace entry is published. Namespace changes
publish atomically; unreferenced replicas may remain after interruption and are
reclaimed by `mammoth admin gc`. Directory syncing is platform dependent; power
loss semantics have not been tested on Windows.

The guide's `.mmeta` files are an earlier teaching layout. The implemented format
separates names from host filenames and accepts paths ending in `.mmeta`, spaces,
Unicode, `#`, `?` and `%` without internal metadata collisions.

## Verification commands

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --locked
MAMMOTH_SIM_SEED=1 MAMMOTH_SIM_COUNT=10000 cargo test --locked -p mammoth-testkit --test sim
cargo +1.85.0 check --workspace --all-features --locked
npm --prefix ui run check
npm --prefix ui test
npm --prefix ui run build
npm --prefix web run build
cargo xtask docs
uv run --no-project --with boto3 --with duckdb python tests/compat/s3_clients.py
```

The dependency policy records one maintenance exception: Ratatui 0.28's
build-time `paste` macro. The upstream version that removes it requires a newer
Rust compiler than the guide's 1.85 baseline. The advisory reports maintenance
status, not a known vulnerability. Revisit it when raising the minimum Rust version.

The real-client test uses its own temporary directory, starts/stops its own service,
verifies MD5 ETags and exact bytes, and queries 1,000 Parquet rows with DuckDB.

## Validation recorded on this machine

Validated on macOS ARM64 on 2026-09-15:

- 38 Rust tests passed across the workspace; 33 dashboard tests passed.
- Formatting, strict Clippy, Rust 1.85 compatibility and dependency policy checks passed.
- 10,000 deterministic GFS teaching-model scenarios passed (seeds 1–10,000).
- Dashboard type checking and production build passed; browser checks covered live
  file listing, folder creation, upload, file details and distribution rendering.
- Boto3 1.43.94 passed exact-byte, ETag, listing and range checks. DuckDB 1.5.5
  queried 1,000 Parquet rows through the S3 endpoint (sum 499,500).
- The service shut down cleanly with an SSE connection still open.
- Documentation built at both `/` and `/Mammoth/`. Mermaid's bundle-size advisory
  remains a non-fatal build warning.
- The native release archive was built locally. Docker Compose configuration
  validated; container execution was not tested because the Docker engine was
  stopped. Linux/Windows CI and hosted release workflows have not run locally.

### Web repair verification

The dashboard repair pass additionally checks reactive chart updates, zero-byte
files without read metrics, access to all block-matrix rows, name filtering before
pagination, rename errors, recursive-delete selection, job completion and failure,
and protection of existing job outputs. Browser checks covered the six routes at
390 px, both themes, Unicode folder creation, file rename/properties, word-count
submission and its output link. Documentation search and diagrams were checked;
1,578 internal links passed the rebuilt site audit. The release service was
restarted at port 8080 and the existing six file paths, lengths and checksums were
unchanged across the restart.

### Branch UI comparison (2026-09-16)

Compared the `main` and `AI_coded` dashboards and public homepages. The Workspace
selector now exposes the full example dashboard without rebuilding, including
historical charts, the six distribution views, distributed job stages, Raft and
warm start. Real local storage has rack cards, four expandable lifecycle cards,
replica-byte diagrams, thirty minutes of in-tab snapshots, file properties and
bounded text previews. Local jobs report their measured single-task execution.

Validated 41 dashboard tests, six gateway integration tests, strict gateway
Clippy, type checking, and both production web builds. Browser checks covered all
six routes in both workspaces at 390 px, desktop charts, workspace switching,
historical replay, text preview, job input prefill, word-count completion and
output preview, replica repair, and both themes. Distributed metrics remain
example data; local snapshots are browser memory and clear on reload.
