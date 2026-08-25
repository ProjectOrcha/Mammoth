---
title: What is Mammoth?
description: Distributed storage for large files, with a great CLI, a great UI, and an S3 API.
sidebar:
  order: 1
---

Mammoth stores very large files across many machines, keeps redundant copies so
machine death is survivable, and tells you exactly where everything is.

That is the same job HDFS does. Mammoth does it as **one Rust binary** with **one
TOML config file**, and it speaks the **S3 API**, so the tools you already use
work against it unchanged.

## The translation table

If you know Hadoop, this is the whole vocabulary change:

| Hadoop says | Mammoth says | Plain English |
| --- | --- | --- |
| NameNode | **master** | the index |
| DataNode | **worker** | the shelves |
| Secondary NameNode | *(gone)* | a checkpoint helper we don't need |
| JournalNode / QJM / ZooKeeper / ZKFC | *(gone)* | HA plumbing, replaced by built-in Raft |
| ResourceManager | **master** (scheduler module) | the job scheduler |
| NodeManager | **worker** (executor module) | the per-machine task launcher |
| ApplicationMaster | *(gone)* | a per-job coordinator we don't need |
| Container | **slot** | a CPU+RAM reservation |
| fsimage + edits | Raft snapshot + Raft log | how the index survives restarts |
| Block | **block** | a 128 MB chunk of a file |
| Safe mode | **safe mode** | read-only until the index is trustworthy |
| Rack awareness | **topology** | which machines share a failure domain |
| `hdfs dfs -ls /` | `mammoth ls /` | list a directory |

New to all of this? Read [Hadoop in 10 minutes](/mammoth/intro/hadoop-primer/) first.

## What Mammoth is not

Not a database, not a stream processor, and not — yet — a query engine. The
query layer will be [Apache DataFusion](https://datafusion.apache.org/) over the
Mammoth object store, not a MapReduce reimplementation.
