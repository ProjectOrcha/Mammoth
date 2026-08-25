---
title: Hadoop architecture in 10 minutes
description: Blocks, replication, NameNode and DataNode, the write pipeline, YARN and MapReduce — the whole model, without the JVM.
sidebar:
  order: 2
---

You cannot simplify something you do not understand. This is the short version of
Hadoop architecture. If you already know it, skip ahead to [What is Mammoth?](/Mammoth/intro/what/).


## 1.1 The problem Hadoop solves

You have a 10 TB file. No single computer has 10 TB of fast disk, and even if it did, reading it at 200 MB/s takes **14 hours**.

The idea: chop the file into pieces, put the pieces on 100 machines, and have all 100 machines read their piece _at the same time_. Now it takes 8 minutes.

That's it. That's the whole idea. Everything else is bookkeeping.

```
One machine:                     100 machines:
┌──────────────┐                 ┌────┐┌────┐┌────┐      ┌────┐
│  10 TB file  │  14 hours       │100G││100G││100G│ ...  │100G│   8 minutes
└──────────────┘                 └────┘└────┘└────┘      └────┘
```

The bookkeeping problems this creates:

1. **Where is each piece?** → you need an index.
2. **What if a machine dies?** → you need copies.
3. **How do you run code on 100 machines?** → you need a scheduler.

Hadoop's three answers: **HDFS** (storage + index), **replication** (copies), **YARN** (scheduler).

## 1.2 Blocks — how a file is chopped up

A file is split into fixed-size **blocks**. Hadoop's default is 128 MB.

```
sales.csv  (350 MB)
│
├── block 1  ──  128 MB
├── block 2  ──  128 MB
└── block 3  ──   94 MB    ← last block is partial, not padded
```

Each block gets a globally unique ID. The file itself becomes nothing more than an **ordered list of block IDs**:

```
/data/sales.csv → [blk_1001, blk_1002, blk_1003]
```

To read the file, you fetch those three blocks in order and concatenate them.

**Why 128 MB and not 4 KB?** Because the index has to remember every block. A 4 KB block size on a 10 TB file means 2.5 billion index entries — the index won't fit in RAM. Bigger blocks = smaller index. The tradeoff is that a 1 KB file still occupies one block _entry_ in the index (though not 128 MB of disk). This is the famous **small-file problem**, and we fix it in Mammoth (§6.3).

## 1.3 Replication — surviving machine death

Every block is stored on **3 different machines** by default. If one dies, two copies remain, and the system quietly makes a third copy somewhere else.

```
blk_1001  →  worker-02, worker-07, worker-15
blk_1002  →  worker-03, worker-07, worker-22
blk_1003  →  worker-02, worker-11, worker-22
```

Cost: 10 TB of data uses 30 TB of disk. (Erasure coding later reduces this to ~15 TB — see §6.6.)

**Rack awareness.** Machines live in racks. A whole rack can lose power at once. So the placement rule is:

```
replica 1 → same machine as the writer (or nearest)   ← fast write
replica 2 → a machine in a DIFFERENT rack             ← survives rack failure
replica 3 → a different machine in the SAME rack as replica 2   ← cheap, one cross-rack hop
```

You get rack-failure survival while only sending data across the (expensive, slow) rack-to-rack link once instead of twice.

## 1.4 NameNode and DataNode — the librarian and the shelves

|Role|Hadoop name|What it actually does|
|---|---|---|
|**The index**|NameNode|Remembers the directory tree, file→block mapping, permissions. Holds it all **in RAM**.|
|**The shelves**|DataNode|Stores actual block bytes on local disks. Knows nothing about filenames.|

Think of a library: the **card catalog** (NameNode) tells you shelf 12, row 4. The **shelves** (DataNodes) hold the books. The catalog never holds a book; the shelves never hold the catalog.

Reading a file is a two-step dance:

```
1. client → NameNode:  "where is /data/sales.csv?"
   NameNode → client:  "blk_1001 on [w2,w7,w15], blk_1002 on [w3,w7,w22], ..."

2. client → w2:  "give me blk_1001"        ← data never touches the NameNode
   client → w3:  "give me blk_1002"           (this is why it scales)
```

**Critical detail:** the NameNode never sees file data. It only handles metadata. That's what lets one NameNode serve thousands of DataNodes.

**How the NameNode learns where blocks are:** it doesn't store that on disk. Every DataNode sends a **block report** on startup ("I have these 4 million blocks") and a **heartbeat** every 3 seconds ("still alive, here's my free space"). The block→location map is rebuilt in RAM every time the NameNode restarts. That's why big Hadoop clusters take 30+ minutes to boot.

**Safe mode:** on startup the NameNode refuses writes until enough DataNodes have reported in that it's confident it knows where 99.9% of blocks live. Otherwise it'd think blocks are missing and start replicating like crazy.

## 1.5 The write path — the replication pipeline

Writing is more interesting than reading. The client does **not** send the data three times.

```
client ──64KB packet──▶ w2 ──forward──▶ w7 ──forward──▶ w15
       ◀─────ack───────    ◀────ack───    ◀────ack────
```

The client sends each 64 KB packet **once**, to the first DataNode. That node writes it to disk _and simultaneously_ forwards it to the second, which forwards to the third. Acks flow back down the chain. This is **chain replication**, and it means the client's upload bandwidth isn't tripled.

**Lease:** while a file is open for writing, the client holds a _lease_ (an exclusive lock with a timeout). If the client crashes, the lease expires and the NameNode recovers the half-written file. This is how HDFS gets single-writer semantics without distributed locks.

## 1.6 YARN and MapReduce — the compute half

**YARN** is the job scheduler. Its key insight is **data locality**: don't move 128 MB of data to the code, move the 5 KB of code to the machine that already has the data.

```
ResourceManager  ← the scheduler (one per cluster)
NodeManager      ← the agent on each machine that launches tasks
ApplicationMaster← a per-job coordinator that YARN itself launches (yes, really)
Container        ← a CPU+memory allocation on one machine
```

**MapReduce** is the original programming model. Word count, the "hello world":

```
Input:  "the cat sat on the mat"

MAP     (runs on each block, in parallel)
        → (the,1) (cat,1) (sat,1) (on,1) (the,1) (mat,1)

SHUFFLE (the expensive part — network sort, groups by key)
        → the:[1,1]  cat:[1]  sat:[1]  on:[1]  mat:[1]

REDUCE  (runs per key group, in parallel)
        → (the,2) (cat,1) (sat,1) (on,1) (mat,1)
```

The **shuffle** is where MapReduce jobs spend most of their time — it's an all-to-all network transfer plus a disk sort. Every performance conversation about Hadoop eventually becomes a conversation about shuffle.

## 1.7 Why Hadoop is slow and complicated

Now the important part — what you're actually fixing.

|Problem|Why it hurts|
|---|---|
|**JVM + garbage collection**|A NameNode with 200 GB heap can pause for _seconds_ during a full GC. Everything stalls.|
|**One global lock**|The NameNode serializes nearly all namespace operations behind `FSNamesystem`'s lock. One slow op blocks thousands.|
|**Metadata in RAM only**|Namespace size is capped by one machine's RAM. ~400 bytes/file means 100M files ≈ 40 GB heap.|
|**Small-file problem**|1 million 10 KB files consume the same index space as 1 million 128 MB files. Clusters die from this.|
|**6+ XML config files**|`core-site.xml`, `hdfs-site.xml`, `yarn-site.xml`, `mapred-site.xml`... with 1000+ tunable properties.|
|**ZooKeeper + JournalNodes + ZKFC**|HA requires _three additional distributed systems_ just to fail over one NameNode.|
|**Kerberos**|Security is all-or-nothing and famously painful to configure.|
|**Full block reports**|A 10M-block DataNode reporting in can pause the NameNode for seconds.|
|**Slow startup**|Rebuilding the block map from reports takes 30+ minutes on large clusters.|
|**Java everywhere**|A separate script for each subsystem: `hdfs`, `yarn`, `mapred`, `hadoop`.|

## 1.8 Translation table — keep this handy

|Hadoop says|Mammoth says|Plain English|
|---|---|---|
|NameNode|**master**|the index|
|DataNode|**worker**|the shelves|
|Secondary NameNode|_(gone)_|a checkpoint helper we don't need|
|JournalNode / QJM / ZooKeeper / ZKFC|_(gone)_|HA plumbing, replaced by built-in Raft|
|ResourceManager|**master** (scheduler module)|the job scheduler|
|NodeManager|**worker** (executor module)|the per-machine task launcher|
|ApplicationMaster|_(gone)_|a per-job coordinator we don't need|
|Container|**slot**|a CPU+RAM reservation|
|fsimage + edits|Raft snapshot + Raft log|how the index survives restarts|
|Block|**block**|a 128 MB chunk of a file|
|Block report|**block report**|"here's everything I'm storing"|
|Safe mode|**safe mode**|read-only until the index is trustworthy|
|Rack awareness|**topology**|which machines share a failure domain|
|`hdfs dfs -ls /`|`mammoth ls /`|list a directory|
