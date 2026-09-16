# Indexed metadata and streaming storage

These v2 storage changes are retained by `local-memory-parallel-v3`. See
[the memory engine](MEMORY-ENGINE.md) for cache, compute and spill behavior.
The on-disk namespace format remains version 2.

The `local-wal-stream-v2` engine removes three concrete costs from the previous
implementation. Performance changes must be measured on Linux; no new speedup
or Hadoop/Spark win has been established.

| Path | Previous implementation | Current implementation |
| --- | --- | --- |
| Stat/list | Reload and decode the whole JSON namespace under one process lock | Indexed SQLite queries; independent read transactions |
| Namespace mutation | Rewrite and sync the whole namespace | Transaction changes only affected rows; WAL with `synchronous=FULL` |
| Upload | Stage the entire request, reread it under the namespace lock, write each replica serially | Buffer at most a block or inline threshold plus one byte; write replicas concurrently; commit metadata after every replica is durable |
| Download | Verify full blocks and copy the response to a temporary file | Read 64 KiB chunks; verify overlapping 4 KiB checksum chunks; retry another replica on corruption |
| S3 range/suffix | Read and discard the preceding file bytes | Open the requested range directly from the captured generation |

`stat` and directory listing do not deserialize inline payloads. Parent and path
indexes avoid full-namespace scans. SQLite still permits one writer at a time;
this is not a lock-free metadata server. Connections use prepared statement
caches and a pool of up to eight idle connections. Uploads share eight slots per
backend instance. Replica tasks share byte buffers and wait for all requested
replicas, retaining data/header/directory synchronization. Block IDs are reserved
durably in batches of 64; gaps after interruption are expected and never reused.

## Consistency and interrupted operations

The namespace publishes a new file generation only after its input finishes and
all replicas are synced. A failed or cancelled stream leaves the old committed
file intact. Its unpublished blocks can be reclaimed with `mammoth admin gc`.
A process may finish a transaction if cancellation arrives after commit starts.

Read snapshots hold a shared OS lifetime lock through stream consumption. The
lock protects immutable blocks from reclamation across processes; it does not
serialize normal writes or metadata queries. An overwrite/delete can complete
while a reader still sees the old generation. Cleanup skips busy stores and GC
reclaims the retired blocks later. GC returns zero while a stream/upload is
active; zero does not prove the absence of reclaimable blocks. Finish/drop the
streams and retry. Repair and replica-count changes return a retryable I/O
`WouldBlock` error while data operations are active. A replica-count change holds
a metadata write transaction during maintenance; ordinary uploads do not.

Streaming can surface corruption after a response starts. Every emitted chunk
is verified; exhaustion of healthy replicas ends the stream with an error.
Unrequested chunks are not scanned by range reads. A health report or repair
performs full-block verification. Checksums detect accidental corruption, not
malicious edits. OS filesystem caches remain enabled.

## Upgrade and backup

Stop all services/clients using the store and back up the **complete store** before
opening it with this build. The first open migrates version-1 JSON entries and the
block-ID counter into SQLite in one transaction, retains
`ns/namespace.legacy.json`, then atomically publishes a version-2 marker at
`ns/namespace.json`. Worker blocks stay in place. If migration is interrupted
before the marker, it retries from the legacy snapshot. If a new store has a
database but no marker and no legacy snapshot, open fails instead of silently
reinitializing potentially recoverable metadata. Older binaries reject
the new marker instead of writing a diverging namespace.

The legacy JSON file is a migration archive, **not a current backup**: it cannot
restore changes made after migration. Do not delete a missing/corrupt database
and recreate it from this archive. Restore a consistent complete backup instead.
For a simple file-copy backup, stop every process using the store and copy the
whole root, including SQLite's database, WAL and shared-memory files if present.
Never copy just `namespace.sqlite3` while a service is writing.

```
<root>/
  store.lock                 # migration compatibility lock
  activity.lock              # protects live immutable generations
  ns/namespace.json          # version-2 marker
  ns/namespace.sqlite3       # indexed metadata and ID counter
  ns/namespace.sqlite3-wal    # may exist while connections are open
  ns/namespace.sqlite3-shm    # may exist while connections are open
  ns/namespace.legacy.json    # retained only for migrated stores
  workers/w1/blocks/.../data
  workers/w1/blocks/.../meta.json
  ... w2 through w6
```

Keep this database on a local filesystem. SQLite WAL is not a distributed
metadata/consensus protocol. Physical master/worker deployment, network shuffle,
HA/fencing, cross-machine replication and TeraSort remain unimplemented. Actual
power-loss qualification remains open; restart/cancellation tests are not proof
against every hardware failure.

## Validation and next measurements

Correctness coverage includes restart/migration, abrupt process exit with a live
WAL, separate-process writers,
interrupted/cancelled overwrites, metadata during a paused upload, generation
retention during delete/overwrite/GC, unaligned range/tail verification, truncated
blocks, corrupted replicas and S3 suffix reads that bypass a damaged prefix.

Run `python3 bench-suite/run_linux.py --output /path/on/test-disk/results` on an
idle Linux host. The directory must not already exist. It builds release mode,
runs five measured iterations after one warmup for one and three replicas, and
records all raw samples, compiler, source hashes, machine, filesystem and load.
The manual **Linux storage benchmark** workflow collects the same artifacts on
an Ubuntu runner; shared CI hardware is not a competitive baseline. Compare
HDFS on identical dedicated hardware and settings after Mammoth supports the
same deployment. Compare Spark only with equivalent compute workloads.
