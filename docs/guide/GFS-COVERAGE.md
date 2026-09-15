# GFS video: project coverage and remaining gaps

Reviewed against the supplied [video summary](https://www.youtube.com/watch?v=C3-FIM2xTIw)
on 2026-09-15. This is a source audit, not a claim that the distributed service
works. A type, config field, diagram, or `.proto` declaration is **design
coverage**; executable behavior needs a runnable implementation and checks.

## Item-by-item audit

**Before** describes this checkout before the GFS review. **Now** describes the
local teaching update. Production gaps remain open even when the model works.

| Video item | Before this review | Now / evidence | Still needed in the service |
| --- | --- | --- | --- |
| 00:00 · massive data and routine hardware failure | Capacity and failure explained in `CONCEPTS.md` §1 | [Chapter 13](13-gfs-reliability.md) explains workload assumptions; worker disk loss is executable | Multi-machine load and fault testing; no YouTube-scale claim |
| 01:33 · local filesystem blocks | 4 KB comparison existed, presented too universally | Concepts now distinguishes variable local allocation blocks from distributed chunks; model tests boundary lengths | Real on-disk block format in M4 |
| 02:48 · master index and reassembly | `FileStatus`, `BlockId`, `BlockPlacement`, and guide snippets; no implementation | `Simulation::create_file`, `locate`, `read_file`: ordered chunk IDs and independent worker byte stores | Persistent namespace, file-to-block index and location reports |
| 03:20 · limits of one machine/building | Capacity and racks covered | Guide covers shared power/site loss, replica failure domains and backups | Site recovery design and tested backups; six simulated workers share one process |
| 04:23 · chunkservers | `mammoth-worker` and `mammoth-storage` were empty | Model has six independent stores with rack-aware placement | Worker processes, volumes, RPC, restart and discovery |
| 05:04 · master returns locations; direct downloads | Architecture diagrams and trait only | `locate` returns worker IDs; cached `read_location` works while the active master is down | Advertised network addresses, location cache invalidation, streaming worker reads |
| 05:37 · a dead worker can lose unique data | `NodeState::Dead` and health types only | `lose_worker_data`; all copies lost produces `ChunkUnavailable`, never invented bytes | Disk-error detection, checksums, readable/missing-block reporting |
| 07:08 · default replication factor 3 | Config default 3; placement tutorial | Model creates three distinct replicas, favors separate racks, rejects insufficient workers | Durable placement with capacity limits and verified acknowledgements |
| 08:20 · heartbeats and three-miss detection | Starter heartbeat RPC; Mammoth defaults 3 s / 10 min | Model uses 30 s × 3; tests assert no death at 30/60 s and detection at 90 s | Live heartbeat stream, configurable failure detector, reconciled block reports |
| 08:20 · automatic re-replication | Roadmap and repair config only | Every `tick` detects damage, copies surviving bytes and retries shortages | Durable repair jobs, throttling, priorities, idempotent completion and parallel workers |
| 09:29 · protect master metadata | Raft described; `mammoth-meta` empty | Two in-memory metadata snapshots updated on each successful event | Replicated durable operation log, checkpoints and replay; M6 consensus |
| 10:02 · health checker and failover routing | Three-master Raft diagram; no controller | Logical external monitor changes `active_master`, fences old leases and preserves committed metadata | Real leader discovery/redirect/retry, quorum and fencing; DNS only if deliberately integrated |
| 10:57 · concurrent changes can diverge | Single-writer explanation; `LeaseHeld`; no ordering implementation | Clients stage first; all six orders of three overlapping writes are tested | Define overwrite/append semantics and handle failures during an ordered mutation |
| 13:16 · primary determines one order | `ReplicaState::Primary` was only a display label | Actual model lease, version, epoch and primary sequence; stale authority rejected | Worker-enforced durable lease/version checks and mutation protocol |
| 14:22 · bandwidth over latency | Fast-path targets and performance prose | Guide explains aggregate throughput and distinguishes targets from measurements | Reproducible throughput, tail-latency and repair-under-load benchmarks |

Code: [model](../../crates/mammoth-local/src/gfs.rs),
[demo](../../crates/mammoth-local/examples/gfs-demo.rs),
[behavior tests](../../crates/mammoth-local/tests/gfs.rs).
The `gfs` module does **not** implement `Backend`; `quickstart`, `serve`, the
storage CLI, live dashboard API, and real master/worker crates remain unfinished.

## Video assumptions versus Mammoth

| Setting or idea | Video / GFS context | Mammoth choice and current status |
| --- | --- | --- |
| Local block size | 4 KiB is a useful example, not a universal OS rule | Distinguish local allocation blocks, transfer packets and distributed chunks |
| Distributed chunk size | Original GFS used 64 MB chunks | Config says `128MiB`; demo uses 8 bytes so boundaries are visible |
| Replication | Three copies | `storage.replication = 3`; demo implements it |
| Failure timing | Video: 30-second beats, three missed beats | Config says `heartbeat_ms = 3000`, `dead_after = "10m"`; demo settings deliberately follow the video |
| Repair delay | Re-copy after declaring a worker dead | The future scheduler must define how `repair.delay` interacts with the `dead_after` silence deadline and avoid accidentally adding two grace periods. This is an open service contract; the model has just one three-miss deadline |
| Master protection | Replicated metadata and takeover | Planned three-master Raft; demo uses two snapshots and an idealized fencing controller |
| Concurrent writers | Chunk primary assigns mutation order | `Backend::write` exposes whole-file overwrite only. In-place chunk mutation is an isolated model API, not a newly promised storage operation |
| Transfer mode | Sending bytes and deciding their order are separate jobs | `disperse`, `mirror`, and `pipeline` are planned data paths; none alone defines consistency |
| Durability acknowledgements | Depends on what has been replicated and persisted | Demo requires every selected replica before applying one atomic event; production config's `quorum` is not exercised |

The original [GFS paper](https://research.google/pubs/the-google-file-system/)
describes rack-aware clusters, 64 MB chunks, three replicas, primary leases,
replicated logs and relaxed consistency. DNS routing appears in its master
recovery discussion, but routing alone does not prevent two writers. The
video's exact heartbeat and failure rates should be treated as teaching
assumptions, not universal measurements or fixed GFS requirements. Geographic
disaster recovery needs its own placement and recovery design.

## Acceptance checklist

Completed **only for the local model**:

- [x] Split and reconstruct empty, exact-boundary, partial-tail and multi-chunk files.
- [x] Keep file bytes on workers; return an index of chunk locations.
- [x] Put three copies on distinct workers and prefer distinct racks.
- [x] Read another copy after worker loss, including through cached locations.
- [x] Detect failure at the third missed beat; a fresh beat resets the deadline.
- [x] Repair from an existing copy; retry when there are too few destinations.
- [x] Report all copies unavailable without manufacturing replacement data.
- [x] Stage several clients' bytes before the primary assigns mutation order.
- [x] Verify identical replicas for every ordering of three overlapping writes.
- [x] Reject expired leases, old master epochs and unprepared/duplicate mutations.
- [x] Quarantine returning workers, reconcile their versions and remove stale copies.
- [x] Preserve acknowledged model state on standby takeover; reject both-masters-down writes.
- [x] Provide a runnable demonstration and workspace-integrated tests.

Remaining **production work**, with owners expressed as crates:

- [ ] **M4 · `mammoth-storage`:** chunk files, per-range checksums, durable
  acknowledgements, crash-safe publication and restart tests.
- [ ] **M5 · `mammoth-proto`, `mammoth-rpc`:** typed file lookup/allocation,
  worker registration and addresses, inventory/version reports, primary leases,
  staging, mutation ordering, replica acknowledgements and repair messages.
  The current `MasterCommand.kind` string and digest cannot express this protocol.
- [ ] **M5 · `mammoth-master`, `mammoth-worker`:** failure detection with
  explicit timer semantics; source verification, capacity-aware placement,
  bounded repair traffic, and idempotent report/repair handling.
- [ ] **M5 · `mammoth-client`:** metadata caching, direct streaming/range reads,
  replica retry, stale-location refresh and bounded write retry.
- [ ] **M5 · metadata + workers:** settle file overwrite versus record append
  semantics; define lease fencing, mutation identity/order, partial failure,
  retry deduplication and stale-replica rejection before advertising concurrent writes.
- [ ] **M6 · `mammoth-meta`, master and client:** durable replicated log,
  snapshots/replay, quorum election, former-leader fencing, endpoint discovery
  and retry; test takeover during a write and lost quorum.
- [ ] **M5–M6 · `mammoth-testkit`:** multi-process crashes, packet loss/reorder,
  partitions, clock skew, partial writes, stale standby restart, disk corruption
  and whole-process restart. These are not covered by atomic model events.
- [ ] **M5–M6 · operations:** expose missing/critical/under-replicated blocks,
  repair backlog, last heartbeat, active master/epoch and degraded metadata
  redundancy in the real dashboard; test backup restore and site loss separately.
- [ ] **Performance:** measure aggregate client bandwidth and p50/p99 latency
  with reproducible hardware, dataset, replication and repair settings.

Run the model checks from the repository root:

```bash
cargo run -p mammoth-local --example gfs-demo
cargo test -p mammoth-local --test gfs
```

Next: [Chapter 13 — follow every event](13-gfs-reliability.md) ·
[Roadmap](../ROADMAP.md) · [Guide index](README.md).
