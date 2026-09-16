# Memory and parallel compute engine

`local-memory-parallel-v3` adds verified memory reuse and parallel local text
operators to the durable WAL/streaming engine. These changes are implemented on
`AI_coded`. They do not establish a performance win over Hadoop or Spark. [Verified Mac measurements](BENCHMARKS.md) now compare local storage with HDFS
and local compute with Spark on HDFS. Linux measurements remain pending.

## Repeated reads

A process-local LRU cache holds verified immutable block ranges as shared `Bytes`.
It defaults to 256 MiB and can be disabled with `read.cache_size = "0"`. Reads still
look up committed namespace metadata. An overwrite publishes new block IDs, so
another process changing the path cannot return stale data from this cache.
Active streams keep their original generation, as before.

Misses read and verify storage checksums before insertion; hits reuse those bytes
without another disk read or blocking-task handoff after the initial open. Inline
files reuse one verified buffer per stream but do not enter this block cache.
A cache entry includes a conservative 128-byte bookkeeping charge, with a separate
65,536-entry cap for tiny ranges. Payloads are at most 64 KiB. Returned buffers
can outlive eviction until the consumer releases them, so the cache budget is
not a process RSS limit. Exact ranges are keys: different range alignments may
miss even when they overlap previously read bytes.

A valid cached copy may still serve reads after an on-disk copy is damaged.
Health inspection and repair bypass the cache and continue checking the disk.
The Overview/API expose capacity, resident bytes, entries, hits, misses and
evictions. Counters and contents reset on process restart; independent CLI
invocations do not share a cache.

## Local sort and word count

The engine streams UTF-8 input into packed batches and uses a shared Rayon pool
(up to 32 CPU threads) to sort record references and combine partition-local word
counts. It avoids allocating a String for each token. Small jobs keep output in
memory. Larger jobs write sorted intermediate runs and use bounded fan-in merges;
word counts are combined again during merge. Output is streamed to the existing
durable storage writer. Data and metadata acknowledgement rules are unchanged.

```toml
[read]
cache_size = "256MiB"

[compute]
memory_budget = "128MiB"
spill_directory = ""
```

The working-set target is per job, between 64 KiB and 4 GiB. Packed batches charge
payload length plus 128 bytes per record and flush at a quarter of the target,
leaving room for parallel aggregation and sorting. Merge input buffers and head
records scale with the target; fan-in is capped at 32 open runs. The target
excludes fixed I/O buffers, storage block buffers, the read cache, thread stacks,
allocator overhead and run-path bookkeeping. It is not an enforced RSS ceiling.
Dashboard jobs admit at most two simultaneous jobs; other clients have independent
budgets. Select targets based on total concurrent work and available RAM.

There is no total input-size cap. Individual lines must fit within the smaller
of 16 MiB and one eighth of the target; oversized records fail with a clear error.
Sort preserves duplicate lines and emits newline-delimited output in UTF-8 byte
order, matching Rust string ordering. CRLF and final unterminated lines follow
Rust `str::lines` semantics. Word count uses Unicode whitespace boundaries and
emits lexically sorted `word<TAB>count` lines.

Spill files use a versioned binary header, bounded record lengths and CRC32C over
counts and payloads. They are disposable intermediates, so they are flushed but
not fsynced. Final output retains normal storage durability. Temporary directories
are guarded through asynchronous/blocking work and removed on completion, ordinary
failure or cancellation. Invalid UTF-8 or oversized input fails before output is
replaced. An output-stream failure also retains the prior committed generation.
Forced termination can leave scratch directories; remove `mammoth-compute-*` only
after confirming the owner job has stopped. A configured spill directory must
already exist and have room for intermediate merges plus output.

CLI job JSON and dashboard jobs report input/output bytes and records, memory or
spill mode, worker-pool size, memory target, spill runs/bytes and merge passes.
Thread count describes available workers, not measured CPU utilization. CLI jobs
using an HTTP backend execute compute in the CLI process; they are not distributed
jobs or dashboard submissions.

## Benchmark evidence

Use the same harness from the CLI and Benchmarks page. The full suite measures
write, read, read_cached, create, stat, rename, delete, sort and wordcount.
`read` starts with an empty Mammoth cache; `read_cached` reuses it. Both retain OS
caches and verify all bytes. Cache counters show whether reuse actually occurred.
Compute uses deterministic 25-byte records with 4,096 numeric keys and three
repeated words. Exact validation catches missing/duplicate/unsorted records and
incorrect frequencies outside the timed job. This is not TeraSort's data or
network shuffle, and finite-key word count is not a general cardinality study.

```bash
# On the Linux host being measured, with a new output directory:
python3 bench-suite/run_linux.py --output /path/on/test-disk/results \
  --read-cache 256MiB --compute-memory 32MiB

# Isolate local compute or disable Mammoth's read cache:
mammoth bench compute --size 32MiB --compute-memory 32MiB --read-cache 0
```

Reports tag the engine version and retain source/hardware provenance through the
Linux collector. Compare both memory-fitting and spilling datasets on identical
hardware with matching durability, cache state, parallelism and semantics. Actual
physical networking, fault-tolerant distributed scheduling and shuffle remain
future work. [Spark's persistence model](https://spark.apache.org/docs/latest/rdd-programming-guide.html#rdd-persistence)
is useful inspiration for reuse and spilling; Mammoth does not implement Spark's
RDD API, lineage recovery or distributed execution.

Regression coverage includes memory and multi-pass spill results against a
sequential Unicode oracle, malformed input, oversized lines, spill corruption,
truncation, cancelled jobs, cleanup, streaming beyond the former 64 MiB input cap,
cache eviction, cross-instance overwrite visibility and health checks after disk
corruption. These tests establish behavior, not comparative speed.
