# Testing

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

Nightly runs 10,000 seeds and files an issue with the seed on any failure. To
reproduce one locally:

```bash
MAMMOTH_SIM_SEED=8412337 cargo nextest run -p mammoth-testkit --test sim
```
