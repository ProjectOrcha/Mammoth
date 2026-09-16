---
title: GFS reliability and project coverage
description: Run the local GFS model and distinguish tested behavior from planned distributed storage.
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


Mammoth now has a runnable **in-memory GFS teaching model**. The real filesystem,
worker services and master failover remain under development.

```bash
cargo run -p mammoth-local --example gfs-demo
cargo test -p mammoth-local --test gfs
```

Run these from a checkout of the repository. The example asserts its results.
Each heartbeat advances a logical clock, so a 90-second failure deadline runs
immediately without waiting 90 real seconds.

## What runs today

| Topic | Demonstration |
| --- | --- |
| File chunks and master index | Split a 20-byte file into 8, 8 and 4 bytes; reconstruct it from ordered chunk IDs |
| Direct worker reads | Locate chunks once, then read through cached worker locations even while the active master is down |
| Replication factor 3 | Store each chunk on distinct workers, favoring separate racks |
| Worker loss and heartbeats | Erase one worker's data; detect it after three missed 30-second heartbeats |
| Automatic repair | Copy surviving bytes to a spare worker; report and retry when destinations are insufficient |
| Concurrent clients | Stage several clients' bytes; the chunk primary chooses one application order for every replica |
| Master metadata copies | Copy metadata to each online master after successful model events |
| Standby takeover | External logical monitor switches the simulated endpoint and fences old authority with a new epoch |

The example ends with `DONEefgh01234567TAIL`. Tests check chunk boundaries,
all six orderings of three overlapping writers, failure deadlines, insufficient
replicas, data unavailability, stale worker return, lease expiry, takeover and
both masters unavailable.

## Follow the full guide

- [Chapter 13: source walkthrough and exercises](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/13-gfs-reliability.md)
- [Every video timestamp: evidence and production gap checklist](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/GFS-COVERAGE.md)
- [Executable model](https://github.com/ProjectOrcha/Mammoth/blob/main/crates/mammoth-local/src/gfs.rs)
- [Behavior tests](https://github.com/ProjectOrcha/Mammoth/blob/main/crates/mammoth-local/tests/gfs.rs)

These files are included in the checkout containing this update. The audit
compares the supplied [GFS video](https://www.youtube.com/watch?v=C3-FIM2xTIw)
with the actual source, rather than counting declarations as working features.

## Settings are design choices

The video uses 30-second heartbeats and three missed beats. Mammoth's existing
configuration declares a 3-second heartbeat and a 10-minute silence deadline;
the live detector has not been implemented. The model follows the video's
timing without changing those service defaults.

The model uses three replicas. The demo shrinks chunks to eight bytes for
readable output. Mammoth's planned default is 128 MiB. Local filesystem
allocation blocks, often a few KiB, are a different layer.

A visualization's `Primary` label is not a write lease. Sending data using
`disperse`, `mirror` or `pipeline` does not decide the order of concurrent
updates. See [architecture](/concepts/architecture/) and
[configuration](/ops/configuration/) for the planned service choices.

## Limits and next milestones

The simulation runs in one process and stores everything in memory. Each call
is atomic. Its failover controller can fence old authority immediately; real
systems need an independently enforced protocol. It changes no real DNS.

The model is not `LocalBackend`, a network storage service or a benchmark. A
single surviving master accepts model writes with reduced metadata redundancy.
No disk flush, process-wide restart, partial write, clock skew or network
partition guarantee follows from its tests.

| Milestone | Still required |
| --- | --- |
| M4 | Durable chunk files, checksums and crash-safe storage |
| M5 | RPC and streaming reads/writes, actual heartbeats and repair, defined mutation/retry semantics |
| M6 | Durable replicated metadata, Raft, former-leader fencing, leader discovery and failover tests |
| Validation | Distributed fault injection, backup restore and throughput/latency measurements under repair load |

The [original GFS paper](https://research.google/pubs/the-google-file-system/)
is the reference for its primary leases and relaxed consistency model. This
demo performs fixed-length chunk overwrites; it does not implement GFS record
append or exactly-once writes across network failures.

Bulk storage emphasizes aggregate **bandwidth**, while **latency** describes
individual request waits. The model illustrates correctness mechanisms; the
[performance goals](/concepts/performance/) still require measurements.
