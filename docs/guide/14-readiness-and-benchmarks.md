# 14 · From a working demo to a reliable local service

This chapter belongs to the manual `main` track. It defines work your team must
implement and verify after chapters 5–9; it does not claim that those features
already exist here. Keep the [branch contract](BRANCHES.md) in view.

## Agree on what “ready” means

A reliable local service is a useful milestone. A production distributed system
also needs real worker processes, network failure handling, authentication,
authorization, transport security, quotas, durable recovery and operational
qualification. A successful UI demo or fast laptop benchmark proves none of those.

| Owner | Next handoff | Acceptance evidence |
| --- | --- | --- |
| A · Storage | Durable publication and consistent reads | Failed/cancelled uploads preserve old bytes; concurrent creates have one winner; reopened data matches checksums; GC respects active reads |
| B · CLI | Predictable administration and outputs | JSON stdout stays parseable; failure exits nonzero; overwrite requires intent; status/stop target the correct service instance |
| C · Dashboard | Honest live behavior | Demo is labelled; unavailable metrics remain absent; errors and reconnects are visible; stale responses cannot replace a newer view |
| D · Integration | Safe listeners and reproducible release checks | Loopback defaults, rejected cross-origin mutations, explicit unsupported settings, readiness probe, clean builds and backup/restore drill |

All four review shared types and wire contracts. Do not let one person become
the only tester. Continue the reviewer ring in [the team plan](TEAM-PLAN.md).

## Prevent the check-then-write race

A job can check that `/output` is absent, spend time computing, then overwrite
someone else's newly created file. A second check immediately before a normal
write still leaves a race. The absence check and namespace publication must be
one atomic operation.

Run the isolated example:

```bash
cargo run -p mammoth-parts --example 17-atomic-publication
cargo test -p mammoth-parts --example 17-atomic-publication
```

The example holds one mutex during check-and-insert. Two simultaneous creators
produce exactly one success. A captured read keeps an immutable `Arc` containing
the original bytes even after the path is replaced. This model has no durability
or networking. In your storage implementation, use an atomic namespace
transaction and keep referenced blocks alive until readers finish. Do not hold a
namespace write lock while a client slowly uploads or a job computes.

Write a deterministic regression using a barrier or channel: pause an upload,
create the destination from a second client, release the upload, then assert the
second client's bytes survive. Avoid timing sleeps that only occasionally hit
the race. Separately test explicitly authorized replacement.

## Bound work and describe what the settings do

Validate sizes, replica counts and memory targets before allocation. Stream
uploads and reads; limit concurrent uploads/jobs. A compute memory target is not
a process RSS limit: indexes, caches, buffers, worker stacks and the allocator
consume memory too. Exercise both memory-fitting and spilling inputs. Preserve
output on invalid UTF-8, disk errors, checksum failures and cancellation.

A `[security] auth = "token"` setting must not silently start an unauthenticated
server. Until implemented, reject it. Defaults should bind to loopback. A flag
allowing a remote listener is not authentication. Browser-origin protections
are also not authentication; native clients with network access still reach the
service. Do not publish real credentials or private test data in fixtures.

## Backups and recovery

Start with an offline backup procedure: stop the service and all CLI writers,
copy the complete store to a new directory, preserve the old copy, open the copy
with the same binary and verify known file hashes. With SQLite WAL, the WAL can
contain committed data that is not in the main database. A database-file-only
copy can lose that data. See [SQLite WAL](https://sqlite.org/wal.html).

Crash tests and process restarts are useful, but are not physical power-loss
tests. List which platforms, filesystems and failure modes you actually tested.
Do not advertise high availability until separate machines survive failures.

## Choose benchmarks that test the implemented system

Use a DFSIO-inspired concurrent read/write workload and create/stat/rename/delete
load for a storage engine. Add sort and word count when local compute exists.
Distributed TeraSort requires real partitioning, shuffle, global ordering and
validation; a local text sort is not TeraSort.

For each run record:

- Exact source, binary/build mode, runtime versions, OS, CPU, RAM and filesystem.
- Dataset bytes and record distribution, file count, clients, blocks and replicas.
- Warmups and every measured iteration, including failures; do not select only the fastest run.
- Timing boundaries, output/checksum verification and cleanup outcome.
- Cache state, memory policy, spill behavior and whether traffic crosses real NICs.

Aggregate throughput is logical bytes divided by phase wall time. Metadata rate
is completed calls divided by phase wall time. Keep raw samples and show the
median plus range. Distinguish per-operation latency percentiles from throughput.
Do not confuse a warm memory read with cold SSD bandwidth.

Compare storage with HDFS and compute with Spark using the same data and timing
boundaries. Disclose direct local calls versus RPC, different memory limits and
output layouts. A same-Mac result is a local baseline; it cannot establish a
multi-machine winner. The reference branch's existing reports describe its exact
measured build, not your manual implementation.

## Release gate

From a clean checkout, run formatting, strict Clippy, workspace tests, minimum
Rust checks, all teaching examples, frontend checks/tests/builds and generated-doc
drift checks. Check dependencies, package the actual embedded UI, verify archive
contents and hashes, then rehearse startup, readiness, shutdown and restoration
using disposable data. Keep benchmark publication checks in CI so copied tables
cannot drift from their evidence. Record missing gates rather than calling a
partially implemented system production-ready.

Back to [the guide](README.md) · [checklists](CHECKLISTS.md).
