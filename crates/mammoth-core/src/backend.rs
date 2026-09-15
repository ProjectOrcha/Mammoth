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

/// Metadata and bytes captured from one committed file generation.
pub struct ReadSnapshot {
    /// Metadata for the captured generation.
    pub status: FileStatus,
    /// Entity tag for the captured generation.
    pub etag: String,
    /// Actual byte range after clamping to the captured file length.
    pub range: Range<u64>,
    /// Bytes from that range.
    pub data: ByteStream,
}

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
    /// Create a directory, optionally creating missing parents.
    async fn mkdir(&self, _path: &Path, _parents: bool) -> Result<()> {
        Err(crate::Error::NotImplemented("mkdir"))
    }

    /// Atomically rename an entry; existing destinations are rejected.
    async fn rename(&self, _from: &Path, _to: &Path) -> Result<()> {
        Err(crate::Error::NotImplemented("rename"))
    }

    /// Update POSIX metadata. These are descriptive in local development mode.
    async fn set_attributes(
        &self,
        _path: &Path,
        _mode: Option<u32>,
        _owner: Option<String>,
        _group: Option<String>,
    ) -> Result<()> {
        Err(crate::Error::NotImplemented("set_attributes"))
    }

    /// Change replica count without changing file contents.
    async fn set_replication(&self, _path: &Path, _replication: u8) -> Result<()> {
        Err(crate::Error::NotImplemented("set_replication"))
    }

    /// Verify replicas and restore damaged copies from a checked source.
    async fn repair(&self) -> Result<u64> {
        Err(crate::Error::NotImplemented("repair"))
    }

    /// Reclaim blocks that no namespace entry references.
    async fn gc(&self) -> Result<u64> {
        Err(crate::Error::NotImplemented("garbage collection"))
    }
    /// Opaque entity tag for conditional HTTP operations.
    async fn etag(&self, _path: &Path) -> Result<String> {
        Err(crate::Error::NotImplemented("entity tags"))
    }
    /// Open metadata and a byte stream from the same committed generation.
    async fn open_read(&self, _path: &Path, _range: Range<u64>) -> Result<ReadSnapshot> {
        Err(crate::Error::NotImplemented("atomic read snapshots"))
    }
}

/// Iterative traversal with no recursion-depth dependency.
pub async fn walk(be: &dyn Backend, path: &Path) -> Result<Vec<FileStatus>> {
    let root = be.stat(path).await?;
    let mut todo = vec![root];
    let mut out = vec![];
    while let Some(s) = todo.pop() {
        if s.is_dir {
            todo.extend(be.list(&s.path).await?);
        }
        out.push(s);
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}
