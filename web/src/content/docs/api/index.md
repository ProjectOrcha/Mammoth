---
title: HTTP API
description: The endpoints the CLI and the Web UI both consume.
---

The CLI is just another client of this API. Anything the UI can show, a script
can fetch — which is what stops the two from drifting apart.

```
GET  /api/v1/cluster/report
GET  /api/v1/nodes
GET  /api/v1/nodes/:id
GET  /api/v1/fs?path=/data&limit=200
GET  /api/v1/fs/blocks?path=/data/sales.csv     ← block placement matrix
GET  /api/v1/distribution/heat?metric=usage     ← node heatmap
GET  /api/v1/distribution/treemap?path=/&depth=3
GET  /api/v1/distribution/skew?path=/warehouse
GET  /api/v1/distribution/topology
GET  /api/v1/events                             ← SSE
POST /api/v1/admin/decommission
```

`/api/v1/events` is Server-Sent Events, carrying `node_state`, `block_health`,
`throughput`, `job_update` and `alert`.

## S3 API

The gateway serves an S3-compatible API on `:9000`: `ListObjectsV2`,
`GetObject` (with `Range`), `PutObject`, `DeleteObject(s)`, `CopyObject`,
multipart upload, and SigV4 verification.

## Rust SDK

Published as `mammoth-client`. Generated rustdoc is at
[`/mammoth/rustdoc/`](/mammoth/rustdoc/).
