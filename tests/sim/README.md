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

## Planned: distributed fault harness

Every source of nondeterminism — time, the network, thread scheduling, disk
latency — is driven by a seeded PRNG. The same seed produces the same execution,
byte for byte, on any machine.

The commands below are future harness commands. There is no `sim` test target
yet; nightly seeded runs remain gated on its creation. The GFS model is the
`gfs` target and runs in ordinary workspace CI.

```bash
cargo nextest run --test sim                       # random seed, printed on failure
MAMMOTH_SIM_SEED=8412337 cargo nextest run --test sim   # reproduce exactly
```

Scenarios to cover, in the order they are worth writing:

- leader election under a symmetric network partition
- a write in flight when the leader is killed
- a worker that comes back with stale blocks after a long absence
- clock skew past the lease timeout
- a disk that returns correct data slowly, and one that returns wrong data fast
- a retry storm after a master restart
