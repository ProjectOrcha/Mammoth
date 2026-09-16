---
title: Files
description: File commands supported by Mammoth's local store and HTTP gateway.
sidebar:
  order: 2
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


These examples use the current CLI. See the [generated reference](/cli/reference/)
for every accepted argument. Set `MAMMOTH_LOCAL_ROOT` to the same store as your
running dashboard, or pass `--masters http://127.0.0.1:8080`.

## List and inspect

```bash
mammoth ls /sample
mammoth stat /sample/hello.txt
mammoth find /sample --name hello
mammoth du /sample
mammoth df
mammoth checksum /sample/hello.txt
```

`ls` lists direct children. `find --name` matches a substring, not a shell glob.
`du` totals logical file bytes in a subtree. `df` reports stored replica bytes
and simulated worker capacities. Small files are stored inline in metadata.

## Upload and download

```bash
mammoth put ./report.txt /data/report.txt
mammoth put ./large.bin /data/large.bin --replication 3 --block-size 1MiB
mammoth get /data/report.txt ./downloaded-report.txt
mammoth get /data/report.txt ./downloaded-report.txt --force
```

Use a complete destination filename. Uploads create missing parent folders and
replace existing file contents. Downloads refuse to replace a local file unless
`--force` is supplied. `put - /data/input.txt` reads from standard input.

If the source is empty (0 bytes), `put` stops before changing the destination.
Choose a complete local copy if you expected data. Add `--allow-empty` when an
empty file is intentional; this also applies to empty standard input. The web
dashboard asks for confirmation when the selected local file is empty.

Per-upload `--block-size` is available for local CLI access. With HTTP access,
configure the gateway's block size. Erasure-coding policies and recursive `put`
are not implemented; use [tree transfers](/cli/operations/#tree-transfers) for directories.

## Read file contents

```bash
mammoth cat /sample/hello.txt
mammoth head /sample/words.txt -n 5
mammoth tail /sample/words.txt -n 5
```

These commands preserve the original bytes and line endings. `tail -f`,
`cat --range`, and byte-count flags are not accepted by this build.

## Organize folders and files

```bash
mammoth mkdir /data
mammoth mkdir -p /archive/2026
mammoth cp /data/report.txt /archive/2026/report.txt
mammoth cp -r /archive/2026 /archive/backup
mammoth mv /data/report.txt /data/renamed.txt
mammoth rm /data/renamed.txt
mammoth rm -r /archive/backup
```

Copy and move destinations are complete paths. Moves reject existing
destinations. Copying a directory requires `-r`. Removing a non-empty folder
also requires `-r`; removal is permanent.

## Metadata and replication

```bash
mammoth chmod 640 /archive/2026/report.txt
mammoth chown analytics:data /archive/2026/report.txt
mammoth setrep 2 /archive/2026/report.txt
mammoth viz blocks /archive/2026/report.txt --output table
```

Permissions use octal digits. Owner, group, and permissions are descriptive in
local storage. `setrep` changes the number of whole replicas; it does not convert
to erasure coding. Inline files live in the namespace and have no block replicas.

## In the dashboard

The **Files** page lets you upload, download, create folders, filter entries,
rename or move paths, delete files or folders, and edit file properties. Open a
file to inspect its [block placement](/cli/viz/#viz-blocks). Large layouts have
Previous/Next controls so every block is reachable.
