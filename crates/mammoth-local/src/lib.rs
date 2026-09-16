//! Durable local filesystem using six simulated workers on one host.
#![forbid(unsafe_code)]
mod cache;
pub mod gfs;
mod metadata;
mod reader;

use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use mammoth_core::{
    backend::ByteStream,
    place::{place, Candidate},
    types::ReplicationHealth,
    Backend, BlockId, BlockPlacement, ClusterReport, Error, FileStatus, NodeId, NodeReport,
    NodeState, Replica, ReplicaState, Result,
};
use mammoth_storage::BlockStore;
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::Semaphore;

pub const WORKERS: [(&str, &str); 6] = [
    ("w1", "rack-a"),
    ("w2", "rack-a"),
    ("w3", "rack-b"),
    ("w4", "rack-b"),
    ("w5", "rack-c"),
    ("w6", "rack-c"),
];
/// Reference capacity for each simulated worker; not enforced or a disk measurement.
pub const CAPACITY: u64 = 100 * 1024 * 1024 * 1024;

#[derive(Clone)]
pub struct LocalBackend {
    root: PathBuf,
    block_size: u64,
    inline_threshold: u64,
    replication: u8,
    metadata: Arc<metadata::Metadata>,
    stores: Arc<BTreeMap<String, BlockStore>>,
    uploads: Arc<Semaphore>,
    cache: Arc<cache::ReadCache>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Entry {
    #[serde(default)]
    etag: String,
    status: FileStatus,
    inline: Option<Vec<u8>>,
    blocks: Vec<BlockPlacement>,
}
#[derive(Serialize, Deserialize)]
struct Namespace {
    version: u32,
    next_block: u64,
    entries: BTreeMap<String, Entry>,
}

pub use mammoth_core::path::normalize;

fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()
        as i64
}
fn invalid(e: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()))
}
fn directory(path: &str) -> Entry {
    Entry {
        etag: String::new(),
        status: FileStatus {
            path: path.into(),
            is_dir: true,
            len: 0,
            block_size: 0,
            replication: None,
            blocks: 0,
            inlined: false,
            mode: 0o755,
            owner: "local".into(),
            group: "local".into(),
            modified: now(),
            checksum: None,
        },
        inline: None,
        blocks: vec![],
    }
}
impl LocalBackend {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        let root = fs::canonicalize(root.as_ref())?;
        let guard = lock_file(&root.join("store.lock"))?;
        fs2::FileExt::lock_exclusive(&guard)?;
        fs::create_dir_all(root.join("ns"))?;
        fs::create_dir_all(root.join("staging"))?;
        let mut stores = BTreeMap::new();
        for (id, _) in WORKERS {
            stores.insert(id.to_owned(), BlockStore::open(root.join("workers").join(id))?);
        }
        let metadata = Arc::new(metadata::Metadata::open(&root)?);
        Ok(Self {
            root,
            block_size: 128 * 1024 * 1024,
            inline_threshold: 1024 * 1024,
            replication: 3,
            metadata,
            stores: Arc::new(stores),
            uploads: Arc::new(Semaphore::new(8)),
            cache: Arc::new(cache::ReadCache::new(256 * 1024 * 1024)),
        })
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    /// A process-local cache of verified immutable bytes. Zero disables it.
    /// Existing clones retain their previous cache when this builder is used.
    pub fn with_cache_size(mut self, bytes: u64) -> Self {
        self.cache = Arc::new(cache::ReadCache::new(bytes));
        self
    }
    pub fn with_block_size(mut self, n: u64) -> Self {
        self.block_size = n;
        self
    }
    pub fn with_inline_threshold(mut self, n: u64) -> Self {
        self.inline_threshold = n;
        self
    }
    pub fn with_replication(mut self, n: u8) -> Self {
        self.replication = n;
        self
    }
    fn validate(&self) -> Result<()> {
        if self.block_size == 0
            || self.block_size > 256 * 1024 * 1024
            || self.inline_threshold > 16 * 1024 * 1024
        {
            return Err(Error::InvalidInput("invalid block size or inline threshold".into()));
        }
        self.check_replication(self.replication)
    }
    fn check_replication(&self, n: u8) -> Result<()> {
        if n == 0 {
            return Err(Error::InvalidInput("replication must be positive".into()));
        }
        if n as usize > WORKERS.len() {
            return Err(Error::NotEnoughWorkers { wanted: n, available: WORKERS.len() as u8 });
        }
        Ok(())
    }
    fn activity(&self) -> Result<Arc<File>> {
        let file = lock_file(&self.root.join("activity.lock"))?;
        fs2::FileExt::lock_shared(&file)?;
        Ok(Arc::new(file))
    }
    fn exclusive_activity(&self) -> Result<Option<File>> {
        let file = lock_file(&self.root.join("activity.lock"))?;
        match fs2::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(file)),
            Err(e) if e.raw_os_error() == fs2::lock_contended_error().raw_os_error() => Ok(None),
            Err(e) => Err(e.into()),
        }
    }
    fn store(&self, id: &str) -> Result<&BlockStore> {
        self.stores.get(id).ok_or_else(|| invalid("unknown worker in namespace"))
    }
    fn placements(&self, id: BlockId, n: u8) -> Vec<Replica> {
        let candidates: Vec<_> = WORKERS
            .iter()
            .enumerate()
            .map(|(i, (id, rack))| Candidate {
                id: id.to_string(),
                rack: rack.to_string(),
                seed: (i as u64).wrapping_mul(0x9e3779b97f4a7c15),
                weight: 1.0,
            })
            .collect();
        place(id.0, &candidates, n as usize)
            .iter()
            .enumerate()
            .map(|(i, c)| Replica {
                node: NodeId(c.id.clone()),
                rack: c.rack.clone(),
                state: if i == 0 { ReplicaState::Primary } else { ReplicaState::Replica },
            })
            .collect()
    }
    async fn transaction<T: Send + 'static>(
        &self,
        write: bool,
        f: impl FnOnce(&Self, &rusqlite::Transaction<'_>) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let be = self.clone();
        tokio::task::spawn_blocking(move || be.metadata.transaction(write, |tx| f(&be, tx)))
            .await
            .map_err(invalid)?
    }
    async fn shared_activity(&self) -> Result<Arc<File>> {
        let be = self.clone();
        tokio::task::spawn_blocking(move || be.activity()).await.map_err(invalid)?
    }
    async fn put_block(
        &self,
        bytes: Bytes,
        index: usize,
        ids: &mut Range<u64>,
        guard: &Arc<File>,
    ) -> Result<BlockPlacement> {
        if index >= u32::MAX as usize {
            return Err(invalid("file has too many blocks"));
        }
        if ids.is_empty() {
            let first = self.transaction(true, |_, tx| metadata::reserve(tx, 64)).await?;
            *ids = first..first + 64;
        }
        let id = BlockId(ids.next().expect("reserved IDs"));
        let replicas = self.placements(id, self.replication);
        let mut tasks = Vec::new();
        for replica in &replicas {
            let store = self.store(&replica.node.0)?.clone();
            let bytes = bytes.clone();
            // Detached blocking tasks retain the lifetime lock even if the
            // upload is cancelled. GC cannot collect a block being published.
            let guard = guard.clone();
            tasks.push(tokio::task::spawn_blocking(move || {
                let _guard = guard;
                store.put(id.0, &bytes)
            }));
        }
        // Wait for every replica, including when one fails.
        for result in futures_util::future::join_all(tasks).await {
            result.map_err(invalid)??;
        }
        Ok(BlockPlacement { id, index: index as u32, len: bytes.len() as u64, replicas })
    }
    async fn clean_retired(&self, entries: Vec<Entry>) {
        if entries.iter().all(|e| e.blocks.is_empty()) {
            return;
        }
        let be = self.clone();
        let _ = tokio::task::spawn_blocking(move || -> Result<()> {
            // Active snapshots keep their immutable blocks. GC reclaims these
            // later, without making overwrite wait for slow readers.
            let Some(_guard) = be.exclusive_activity()? else {
                return Ok(());
            };
            for entry in entries {
                be.clean(&entry)?;
            }
            Ok(())
        })
        .await;
    }
    fn read_block(&self, block: &BlockPlacement) -> Result<Vec<u8>> {
        let mut last = Error::NotFound(format!("block {}", block.id.0).into());
        for replica in &block.replicas {
            match self.store(&replica.node.0)?.read(block.id.0) {
                Ok(bytes) if bytes.len() as u64 == block.len => return Ok(bytes),
                Ok(_) => last = invalid("block length does not match namespace"),
                Err(e) => last = e,
            }
        }
        Err(last)
    }
    fn clean(&self, old: &Entry) -> Result<()> {
        for b in &old.blocks {
            for r in &b.replicas {
                self.store(&r.node.0)?.remove(b.id.0)?;
            }
        }
        Ok(())
    }
}

impl LocalBackend {
    async fn write_file(&self, path: &Path, mut data: ByteStream, overwrite: bool) -> Result<()> {
        self.validate()?;
        let path = normalize(path)?;
        let _permit = self.uploads.clone().acquire_owned().await.map_err(invalid)?;
        let guard = self.shared_activity().await?;
        let mut buffer = BytesMut::new();
        let mut blocks = Vec::new();
        let mut ids = 0..0;
        let mut len = 0u64;
        let mut crc = 0;
        let mut digest = Md5::new();
        let mut external = false;
        while let Some(chunk) = data.next().await {
            let chunk = chunk?;
            len =
                len.checked_add(chunk.len() as u64).ok_or_else(|| invalid("file size overflow"))?;
            crc = crc32c::crc32c_append(crc, &chunk);
            digest.update(&chunk);
            let mut remaining = &chunk[..];
            while !remaining.is_empty() {
                let target = if external {
                    self.block_size as usize
                } else {
                    self.inline_threshold as usize + 1
                };
                let take = (target.saturating_sub(buffer.len())).min(remaining.len());
                buffer.extend_from_slice(&remaining[..take]);
                remaining = &remaining[take..];
                if !external && buffer.len() as u64 > self.inline_threshold {
                    external = true;
                }
                while external && buffer.len() as u64 >= self.block_size {
                    let bytes = buffer.split_to(self.block_size as usize).freeze();
                    blocks.push(self.put_block(bytes, blocks.len(), &mut ids, &guard).await?);
                }
            }
        }
        let inline = if external {
            if !buffer.is_empty() {
                blocks.push(self.put_block(buffer.freeze(), blocks.len(), &mut ids, &guard).await?);
            }
            None
        } else {
            Some(buffer.to_vec())
        };
        let etag = format!("{:x}", digest.finalize());
        let commit_guard = guard.clone();
        let be = self.clone();
        let old = tokio::task::spawn_blocking(move || {
            let _guard = commit_guard;
            be.metadata.transaction(true, |tx| {
                let old = match metadata::get(tx, &path) {
                    Ok(e) if e.status.is_dir => {
                        return Err(Error::WrongKind {
                            path: path.into(),
                            actual: "directory",
                            expected: "file",
                        })
                    }
                    Ok(_) if !overwrite => return Err(Error::AlreadyExists(path.into())),
                    Ok(e) => Some(e),
                    Err(Error::NotFound(_)) => None,
                    Err(e) => return Err(e),
                };
                metadata::parents(tx, &path, true)?;
                let status = FileStatus {
                    path: path.clone().into(),
                    is_dir: false,
                    len,
                    block_size: be.block_size,
                    replication: Some(be.replication),
                    blocks: blocks.len() as u32,
                    inlined: inline.is_some(),
                    mode: old.as_ref().map_or(0o644, |e| e.status.mode),
                    owner: old.as_ref().map_or("local".into(), |e| e.status.owner.clone()),
                    group: old.as_ref().map_or("local".into(), |e| e.status.group.clone()),
                    modified: now(),
                    checksum: Some(format!("crc32c:{crc:08x}")),
                };
                metadata::put(tx, &path, &Entry { status, inline, blocks, etag })?;
                Ok(old)
            })
        })
        .await
        .map_err(invalid)??;
        drop(guard);
        if let Some(old) = old {
            self.clean_retired(vec![old]).await;
        }
        Ok(())
    }
}

#[async_trait::async_trait]
impl Backend for LocalBackend {
    fn cache_stats(&self) -> Option<mammoth_core::backend::CacheStats> {
        Some(self.cache.stats())
    }
    fn clear_read_cache(&self) {
        self.cache.clear();
    }

    async fn stat(&self, path: &Path) -> Result<FileStatus> {
        let path = normalize(path)?;
        self.transaction(false, move |_, tx| metadata::stat(tx, &path)).await
    }
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>> {
        let path = normalize(path)?;
        self.transaction(false, move |_, tx| {
            if !metadata::stat(tx, &path)?.is_dir {
                return Err(Error::WrongKind {
                    path: path.into(),
                    actual: "file",
                    expected: "directory",
                });
            }
            metadata::list(tx, &path)
        })
        .await
    }
    async fn mkdir(&self, path: &Path, parents: bool) -> Result<()> {
        let path = normalize(path)?;
        self.transaction(true, move |_, tx| {
            match metadata::stat(tx, &path) {
                Ok(e) if parents && e.is_dir => return Ok(()),
                Ok(_) => return Err(Error::AlreadyExists(path.into())),
                Err(Error::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
            metadata::parents(tx, &path, parents)?;
            metadata::put(tx, &path, &directory(&path))
        })
        .await
    }
    async fn write(&self, path: &Path, data: ByteStream) -> Result<()> {
        self.write_file(path, data, true).await
    }
    async fn create(&self, path: &Path, data: ByteStream) -> Result<()> {
        self.write_file(path, data, false).await
    }
    async fn read(&self, path: &Path, range: Range<u64>) -> Result<ByteStream> {
        Ok(self.open_read(path, range).await?.data)
    }
    async fn open_read(
        &self,
        path: &Path,
        range: Range<u64>,
    ) -> Result<mammoth_core::backend::ReadSnapshot> {
        if range.start > range.end {
            return Err(Error::InvalidInput("range start exceeds end".into()));
        }
        let path = normalize(path)?;
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let guard = be.activity()?;
            let entry = be.metadata.transaction(false, |tx| metadata::file(tx, &path))?;
            reader::open(be, entry, range, guard)
        })
        .await
        .map_err(invalid)?
    }
    async fn open_read_suffix(
        &self,
        path: &Path,
        length: u64,
    ) -> Result<mammoth_core::backend::ReadSnapshot> {
        let path = normalize(path)?;
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let guard = be.activity()?;
            let entry = be.metadata.transaction(false, |tx| metadata::file(tx, &path))?;
            let range = entry.status.len.saturating_sub(length)..entry.status.len;
            reader::open(be, entry, range, guard)
        })
        .await
        .map_err(invalid)?
    }
    async fn remove(&self, path: &Path, recursive: bool) -> Result<()> {
        let path = normalize(path)?;
        if path == "/" {
            return Err(Error::InvalidInput("cannot remove the namespace root".into()));
        }
        let old = self
            .transaction(true, move |_, tx| {
                metadata::stat(tx, &path)?;
                let keys = metadata::subtree(tx, &path)?;
                if keys.len() > 1 && !recursive {
                    return Err(Error::WrongKind {
                        path: path.into(),
                        actual: "non-empty directory",
                        expected: "empty directory (pass --recursive)",
                    });
                }
                let mut old = Vec::new();
                for key in keys {
                    old.push(metadata::get(tx, &key)?);
                    metadata::delete(tx, &key)?;
                }
                Ok(old)
            })
            .await?;
        self.clean_retired(old).await;
        Ok(())
    }
    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let from = normalize(from)?;
        let to = normalize(to)?;
        self.transaction(true, move |_, tx| {
            metadata::stat(tx, &from)?;
            if from == to {
                return Ok(());
            }
            if from == "/" || to.starts_with(&format!("{from}/")) {
                return Err(Error::InvalidInput(
                    "cannot move root or move a directory into itself".into(),
                ));
            }
            match metadata::stat(tx, &to) {
                Ok(_) => return Err(Error::AlreadyExists(to.into())),
                Err(Error::NotFound(_)) => {}
                Err(e) => return Err(e),
            }
            metadata::parents(tx, &to, false)?;
            for key in metadata::subtree(tx, &from)? {
                let mut e = metadata::get(tx, &key)?;
                let new = format!("{to}{}", &key[from.len()..]);
                e.status.path = new.clone().into();
                e.status.modified = now();
                metadata::delete(tx, &key)?;
                metadata::put(tx, &new, &e)?;
            }
            Ok(())
        })
        .await
    }
    async fn set_attributes(
        &self,
        path: &Path,
        mode: Option<u32>,
        owner: Option<String>,
        group: Option<String>,
    ) -> Result<()> {
        let path = normalize(path)?;
        if mode.is_some_and(|m| m > 0o7777) {
            return Err(Error::InvalidInput("mode must be octal 0000–7777".into()));
        }
        self.transaction(true, move |_, tx| {
            let mut e = metadata::get(tx, &path)?;
            if let Some(m) = mode {
                e.status.mode = m;
            }
            if let Some(o) = owner {
                e.status.owner = o;
            }
            if let Some(g) = group {
                e.status.group = g;
            }
            e.status.modified = now();
            metadata::put(tx, &path, &e)
        })
        .await
    }
    async fn set_replication(&self, path: &Path, replication: u8) -> Result<()> {
        self.check_replication(replication)?;
        let path = normalize(path)?;
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = be.exclusive_activity()?.ok_or_else(maintenance_busy)?;
            // Maintenance is infrequent; serialize namespace mutations here so
            // it cannot resurrect a concurrently removed or renamed generation.
            be.metadata.transaction(true, |tx| {
                let old = metadata::file(tx, &path)?;
                let mut new = old.clone();
                for b in &mut new.blocks {
                    let bytes = be.read_block(b)?;
                    let replicas = be.placements(b.id, replication);
                    for r in &replicas {
                        let store = be.store(&r.node.0)?;
                        if !store.read(b.id.0).is_ok_and(|data| data.len() as u64 == b.len) {
                            store.remove(b.id.0)?;
                            store.put(b.id.0, &bytes)?;
                        }
                    }
                    b.replicas = replicas;
                }
                new.status.replication = Some(replication);
                new.status.modified = now();
                metadata::put(tx, &path, &new)?;
                Ok(())
            })?;
            be.gc_locked()?;
            Ok(())
        })
        .await
        .map_err(invalid)?
    }
    async fn block_layout(&self, path: &Path) -> Result<Vec<BlockPlacement>> {
        let path = normalize(path)?;
        self.data_task(move |be| {
            let mut blocks = be.metadata.transaction(false, |tx| metadata::file(tx, &path))?.blocks;
            for b in &mut blocks {
                for r in &mut b.replicas {
                    if be.store(&r.node.0)?.read(b.id.0).is_err() {
                        r.state = ReplicaState::Corrupt;
                    }
                }
            }
            Ok(blocks)
        })
        .await
    }
    async fn cluster_report(&self) -> Result<ClusterReport> {
        self.data_task(|be| {
            let entries = be.metadata.transaction(false, |tx| metadata::all(tx))?;
            let mut nodes = Vec::new();
            let mut used = 0;
            for (id, rack) in WORKERS {
                let blocks = be.store(id)?.blocks()?;
                let bytes = blocks.iter().map(|(_, len)| len).sum::<u64>();
                used += bytes;
                nodes.push(NodeReport {
                    id: NodeId(id.into()),
                    address: format!("local://{id}"),
                    rack: rack.into(),
                    state: if bytes >= CAPACITY { NodeState::Warn } else { NodeState::Healthy },
                    used: bytes,
                    capacity: CAPACITY,
                    blocks: blocks.len() as u64,
                    volumes: 1,
                    disk_p99_ms: 0.0,
                });
            }
            let mut health = ReplicationHealth::default();
            for e in &entries {
                for b in &e.blocks {
                    let mut good = 0;
                    let mut corrupt = false;
                    for r in &b.replicas {
                        match be.store(&r.node.0)?.read(b.id.0) {
                            Ok(bytes) if bytes.len() as u64 == b.len => good += 1,
                            Err(Error::Io(e)) if e.kind() == std::io::ErrorKind::NotFound => {}
                            _ => corrupt = true,
                        }
                    }
                    let target = e.status.replication.unwrap_or(0) as usize;
                    if good == 0 {
                        health.missing += 1;
                    } else if good == 1 && good < target {
                        health.critical += 1;
                    } else if corrupt {
                        health.corrupt += 1;
                    } else if good < target {
                        health.under_replicated += 1;
                    } else {
                        health.healthy += 1;
                    }
                }
            }
            Ok(ClusterReport {
                name: "local".into(),
                leader: None,
                safe_mode: false,
                used,
                capacity: CAPACITY * WORKERS.len() as u64,
                nodes,
                health,
            })
        })
        .await
    }
    async fn repair(&self) -> Result<u64> {
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = be.exclusive_activity()?.ok_or_else(maintenance_busy)?;
            let entries = be.metadata.transaction(false, |tx| metadata::all(tx))?;
            let mut repaired = 0;
            for e in entries {
                for b in &e.blocks {
                    let bytes = be.read_block(b)?;
                    for r in &b.replicas {
                        let store = be.store(&r.node.0)?;
                        if !store.read(b.id.0).is_ok_and(|data| data.len() as u64 == b.len) {
                            store.remove(b.id.0)?;
                            store.put(b.id.0, &bytes)?;
                            repaired += 1;
                        }
                    }
                }
            }
            Ok(repaired)
        })
        .await
        .map_err(invalid)?
    }
    async fn etag(&self, path: &Path) -> Result<String> {
        let path = normalize(path)?;
        self.data_task(move |be| {
            let e = be.metadata.transaction(false, |tx| metadata::file(tx, &path))?;
            if !e.etag.is_empty() {
                return Ok(e.etag);
            }
            let mut digest = Md5::new();
            if let Some(bytes) = e.inline {
                digest.update(bytes);
            } else {
                for b in &e.blocks {
                    digest.update(be.read_block(b)?);
                }
            }
            Ok(format!("{:x}", digest.finalize()))
        })
        .await
    }
    async fn gc(&self) -> Result<u64> {
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let Some(_guard) = be.exclusive_activity()? else {
                return Ok(0);
            };
            be.gc_locked()
        })
        .await
        .map_err(invalid)?
    }
}

/// A byte stream containing one owned buffer, useful for SDK callers.
pub fn body(data: impl Into<Bytes>) -> ByteStream {
    let data = data.into();
    Box::pin(futures_util::stream::once(async move { Ok(data) }))
}

fn lock_file(path: &Path) -> Result<File> {
    Ok(OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?)
}
fn maintenance_busy() -> Error {
    Error::Io(std::io::Error::new(
        std::io::ErrorKind::WouldBlock,
        "active reads or writes; retry maintenance after they finish",
    ))
}
impl LocalBackend {
    async fn data_task<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Self) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = be.activity()?;
            f(&be)
        })
        .await
        .map_err(invalid)?
    }
    fn gc_locked(&self) -> Result<u64> {
        let entries = self.metadata.transaction(false, |tx| metadata::all(tx))?;
        let refs: BTreeSet<_> = entries
            .iter()
            .flat_map(|e| &e.blocks)
            .flat_map(|b| b.replicas.iter().map(move |r| (r.node.0.clone(), b.id.0)))
            .collect();
        let mut removed = 0;
        for (id, _) in WORKERS {
            let store = self.store(id)?;
            for (block, _) in store.blocks()? {
                if !refs.contains(&(id.to_owned(), block)) {
                    store.remove(block)?;
                    removed += 1;
                }
            }
        }
        Ok(removed)
    }
}
