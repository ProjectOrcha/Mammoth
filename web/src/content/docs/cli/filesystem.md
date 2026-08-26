---
title: Files
description: Every file verb — ls, put, get, cat, rm, stat, du, df, find, setrep and the rest — with what each flag does and what its output means.
sidebar:
  order: 2
---

The file verbs are POSIX. If a flag exists in coreutils and means something
here, it has the same name and the same behaviour: `-l`, `-h`, `-r`,
`--recursive`, `-n`.

Everything on this page accepts the [global flags](/Mammoth/cli/), so any of it
can be piped as JSON.

## `ls` — list a directory

```
mammoth ls [PATH] [-l] [-a] [-h] [-R] [--sort name|size|time] [--reverse] [--limit N]
```

| Flag | Does |
| --- | --- |
| `-l` | Long form: mode, owner, size, policy, block count, modified. |
| `-a` | Include dot-entries. |
| `-h` | Human sizes (`1.2 GB` instead of `1200000000`). Implied by `-l`. |
| `-R` | Recurse. |
| `--sort` | `name` (default), `size`, `time`. |
| `--limit N` | Stop after N entries. Default 1000; the footer says if it truncated. |

```console
$ mammoth ls -l /data
 MODE       OWNER      SIZE     POLICY          BLOCKS  MODIFIED
 -rw-r--r-- analytics  350 MB   replication-3        3  2 days ago
 -rw-r--r-- analytics  1.2 GB   lrc-6-2-2            9  5 days ago
 -rw-r--r-- analytics  8.2 GB   lrc-6-2-2           62  9 days ago
 -rw-r--r-- analytics  4.2 KB   inline               —  1 day ago
 4 files · 9.8 GB
```

**`POLICY` is the column to read.** `inline` means the file is small enough
(under `storage.inline_threshold`) that it never became blocks at all — its
bytes live in the metadata, so `BLOCKS` is `—`. That is the fix for the
small-file problem, and it is why a million tiny files no longer cost a million
block-map entries.

## `put` — upload a local file

```
mammoth put <LOCAL> <REMOTE> [--replication N] [--policy P] [--block-size S]
                             [-r] [-f] [--progress auto|always|never]
```

| Flag | Does |
| --- | --- |
| `--replication N` | Whole copies instead of erasure coding. `--replication 3` = `--policy replication-3`. |
| `--policy P` | `lrc-6-2-2` (default), `rs-6-3`, `replication-N`, `inline`. |
| `--block-size S` | Override for this file only: `--block-size 512MB`. |
| `-r` | Recurse into a local directory. |
| `-f` | Overwrite if the destination exists. Without it, an existing path is an error. |

A trailing `/` on the destination means "into this directory", exactly as `cp`
does:

```console
$ mammoth put ./nyc-taxi.parquet /data/
  ✔ /data/nyc-taxi.parquet   1.2 GB   lrc-6-2-2   9 blocks   4.1s (293 MB/s)
```

Hot data that is read constantly is usually better mirrored than coded —
erasure-coded reads cost CPU whenever a fragment is missing:

```console
$ mammoth put ./dim-customer.parquet /warehouse/dim/ --replication 3
  ✔ /warehouse/dim/dim-customer.parquet   82 GB   replication-3   612 blocks
```

Uploads over a second show a progress bar, and it disappears automatically when
stdout is not a terminal, so CI logs stay clean.

## `get` — download to a local file

```
mammoth get <REMOTE> <LOCAL> [-r] [-f] [--verify]
```

`--verify` re-reads the checksum after writing and fails loudly if it does not
match. It costs a second pass over the data; use it when the copy matters.

```console
$ mammoth get /data/nyc-taxi.parquet ./local.parquet --verify
  ✔ 1.2 GB in 3.8s (316 MB/s)   crc32c ok
```

## `cat`, `head`, `tail` — read without downloading

```
mammoth cat  <PATH> [--range START-END]
mammoth head <PATH> [-n LINES] [-c BYTES]
mammoth tail <PATH> [-n LINES] [-f]
```

`cat` streams to stdout, so it composes with everything you already use:

```bash
mammoth cat /logs/2026-08/app.log | grep -c ERROR
mammoth cat /data/sales-2026.csv --range 0-1048576 > first-mib.csv
```

`tail -f` follows a file that is still being written. It is the fastest way to
watch a job's output land.

**A range read is genuinely a range read** — the client works out which block
holds the offset and asks only that node for only those bytes. It does not pull
the file and throw most of it away.

## `mkdir`, `rm`, `mv`, `cp`

```
mammoth mkdir <PATH> [-p]
mammoth rm    <PATH> [-r] [-f] [--older-than DURATION] [--dry-run]
mammoth mv    <SRC> <DST>
mammoth cp    <SRC> <DST> [-r] [--policy P]
```

`rm` refuses to delete a non-empty directory without `-r`, and the error says
so. `--older-than` takes a duration (`30d`, `12h`, `90m`):

```console
$ mammoth rm /tmp --older-than 30d --dry-run
  would remove 71,204 files · 66.7 TB · oldest 2025-11-04
  nothing was deleted — drop --dry-run to do it
```

**Always `--dry-run` first.** It is the only difference between tidying `/tmp`
and an incident.

`mv` is a metadata operation: it renames an inode and moves no bytes, so it is
instant regardless of file size. `cp` copies data, and can change policy on the
way:

```bash
mammoth cp /warehouse/events /warehouse/archive/events -r --policy rs-6-3
```

## `stat` — everything about one path

```
mammoth stat <PATH> [--blocks]
```

```console
$ mammoth stat /data/nyc-taxi.parquet
  path         /data/nyc-taxi.parquet
  size         1.2 GB
  policy       lrc-6-2-2   (6 data + 2 local + 2 global · 1.67× · survives 3 losses)
  blocks       9 · 128 MiB each
  fragments    90 across 11 nodes
  owner        analytics:data   mode 0644
  modified     2026-08-21 09:14:02
  checksum     crc32c:7b415748
  epoch        41
  degraded     1 fragment rebuilding (blk 3, was on w12)
```

`--blocks` adds the per-block fragment list. For the visual version of the same
information, use [`viz blocks`](/Mammoth/cli/viz/#viz-blocks).

## `du` and `df` — space used, space left

```
mammoth du [PATH] [-h] [--depth N] [--sort size] [--apparent]
mammoth df [-h] [--by-rack]
```

`du` reports **raw bytes on disk** by default — what the data actually costs
after erasure coding or replication. `--apparent` reports logical size, which is
what `ls` shows.

```console
$ mammoth du / --depth 1
 PATH         LOGICAL   ON DISK   POLICY        %
 /warehouse    842 TB   1.40 PB   lrc-6-2-2    68%
 /logs         310 TB    517 TB   lrc-6-2-2    25%
 /tmp           71 TB    142 TB   replication-2 6%
 /user          17 TB     51 TB   replication-3 1%
```

The gap between the two columns is your storage policy, priced. `/user` costs 3×
and `/warehouse` costs 1.67×.

```console
$ mammoth df -h --by-rack
 RACK         CAPACITY  USED    FREE    %    NODES
 /dc1/rack-a    640 TB  418 TB  222 TB  65%  4
 /dc1/rack-b    640 TB  352 TB  288 TB  55%  4
 /dc1/rack-c    720 TB  344 TB  376 TB  48%  3 of 4 · w12 dead
 total          2.0 PB  1.1 PB  886 TB  56%  11 of 12
```

## `find` — search the namespace

```
mammoth find <PATH> [--name GLOB] [--size +N|-N] [--older-than D] [--newer-than D]
                    [--policy P] [--degraded] [--type f|d] [--limit N]
```

This runs on the master against the in-memory namespace, so it is fast even over
millions of paths.

```bash
# Big files that are still mirrored — candidates for erasure coding
mammoth find /warehouse --size +100GB --policy replication-3

# Anything that lost a fragment and has not been rebuilt yet
mammoth find / --degraded

# The classic: what is filling the disk
mammoth find / --size +1GB --older-than 90d --output json | jq -r '.[].path'
```

## `chmod`, `chown` — permissions

```
mammoth chmod <MODE> <PATH> [-R]
mammoth chown <OWNER>[:<GROUP>] <PATH> [-R]
```

POSIX modes, octal or symbolic:

```bash
mammoth chmod 0640 /warehouse/sales -R
mammoth chown analytics:data /warehouse/sales -R
```

## `setrep` — change the durability of existing data

```
mammoth setrep <PATH> --policy P [-R] [--priority low|normal|high] [--dry-run]
```

This is a background conversion, not an instant edit — the data has to be read,
re-encoded and re-written. The command queues it and returns.

```console
$ mammoth setrep /warehouse/archive --policy rs-6-3 -R --dry-run
  412 TB across 3.1M blocks would be re-encoded
  storage        687 TB → 618 TB     (frees ~69 TB)
  repair cost    one loss would read 6 fragments instead of 3
  estimated      9h 40m at the current repair budget
  nothing was queued — drop --dry-run to start
```

**Read the `repair cost` line before you accept the space saving.** `rs-6-3` is
cheaper on disk than `lrc-6-2-2` and twice as expensive to repair, which is the
wrong trade for anything you might have to rebuild in a hurry. See
[the four fast paths](/Mammoth/concepts/fast-paths/#3--declustered-parallel-repair).

Track it afterwards with `mammoth admin ec status`.

## `checksum` — verify without downloading

```
mammoth checksum <PATH> [--verify LOCAL] [--algorithm crc32c|md5-of-md5]
```

```console
$ mammoth checksum /data/nyc-taxi.parquet --verify ./local.parquet
  remote  crc32c:7b415748
  local   crc32c:7b415748
  ✔ identical
```

`--algorithm md5-of-md5` computes HDFS's composite
`MD5-of-MD5-of-CRC32C`, so you can compare a Mammoth file against an HDFS file
without reading either of them twice. That is the check that makes a
[migration](/Mammoth/migration/) trustworthy.

## Common recipes

```bash
# What is in this directory, biggest first
mammoth ls -l /warehouse --sort size --reverse

# Upload a directory tree, three copies, overwriting
mammoth put ./exports /data/exports -r --replication 3 -f

# Copy a subtree to cold storage and re-encode it on the way
mammoth cp /warehouse/events /archive/events -r --policy rs-6-3

# Everything at risk, as JSON, for an alerting script
mammoth find / --degraded --output json | jq 'length'

# Free space by rack, refreshed every 5s
watch -n5 'mammoth df --by-rack'
```
