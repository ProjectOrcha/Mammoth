# Deterministic simulation

## Available now: GFS event model

```bash
cargo test -p mammoth-local --test gfs
cargo run -p mammoth-local --example gfs-demo
```

This in-memory model uses an explicit clock and enumerated client schedules.
It tests replica loss/repair, heartbeat boundaries, primary leases, write order,
stale worker return and master takeover. Each operation is atomic; it does not
model network partitions, clock skew or partial disk writes. See the
[coverage audit](../../docs/guide/GFS-COVERAGE.md).

## Available now: seeded teaching-model checks

The `mammoth-testkit` `sim` target checks deterministic generated inputs against
the GFS teaching model. Nightly CI runs 10,000 seeds. Reproduce a reported seed:

```bash
MAMMOTH_SIM_SEED=8412337 cargo test --locked -p mammoth-testkit --test sim
MAMMOTH_SIM_SEED=1 MAMMOTH_SIM_COUNT=10000 cargo test --locked -p mammoth-testkit --test sim
```

This is an in-memory model, not the production storage path. It does not inject
real network faults or establish power-loss durability.

## Planned: distributed fault harness

A future service harness must control time, network delivery, scheduling and disk
failure to replay an execution. Record the initial seed and every injected fault.

Scenarios to cover, in the order they are worth writing:

- leader election under a symmetric network partition
- a write in flight when the leader is killed
- a worker that comes back with stale blocks after a long absence
- clock skew past the lease timeout
- a disk that returns correct data slowly, and one that returns wrong data fast
- a retry storm after a master restart
