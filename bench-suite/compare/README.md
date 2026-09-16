# Same-Mac comparison

The [published report](../../docs/BENCHMARKS.md) records actual measurements of
Mammoth's memory engine, Hadoop HDFS 3.5.0 and Spark 4.2.0, using Temurin Java
17.0.20.1+1 on macOS ARM64. These are small local workloads, not a distributed
cluster contest. No expected or estimated competitor scores are used.

```bash
cargo build --release --locked -p mammoth-cli
python3 bench-suite/compare/setup.py
python3 bench-suite/compare/run.py --output target/mac-comparison-new
python3 bench-suite/compare/publish.py --from-run target/mac-comparison-new
```

Use Python 3.11+ and an otherwise idle Mac. Downloads are official archives with
official checksums, kept inside ignored `target/comparison-tools`. Ports and
storage are isolated; existing Mammoth data is untouched. The runner requires a
new output directory and stops all its daemons in `finally`, including failures.
Logs, configurations and daemon storage remain in the output directory for
inspection. Successful reports require verified outputs and cleaned namespaces.

`publish.py` validates all expected phases, replica counts, iteration identities,
workload sizes and rate arithmetic before generating the docs, website and
dashboard snapshot. Running it without arguments regenerates the current snapshot
from committed evidence. The older `bench-suite/publish.py` only renders the
retired JSON engine archive.

## Workload controls

- Eight 8 MiB seeded pseudorandom files; four concurrent clients; 4 MiB blocks;
  one and three replicas. Writes include generation and CRC32C; reads include
  byte-count and CRC32C verification. HDFS checks actual block locations.
- 200 empty files for each create/stat/rename/delete phase. Empty metadata files
  do not exercise replicated data writes. HDFS uses NameNode RPCs; Mammoth uses
  direct local SQLite namespace calls.
- Sort and word count use the same 335,544 25-byte lines per file. Four-digit
  keys span 4,096 values; three words repeat. Each file is a separate job, with
  four concurrent jobs. Spark uses `local[18]`, FAIR scheduling and four
  partitions per job; Mammoth uses its shared CPU pool. Exact output order,
  records, frequencies and requested replica placement are checked after timing.
- Spark's `DurableTextOutput` sets replica count explicitly and syncs every part
  before close. This avoids HDFS client caching freezing the first replica
  setting. Spark creates four part files and commit markers for each job;
  Mammoth creates one output file. HDFS sync-on-close is enabled.
- One warmup and three recorded repetitions, with alternating replica order.
  Engine groups run sequentially. JVM/service startup, data preparation, compute
  validation and cleanup are outside timing. Job scheduling and output commits
  are included. Aggregate rate is logical bytes / phase wall time, not Apache
  TestDFSIO's task-duration metric. Raw samples retain per-iteration p99 latency.
- OS caches are retained. Mammoth starts the first read and each compute phase
  with an empty application cache; its second read reuses verified cached bytes.
  HDFS's second read is a normal repeated read. No cold-disk claim is valid.
- Mammoth: 256 MiB read cache and 32 MiB compute target per job. Spark driver:
  1 GiB heap. HDFS: one NameNode and six DataNodes, 512 MiB heap each. These
  settings are not equal memory/RSS limits. The Mac Hadoop build logs that native
  libraries are unavailable and uses Java fallbacks.

Storage compares Mammoth with HDFS. Compute compares Mammoth with Spark on HDFS.
No Hadoop MapReduce, HiBench, distributed TeraSort, physical NIC saturation,
peak-memory, power, or failure-recovery benchmark is included. Repeating on
larger datasets and physical Linux clusters is necessary before drawing broader
conclusions. This suite is not Apache's official TestDFSIO.

The environment record hashes the measured binary and Rust/Java/runner sources,
and records the base Git revision of the dirty working tree. Published copies
replace absolute workspace paths with `<workspace>` without changing numbers.
The retained source hashes identify exactly which implementation was measured.

Official references: [Hadoop 3.5.0](https://hadoop.apache.org/docs/r3.5.0/),
[Spark 4.2.0](https://spark.apache.org/docs/4.2.0/),
[Hadoop native libraries](https://hadoop.apache.org/docs/r3.4.3/hadoop-project-dist/hadoop-common/NativeLibraries.html).
