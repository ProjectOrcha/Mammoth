# ADR 0002 — One `Backend` trait, two implementations

- **Status:** accepted
- **Date:** 2026-08-25

## Context

The interesting parts of this project — the CLI, the visualizations, the web UI
— are the parts users judge it by, and they are also the parts that are cheapest
to get wrong and most expensive to fix late. The distributed storage layer
underneath them takes months.

Building bottom-up means the first demoable product arrives around week 30, and
every UX problem found then is expensive.

## Decision

Define one trait in `mammoth-core`:

```rust
#[async_trait]
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

Write `LocalBackend` first — blocks and workers simulated as subdirectories on
one machine's disk. Build the entire CLI and web UI against it. Write
`ClusterBackend` afterwards and swap the trait object.

## Consequences

**Good**

- A demoable product at week 8 instead of week 30, with real visualizations,
  because `LocalBackend` reports plausible block placements.
- UX problems surface while they are still cheap to fix.
- `LocalBackend` survives as a permanent test fixture and as the engine behind
  `mammoth quickstart`, so it never becomes dead code.
- The trait is a hard architectural boundary: if a CLI command needs something
  the trait cannot express, that is a design signal worth stopping for.

**Bad**

- The trait is designed before the distributed system that must satisfy it, so
  some methods will need to change. `block_layout` returning a `Vec` rather than
  a stream is the most likely one to hurt on a file with millions of blocks.
- A simulated backend can make placement look tidier than a real cluster ever
  is. The visualizations must be re-checked against `ClusterBackend` at M5, not
  assumed correct.
- `async_trait` boxes every call. Irrelevant at metadata-operation rates;
  revisit if it ever shows up in a profile.
