//! The one trait, two implementations.
//!
//! See `docs/adr/0002-backend-trait.md` for why this boundary sits here.

use std::ops::Range;
use std::path::Path;
use std::pin::Pin;

use bytes::Bytes;
use futures_core::Stream;

use crate::error::Result;
use crate::types::{BlockPlacement, ClusterReport, FileStatus};

/// A stream of byte chunks. Chunks are [`Bytes`] so slicing and cloning are
/// refcount bumps rather than copies — see Part VIII §3 of the design notes.
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;

/// Everything the CLI, the gateway and the SDK need from a Mammoth filesystem.
///
/// Implementors:
/// - `mammoth_local::LocalBackend`   — single machine, simulated workers
/// - `mammoth_client::ClusterBackend` — real masters and workers over gRPC
#[async_trait::async_trait]
pub trait Backend: Send + Sync {
    /// List the direct children of a directory.
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>>;

    /// Metadata for a single path.
    async fn stat(&self, path: &Path) -> Result<FileStatus>;

    /// Read a byte range. The range is clamped to the file length.
    async fn read(&self, path: &Path, range: Range<u64>) -> Result<ByteStream>;

    /// Write (or overwrite) a file from a stream of chunks.
    async fn write(&self, path: &Path, data: ByteStream) -> Result<()>;

    /// Remove a path. Fails on a non-empty directory unless `recursive`.
    async fn remove(&self, path: &Path, recursive: bool) -> Result<()>;

    /// Where this file's blocks physically live — the input to `mammoth viz blocks`.
    async fn block_layout(&self, path: &Path) -> Result<Vec<BlockPlacement>>;

    /// Cluster-wide capacity, node states and replication health.
    async fn cluster_report(&self) -> Result<ClusterReport>;
}
