# Implementation status on AI_coded

## Working application

| Area | Implemented behavior |
| --- | --- |
| Local filesystem | Persistent namespace; automatic parent creation on upload; list/stat/read/write/remove; mkdir; atomic rename; recursive copy; POSIX metadata; replica changes |
| Block storage | Immutable block directories; per-4-KiB CRC32C; file sync, atomic publication and directory sync on Unix; rack-aware rendezvous placement; corruption fallback; explicit repair and garbage collection |
| Concurrency | Indexed SQLite WAL metadata with one writer and concurrent readers; bounded streaming uploads, concurrent replicas and a verified-read memory cache; read snapshots retain one generation across overwrites |
| CLI | Filesystem commands, human/JSON/YAML/CSV output, terminal logo/help, local service status and graceful stop/shutdown, stable errors, health checks, configuration validation, completion scripts, basic HDFS command translation |
| Visualization | Block matrix, capacity tables, topology, skew, namespace sizes, replication health and an interactive `top` dashboard |
| Dashboard | Embedded production assets, live filesystem and node views, upload/download, folder creation, filtering, rename/move, properties, recursive deletion, paginated layouts, replica repair, local job submission, SSE refresh and errors |
| HTTP client | Backend implementation over the versioned filesystem API; encoded paths and streamed request/response bodies |
| S3 subset | Bucket/object CRUD, copy, list v1/v2, prefix/delimiter/pagination, HEAD, ranges, ETags, conditional GET and upload checksum verification |
| Benchmarks | Verified local I/O with separate cache phases, metadata create/stat/rename/delete, sort and word count; concurrency, repeated runs, p50/p95/p99, replica comparisons, persisted JSON and dashboard results |
| Configuration UI | Active storage, read cache, compute memory/spill and listener settings; validated TOML drafts and download; restart required |
| Compute | Parallel UTF-8 word count and line sort; in-memory batches, checksummed spill runs and bounded merges; no total input-size cap |
| Migration | Local file/tree import/export with staged downloads; no symlink traversal |
| Tooling | UI build, generated CLI docs, Docker local-service configuration, CI and draft-release workflow |

## Exact boundaries

The full Mammoth project is **not complete**. The guide's runnable local
M1–M3 application and a durable block-storage foundation are implemented. Passing
these tests is not evidence that the production distributed milestones are done.

- **M4:** production throughput and crash/power-loss qualification are still open.
  Normal I/O now streams without temporary files; indexed SQLite WAL replaces full
  JSON namespace rewrites. Cluster health still verifies every block; this is
  unsuitable for a very large deployment. See [engine details](STORAGE-ENGINE.md).
- **M5:** separate master/worker services, gRPC, direct worker streaming,
  heartbeats, versioned topology, worker-enforced leases and automatic repair are
  not implemented. `serve --role master|worker` reports this explicitly. The
  HTTP client is a gateway client; it does not discover leaders. The S3 subset
  has no multipart uploads, AWS chunked uploads, IAM, versioning, ACLs or encryption.
  S3 range and suffix reads use the captured generation without scanning its prefix.
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
  activity.lock
  ns/namespace.json         # format marker
  ns/namespace.sqlite3     # metadata; also retain WAL/SHM files in backups
  ns/namespace.legacy.json # migration archive when upgrading
  workers/w1/blocks/<shard>/<shard>/blk_<id>/data
  workers/w1/blocks/<shard>/<shard>/blk_<id>/meta.json
  workers/w1/tmp/
  ... w2 through w6
```

The indexed namespace stores file metadata, MD5 entity tags, inline bytes and block
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

### Benchmark and configuration verification (2026-09-16)

The release benchmark suite completed three measured rounds and one excluded
warmup for each of 1 and 3 replicas: eight 16 MiB files, four concurrent clients,
and 1,000 files in each metadata phase. Every read passed byte-count and CRC32C
verification. All temporary stores were cleaned, all 36 measured phase records
were retained, and the 12 summaries were checked against their raw values.
These are historical Mac results for the retired JSON engine; they are excluded
from current performance claims. See [the archived results](BENCHMARKS-LEGACY.md).

Validated 24 tests across the benchmark, CLI and gateway packages, 48 dashboard
tests, strict Clippy for the affected Rust packages, formatting, dashboard type
checking, release builds, and documentation builds at both `/` and `/Mammoth/`.
Browser verification covered persisted results, configuration validation, and a
small suite submitted through the dashboard in an isolated test store. The live
service was updated; its filesystem namespace was unchanged across the restart.


### Storage engine update (2026-09-16)

`local-wal-stream-v2` replaces full JSON namespace rewrites with indexed SQLite
WAL transactions, removes request/read staging, writes replicas concurrently, and
streams verified ranges including S3 suffix requests. Migration retains a legacy
archive and fences old binaries. See [upgrade details](STORAGE-ENGINE.md).

Validation: 63 Rust tests and 48 dashboard tests passed; strict workspace Clippy,
Rust 1.85 compatibility, dependency policy, formatting, dashboard checks and both
production builds passed. Real-client checks passed with Boto3 1.43.95 and
DuckDB 1.5.5 (1,000 rows, sum 499,500), including graceful SSE shutdown.
Tests cover separate processes, abrupt exit with an
open WAL, cancelled streams, generation retention and late checksum failures.
The existing local store was backed up, migration was checked on a copy, and the
service was restarted. All three file hashes and five namespace entries were
preserved; SQLite's integrity check passed. Browser checks confirmed the updated
homepage and the dashboard's separation of historical reports.

No new Mac performance measurements were published. The Linux collector and
manual Ubuntu workflow are ready; neither has produced a Linux result in this
session. Linux/Windows runtime checks and physical power-loss testing remain open.


### Memory and parallel compute update (2026-09-16)

`local-memory-parallel-v3` adds a bounded verified-read cache, parallel packed
sort/word-count batches, checksummed spill runs with record-count validation,
bounded multi-pass merges and streamed durable output. The former 64 MiB total
input cap is removed; individual line and working-set limits remain documented.
The namespace format stays at version 2. See [memory behavior and limits](MEMORY-ENGINE.md).

CLI and dashboard benchmarks now have nine phases, including distinct empty-cache
and repeated-cache reads plus exactly validated local sort and word count. Reports
retain cache/spill evidence and identify their engine. Configuration exposes read
cache capacity, per-job memory target and spill directory. Overview shows cache
counters; Jobs shows actual memory/spill metrics. Historical reports retain their
original meaning and are not selected as current-engine results by default.
Homepage and docs exclude Mac figures from competitive claims; Linux results
and matching Hadoop/Spark baselines remain pending.

Validation: 72 Rust tests and 52 dashboard tests passed. Formatting, strict
workspace Clippy, Rust 1.85 compatibility, dependency policy and release/UI/docs
builds passed, including root and `/Mammoth/` documentation paths. The new cache
uses the linked-map library already used by metadata instead of adding the older
LRU dependency. Failure tests cover damaged/truncated spill records (including
record-boundary truncation), cancellation cleanup and preserving old output.
A generated-input regression verifies streaming beyond 64 MiB without collecting
performance scores. Isolated HTTP checks verified exact sort and word-count
outputs in spill mode, cache reuse and scratch cleanup. Browser checks covered
configuration validation, benchmark memory controls, job metrics and the homepage.

The running local service was backed up and updated; all three existing file
hashes match the pre-update values. All source changes remain on `AI_coded`.
Physical multi-host execution, distributed shuffle and proof of a Hadoop/Spark
win are still unimplemented/unmeasured.


### Same-Mac competitor measurements (2026-09-16)

The [current Mac snapshot](BENCHMARKS.md) includes verified Mammoth, Hadoop HDFS
3.5.0 and Spark 4.2.0 runs. The new isolated runner downloads official pinned
runtimes with checksum verification, validates output and replica placement,
retains all measured iterations, and stops its test services. The website,
dashboard and docs share a generated snapshot from the raw evidence. Earlier
Mac exclusion notes above describe prior checkpoints, not the current policy.

These 64 MiB local workloads use different execution paths and memory budgets:
Mammoth direct calls, HDFS loopback RPC, and Spark local mode on HDFS. OS caches
are retained. They establish no physical-network or distributed winner.


### Local preview hardening and manual-track handoff (2026-09-16)

Jobs now publish through atomic create-if-absent unless overwrite is explicitly
authorized. A destination created during computation is preserved; deterministic
race tests cover independent backend handles. The HTTP client exposes the same
contract. Browser-origin and hostname protections cover both listeners, response
headers restrict framing and cross-origin resource use, and `/readyz` checks the
namespace root. These guards do not provide authentication.

Local startup and configuration validation reject unimplemented TLS/authentication
settings. Defaults bind to loopback and exported templates contain supported
settings. The CLI job reference, API docs, homepage and operating/configuration
pages reflect the implemented behavior. The nonfunctional Helm chart was removed.
Compose remains one service, with a read-only container root and a writable data
volume. The new runbook describes full-store offline backup/restore, upgrade and
resource limits; release gates list outstanding production blockers.

The publisher validates evidence even when Python optimization is enabled. CI
checks snapshot drift, locked dependencies and all teaching examples. Native and
CI packaging share a checksummed archive with an embedded dashboard, build record,
supported config, licenses and operating notes. The UI test dependency was updated
to Vitest 4.1.11 after a dependency scan found an advisory in the older version.
Both branches now report zero npm audit findings for their dashboard dependencies.

Local validation passed: 77 Rust workspace tests plus two teaching-example tests,
52 dashboard tests, strict Clippy, formatting, Rust 1.85 compatibility, dependency
policy checks, six publisher tests (also with Python `-O`), generated CLI docs,
and website builds at root and `/Mammoth/`. The teaching-model simulation passed
10,000 seeds; this is not distributed storage failure qualification.

A native Mac archive was built and its archive/binary hashes verified. Tests of
the extracted binary covered configuration, readiness, multi-block byte equality,
atomic-create conflict, foreign browser rejection, explicit job overwrite,
graceful shutdown and an offline restore. Real Boto3 1.43.95 and DuckDB 1.5.5
checks passed, including 1,000 rows with sum 499,500 and SSE shutdown. The existing
local store was backed up and restored to a separate verification directory before
restart; all three file hashes and five namespace entries were preserved. Browser
configuration validation passed through both the packaged service and Vite proxy.

`main` is updated separately with branch/build guides, a readiness chapter, a
small atomic-publication teaching example, locked CI checks, the same test-tool
security update and an explicit unsupported-packaging error. Its manual scaffold
remains intact. Validation there passed 19 Rust workspace tests, two example tests,
24 dashboard tests, strict Clippy, Rust 1.85 and the dashboard build.

No new performance scores are claimed for these later changes. The saved Mac
Mammoth/HDFS/Spark comparison remains visible with its measured source record.
Docker Compose syntax was checked, but the Docker daemon was unavailable; container
runtime, Linux/Windows execution and physical power-loss/multi-host tests remain
open. Nothing has been tagged, published or deployed remotely in this session.
