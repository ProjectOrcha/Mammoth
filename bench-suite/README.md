# Benchmark suite

The implemented suite uses the shared `mammoth-bench` crate from both the CLI and
dashboard. The best fit today is DFSIO-inspired concurrent I/O, metadata
throughput and verified local sort/word count: Mammoth currently has durable local storage, six simulated workers
and local text jobs, but no distributed shuffle or physical worker network.

```bash
cargo build --release --locked -p mammoth-cli
./target/release/mammoth --local-root .mammoth bench suite \
  --size 16MiB --files 8 --concurrency 4 --ops 1000 \
  --iterations 3 --warmups 1 --replication 1,3 \
  --block-size 4MiB --seed 42 --report bench.json --output table
```

Use `bench dfsio`, `bench metadata` or `bench compute` to isolate a workload. `bench --size 8MiB`
still works and now runs the suite. Size is per file. Warmups are excluded from
summaries; all measured runs remain in JSON. CLI and dashboard reports persist at
`<local-root>/benchmarks/reports/`. Temporary benchmark stores use the same parent
filesystem and never modify the application's namespace.

Measured phases: write, read with an empty Mammoth cache, read_cached reusing
that cache (both verify full length + CRC32C), create empty files, stat, rename,
delete, sort and wordcount. Compute uses deterministic 25-byte records with
4,096 numeric keys plus three repeated words; requested bytes round down to
whole records. Exact output validation is outside compute timing. This is not
the TeraSort dataset. Reports contain wall time, logical bytes,
aggregate MiB/s, task-time MiB/s, mean per-task MiB/s, ops/s, p50/p95/p99/max
latency, repetitions, configuration and execution scope. Replica layout is checked
outside the timing window; inlining is disabled for I/O.

Limits: 32 concurrent clients, 1,024 I/O files, 1 GiB per file, 100,000 metadata
files per phase, 10 measured runs, 3 warmups, distinct replica counts 1–6,
1 B–256 MiB blocks, and 8 GiB of replicated I/O data per iteration. Validation
occurs before work. A shared OS lock rejects overlapping runs for one local root.
Successful runs and ordinary errors remove scratch stores; forced termination may
leave a `scratch-*` directory, which can be removed after ensuring no run is active.

Current-engine Linux results are pending. Run:

```bash
python3 bench-suite/run_linux.py --output /path/on/test-disk/results
```

The output directory must be new. The collector records five measured iterations,
machine/filesystem/compiler information, source hashes and raw reports. It rejects
macOS. The manual Linux storage benchmark workflow uploads the same evidence;
shared CI hardware does not establish a competitive win. Reports identify the
engine as `local-memory-parallel-v3`, separating them from previous measurements.

[Current Mac results](../docs/BENCHMARKS.md) include measured Hadoop HDFS and
Spark baselines on the same Apple M5 Pro. See [the comparison runner](compare/README.md)
for exact settings, reproduction and limitations. Run
`python3 bench-suite/compare/publish.py` to validate and regenerate the published
snapshot from committed raw reports; `--from-run PATH` imports a new verified run.

[Historical Mac results](../docs/BENCHMARKS-LEGACY.md) archive the retired JSON
engine. `python3 bench-suite/publish.py` only refreshes that historical archive.
Neither snapshot measures a physical worker network or distributed TeraSort.
Criterion microbenchmarks remain planned. [Full methodology](../web/src/content/docs/ops/benchmarks.mdx)
explains cache effects and Hadoop's differing throughput formula.

Use `--read-cache 0` to disable the read cache, or set a byte size up to 1 GiB.
`--compute-memory` sets the per-job working-set target (64 KiB–256 MiB;
concurrent targets together must not exceed 1 GiB). These are separate budgets,
not a process RSS ceiling. Reports include cache hit/miss/eviction counters and
compute input/output records, threads, spill bytes and merge passes. Memory and
spill settings are also configurable in the dashboard. Replicated input limits
exclude output, scratch runs, checksums and metadata; allow extra disk space.
