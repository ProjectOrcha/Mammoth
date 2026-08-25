# Micro-benchmarks

`criterion` benchmarks for the hot paths. Regressions here are cheap to catch
and expensive to find later.

```bash
cargo bench
cargo flamegraph --bin mammoth -- bench dfsio --write --size 10GB
tokio-console     # find async tasks that stall the runtime
```

Worth benchmarking first:

- namespace read throughput vs. core count (the `ArcSwap` claim)
- CRC32C throughput, hardware vs. software
- block open + first byte, cold and warm page cache
- protobuf decode vs. `rkyv` on hot-path messages
