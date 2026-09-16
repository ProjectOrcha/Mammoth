#!/usr/bin/env python3
"""Render the committed local measurement into docs and downloadable web assets."""
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / "bench-suite/results/2026-09-16-local.json"
MACHINE = ROOT / "bench-suite/results/2026-09-16-environment.json"
report = json.loads(SOURCE.read_text())
assert report["verified"] and report["cleanup_complete"]
assert report["environment"]["profile"] == "release"
assert len(report["samples"]) == 6 * len(report["options"]["replications"]) * report["options"]["iterations"]
for sample in report["samples"]:
    assert sample["wall_seconds"] > 0
    if sample["aggregate_mib_s"] is not None:
        assert math.isclose(sample["aggregate_mib_s"], sample["bytes"] / 1048576 / sample["wall_seconds"])
public = ROOT / "web/public/benchmarks"
public.mkdir(parents=True, exist_ok=True)
# Do not publish the developer's absolute home directory. All numerical evidence
# is copied without rounding; the table is presentation only.
report["environment"]["data_directory"] = "<local-root>/benchmarks"
report["environment"]["engine"] = "legacy-json-v1"
report["historical"] = True
report["notes"].insert(0, "Historical Mac measurement of the retired JSON engine; excluded from current-engine performance claims.")
(public / "legacy.json").write_text(json.dumps(report, indent=2) + "\n")
(public / "legacy-environment.json").write_text(MACHINE.read_text())

table = "| Operation | Replicas | Median rate | Min–max | Median p99 |\n| --- | ---: | ---: | ---: | ---: |\n"
for row in report["summary"]:
    table += f'| {row["phase"]} | {row["replication"]} | {row["median"]:,.2f} {row["unit"]} | {row["min"]:,.2f}–{row["max"]:,.2f} | {row["median_p99_ms"]:,.2f} ms |\n'
(ROOT / "docs/BENCHMARKS-LEGACY.md").write_text("""# Historical local benchmarks (retired JSON engine)

These Mac measurements predate the SQLite WAL and streaming update. They are
retained as an archive and excluded from current performance claims. Current
Linux measurements are pending; see [the engine update](STORAGE-ENGINE.md).

Measured 2026-09-16 on an Apple M5 Pro, 18 cores, 48 GiB RAM, macOS 26.6.2,
Rust 1.96.1, release build. One physical host; six simulated worker directories.
Three measured runs per replica count, one warmup excluded, four concurrent
clients, eight 16 MiB files, 4 MiB blocks, 1,000 files per metadata phase.
Seed 42; inline threshold 0. All measured runs are retained.

""" + table + """
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
./target/release/mammoth --local-root .mammoth bench suite \\
  --size 16MiB --files 8 --concurrency 4 --ops 1000 \\
  --iterations 3 --warmups 1 --replication 1,3 \\
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
""")
print("Updated benchmark downloads and docs/BENCHMARKS-LEGACY.md")
