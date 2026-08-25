---
title: Performance
description: Lock-free metadata reads, short-circuit reads, zero-copy, io_uring, hardware CRC32C, quorum acks, hedged reads.
---

Explained so you know *why*, not just *what*. Ordered by impact.

Explained so you know _why_, not just _what_. Ordered by impact.

### 1 · Lock-free metadata reads — the big one

**Hadoop's problem:** the NameNode guards nearly the whole namespace with one lock (`FSNamesystem`). One slow `listStatus` on a directory with 500k entries blocks every other client.

**Mammoth's fix:** the Raft state machine is single-writer by construction. So keep the namespace in an **immutable tree** behind `arc_swap::ArcSwap`. Writers build a new version (structural sharing — only the changed path is copied) and atomically swap the pointer. Readers do `Arc::clone` and never block, ever.

```rust
pub struct Namespace {
    current: ArcSwap<NamespaceSnapshot>,   // readers: lock-free
}
impl Namespace {
    pub fn read(&self) -> Arc<NamespaceSnapshot> { self.current.load_full() }
    fn apply(&self, op: Op) {                       // single writer, from Raft
        let next = self.current.load().with(op);    // COW
        self.current.store(Arc::new(next));
    }
}
```

**Expected effect:** metadata read throughput scales with cores instead of flatlining. This is the mechanism behind a "5–10× faster metadata" claim — and you can benchmark it.

### 2 · Short-circuit local reads

If a replica lives on the same machine as the reader, don't use the network at all. The worker passes the **open file descriptor** over a Unix domain socket (`SCM_RIGHTS`), and the client `pread`s the file directly.

Network hop: gone. Copy: gone. For co-located compute this is often a 3–5× read win. Crates: `passfd` / `sendfd`, `nix`.

### 3 · Zero-copy data path

- `bytes::Bytes` everywhere — reference-counted buffers, slicing is free
- `sendfile()` / `splice()` for serving whole blocks straight from page cache to socket — the bytes never enter userspace
- `writev()` for scatter-gather so header + payload go out in one syscall

```rust
#[cfg(target_os = "linux")]
nix::sys::sendfile::sendfile(socket.as_raw_fd(), file.as_raw_fd(), Some(&mut off), len)?;
```

### 4 · `io_uring` for disk I/O

`tokio-uring` or `monoio`. Batches syscalls and supports registered buffers. **2–3× on small random reads.** Gate behind a feature flag with an epoll fallback — `io_uring` needs kernel 5.10+ and some hosts disable it.

### 5 · Hardware CRC32C

Data integrity requires checksumming every byte. Software CRC32 runs at ~400 MB/s and becomes your bottleneck. The `crc32c` crate uses the SSE4.2 / ARMv8 CRC instruction and hits **~20 GB/s**. Same algorithm HDFS uses, so checksums stay comparable during migration.

### 6 · Quorum acks on writes

Default HDFS waits for all 3 replicas. If one disk hiccups, the client waits. With `ack_policy = "quorum"`, ack after 2 of 3 are durable and repair the third asynchronously. **Cuts p99 write latency substantially** at a small durability cost — make it configurable, default to quorum, document the tradeoff honestly.

### 7 · Hedged reads

Same idea on the read side: if replica 1 hasn't responded within `p99 × 1.5`, fire the same request at replica 2 and take whichever returns first. Kills tail latency caused by one slow disk.

### 8 · Digest-based block reports

Instead of a 10-million-entry full report, each worker keeps a rolling `xxhash3` digest over its sorted block-ID set and sends it every heartbeat, plus any incremental changes. The master compares digests; only on mismatch does it request a full report — streamed in 10k chunks with yields between them. **Removes the multi-second metadata pauses.**

### 9 · Thread-per-core sharding

Pin storage-path threads with `core_affinity` and shard the block map by `block_id % num_cores`. Each core owns its shard, so there's no cross-core cache-line contention on the hottest data structure.

### 10 · Everything else worth doing

|Technique|Why|
|---|---|
|`mimalloc` allocator|measurable on shuffle-heavy paths|
|`rkyv` for hot-path messages|zero-copy deserialization, no protobuf decode cost|
|`foyer` hybrid cache|memory + SSD read cache; don't just trust the page cache|
|`O_DIRECT` for big sequential reads|avoid polluting page cache with data you read once|
|LZ4 for shuffle, Zstd for cold storage|LZ4 is fast enough to be free; Zstd compresses harder|
|Connection pooling + HTTP/2 multiplexing|avoid TCP handshake per request|
|Batch small RPCs|100 `stat`s in one call beats 100 round trips|

### Measuring — don't guess

```bash
cargo flamegraph --bin mammoth -- bench dfsio --write --size 10GB
tokio-console                    # find async tasks that stall the runtime
cargo bench                      # criterion micro-benchmarks
mammoth bench terasort --size 100GB --report bench.json
```

Publish a reproducible benchmark page on the website — harness, hardware spec, raw numbers. Never cherry-pick. A public, repeatable benchmark is your best marketing asset, and a contested one is your worst.
