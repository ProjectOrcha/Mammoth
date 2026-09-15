# ADR 0003: An executable GFS teaching model

Status: accepted for the local demonstration; distributed implementation remains
planned in M5–M6.

## Context

The GFS video review exposed a gap between Mammoth's architecture documentation
and its executable behavior. The Rust service crates and `LocalBackend` are
placeholders. A learner cannot yet exercise replication, repair, concurrent
mutation ordering, or master takeover.

## Decision

Add `mammoth_local::gfs`, an in-memory, deterministic event simulation, and a
runnable `gfs-demo` example in that package. Model separate worker byte stores,
a file-to-chunk index, three replicas across racks, heartbeats, repair, a
primary lease with ordered mutations, and two copies of master metadata.

An explicit logical clock delivers heartbeats and drives an external failover
controller. The controller fences the old master with a new epoch and changes
a simulated service endpoint. Each operation is one atomic simulation event.
Worker data is staged before the primary orders it; missing participants cause
an error before application. Staged writes must be retried after lease changes.

This is an executable explanation, not `Backend`, a Raft implementation, DNS
automation, or durable storage. Memory copies demonstrate metadata replication;
they do not demonstrate disk flushes or recovery after the whole process dies.
Network partitions, mid-event crashes, clock skew, and partial replica writes
need the future distributed harness. Keep those limitations next to the demo
and in the coverage checklist.

Use 30-second heartbeats and three missed beats for the video demonstration,
with tiny chunks for visible output. Preserve Mammoth's existing configuration
defaults and its Raft roadmap; its data-transfer mode does not establish write
ordering. Do not treat `ReplicaState::Primary`, a display label, as a lease.

## Validation

Tests must exercise byte reconstruction, distinct placement, the exact failure
deadline, repeated repair, insufficient destinations, all replicas lost, staging
versus ordering, expired and superseded leases, stale-worker return, metadata
takeover, and both masters unavailable. Run the demo and the workspace checks.

## Consequences

The model gives contributors a small executable specification to discuss before
building network services. The guide retains an explicit production gap list;
passing model tests cannot mark M5 or M6 complete.
