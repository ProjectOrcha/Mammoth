---
title: Performance
description: Implemented memory reuse, parallel compute and durable streaming, with measurable limits.
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


## Implemented in the local engine

Mammoth's current `local-memory-parallel-v3` engine targets avoidable work in
storage and local compute. [Measured Mac results](/ops/benchmarks/) compare
the current engine with Hadoop HDFS for storage and Spark on HDFS for compute.
These small same-host workloads have different execution paths and memory policies;
they do not establish distributed performance. Current Linux scores remain pending.

| Path | Implemented behavior |
| --- | --- |
| Metadata | Indexed SQLite WAL transactions, prepared statements and concurrent readers; one metadata writer |
| Upload | Bounded streaming buffers and concurrent replica writes; all requested replicas are durable before publication |
| Read | Verified 64 KiB chunks, direct range/suffix reads, and a bounded memory cache for repeated reads |
| Sort | Parallel sorting of packed batches; small jobs stay in memory, larger jobs spill sorted runs |
| Word count | Parallel partition-local aggregation with borrowed keys and sorted merge of counts |
| Larger jobs | Checksummed temporary runs, bounded merge fan-in and streamed durable output; no total input-size cap |

The cache defaults to 256 MiB. Fresh metadata lookups select the current file
generation; cache hits reuse verified bytes from immutable block IDs. Overwrites
cannot select a stale generation. Health and repair inspect storage even when
reads can use a cached copy. Cache counters are visible in Overview.

Compute defaults to a 128 MiB working-set target per job. Packed batches reserve
room for sorting and aggregation; larger jobs spill to disk. A shared worker pool
uses up to 32 CPU threads. Individual lines are limited to the smaller of 16 MiB
and one eighth of the target. Cache capacity and job targets are separate from
storage buffers, allocator overhead and total process memory. See
[configuration](/ops/configuration/) to adjust memory and the spill directory.

Jobs report memory/spill mode, input/output records and bytes, worker count,
spill bytes and merge passes. Temporary files are cleaned on normal completion,
errors and cancellation. Final output uses the same durable storage writer.
The namespace format remains version 2, so this memory update needs no new
storage-format migration after the WAL update.

## Measure the actual workload

The [benchmark suite](/ops/benchmarks/) measures writes, separate fresh-cache and
repeated reads, metadata, sort and word count. Every result is verified and all
measured iterations are retained. The CLI and dashboard use the same harness.
OS caches remain enabled; even fresh Mammoth-cache reads are not cold-disk tests.
Compute uses a documented finite-key text dataset, not TeraGen's records.

```bash
python3 bench-suite/run_linux.py --output /path/on/test-disk/results \
  --read-cache 256MiB --compute-memory 32MiB
```

The collector builds release mode and records source hashes, hardware, filesystem,
cache policy and raw reports. It refuses macOS. Measure memory-fitting and spilling
inputs separately; repeat with the cache disabled to isolate its effect. Never
compare a warm cached read against a competitor's cold storage read.

## Distributed design remains future work

The [four fast paths](/concepts/fast-paths/) describe proposed distributed mechanisms.
Physical worker networking, Raft, distributed shuffle, lineage recovery, hedged
reads, io_uring and sendfile are not implemented by this local engine. There is
no claim that a language change alone beats Hadoop or Spark. A valid comparison
requires equivalent workloads, semantics, durability and cache settings on the
same hardware, including real network traffic for distributed tests.
