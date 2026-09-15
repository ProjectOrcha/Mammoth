# Testing

**Current status:** the distributed, fault-injection, property and compatibility
harnesses below are planned. Their folders currently contain design notes.
Do not treat the seed command as runnable until a `sim` test target exists.
Rust checks run with `cargo test --workspace`; dashboard regressions run with
`npm test` in `ui/`. See [CONTRIBUTING.md](../CONTRIBUTING.md) for all checks.

The separate GFS event model is available now:
`cargo test -p mammoth-local --test gfs`. It exercises real in-memory bytes,
replica repair, concurrent mutation schedules and standby takeover. It runs in
workspace CI but does not replace the planned distributed fault harness.
See [simulation status](sim/README.md) and the
[GFS acceptance checklist](../docs/guide/GFS-COVERAGE.md).

Distributed systems fail in ways unit tests never find.

| Layer | Tool | Catches |
| --- | --- | --- |
| Unit | `cargo nextest` | logic bugs |
| Property | `proptest` | namespace invariants, checksum round-trips |
| **Deterministic sim** | `madsim` / `turmoil` | races, partitions, retry storms — **reproducible from a seed** |
| Fault injection | `mammoth-testkit` | kill nodes, corrupt blocks, fill disks, skew clocks |
| Fuzzing | `cargo-fuzz` | protocol parser panics |
| Compat | `tests/compat/` | runs real Hadoop in Docker, verifies WebHDFS + checksum parity |
| Load | `bench-suite/` | performance regressions |

## Directories

- `e2e/` — a real (single-host) cluster from `mammoth-testkit`, driven through the CLI
- `sim/` — deterministic simulation. Every failure is reproducible from its seed
- `compat/` — parity against a real Hadoop in Docker

## The rule that matters

Build the deterministic simulation harness in **M5, not M9**. Retrofitting it is
painful. Having it means every distributed bug reduces to a seed number in a CI
log — the practice that makes TigerBeetle and FoundationDB trustworthy.

Once the distributed `sim` test target exists, nightly is configured to run
10,000 seeds and file an issue with the seed on failure. Its future reproduction
command is:

```bash
MAMMOTH_SIM_SEED=8412337 cargo nextest run -p mammoth-testkit --test sim
```
