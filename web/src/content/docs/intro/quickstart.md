---
title: 5-minute cluster
description: One command to a running cluster with sample data and an open browser.
sidebar:
  order: 4
---

```console
$ mammoth quickstart

  Mammoth v0.1.0

  ✔ config written        ~/.mammoth/mammoth.toml
  ✔ data dir created      ~/.mammoth/data
  ✔ started master        127.0.0.1:7000
  ✔ started 3 workers     w1 w2 w3  (simulated, single machine)
  ✔ started gateway       S3 :9000 · UI :8080
  ✔ sample data loaded    /sample/nyc-taxi.parquet (120 MB)

  Web UI  →  http://localhost:8080
  Try     →  mammoth ls /sample
             mammoth viz blocks /sample/nyc-taxi.parquet

  Stop with: mammoth serve stop
```

## Put a file and look at where it went

```console
$ mammoth put ./sales-2026.csv /data/sales-2026.csv
  uploading  ████████████████████████  350 MB / 350 MB  ·  412 MB/s  ·  0s
  ✔ /data/sales-2026.csv   350 MB · 3 blocks · replication 3

$ mammoth viz blocks /data/sales-2026.csv

  /data/sales-2026.csv   350 MB · 3 blocks · replication 3

           w1    w2    w3
  blk 1    ●     ●     ●
  blk 2    ·     ●     ●
  blk 3    ●     ·     ●
```

## Point DuckDB at it

```python
import duckdb
duckdb.sql("SET s3_endpoint='localhost:9000'")
duckdb.sql("SELECT count(*) FROM 's3://sample/*.parquet'")
```

Every command has `--json`, so the same session scripts cleanly:

```console
$ mammoth stat /data/sales-2026.csv --json | jq '.blocks[].replicas'
```
