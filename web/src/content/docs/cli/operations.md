---
title: Operations
description: Start and inspect the local service, repair replicas, run jobs, and transfer directory trees.
sidebar:
  order: 4
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


## Lifecycle

Build the dashboard before compiling the Rust binary so it contains the latest
pages. See the [quickstart](/intro/quickstart/) for the complete setup.

```bash
mammoth --local-root .mammoth init
mammoth --local-root .mammoth quickstart
```

`quickstart` runs the dashboard on port 8080 and S3 on 9000, with sample files.
Use `--no-sample` to skip sample data or `--ui-listen` and `--s3-listen` to change
the addresses. It runs in the foreground; **Ctrl-C** stops the service.
From another terminal, use the same local root:

```bash
mammoth --local-root .mammoth status
mammoth --local-root .mammoth stop
mammoth --local-root .mammoth shutdown  # alias for stop
```

`stop` closes the dashboard and S3 listeners after accepted requests and dashboard
jobs finish. It waits up to 30 seconds; use `stop --timeout 60` for a longer wait.
If the wait expires, the stop request stays pending. Repeating `stop` is safe.
`status --json` reports `starting`, `running`, or `stopped`; a running record includes
the process ID and actual listener addresses. One service can run per local root.
These commands act on the local store and reject `--masters`.

Restart with the same root to retain your files. For complete backup, restore,
security and release limits, see [operating the local service](/ops/local-service/).

```bash
mammoth --local-root .mammoth serve --role all
mammoth ui
```

`serve --role all` and `serve --role gateway` run the local service. Separate
master and worker roles are not implemented. `ui` prints the configured address;
it does not start the service or open a browser.

The terminal logo appears when starting either service command and in top-level
help. Run `mammoth` with no arguments for the logo and command list, or `mammoth logo`
for just the art. `--json` and explicit YAML/CSV service output omit the logo.

## Health and maintenance

```bash
mammoth doctor
mammoth doctor --node w1
mammoth node list
mammoth node inspect w1
mammoth cluster status
mammoth admin report
mammoth admin repair
mammoth admin gc
```

Repair checks replicas and restores damaged or missing copies from a healthy
source. Garbage collection removes blocks no longer referenced by the namespace.
The dashboard's **Cluster** page includes **Repair replicas** and links to workers.
There are no decommission, balancer, safemode, or Raft-management commands in this build.

## Text jobs

```bash
mammoth job wordcount /sample/words.txt /sample/counts.txt
mammoth job sort /sample/words.txt /sample/sorted.txt
```

Jobs stream UTF-8 input, process batches across CPU threads, and spill larger jobs to disk. There is no total input-size cap. Configure `compute.memory_budget` (default 128 MiB) and `compute.spill_directory`; individual lines are limited to the smaller of 16 MiB and one eighth of the memory target. Jobs write durable results to Mammoth and report memory mode, threads and spill statistics. The
output must differ from the input. Existing output is preserved unless you pass
`--overwrite`. The check also protects a file created while the job is running.

The **Jobs** dashboard also runs word-count and line-sort jobs. It requires an
explicit choice before replacing an existing output file. It shows running,
succeeded, and failed submissions and links to the resulting files. The latest
100 dashboard submissions are retained for the current service session. Restarting
the service clears this history; it does not remove output files. CLI submissions
are not included in dashboard history.

These are local text operations. Distributed stages, shuffle, and a persistent
job scheduler remain future work.

## Tree transfers

```bash
mammoth migrate import ./dataset /dataset
mammoth migrate export /dataset ./exported-dataset
```

Transfers process files individually and refuse symlinks. They do not implement
native HDFS migration, resumable journals, or cluster cutover.

## Benchmark and configuration

```bash
mammoth bench suite --size 8MiB --files 8 --replication 1,3 --report bench.json
mammoth config show
mammoth config validate
mammoth config template
mammoth completions zsh
```

The benchmark measures concurrent writes, separate fresh-cache and repeated reads, metadata operations, and exact-validated sort/word count
in isolated temporary stores. It reports repeated measurements, latency percentiles,
replication comparisons, cache/spill statistics and full JSON results. Use `--read-cache` and `--compute-memory` for benchmark memory settings. These are local, warm-cache
measurements. See [benchmarks and measured results](/ops/benchmarks/) for methodology
and limits. The dashboard has Benchmarks and Configure sections.
Configuration is edited in a TOML file; `config set` is not an implemented command.

See the [generated reference](/cli/reference/) for all supported options.
