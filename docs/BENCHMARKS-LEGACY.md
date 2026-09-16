# Historical local benchmarks (retired JSON engine)

These Mac measurements predate the SQLite WAL and streaming update. They are
retained as an archive and excluded from current performance claims. Current
Linux measurements are pending; see [the engine update](STORAGE-ENGINE.md).

Measured 2026-09-16 on an Apple M5 Pro, 18 cores, 48 GiB RAM, macOS 26.6.2,
Rust 1.96.1, release build. One physical host; six simulated worker directories.
Three measured runs per replica count, one warmup excluded, four concurrent
clients, eight 16 MiB files, 4 MiB blocks, 1,000 files per metadata phase.
Seed 42; inline threshold 0. All measured runs are retained.

| Operation | Replicas | Median rate | Min–max | Median p99 |
| --- | ---: | ---: | ---: | ---: |
| create | 1 | 110.98 ops/s | 109.59–118.09 | 42.86 ms |
| create | 3 | 110.35 ops/s | 109.76–111.00 | 42.01 ms |
| delete | 1 | 112.43 ops/s | 110.32–112.73 | 42.04 ms |
| delete | 3 | 111.98 ops/s | 110.91–112.27 | 42.93 ms |
| read | 1 | 1,302.59 MiB/s | 1,249.92–1,306.42 | 55.97 ms |
| read | 3 | 1,335.79 MiB/s | 1,326.00–1,361.98 | 53.93 ms |
| rename | 1 | 101.31 ops/s | 101.01–106.57 | 45.15 ms |
| rename | 3 | 101.78 ops/s | 101.48–103.68 | 45.10 ms |
| stat | 1 | 2,044.76 ops/s | 2,039.35–2,059.82 | 3.90 ms |
| stat | 3 | 2,062.89 ops/s | 2,054.13–2,067.81 | 4.02 ms |
| write | 1 | 119.21 MiB/s | 113.97–122.16 | 545.73 ms |
| write | 3 | 51.01 MiB/s | 50.96–51.12 | 1,261.81 ms |

Aggregate I/O rate is logical bytes / phase wall time. Reads follow writes with
OS caches retained and verify length plus CRC32C. Writes include pseudorandom
generation, checksum, staging and durable replica writes. Metadata creates empty
files, stats, renames and deletes each; replication is not exercised by empty
files. p99 is the median of per-iteration operation p99s, not a pooled percentile.

The namespace serializes operations with a global file lock and JSON snapshots.
These results establish a baseline for that implementation. They do not establish
network saturation, distributed scalability, shuffle performance or a speedup
over Hadoop/Spark. No Hadoop/Spark baseline was run. Rust alone implies no win.

```bash
cargo build --release --locked -p mammoth-cli
./target/release/mammoth --local-root .mammoth bench suite \
  --size 16MiB --files 8 --concurrency 4 --ops 1000 \
  --iterations 3 --warmups 1 --replication 1,3 \
  --block-size 4MiB --seed 42 --report bench.json --output table
```

- [Raw report](../bench-suite/results/2026-09-16-local.json)
- [Machine, compiler and source record](../bench-suite/results/2026-09-16-environment.json)
- [Harness and limits](../bench-suite/README.md)
- [Full methodology](../web/src/content/docs/ops/benchmarks.mdx)

The source record identifies a modified working tree based on its recorded Git
revision and hashes the measured Rust source files. Subsequent engine changes mean these values do not measure the current source. Benchmark data was cleaned after
each iteration. Reports persist in `<local-root>/benchmarks/reports/` and appear
in the dashboard's historical Benchmarks reports when using the same local root.

To refresh the website and this table from the committed measurement, run
`python3 bench-suite/publish.py`. To publish a new measurement, update the source
and machine paths in that script and retain the previous raw evidence.
