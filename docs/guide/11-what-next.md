# Chapter 11 — Where to go next

**Time:** 15 minutes to read, then months of work if you want it.

---

## What you have

If you worked through chapters 0–10, the project now has:

- a filesystem that chops files into blocks and places replicas rack-aware
- small-file inlining, which is Mammoth's answer to the problem that kills real
  Hadoop clusters
- a CLI with POSIX verbs, `--json` on everything, and errors that teach
- `viz blocks` and `viz cluster` — the visualization nobody else in this space
  has
- a web dashboard served from inside the binary
- a live docs site that rebuilds itself

That is **milestones M1–M3** in the [roadmap](../ROADMAP.md), and it is a real
demo. Everything after this is the distributed system underneath.

## The honest assessment

What you built is a *simulation*. It has no network, no failure, no consensus,
and it runs on one machine. That is not a criticism — it was the plan, and it is
why you have something to show at week 9 instead of week 30.

But be clear about the gap, especially if you are showing this to anyone:

| You have | A real cluster needs |
| --- | --- |
| six directories | six machines, any of which can die mid-write |
| `place()` recomputed on demand | the same idea, done properly — rendezvous hashing over a versioned topology, with the exceptions memory-mapped on disk ([ch. 12](12-the-fast-paths.md)) |
| whole files buffered in memory | streaming, because files are terabytes |
| one process | Raft consensus across three masters |
| `std::fs::write` | erasure-coded fragments dispersed in parallel, checksummed on arrival, acked on a quorum ([ch. 12](12-the-fast-paths.md)) |
| nothing can fail | everything fails, constantly, in combinations you did not imagine |

## What to build next, in order

### 1 · Finish M2 properly

The cheapest valuable work, and it is all in code you already understand.

- `viz topology`, `viz treemap`, `viz skew` — all fed by `cluster_report` and
  `list`. `viz skew` is the most useful command in the whole tool for anyone
  debugging a slow job.
- `mammoth top` — the live TUI, built with
  [`ratatui`](https://ratatui.rs/). One screen, works over SSH.
- `mammoth doctor` — check ports, disk space, `ulimit -n`, clock skew. Every
  check you add is a support question you never answer.
- `--json` on the `viz` commands, and colour via `owo-colors` gated on
  `is_terminal()`.

### 2 · Real block storage (M4)

Replace `LocalBackend`'s block files with the real layout:

```
/data/mammoth/vol0/
├── VERSION                      {layout_version, cluster_id, volume_uuid}
├── blocks/ab/cd/
│   ├── blk_0000000000012345.data
│   └── blk_0000000000012345.meta    header + [u32 crc32c; n_chunks]
├── tmp/                         in-flight; fdatasync + rename() to commit
└── trash/                       GC'd after a retention window
```

Three things to get right:

- **CRC32C per 4 KB chunk**, verified on every read. Use the
  [`crc32c`](https://crates.io/crates/crc32c) crate — it uses the SSE4.2 / ARMv8
  instruction and runs at ~20 GB/s. A software CRC at ~400 MB/s becomes your
  bottleneck.
- **`fdatasync` then `rename()`** to commit a block. `rename` is atomic on
  POSIX, so a crash mid-write leaves a file in `tmp/`, never a corrupt block in
  `blocks/`.
- **Never touch `std::fs` on an async thread.** Use `tokio::task::spawn_blocking`.
  A blocking disk read on the runtime thread stalls every other task on that
  thread.

### 3 · Make it distributed (M5)

This is the big one, and it is where the project stops being a toy.

- `mammoth-proto` — the gRPC surface, with `tonic`. There is already a starter
  `.proto` in `crates/mammoth-proto/proto/`.
- `mammoth-master` — namespace, block map, lease management, safe mode.
- `mammoth-worker` — block serving, heartbeats every 3 s, Merkle roots.

  **Read [chapter 12](12-the-fast-paths.md) before you write either of these.**
  The read path, the write path, repair and startup are all cheaper to build the
  fast way than to build the HDFS way and then fix — and two of the four are
  *simpler*, not harder.
- `mammoth-client` — `ClusterBackend`, the second implementation of the trait.
  **When this compiles, your CLI and web UI work against a real cluster with no
  changes.** That is the payoff for chapter 4.
- The **S3 gateway**. Do not leave this late. The moment it exists, DuckDB,
  Spark, Polars, Trino and everything else work against your cluster with a
  one-line config change. It is the entire adoption story.

**Build `mammoth-testkit` at the same time, not afterwards.** Deterministic
simulation — every source of nondeterminism driven by a seeded PRNG — is what
makes distributed bugs reproducible. Retrofitting it is painful; having it means
every nightly failure reduces to a seed number. It is the practice that makes
TigerBeetle and FoundationDB trustworthy, and the
[nightly workflow](../../.github/workflows/nightly-sim.yml) is already written
and waiting for tests to run.

### 4 · Then decide

Ship at M5 and get feedback before building compute. A fast, easy,
S3-compatible distributed filesystem with the best data-distribution UI in the
category is already a real product. What people tell you at that point should
decide whether M7 is worth ten weeks.

## Skills to pick up along the way

| When you get to | Learn |
| --- | --- |
| M4 | `tokio::spawn_blocking`, `io_uring` basics, page cache behaviour |
| M5 | gRPC and `tonic`, `bytes::Bytes` and zero-copy, backpressure |
| M5 | property testing with `proptest`, simulation with `madsim` or `turmoil` |
| M6 | the [Raft paper](https://raft.github.io/raft.pdf) — read it twice, it is unusually readable |
| M7 | the Spark shuffle design, delay scheduling, cgroups v2 |

## Read the source of things that already work

The fastest way to get good at this is to read systems that solved these
problems already:

- **[SeaweedFS](https://github.com/seaweedfs/seaweedfs)** — small-file packing,
  done well. Directly relevant to Mammoth's inlining.
- **[MinIO](https://github.com/minio/minio)** — the S3 surface and erasure
  coding. Read their S3 handler when you build the gateway.
- **[TigerBeetle](https://github.com/tigerbeetle/tigerbeetle)** — deterministic
  simulation testing. Read their `vsr` module and their blog on it.
- **[JuiceFS](https://github.com/juicedata/juicefs)** — pluggable metadata.
  Useful when you decide what `mammoth-meta` should look like.
- **[Apache Ozone](https://ozone.apache.org/)** — what the HDFS team built after
  learning what HDFS got wrong. Their metadata design is the most direct prior
  art you have.

And the [DataFusion](https://datafusion.apache.org/) docs, when you get to the
query layer — you implement their `ObjectStore` trait and get SQL for free
rather than writing a MapReduce engine.

## Things that will bite you

Written down now, because everyone learns them the expensive way:

1. **Clock skew breaks leases.** If a worker's clock is 10 seconds ahead, it
   thinks a lease expired that has not. NTP is not optional, and `mammoth
   doctor` should check it.
2. **A slow disk is worse than a dead disk.** A dead node gets detected and
   routed around in seconds. A node whose p99 read is 340 ms just quietly makes
   everything slow. Track per-volume latency and demote automatically.
3. **Retry storms.** Every client retrying a failed master at the same instant
   makes recovery impossible. Exponential backoff with jitter, from day one.
4. **The full block report is a stall.** A worker with 10 million blocks
   reporting in can pause the master for seconds. Make the digest a shallow
   Merkle tree rather than one hash and the same structure also fixes startup —
   a matching root confirms millions of blocks in 32 bytes
   ([ch. 12 §4](12-the-fast-paths.md#4--warm-start)).

5. **Repair will take your cluster down if you let it.** An uncapped rebuild
   after a node failure saturates the network and turns a redundancy problem
   into an outage. Token-bucket it, make it yield to client traffic, and give an
   absent-but-not-confirmed-dead node a grace period before you copy a petabyte
   that turns out to be unnecessary.
6. **`unwrap()` in a server is a crash.** It is fine in tests. On a request
   path it is a denial of service that a user can trigger with a bad path.

## Keeping the project healthy

- **Write the ADR before the code.** [ADR 0001](../adr/0001-single-binary.md)
  and [0002](../adr/0002-backend-trait.md) show the format. Forcing yourself to
  justify a design in prose surfaces half the problems for free, and the "Bad"
  section is the part that earns its keep.
- **Keep `main` green.** Every merge, no exceptions.
- **Never cherry-pick a benchmark.** Publish the harness, the hardware, and the
  raw numbers. A public repeatable benchmark is the best marketing this project
  can have; a contested one is the worst.
- **Update the docs in the same PR as the code.** The CLI reference is
  generated and CI-checked, so that one cannot drift. Everything else is on you.

## Where to ask

- [Issues](https://github.com/ProjectOrcha/Mammoth/issues) for bugs — paste the
  full error and `mammoth doctor` output
- [Discussions](https://github.com/ProjectOrcha/Mammoth/discussions) for
  questions and design debates
- [CONTRIBUTING.md](../../CONTRIBUTING.md) for the conventions

---

**You have built a distributed filesystem's front half, and you can see your
data.** That is further than most people who start this get. The rest is
engineering, and now you know where it goes.

---

**Next:** [Chapter 12 — The four fast paths](12-the-fast-paths.md) — the design
for reads, writes, repair and startup, and the one idea all four rest on.

← [Back to the guide index](README.md)
