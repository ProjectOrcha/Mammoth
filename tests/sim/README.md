# Deterministic simulation

Every source of nondeterminism — time, the network, thread scheduling, disk
latency — is driven by a seeded PRNG. The same seed produces the same execution,
byte for byte, on any machine.

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
