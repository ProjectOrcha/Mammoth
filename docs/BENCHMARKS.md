# Measured Mac comparison

Measured 2026-09-16. All three engines completed with verified results and cleaned benchmark namespaces.

Mammoth `local-memory-parallel-v3`; Hadoop HDFS 3.5.0; Spark 4.2.0 on HDFS. Apple M5 Pro · 18 logical cores · 48 GiB · macOS 26.6.2.

## 1 replica

| Operation | Unit | Mammoth | Hadoop HDFS | Spark on HDFS |
| --- | --- | ---: | ---: | ---: |
| Write | MiB/s | 229.37 | 160.07 | — |
| Read · empty Mammoth cache | MiB/s | 9,985.11 | 3,084.94 | — |
| Repeated read* | MiB/s | 17,892.71 | 3,665.97 | — |
| Line sort | MiB/s | 157.15 | — | 47.52 |
| Word count | MiB/s | 178.82 | — | 41.31 |
| Create empty file | ops/s | 4,949.49 | 290.60 | — |
| Stat | ops/s | 219,659.53 | 22,787.80 | — |
| Rename | ops/s | 8,829.42 | 584.53 | — |
| Delete | ops/s | 8,818.96 | 549.65 | — |

## 3 replicas

| Operation | Unit | Mammoth | Hadoop HDFS | Spark on HDFS |
| --- | --- | ---: | ---: | ---: |
| Write | MiB/s | 124.18 | 80.51 | — |
| Read · empty Mammoth cache | MiB/s | 10,220.85 | 3,037.92 | — |
| Repeated read* | MiB/s | 18,810.85 | 3,642.81 | — |
| Line sort | MiB/s | 95.43 | — | 30.59 |
| Word count | MiB/s | 131.06 | — | 41.50 |
| Create empty file | ops/s | 3,086.07 | 302.46 | — |
| Stat | ops/s | 164,795.55 | 20,989.02 | — |
| Rename | ops/s | 4,822.74 | 633.32 | — |
| Delete | ops/s | 4,822.39 | 569.78 | — |

Values are medians; higher rates mean more work per second. A dash means that engine was not measured for that operation. Ranges and per-iteration p99 latency are retained in the downloadable comparison JSON.

## Interpretation and limits

The source manifest identifies the measured build. Later code changes require new measurements; this snapshot does not certify the current branch head.

Single physical Mac. These are small same-host workloads, not a physical distributed cluster or an overall winner.

Mammoth uses direct local backend calls and six worker directories. HDFS uses a Java RPC client, one NameNode and six DataNode JVMs over loopback. Spark runs local[18] on HDFS, with four partitions per job and four concurrent per-file jobs; it is the compute comparison, not a storage alternative.

Eight 8 MiB files, four clients, 4 MiB blocks, seed 42, one and three replicas; 200 empty files per metadata phase. Median of three measured iterations after one excluded warmup. Engine groups run sequentially; service startup, setup, compute validation, layout checks and cleanup are excluded. Read byte-count and checksum verification are timed. All measured runs are retained.

OS caches stay warm. The first read clears only Mammoth’s application cache. Repeated read* reuses Mammoth’s verified memory cache; HDFS performs a second normal read. Neither measures cold storage bandwidth. Logical MiB/s can exceed SSD bandwidth.

Memory policies differ: Mammoth has a 256 MiB read cache and a 32 MiB accounting target per job; Spark has a 1 GiB driver heap, and each HDFS daemon a 512 MiB heap. These are not equivalent RSS limits. CPU, peak RSS and network saturation were not measured.

Compute uses 335,544 fixed 25-byte records per file, 4,096 numeric keys and three repeated words. Outputs have identical logical records, but Mammoth writes one file per job and Spark writes four part files plus commit markers. This is not TeraSort, HiBench or Hadoop MapReduce.

Writes wait for requested replicas. Mammoth syncs replica files and SQLite WAL metadata. HDFS uses hsync and DataNode sync-on-close; Spark uses an explicit replicated, synced text output writer. Replica placement and all output records/checksums are verified. These checks do not constitute a crash-durability test.

No native Hadoop library is available in this Mac distribution, so Hadoop uses Java fallbacks. Do not extrapolate these results to tuned Linux clusters. Empty metadata files do not exercise replication.

## Reproduce and inspect

```bash
cargo build --release --locked -p mammoth-cli
python3 bench-suite/compare/setup.py
python3 bench-suite/compare/run.py --output target/mac-comparison-new
python3 bench-suite/compare/publish.py --from-run target/mac-comparison-new
```

Pinned Mac ARM64 runtimes are checksum-verified and stored under ignored `target/comparison-tools`. The runner uses a new disposable directory, binds test services to loopback, and stops them on success or failure. Namespace cleanup excludes the retained logs and daemon storage directories. Do not run other benchmarks or builds concurrently. The source manifest records the measured binary, base revision and dirty source hashes; it identifies the measurement, not a pristine Git commit.

- [Mammoth raw evidence](../bench-suite/results/2026-09-16-mac-mammoth.json)
- [Hdfs raw evidence](../bench-suite/results/2026-09-16-mac-hdfs.json)
- [Spark raw evidence](../bench-suite/results/2026-09-16-mac-spark.json)
- [Environment raw evidence](../bench-suite/results/2026-09-16-mac-environment.json)
- [Comparison raw evidence](../bench-suite/results/2026-09-16-mac-comparison.json)

[Harness source and methodology](../bench-suite/compare/README.md) · [Historical JSON engine report](BENCHMARKS-LEGACY.md) · [Linux collector](../bench-suite/run_linux.py). No current Linux or multi-machine competitive result has been collected.
