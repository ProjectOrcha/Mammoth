# Chapter 4 — Understanding the Backend trait

**What you'll build:** nothing. This chapter is reading and one experiment.

**Time:** 30 minutes. **Read this as a team.**

---

## Before you start

```markdown
- [ ] Chapter 1 is read — especially §5, "Traits: the shape of a thing"
- [ ] All three of you are in the same room, or on the same call
```

**Read this one together.** It is thirty minutes and it is the only chapter that
is worth doing as a group. The `Backend` trait is the contract between all three
tracks — storage, CLI, web — and if two of you have different mental models of
it, you will not find out until week four, at a merge conflict.

### What you will have open

```
crates/mammoth-core/src/
├── backend.rs      the trait itself — the whole chapter is about this file
├── types.rs        FileStatus, BlockPlacement, Replica, ClusterReport
└── error.rs        the Error type and the Result alias
```

You will not change any of them. You will read `backend.rs` line by line, and
then write one throwaway implementation of it to feel what the compiler makes
you do.

---

Everything else in this guide depends on getting this one idea right. It is
worth half an hour before anyone writes code.

## The problem it solves

Mammoth is going to be a distributed system: masters, workers, a network,
replication, failure. That takes months to build.

But the parts anyone will *judge* the project by — the CLI, the visualizations,
the web UI — are also the parts that are cheapest to get wrong and most
expensive to fix late.

If you build bottom-up, the first thing you can demo arrives around week 30, and
every design mistake you find then costs a rewrite.

## The trick

Define **one trait**. Write **two implementations**.

```rust
// crates/mammoth-core/src/backend.rs
#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>>;
    async fn stat(&self, path: &Path) -> Result<FileStatus>;
    async fn read(&self, path: &Path, range: Range<u64>) -> Result<ByteStream>;
    async fn write(&self, path: &Path, data: ByteStream) -> Result<()>;
    async fn remove(&self, path: &Path, recursive: bool) -> Result<()>;
    async fn block_layout(&self, path: &Path) -> Result<Vec<BlockPlacement>>;
    async fn cluster_report(&self) -> Result<ClusterReport>;
}
```

- **Now (chapters 5–6):** `LocalBackend`. Blocks are files in a directory.
  "Workers" are six subdirectories. It all runs on your laptop.
- **Later (milestone M5):** `ClusterBackend`. Real masters, real workers, real
  network, real replication.

The CLI and the UI are written against `Backend`, so **when you swap the
implementation, not one line of them changes.**

You get something you can demo in week 8 instead of week 30, and you find your
UX problems while they are still cheap.

The reasoning is written up as [ADR 0002](../adr/0002-backend-trait.md).

## Reading the trait, method by method

Open the real file and follow along:

```bash
cat crates/mammoth-core/src/backend.rs
```

### `list` and `stat` — the cheap ones

```rust
async fn list(&self, path: &Path) -> Result<Vec<FileStatus>>;
async fn stat(&self, path: &Path) -> Result<FileStatus>;
```

`list` gives the direct children of a directory. `stat` gives one path's
metadata. Together they are `mammoth ls`, `mammoth du`, `mammoth find`, and the
file browser in the web UI.

These are **metadata only** — they never touch file bytes. In a real cluster
they hit the master and return in microseconds. That separation is the whole
reason one master can serve thousands of workers.

### `read` and `write` — the data path

```rust
async fn read(&self, path: &Path, range: Range<u64>) -> Result<ByteStream>;
async fn write(&self, path: &Path, data: ByteStream) -> Result<()>;
```

Both deal in **streams**, not `Vec<u8>`. That matters: a Mammoth file can be
10 TB, and no machine can hold that in memory.

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;
```

Read that from the inside out:

| Piece | Meaning |
| --- | --- |
| `Bytes` | a chunk of bytes that is cheap to clone and slice (it is refcounted) |
| `Result<Bytes>` | each chunk might fail — a disk can die mid-read |
| `dyn Stream<Item = ...>` | an async sequence of those chunks |
| `Box<...>` | on the heap, because the concrete type varies |
| `Pin<...>` | required by async internals. Just write `Box::pin(...)` |

`range` on `read` lets you fetch bytes 1,000,000 to 1,000,100 of a 10 TB file
without reading the rest. That is what makes `mammoth head` fast, and it is what
S3's `Range` header maps onto.

> **In chapter 6 you will cheat** and buffer whole files in memory, because
> `LocalBackend` only ever handles test data. The signature is still a stream so
> that `ClusterBackend` can do it properly later without changing any caller.

### `remove`

```rust
async fn remove(&self, path: &Path, recursive: bool) -> Result<()>;
```

`recursive` is the `-r` in `rm -r`. Without it, removing a non-empty directory
must fail — the same safety `rm` gives you.

### `block_layout` — the interesting one

```rust
async fn block_layout(&self, path: &Path) -> Result<Vec<BlockPlacement>>;
```

**This method is why Mammoth is worth building.** It answers "where did my file
actually go?" — and it is what `mammoth viz blocks` and the web UI's block
matrix are built on. Hadoop makes you dig for this. Mammoth makes it a verb.

```rust
pub struct BlockPlacement {
    pub id: BlockId,
    pub index: u32,          // position within the file
    pub len: u64,            // last block is partial, not padded
    pub replicas: Vec<Replica>,
}

pub struct Replica {
    pub node: NodeId,        // "w3"
    pub rack: String,        // "/dc1/rack-b"
    pub state: ReplicaState, // Primary | Replica | Corrupt
}
```

### `cluster_report`

```rust
async fn cluster_report(&self) -> Result<ClusterReport>;
```

One snapshot of the whole cluster: every node, its capacity, its health, plus
replication accounting. It powers `mammoth viz cluster`, `mammoth top`,
`mammoth df`, and the web UI's overview page.

## Two details that trip people up

### `Send + Sync`

```rust
pub trait Backend: Send + Sync {
```

`Send` means "safe to move to another thread". `Sync` means "safe to share
between threads". The web server handles many requests at once on many threads,
so any `Backend` must be both. If you later store something non-thread-safe in
`LocalBackend` (like `Rc` or `RefCell`), the compiler will refuse — and this
line is why.

### `&self`, not `&mut self`

Every method takes `&self`. A `Backend` is **shared, not mutated**. Many readers
can call it at once and none of them block each other.

That is not an accident. It is the same design as the real thing: Hadoop's
NameNode serializes almost everything behind one lock, so a single slow
`listStatus` on a huge directory stalls thousands of clients. Mammoth's
namespace is immutable behind an `ArcSwap` — writers build a new version and
swap a pointer, readers never wait. `&self` is that promise, written in the type
system.

## Try it: the smallest possible Backend

Understanding beats reading. Write a fake backend that serves one hard-coded
file, and see the trait click.

Create `crates/mammoth-core/src/demo.rs`:

```rust
//! Scratch: delete this file after chapter 4.

use std::ops::Range;
use std::path::Path;

use bytes::Bytes;

use crate::backend::{Backend, ByteStream};
use crate::error::{Error, Result};
use crate::types::*;

/// A read-only backend that serves exactly one hard-coded file.
pub struct HelloBackend;

#[async_trait::async_trait]
impl Backend for HelloBackend {
    async fn list(&self, _path: &Path) -> Result<Vec<FileStatus>> {
        Ok(vec![self.stat(Path::new("/hello.txt")).await?])
    }

    async fn stat(&self, path: &Path) -> Result<FileStatus> {
        Ok(FileStatus {
            path: path.to_path_buf(),
            is_dir: false,
            len: 5,
            block_size: 128 * 1024 * 1024,
            replication: Some(1),
            blocks: 0,
            inlined: true,
            mode: 0o644,
            owner: "you".into(),
            group: "you".into(),
            modified: 0,
            checksum: None,
        })
    }

    async fn read(&self, _path: &Path, _range: Range<u64>) -> Result<ByteStream> {
        let chunk = Bytes::from_static(b"hello");
        Ok(Box::pin(futures_util::stream::once(async move { Ok(chunk) })))
    }

    async fn write(&self, path: &Path, _data: ByteStream) -> Result<()> {
        Err(Error::WrongKind {
            path: path.to_path_buf(),
            actual: "read-only demo backend",
            expected: "a writable backend",
        })
    }

    async fn remove(&self, _path: &Path, _recursive: bool) -> Result<()> {
        Ok(())
    }

    async fn block_layout(&self, _path: &Path) -> Result<Vec<BlockPlacement>> {
        Ok(Vec::new())
    }

    async fn cluster_report(&self) -> Result<ClusterReport> {
        Ok(ClusterReport {
            name: "demo".into(),
            leader: None,
            safe_mode: false,
            used: 5,
            capacity: 1024,
            nodes: Vec::new(),
            health: ReplicationHealth::default(),
        })
    }
}
```

Add two lines to `crates/mammoth-core/src/lib.rs`:

```rust
pub mod demo;
```

and add `futures-util = { workspace = true }` to
`crates/mammoth-core/Cargo.toml` under `[dependencies]`.

Then:

```bash
cargo build -p mammoth-core
```

It compiles. You have a working `Backend`. **Now delete it** — it has done its
job:

```bash
rm crates/mammoth-core/src/demo.rs
```

and undo the two edits.

### The point of that exercise

Notice what the compiler forced you to do: implement **all seven methods**. You
cannot half-implement a trait. That is the guarantee that makes swapping
`LocalBackend` for `ClusterBackend` safe — if the new one compiles, it does
everything the old one did.

## Check you understand it

Answer these before moving on. If you cannot, re-read the section named.

1. Why does `read` return a stream instead of `Vec<u8>`? *(the data path)*
2. Why does every method take `&self` rather than `&mut self`? *(`&self`)*
3. Which method does `mammoth viz blocks` depend on? *(`block_layout`)*
4. If you added an eighth method to the trait, what would break? *(every
   implementation stops compiling until it implements it — which is the point)*
5. Where does `ByteStream` come from, and what does `Box::pin` do?

## Done when

This chapter produces no committed code, so the checklist is about
understanding — and about agreement, which matters more.

Individually:

```markdown
- [ ] I read `crates/mammoth-core/src/backend.rs` top to bottom
- [ ] I wrote the throwaway `Backend` implementation and it compiled
- [ ] I can answer all five questions above without looking
- [ ] I can name the seven methods, roughly
- [ ] I can explain to someone why the CLI talks to `&dyn Backend` rather than
      to `LocalBackend`
```

As a team:

```markdown
- [ ] All three of us agree the trait as written is the one we are building against
- [ ] Anything we want to change is an open issue **now**, not a conversation in week 4
- [ ] We know which methods chapter 5 implements (`list`, `stat`) and which wait
      for chapter 6 (the other five)
- [ ] Ben and Cai know they can start their chapters against
      [fake data](TEAM-PLAN.md#nobody-waits-work-against-fake-data) before Ana
      finishes
```

That last box is what stops two thirds of the team idling for a week. The trait
fixes every method signature the moment it compiles — so the CLI, the
visualizations and the web UI can all be built against those signatures while
the bodies are still stubs.

## If it went wrong

**`` error[E0046]: not all trait items implemented ``** — you missed a method.
The error lists exactly which ones. This is the trait doing its job.

**`` error: the trait cannot be made into an object ``** — you forgot
`#[async_trait::async_trait]` above `impl`. It must be on the trait *and* on
every `impl` block.

**`` error[E0432]: unresolved import `futures_util` ``** — you did not add the
dependency to `Cargo.toml`. Every crate declares its own dependencies, even if
another crate in the workspace already uses it.

**`` future cannot be sent between threads safely ``** — you put something
non-thread-safe inside your backend. Use `Arc<Mutex<T>>` instead of
`Rc<RefCell<T>>`.

**`warning: missing documentation for a struct`** — `mammoth-core` sets
`#![warn(missing_docs)]`, so every `pub` item needs a `///` comment. Add one.
This lint is on deliberately: `mammoth-core` is the vocabulary every other crate
reads, so it is the one place where undocumented public items are not acceptable.

---

**Next:** [Chapter 5 — LocalBackend, part 1](05-localbackend-part-1.md)
