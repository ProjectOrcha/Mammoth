<p align="center">
  <img src="assets/logo/mammoth-logo.svg" alt="Mammoth" width="360">
</p>

<h1 align="center">Mammoth</h1>

<p align="center">
  <strong>A Hadoop-class distributed storage engine in Rust.</strong><br>
  The elephant, but faster, and without the JVM.
</p>

<p align="center">
  <a href="https://github.com/ProjectOrcha/Mammoth/actions/workflows/ci.yml"><img alt="CI" src="https://img.shields.io/github/actions/workflow/status/ProjectOrcha/Mammoth/ci.yml?branch=main&label=ci"></a>
  <a href="#licence"><img alt="Licence" src="https://img.shields.io/badge/licence-Apache--2.0%20OR%20MIT-blue"></a>
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.82%2B-orange">
  <img alt="Status" src="https://img.shields.io/badge/status-pre--release-yellow">
</p>

---

> [!WARNING]
> **Mammoth is pre-release and does not work yet.** This repository is a
> scaffold: the architecture, the command surface and the documentation are
> real, most of the implementation is not. See the [roadmap](docs/ROADMAP.md)
> for what exists and what does not. The first usable release is **M5**.

## What it is

You have a 10 TB file. No single machine has 10 TB of fast disk, and reading it
at 200 MB/s would take 14 hours. So you chop it into pieces, put the pieces on
100 machines, and read all 100 at once. Now it takes 8 minutes.

That is the whole idea. Everything else is bookkeeping — an index of where each
piece went, redundant copies so machine death is survivable, and a scheduler to
run code next to the data instead of shipping the data to the code.

HDFS does that job. Mammoth does it as **one binary** with **one TOML file**,
with no garbage collector pauses, no ZooKeeper, no JournalNodes, no XML, and an
**S3 API** so the tools you already use work unchanged.

New to any of this? Read
**[Hadoop architecture in 10 minutes](web/src/content/docs/intro/hadoop-primer.md)** first.

## Why bother

| Hadoop's problem | What it costs you | Mammoth's answer |
| --- | --- | --- |
| JVM garbage collection | a 200 GB-heap NameNode pauses for *seconds* | no GC; Rust |
| One global namespace lock | one slow `listStatus` blocks thousands of clients | immutable namespace behind `ArcSwap` — readers never block |
| Metadata in RAM only | namespace capped by one machine's RAM | Raft-backed metadata store |
| The small-file problem | 1M tiny files kill a cluster | files under 1 MiB skip the block layer entirely |
| 6+ XML files, 1000+ properties | nobody knows what is actually set | one `mammoth.toml`, env-overridable |
| ZooKeeper + JournalNodes + ZKFC | three extra distributed systems to fail over one process | Raft, built in |
| Full block reports | multi-second metadata stalls | rolling `xxhash3` digests, full report only on mismatch |
| Four CLI scripts, Hadoop verbs | `hdfs dfs -ls /data` | `mammoth ls /data` |

Full vocabulary mapping in
[What is Mammoth?](web/src/content/docs/intro/what.md).

## Try it

```bash
git clone https://github.com/ProjectOrcha/Mammoth
cd Mammoth
cargo build --release -p mammoth-cli
./target/release/mammoth quickstart
```

```console
$ mammoth quickstart

  Mammoth v0.1.0

  ✔ config written        ~/.mammoth/mammoth.toml
  ✔ started master        127.0.0.1:7000
  ✔ started 3 workers     w1 w2 w3  (simulated, single machine)
  ✔ started gateway       S3 :9000 · UI :8080
  ✔ sample data loaded    /sample/nyc-taxi.parquet (120 MB)

  Web UI  →  http://localhost:8080
```

At `v0.1.0` this becomes a 30-second install:

```bash
curl -fsSL https://projectorcha.github.io/Mammoth/install.sh | sh
cargo install mammoth-cli --locked
brew install ProjectOrcha/tap/mammoth
docker run -p 8080:8080 -p 9000:9000 ghcr.io/projectorcha/mammoth quickstart
```

## You can see your data

Hadoop's web UI shows you tables of numbers. Mammoth shows you where your data
actually is — from the terminal, over SSH, with no browser.

```console
$ mammoth viz blocks /data/sales-2026.csv

  /data/sales-2026.csv   350 MB · 3 blocks · replication 3

           w1    w2    w3    w4    w5    w6
  blk 1    ●     ●     ●     ·     ·     ·
  blk 2    ·     ●     ●     ●     ·     ·
  blk 3    ●     ·     ●     ·     ●     ·

  ● primary   ◐ replica   ✕ corrupt   · absent

  racks:   w1 w2 ∈ rack-a    w3 w4 ∈ rack-b    w5 w6 ∈ rack-c
  ⚠ blk 1 has all 3 replicas in racks a,b — rack-c unused
    this file survives a rack failure, but placement is unbalanced
    fix: mammoth admin balancer start --scope /data/sales-2026.csv
```

```console
$ mammoth viz skew /warehouse/events

  PARTITION SIZE DISTRIBUTION           1,024 files · 4.2 TB

    dt=2026-08-01  ██                    1.2 GB
    dt=2026-08-03  ████████████████████ 89.0 GB  ⚠ 68× median
    dt=2026-08-04  ██                    1.1 GB
    ... 1,019 more

  ⚠ severe skew — one task will process 89 GB while 1,023 process ~1 GB.
    your job's runtime is set by that one task.
```

Also `viz cluster`, `viz topology`, `viz treemap`, `viz health --live`,
`viz flow`, and `mammoth top` — a live TUI dashboard that works over SSH.
Every one of them has a `--json` form.

Full gallery: [Data distribution visualization](web/src/content/docs/concepts/visualization.md).

## It speaks S3

The single most important decision in the project. The moment the gateway
implements the S3 API, every tool in the modern data ecosystem works against
your cluster on day one — Spark, DuckDB, Polars, Trino, ClickHouse, pandas,
Iceberg, Delta Lake — with a one-line config change and zero integration work.

```python
import duckdb
duckdb.sql("SET s3_endpoint='localhost:9000'")
duckdb.sql("SELECT count(*) FROM 's3://warehouse/sales/*.parquet'")
# ↑ this runs against your cluster
```

## Errors that teach

Never a stack trace. What broke, why, and the next command to run.

```console
$ mammoth put ./big.bin /data/big.bin

  error[E0301]: not enough healthy workers for replication 3

    only 2 workers are available, but this file requires 3 replicas

  what you can do:
    · lower replication:   mammoth put ./big.bin /data/big.bin --replication 2
    · check node health:   mammoth node list
    · why is a node down:  mammoth doctor --node w3

  docs: https://projectorcha.github.io/Mammoth/errors/E0301
```

## Architecture

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

ONE binary.  mammoth serve --role master|worker|gateway|all
```

Everything above hides behind one trait, so the CLI and the UI never learn
whether they are talking to a simulation or a real cluster:

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

`LocalBackend` fakes six workers as six directories on one disk.
`ClusterBackend` talks to real masters over gRPC. Same trait, same callers.
Why: [ADR 0002](docs/adr/0002-backend-trait.md).

## Repository layout

```
mammoth/
├── crates/            the Rust workspace
│   ├── mammoth-core/       ★ traits, types, errors, config   ← start here
│   ├── mammoth-cli/        ★ the `mammoth` binary
│   ├── mammoth-viz/        ★ terminal charts, heatmaps, TUI dashboard
│   ├── mammoth-local/      ★ LocalBackend — single-machine simulation
│   ├── mammoth-gateway/    ★ web server, REST/SSE, S3 API, embedded UI
│   ├── mammoth-proto/        protobuf + tonic build
│   ├── mammoth-rpc/          transport, connection pool, auth
│   ├── mammoth-storage/      block files, checksums, volumes, scrubber
│   ├── mammoth-meta/         inode tree, block map, leases, Raft state machine
│   ├── mammoth-master/       master role
│   ├── mammoth-worker/       worker role
│   ├── mammoth-client/       ClusterBackend + Rust SDK
│   ├── mammoth-scheduler/    queues, placement, locality
│   ├── mammoth-compute/      DAG engine, shuffle
│   ├── mammoth-migrate/      HDFS/S3 migration
│   └── mammoth-testkit/      cluster harness, fault injection
│
├── ui/                Svelte 5 + Vite admin GUI, embedded via rust-embed
├── web/               Astro Starlight site + docs → GitHub Pages
├── deploy/            Dockerfile · Compose · systemd · Helm
├── examples/          five numbered, runnable walkthroughs
├── tests/             e2e · deterministic sim · Hadoop compat
├── benches/           criterion micro-benchmarks
├── bench-suite/       full-cluster, publishable benchmarks
├── fuzz/              cargo-fuzz targets
├── xtask/             cargo xtask build-ui | docs | assets | dist
├── docs/adr/          architecture decision records
└── assets/logo/       canonical branding
```

★ = built first.

## Examples

| | |
| --- | --- |
| [01 · Hello Mammoth](examples/01-hello-mammoth/) | put a file, read it back, see it get inlined |
| [02 · See your blocks](examples/02-see-your-blocks/) | visualize where a 350 MB file landed |
| [03 · Kill a node](examples/03-kill-a-node/) | watch re-replication live |
| [04 · DuckDB over S3](examples/04-duckdb-over-s3/) | query the cluster from an unmodified tool |
| [05 · Word count](examples/05-wordcount/) | the DAG engine and the shuffle |

## Status

| Milestone | Weeks | You can demo |
| --- | --- | --- |
| **M1 · CLI + LocalBackend** | 1–4 | `put`, `get`, `ls`, `stat` on one machine |
| **M2 · Visualization + `top`** | 5–6 | `viz blocks`, `viz cluster`, `viz skew`, TUI |
| **M3 · Web UI** | 7–9 | full GUI, distribution page, live SSE |
| **M4 · Real block storage** | 10–13 | throughput near raw disk speed |
| **M5 · Distributed + S3 → v0.1** | 14–20 | kill a node, watch it heal; DuckDB queries it |
| M6 · HA (Raft) | 21–24 | kill the leader mid-write, it survives |
| M7 · Compute | 25–34 | TeraSort beats Hadoop MapReduce |
| M8 · Migration + EC | 35–42 | migrate a real HDFS cluster |

Ship at M5. Details in [docs/ROADMAP.md](docs/ROADMAP.md).

## Documentation

The site is built from `web/` and published to GitHub Pages.

- [What is Mammoth?](web/src/content/docs/intro/what.md)
- [Hadoop in 10 minutes](web/src/content/docs/intro/hadoop-primer.md)
- [5-minute cluster](web/src/content/docs/intro/quickstart.md)
- [Architecture](web/src/content/docs/concepts/architecture.md) · [Performance](web/src/content/docs/concepts/performance.md) · [Visualization](web/src/content/docs/concepts/visualization.md)
- [Data guide](web/src/content/docs/data/index.md) — block size, replication, formats, partitioning, skew
- [Configuration](web/src/content/docs/ops/configuration.md) · [Operations](web/src/content/docs/ops/index.md)
- [HTTP and S3 API](web/src/content/docs/api/index.md) · [Migration](web/src/content/docs/migration/index.md)

## Building it yourself

New to Rust, or to distributed systems? **[The Mammoth build guide](docs/guide/)**
takes you from an empty machine to a working filesystem with block
visualization, in twelve chapters, with every code block compiled and tested.

| | |
| --- | --- |
| [0 · Set up your machine](docs/guide/00-setup.md) | Rust, Git, Node, first build |
| [1 · The Rust you actually need](docs/guide/01-rust-you-need.md) | 30 minutes, not a course |
| [2 · Your first change](docs/guide/02-first-change.md) | a real command, end to end |
| [3 · How the team works together](docs/guide/03-team-workflow.md) | branches, reviews, who does what |
| [4 · Understanding the Backend trait](docs/guide/04-the-backend-trait.md) | the idea everything hangs off |
| [5](docs/guide/05-localbackend-part-1.md) · [6 · LocalBackend](docs/guide/06-localbackend-part-2.md) | blocks, replicas, rack-aware placement |
| [7 · Wiring up the CLI](docs/guide/07-wiring-the-cli.md) | `ls`, `put`, `cat`, `stat` |
| [8 · `viz blocks`](docs/guide/08-viz-blocks.md) | seeing where your data went |
| [9 · The web UI](docs/guide/09-web-ui.md) | REST API and embedded dashboard |
| [10 · GitHub Pages](docs/guide/10-github-pages.md) | publish the docs site |
| [11 · Where to go next](docs/guide/11-what-next.md) | M4 and beyond |

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). The short version:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
```

Architectural decisions get an ADR in [`docs/adr/`](docs/adr/), written *before*
the code. Anything involving more than one node gets a deterministic simulation
test, so the bug reproduces from a seed.

## Prior art worth studying

[Apache Ozone](https://ozone.apache.org/) (post-HDFS metadata) ·
[SeaweedFS](https://github.com/seaweedfs/seaweedfs) (small-file packing) ·
[JuiceFS](https://juicefs.com/) (pluggable metadata) ·
[MinIO](https://min.io/) (S3 surface, erasure coding) ·
[TigerBeetle](https://tigerbeetle.com/) (deterministic simulation testing) ·
[DataFusion](https://datafusion.apache.org/) (the query layer we adopt rather than rebuild)

## Licence

Dual-licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT licence ([LICENSE-MIT](LICENSE-MIT))

at your option. Contributions are dual-licensed on the same terms.
