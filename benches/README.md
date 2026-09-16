# Micro-benchmarks

Criterion hot-path benchmarks are planned; there are no Criterion targets here
yet. The implemented end-to-end suite is in [bench-suite](../bench-suite/README.md).

```bash
cargo run --release -p mammoth-cli -- bench dfsio --size 16MiB --files 8
tokio-console     # find async tasks that stall the runtime
```

Worth benchmarking first:

- namespace read throughput vs. core count (the `ArcSwap` claim)
- CRC32C throughput, hardware vs. software
- block open + first byte, cold and warm page cache
- protobuf decode vs. `rkyv` on hot-path messages
