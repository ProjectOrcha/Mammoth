---
title: HTTP API
description: The endpoints the CLI and the Web UI both consume.
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


This page covers the implemented local HTTP gateway on `AI_coded`. It has no
authentication or TLS. Use the loopback defaults and read the
[operating limits](/ops/local-service/) before changing listener addresses.

## Filesystem and service endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/healthz` | HTTP liveness |
| GET | `/readyz` | Namespace-root readiness; 503 when unavailable |
| GET | `/api/v1/cluster/report` | Local node, replica and cache information |
| GET | `/api/v1/nodes` | Simulated worker directories |
| GET | `/api/v1/fs?path=/data` | List children |
| GET | `/api/v1/fs/stat?path=/data/file` | File metadata |
| GET | `/api/v1/fs/blocks?path=/data/file` | Replica layout |
| GET | `/api/v1/fs/data?path=/data/file` | Stream file bytes |
| PUT | `/api/v1/fs/data?path=/data/file` | Write or replace bytes |
| PUT | `/api/v1/fs/data?path=/data/file&create=true` | Atomically create only when absent; 409 on conflict |
| PUT | `/api/v1/fs/directory?path=/data` | Create a directory |
| DELETE | `/api/v1/fs?path=/data/file` | Remove a file; `recursive=true` for a tree |
| POST | `/api/v1/admin/repair` | Restore damaged replicas from healthy copies |
| POST | `/api/v1/admin/gc` | Remove unreferenced blocks |
| GET, POST | `/api/v1/jobs` | Inspect or submit local text jobs |
| GET | `/api/v1/events` | Server-Sent Events for dashboard refresh |

Browser requests must use localhost or a literal IP and the same origin.
Cross-origin mutations and browser DNS-rebinding hostnames are rejected. This is
not authentication: native clients with network access can use the service.
CLI jobs and dashboard jobs preserve an existing destination unless overwrite is
explicitly requested. Creation is checked at publication, including files created
while computation is running. Ordinary file uploads retain replacement semantics.

## Implemented benchmark and configuration endpoints

The local CLI service exposes these dashboard endpoints:

| Method | Path | Response |
| --- | --- | --- |
| GET | `/api/v1/benchmarks` | Current dashboard run, latest 20 persisted reports and default options |
| POST | `/api/v1/benchmarks` | Validates options and starts one isolated local run; HTTP 202 |
| GET | `/api/v1/configuration` | Active storage/listener settings and a TOML export |
| POST | `/api/v1/configuration/validate` | Validates a draft and returns TOML; does not modify active settings |

Benchmark submission takes JSON. Omitted fields use defaults; unknown fields,
invalid bounds and overlapping runs are rejected. Example:

```json
{"workload":"suite","file_size":16777216,"files":8,"concurrency":4,"operations":1000,"iterations":3,"warmups":1,"replications":[1,3],"block_size":4194304,"seed":42}
```

Poll GET for `active.state` (`running`, `succeeded`, `failed`) and `reports`.
`active` tracks dashboard submissions during this service session; successful CLI
reports using the same local root also appear in `reports`. A failed run includes
an error and is not published as a successful report. Both runners share an OS
lock. The service drains accepted dashboard runs on graceful shutdown.

Configuration validation requires `block_size`, `inline_threshold`, `replication`,
`ui_listen` and `s3_listen`. Optional `read_cache_size`, `compute_memory_budget` and
`spill_directory` fields use their documented defaults when omitted. Responses include
`settings`, `toml`, `restart_required` and explanatory `notes`. Use the downloaded
file with `--config` on restart. The API reports only settings used by the local
service; it does not represent planned security or distributed features as active.

See [benchmark methodology and results](/ops/benchmarks/).

## S3 API

The gateway serves an S3-compatible API on `:9000`: `ListObjectsV2`,
`GetObject` (with `Range`), `PutObject`, `DeleteObject(s)`, `CopyObject`, bucket
operations, ETags and upload checksum verification. Multipart uploads, AWS chunked
uploads, SigV4 authentication, IAM, versioning, ACLs and encryption are not
implemented.

## Rust SDK

Published as `mammoth-client`. Generated rustdoc goes up with the crate, at
[docs.rs/mammoth-client](https://docs.rs/mammoth-client) — live once `v0.1.0`
is on crates.io. Until then, `cargo doc --open -p mammoth-client` builds it
locally from a checkout.
