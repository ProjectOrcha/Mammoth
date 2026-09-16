---
title: Operate the local service
description: Startup, readiness, safe shutdown, backups and the local preview release boundary.
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


`AI_coded` is a durable **local preview**. Six worker directories share one host.
Authentication, authorization, TLS, independent worker processes, high
availability and distributed shuffle are not implemented. Do not expose it as a
public or multi-tenant storage service.

## Start and check

Build the dashboard before the binary as shown in the [quickstart](/intro/quickstart/).
Use an explicit store directory outside your source and build directories.

```bash
mammoth config template > mammoth.toml
mammoth --config mammoth.toml config validate
mammoth --config mammoth.toml --local-root /path/to/store serve --role all
```

`serve` starts an empty store; `quickstart` additionally seeds sample files.
Defaults bind to loopback. `--allow-remote` enables an explicitly configured remote
listener but provides no authentication. Unsupported TLS/authentication settings
are rejected. Browser access requires localhost or an IP address with matching
request origin. Native clients with network access still have full access.

`GET /healthz` checks HTTP liveness. `GET /readyz` checks access to the namespace
root and returns 503 when unavailable. Readiness does not certify free space or
replica health. Use `mammoth doctor` for replica checks; a complete check scans
blocks and may be expensive. Monitor real disk space externally: the dashboard's
worker capacities describe a simulated layout.

## Stop, back up and restore

1. Run `mammoth --local-root /path/to/store stop --timeout 60`. Wait until it
   reports stopped. Slow requests and accepted jobs can delay shutdown.
2. Stop other CLI writers using that store. Copy the **entire idle store** to a
   separate backup directory, including worker blocks, metadata, SQLite and any
   WAL/SHM files. A database-file-only backup can lose committed data.
3. Record the binary version, configuration and known file hashes. Keep a copy
   off the host when protection from disk loss is required.
4. Restore to a new directory. Run `doctor`, list expected files and compare
   downloaded hashes using the same binary first. Leave the original intact.
5. Test upgrades on another restored copy. A migrated version-2 namespace must
   never be reopened by the old JSON engine.

The [operator runbook](https://github.com/ProjectOrcha/Mammoth/blob/AI_coded/docs/OPERATIONS.md)
contains resource planning, recovery and deployment details. The
[release gates](https://github.com/ProjectOrcha/Mammoth/blob/AI_coded/docs/RELEASE-READINESS.md)
separate tested local behavior from outstanding production work. Native archives
include these notes, supported configuration, build information and a companion
SHA256 checksum. Compose and systemd examples run one local service; no deployable
Helm chart is provided.

## Benchmarks

The [measured comparison](/ops/benchmarks/) retains the Mac Mammoth, HDFS and Spark
results with raw evidence and cache/memory limits. Those results describe their
recorded source snapshot. Later hardening changes do not inherit a performance
claim without a new controlled measurement. Linux and physical cluster results
remain unmeasured.
