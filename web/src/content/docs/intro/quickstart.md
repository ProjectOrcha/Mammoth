---
title: 5-minute local service
description: Build Mammoth and run a persistent local filesystem with a live dashboard.
sidebar:
  order: 4
---

On `AI_coded`, the guided local service runs with real persistent files.
Six worker directories simulate three racks on one host. This is a development
service; separate distributed workers, Raft and HA remain future work.

## Build and run

Requires Rust 1.85+ and Node 22.x. From the repository root:

```bash
npm --prefix ui ci
npm --prefix ui run build
cargo build --locked -p mammoth-cli
./target/debug/mammoth --local-root .mammoth quickstart
```

Open [the dashboard](http://127.0.0.1:8080). The S3 endpoint listens on
`127.0.0.1:9000`. Press Ctrl-C to stop; restart with the same root to keep data.
Build the dashboard before the Rust binary, because its files are embedded.

In another terminal:

```bash
export MAMMOTH_LOCAL_ROOT="$PWD/.mammoth"
./target/debug/mammoth ls /sample
./target/debug/mammoth put README.md /sample/readme.md
./target/debug/mammoth cat /sample/readme.md
./target/debug/mammoth viz blocks /sample/blocks.bin --output table
./target/debug/mammoth top
```

Small files live directly in the namespace snapshot. Larger files use immutable
checksummed blocks replicated across the simulated racks. Read errors on one
copy cause the backend to try another copy. `mammoth admin repair` restores
bad or missing copies from a checked source.

## DuckDB

Upload a local Parquet file with a path-style, unsigned S3 client, then query it:

```sql
INSTALL httpfs;
LOAD httpfs;
CREATE SECRET mammoth (
  TYPE S3,
  ENDPOINT '127.0.0.1:9000',
  URL_STYLE 'path',
  USE_SSL false
);
SELECT count(*) FROM read_parquet('s3://warehouse/*.parquet');
```

The development S3 subset supports objects, buckets, listings and byte ranges.
It does not implement multipart upload or authentication. Keep the default
loopback listeners for local use.
