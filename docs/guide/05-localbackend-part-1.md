# Chapter 5 — LocalBackend, part 1: layout, `list`, `stat`

**What you'll build:** a real `Backend` that stores a namespace on your disk.

**Time:** about 2 hours.

---

By the end of chapter 6 you will have a filesystem that chops files into blocks,
places replicas rack-aware across six simulated workers, and can tell you where
every block went. This chapter builds the skeleton and the two read-only
methods; chapter 6 does the data path.

## The design: fake a cluster with directories

The whole trick of `LocalBackend` is that **a directory can pretend to be a
worker**. Six directories, six workers.

```
~/.mammoth/
├── ns/                          the namespace, mirrored as real directories
│   └── data/
│       ├── hello.txt.mmeta      metadata for /data/hello.txt (JSON)
│       └── sub/                 a real directory = a namespace directory
├── workers/
│   ├── w1/blk_0000000000001001.data
│   ├── w2/blk_0000000000001001.data     ← same block, second replica
│   ├── w3/ w4/ w5/ w6/
└── next-block-id                a counter, so block IDs are unique
```

Two decisions worth understanding:

**Why mirror the namespace as real directories?** Because then `list` is just
`read_dir`, and you can `ls` and `cat` your way around the store while
debugging. When something looks wrong, you can *see* it.

**Why a `.mmeta` sidecar per file?** Because a Mammoth file is not its bytes —
it is a *list of block IDs* plus some metadata. The bytes live on the "workers".
The sidecar is Mammoth's equivalent of an inode.

> This design does not scale, and that is fine. One JSON file per namespace entry
> would sink a real cluster — that is exactly the small-file problem from
> [the Hadoop primer](../../web/src/content/docs/intro/hadoop-primer.md). The
> real master keeps this in memory behind Raft. `LocalBackend` is for
> development, and being inspectable matters more here than being fast.

## Step 1 · Declare the dependencies

Open `crates/mammoth-local/Cargo.toml`. It should already read:

```toml
[dependencies]
mammoth-core = { workspace = true }
async-trait  = { workspace = true }
bytes        = { workspace = true }
futures-core = { workspace = true }
futures-util = { workspace = true }
serde        = { workspace = true }
serde_json   = { workspace = true }
tokio        = { workspace = true }
```

`{ workspace = true }` means "use the version the root `Cargo.toml` pins". That
is how every crate in the workspace stays on the same version of everything.

## Step 2 · The skeleton

Replace the whole of `crates/mammoth-local/src/lib.rs` with this. It is long,
but every piece is used — read the annotations after.

```rust
//! LocalBackend — a whole cluster, simulated on one machine's disk.

#![forbid(unsafe_code)]

use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use mammoth_core::types::{BlockId, NodeId, Replica, ReplicaState};
use mammoth_core::{Error, Result};
use serde::{Deserialize, Serialize};

/// The six workers we pretend to have, and the rack each one sits in.
///
/// Two workers per rack means rack-aware placement has something real to do:
/// replica 2 must land in a different rack from replica 1.
pub const WORKERS: [(&str, &str); 6] = [
    ("w1", "/dc1/rack-a"),
    ("w2", "/dc1/rack-a"),
    ("w3", "/dc1/rack-b"),
    ("w4", "/dc1/rack-b"),
    ("w5", "/dc1/rack-c"),
    ("w6", "/dc1/rack-c"),
];

/// Pretend each worker has 160 GB, so capacity numbers look plausible.
const FAKE_CAPACITY: u64 = 160 * 1024 * 1024 * 1024;

/// Suffix for the per-file metadata sidecar.
const META_SUFFIX: &str = ".mmeta";

/// One block of one file: which block it is, where in the file, how big.
#[derive(Debug, Serialize, Deserialize)]
struct BlockMeta {
    id: u64,
    index: u32,
    len: u64,
}

/// Everything we know about one file. This is Mammoth's inode.
#[derive(Debug, Serialize, Deserialize)]
struct FileMeta {
    len: u64,
    block_size: u64,
    replication: u8,
    inlined: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    inline_data: Option<Vec<u8>>,
    blocks: Vec<BlockMeta>,
    modified: i64,
}

/// A cluster, simulated under one root directory.
pub struct LocalBackend {
    root: PathBuf,
    block_size: u64,
    replication: u8,
    inline_threshold: u64,
}

impl LocalBackend {
    /// Open (and create if needed) a store rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        std::fs::create_dir_all(root.join("ns"))?;
        for (id, _) in WORKERS {
            std::fs::create_dir_all(root.join("workers").join(id))?;
        }
        Ok(Self {
            root,
            block_size: 128 * 1024 * 1024,
            replication: 3,
            inline_threshold: 1024 * 1024,
        })
    }

    /// Override the block size. Handy in tests — 100 bytes beats 128 MB.
    pub fn with_block_size(mut self, bytes: u64) -> Self {
        self.block_size = bytes;
        self
    }

    /// Override the inline threshold.
    pub fn with_inline_threshold(mut self, bytes: u64) -> Self {
        self.inline_threshold = bytes;
        self
    }

    /// `/data/sales.csv` -> `<root>/ns/data/sales.csv`
    fn ns_path(&self, path: &Path) -> PathBuf {
        let rel = path.strip_prefix("/").unwrap_or(path);
        self.root.join("ns").join(rel)
    }

    /// `/data/sales.csv` -> `<root>/ns/data/sales.csv.mmeta`
    fn meta_path(&self, path: &Path) -> PathBuf {
        let p = self.ns_path(path);
        let name = format!("{}{META_SUFFIX}", p.file_name().unwrap_or_default().to_string_lossy());
        p.with_file_name(name)
    }

    /// Where worker `node` keeps block `id`.
    fn block_path(&self, node: &str, id: BlockId) -> PathBuf {
        self.root.join("workers").join(node).join(format!("blk_{:016}.data", id.0))
    }

    /// Load a file's metadata, turning "file not found" into a Mammoth error.
    fn read_meta(&self, path: &Path) -> Result<FileMeta> {
        let raw = std::fs::read(self.meta_path(path)).map_err(|e| match e.kind() {
            ErrorKind::NotFound => Error::NotFound(path.to_path_buf()),
            _ => Error::Io(e),
        })?;
        serde_json::from_slice(&raw).map_err(|e| Error::Config(format!("corrupt metadata: {e}")))
    }
}
```

```bash
cargo build -p mammoth-local
```

It compiles, with warnings about unused things. That is expected — nothing calls
these yet.

### What to notice

- **`impl Into<PathBuf>`** on `open` lets callers pass a `&str`, a `String` or a
  `PathBuf`. Small kindness, costs nothing.
- **`with_block_size` takes `mut self` and returns `Self`.** That is the builder
  pattern: `LocalBackend::open(dir)?.with_block_size(100)`. It lets tests use
  100-byte blocks so you do not need a 400 MB file to get four blocks.
- **`read_meta` translates `ErrorKind::NotFound` into `Error::NotFound`.** This
  is design principle 3 in action. A user who typos a path should see
  `no such path: /data/typo`, not `No such file or directory (os error 2)` with
  a path inside your private storage layout.
- **`skip_serializing_if`** keeps `inline_data` out of the JSON when it is
  empty, so the sidecars stay readable when you open them.

## Step 3 · Placement — the rack-aware rule

Add this method inside the `impl LocalBackend` block, after `read_meta`:

```rust
    /// Choose which workers hold the replicas of one block.
    ///
    /// The HDFS rule, and for good reason:
    ///   replica 1 -> anywhere            (fast write, no cross-rack hop)
    ///   replica 2 -> a DIFFERENT rack    (survives losing a whole rack)
    ///   replica 3 -> same rack as 2      (cheap: only one cross-rack hop total)
    fn place(&self, id: BlockId, replication: u8) -> Vec<Replica> {
        let n = WORKERS.len();
        let first = (id.0 as usize) % n;
        let mut chosen = vec![first];

        if replication >= 2 {
            if let Some(i) =
                (1..n).map(|k| (first + k) % n).find(|&i| WORKERS[i].1 != WORKERS[first].1)
            {
                chosen.push(i);
            }
        }
        if replication >= 3 && chosen.len() == 2 {
            let second = chosen[1];
            if let Some(i) = (1..n)
                .map(|k| (second + k) % n)
                .find(|&i| WORKERS[i].1 == WORKERS[second].1 && !chosen.contains(&i))
            {
                chosen.push(i);
            }
        }

        chosen
            .into_iter()
            .enumerate()
            .map(|(rank, i)| Replica {
                node: NodeId(WORKERS[i].0.to_string()),
                rack: WORKERS[i].1.to_string(),
                state: if rank == 0 { ReplicaState::Primary } else { ReplicaState::Replica },
            })
            .collect()
    }
```

### Why this rule

Machines live in racks, and a whole rack can lose power at once. If all three
replicas share a rack, one power failure loses your data.

But sending every replica across the rack-to-rack link is slow and expensive —
that link is shared by the whole rack. So the rule buys rack-failure survival
for exactly **one** cross-rack hop: send it over once, then let the far side
make its own local copy.

**`place` is deterministic.** Given the same block ID it always returns the same
workers, so we never have to store the placement — we recompute it. A real
cluster cannot do that (workers die, disks fill up), which is why the master
keeps a block map and rebuilds it from block reports on startup.

## Step 4 · `list` and `stat`

Two small helpers first — add them inside `impl LocalBackend`:

```rust
    /// Build a FileStatus for a file, from its metadata.
    fn status_from(&self, path: &Path, meta: &FileMeta) -> FileStatus {
        FileStatus {
            path: path.to_path_buf(),
            is_dir: false,
            len: meta.len,
            block_size: meta.block_size,
            replication: Some(meta.replication),
            blocks: meta.blocks.len() as u32,
            inlined: meta.inlined,
            mode: 0o644,
            owner: "local".into(),
            group: "local".into(),
            modified: meta.modified,
            checksum: None,
        }
    }

    /// Build a FileStatus for a directory. Directories have no blocks.
    fn dir_status(&self, path: &Path) -> FileStatus {
        FileStatus {
            path: path.to_path_buf(),
            is_dir: true,
            len: 0,
            block_size: self.block_size,
            replication: None,
            blocks: 0,
            inlined: false,
            mode: 0o755,
            owner: "local".into(),
            group: "local".into(),
            modified: 0,
            checksum: None,
        }
    }
```

Widen the `use` line at the top of the file to bring in the rest of the types:

```rust
use mammoth_core::types::{
    BlockId, BlockPlacement, ClusterReport, FileStatus, NodeId, NodeReport, NodeState, Replica,
    ReplicaState, ReplicationHealth,
};
use mammoth_core::{Backend, Error, Result};
```

Now the trait impl. Add this at the **bottom** of the file, outside the
`impl LocalBackend` block:

```rust
#[async_trait::async_trait]
impl Backend for LocalBackend {
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>> {
        let dir = self.ns_path(path);
        let entries = std::fs::read_dir(&dir).map_err(|e| match e.kind() {
            ErrorKind::NotFound => Error::NotFound(path.to_path_buf()),
            _ => Error::Io(e),
        })?;

        let mut out = Vec::new();
        for entry in entries {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();
            if entry.file_type()?.is_dir() {
                out.push(self.dir_status(&path.join(&name)));
            } else if let Some(stem) = name.strip_suffix(META_SUFFIX) {
                let child = path.join(stem);
                let meta = self.read_meta(&child)?;
                out.push(self.status_from(&child, &meta));
            }
        }
        out.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(out)
    }

    async fn stat(&self, path: &Path) -> Result<FileStatus> {
        if self.ns_path(path).is_dir() {
            return Ok(self.dir_status(path));
        }
        let meta = self.read_meta(path)?;
        Ok(self.status_from(path, &meta))
    }
}
```

```bash
cargo build -p mammoth-local
```

This **will fail**, and the error is the point:

```
error[E0046]: not all trait items implemented, missing: `read`, `write`,
              `remove`, `block_layout`, `cluster_report`
```

The trait will not let you stop halfway. Chapter 6 fills in the rest. To keep
building in the meantime, add temporary stubs at the end of the `impl Backend`
block:

```rust
    async fn read(
        &self,
        _p: &Path,
        _r: std::ops::Range<u64>,
    ) -> Result<mammoth_core::backend::ByteStream> {
        todo!("chapter 6")
    }
    async fn write(&self, _p: &Path, _d: mammoth_core::backend::ByteStream) -> Result<()> {
        todo!("chapter 6")
    }
    async fn remove(&self, _p: &Path, _r: bool) -> Result<()> {
        todo!("chapter 6")
    }
    async fn block_layout(&self, _p: &Path) -> Result<Vec<BlockPlacement>> {
        todo!("chapter 6")
    }
    async fn cluster_report(&self) -> Result<ClusterReport> {
        todo!("chapter 6")
    }
```

### Two details in `list`

**`strip_suffix(META_SUFFIX)`** is how we tell a file from a directory. A
sidecar named `hello.txt.mmeta` means the namespace has a file called
`hello.txt`. Anything without the suffix and not a directory is ignored.

**`out.sort_by(...)`** matters more than it looks. `read_dir` returns entries in
whatever order the filesystem feels like — which differs between macOS, Linux
and Windows. Sorting makes `mammoth ls` output stable, and it makes tests that
compare listings actually pass on everyone's machine.

## Check it works

You cannot run this through the CLI yet — that is chapter 7. Test it directly.

Create `crates/mammoth-local/tests/layout.rs`:

```rust
use std::path::Path;

use mammoth_core::Backend;
use mammoth_local::{LocalBackend, WORKERS};

#[test]
fn open_creates_the_layout() {
    let dir = std::env::temp_dir().join("mm-layout-test");
    let _ = std::fs::remove_dir_all(&dir);

    let _be = LocalBackend::open(&dir).unwrap();

    assert!(dir.join("ns").is_dir(), "namespace root must exist");
    for (id, _) in WORKERS {
        assert!(dir.join("workers").join(id).is_dir(), "worker {id} must have a directory");
    }

    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn missing_path_is_a_teaching_error() {
    let dir = std::env::temp_dir().join("mm-missing-test");
    let _ = std::fs::remove_dir_all(&dir);
    let be = LocalBackend::open(&dir).unwrap();

    let err = be.stat(Path::new("/nope.txt")).await.unwrap_err();
    assert_eq!(err.code(), "E0101");
    assert!(err.to_string().contains("no such path"));

    std::fs::remove_dir_all(&dir).unwrap();
}
```

```bash
cargo test -p mammoth-local
```

```
running 2 tests
test open_creates_the_layout ... ok
test missing_path_is_a_teaching_error ... ok
```

Two tests, and the second one is the more interesting: it asserts that a missing
path produces error code `E0101` with a human-readable message. That is design
principle 3 pinned down by a test, so nobody can accidentally regress it into a
raw OS error later.

## Commit it

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
git add -A && git commit -m "feat(local): add store layout, placement, list and stat"
```

## If it went wrong

**`error[E0046]: not all trait items implemented`** — expected at this stage.
Add the `todo!()` stubs above.

**`error[E0433]: failed to resolve: use of undeclared crate serde_json`** —
missing from `crates/mammoth-local/Cargo.toml`. Every crate declares its own
dependencies, even if another crate in the workspace already uses it.

**`the trait bound LocalBackend: Backend is not satisfied` in your test** — add
`use mammoth_core::Backend;`. In Rust a trait's methods are only callable when
the trait is in scope.

**Tests fail with "Directory not empty" on the cleanup line** — you ran two
tests using the same temp directory at once. `cargo test` runs tests in
parallel, so give each test its own directory name, as these two do.

**`type annotations needed` on a `todo!()`** — write the stub's return type out
in full, exactly as shown. `todo!()` can be any type, so the compiler needs the
signature to pin it down.

**`warning: unused imports: NodeReport, NodeState, and ReplicationHealth`**,
and similar warnings about `FAKE_CAPACITY`, `block_path`, `with_block_size` or
`place` — all correct, and all used in chapter 6. Ignore them until then.

Note that `cargo clippy -- -D warnings` turns these into errors, so the full
check will fail at this halfway point. Either finish chapter 6 before running
it, or commit with `cargo build && cargo test` only and run the full check when
chapter 6 lands. Do not "fix" them by deleting the imports — you will just add
them back in an hour.

---

**Next:** [Chapter 6 — LocalBackend, part 2](06-localbackend-part-2.md)
