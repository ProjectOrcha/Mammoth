---
title: Operations
description: Operate, inspect and maintain the local Mammoth preview.
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


Start with [the local-service runbook](/ops/local-service/) for startup, readiness,
shutdown, backup and restore. This branch runs one durable local service with six
worker directories. Real distributed workers, authentication, TLS and high
availability are unfinished.

- [Configuration](/ops/configuration/) — settings that the local service uses.
- [CLI operations](/cli/operations/) — lifecycle, health, repair, garbage collection and jobs.
- [Benchmarks](/ops/benchmarks/) — measured Mac results, HDFS/Spark comparisons and limits.
- `mammoth doctor` — inspect actual replica health.
- `mammoth top` — local terminal dashboard.

The repository includes a Dockerfile, one-service Compose layout and a systemd
example requiring a pre-created service user and data directory. Review the
runbook before using them. There is no deployable Helm chart, automatic systemd
installer, balancer, safemode or Raft administration in this build.
