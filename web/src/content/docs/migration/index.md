---
title: Migration
description: Getting data out of Hadoop, and upgrading Mammoth itself.
---

Two different meanings of "migration". You need both.

Two different meanings of "migration". You need both.

### 11.1 Getting data out of Hadoop

```bash
# 1 · analyze the source, move nothing
mammoth migrate plan \
    --source hdfs://namenode:8020/user/warehouse \
    --source-auth kerberos --principal u@REALM --keytab /etc/u.keytab \
    --dest /warehouse \
    --include '*.parquet' --exclude '*/_temporary/*' \
    --preserve perms,owner,times,acl,xattr \
    --out plan.json
```

```
  scanned    4.2M files · 890 TB
  skew       12 hotspot directories flagged
  small files 1.1M files under 1 MB → will be inlined (saves ~1.1M metadata entries)
  estimate   9h 12m at 20 GB/s
  ✔ plan.json written
```

```bash
# 2 · run it — resumable, throttled, idempotent
mammoth migrate run --plan plan.json \
    --parallelism 128 --bandwidth-limit 10Gbps \
    --checkpoint .mammoth-migrate.db \
    --on-conflict skip|overwrite|newer \
    --dry-run                    # drop this to actually run

# 3 · resume after any interruption
mammoth migrate resume --checkpoint .mammoth-migrate.db

# 4 · verify without re-reading the data twice
mammoth migrate verify --plan plan.json --mode checksum --report verify.html

# 5 · incremental catch-up
mammoth migrate sync --plan plan.json --since-last-run

# 6 · cutover
mammoth migrate cutover --mode dual-write --duration 72h
mammoth migrate cutover --finalize

# other sources
mammoth migrate run --source s3://bucket/prefix --source-region ap-south-1 --dest /data
mammoth migrate run --source file:///mnt/nas --dest /archive

# rewrite Hive metastore paths
mammoth migrate metastore --hive-uri thrift://hms:9083 \
    --rewrite 'hdfs://namenode:8020/=s3://mammoth/' --databases sales,logs --dry-run
```

**Implementation notes:**

- `trait MigrationSource { fn walk(&self) -> Stream<Item = SourceEntry>; fn open(&self, p) -> AsyncRead; }` with impls for WebHDFS, S3, GCS, local FS.
- Checkpoint in a local `redb`/SQLite: `(source_path, state, bytes_done, checksum, attempts)`. Re-running skips `Done` rows — idempotent by construction.
- Large files split into parallel range reads so a 1 TB file isn't single-threaded.
- **Implement HDFS's `MD5-of-MD5-of-CRC32C` composite checksum** so you can compare source and destination without a second full read. This is the feature that makes people trust the migration.
- Stream live progress to the Web UI too — migration is a great UI demo.

### 11.2 Upgrading Mammoth itself

```bash
mammoth admin upgrade check                    # what would change
mammoth admin upgrade start --to 0.4.0 --dry-run
mammoth admin upgrade start --to 0.4.0         # rolling, one node at a time
mammoth admin upgrade status
mammoth admin upgrade finalize                 # point of no return
mammoth admin rollback --to 0.3.2              # valid only before finalize

mammoth admin metadata backup  --out meta-2026-08-25.snap
mammoth admin metadata restore --in  meta-2026-08-25.snap
```

Rules: bump `layout_version` in `VERSION` on any on-disk change; keep the old layout via hardlinks (free) until `finalize`; a node refuses to start against a newer layout than it understands, with a message that says exactly what to do.
