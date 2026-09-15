//! Durable local filesystem using six simulated workers on one host.
#![forbid(unsafe_code)]
pub mod gfs;

use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt};
use mammoth_core::{
    backend::ByteStream,
    place::{place, Candidate},
    types::ReplicationHealth,
    Backend, BlockId, BlockPlacement, ClusterReport, Error, FileStatus, NodeId, NodeReport,
    NodeState, Replica, ReplicaState, Result,
};
use mammoth_storage::{atomic_write, BlockStore};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, File, OpenOptions},
    io::{Read, Seek, SeekFrom, Write},
    ops::Range,
    path::{Path, PathBuf},
};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

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
fn entry<'a>(ns: &'a Namespace, path: &str) -> Result<&'a Entry> {
    ns.entries.get(path).ok_or_else(|| Error::NotFound(path.into()))
}
fn file<'a>(ns: &'a Namespace, path: &str) -> Result<&'a Entry> {
    let e = entry(ns, path)?;
    if e.status.is_dir {
        return Err(Error::WrongKind { path: path.into(), actual: "directory", expected: "file" });
    }
    Ok(e)
}

impl LocalBackend {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        fs::create_dir_all(root.as_ref())?;
        let be = Self {
            root: fs::canonicalize(root.as_ref())?,
            block_size: 128 * 1024 * 1024,
            inline_threshold: 1024 * 1024,
            replication: 3,
        };
        let _guard = be.lock()?;
        fs::create_dir_all(be.root.join("ns"))?;
        fs::create_dir_all(be.root.join("staging"))?;
        for (id, _) in WORKERS {
            BlockStore::open(be.root.join("workers").join(id))?;
        }
        if !be.manifest().exists() {
            be.save(&Namespace {
                version: 1,
                next_block: 1001,
                entries: BTreeMap::from([("/".into(), directory("/"))]),
            })?;
        }
        be.load()?;
        Ok(be)
    }
    pub fn root(&self) -> &Path {
        &self.root
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
    fn lock(&self) -> Result<File> {
        let f = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join("store.lock"))?;
        fs2::FileExt::lock_exclusive(&f)?;
        Ok(f)
    }
    fn manifest(&self) -> PathBuf {
        self.root.join("ns/namespace.json")
    }
    fn load(&self) -> Result<Namespace> {
        let ns: Namespace = serde_json::from_slice(&fs::read(self.manifest())?).map_err(invalid)?;
        if ns.version != 1 || !ns.entries.get("/").is_some_and(|e| e.status.is_dir) {
            return Err(invalid("invalid namespace version or root"));
        }
        Ok(ns)
    }
    fn save(&self, ns: &Namespace) -> Result<()> {
        atomic_write(&self.manifest(), &serde_json::to_vec(ns).map_err(invalid)?)
    }
    fn store(&self, id: &str) -> Result<BlockStore> {
        if !WORKERS.iter().any(|(worker, _)| *worker == id) {
            return Err(invalid("unknown worker in namespace"));
        }
        BlockStore::open(self.root.join("workers").join(id))
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
    fn parents(ns: &mut Namespace, path: &str, create: bool) -> Result<()> {
        let mut parent = String::new();
        let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            parent.push('/');
            parent.push_str(part);
            match ns.entries.get(&parent) {
                Some(e) if !e.status.is_dir => {
                    return Err(Error::WrongKind {
                        path: parent.into(),
                        actual: "file",
                        expected: "directory",
                    })
                }
                Some(_) => {}
                None if create => {
                    ns.entries.insert(parent.clone(), directory(&parent));
                }
                None => return Err(Error::NotFound(parent.into())),
            }
        }
        Ok(())
    }
    async fn transaction<T: Send + 'static>(
        &self,
        f: impl FnOnce(&Self, &mut Namespace) -> Result<T> + Send + 'static,
    ) -> Result<T> {
        let be = self.clone();
        tokio::task::spawn_blocking(move || {
            let _guard = be.lock()?;
            let mut ns = be.load()?;
            f(&be, &mut ns)
        })
        .await
        .map_err(invalid)?
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

#[async_trait::async_trait]
impl Backend for LocalBackend {
    async fn stat(&self, path: &Path) -> Result<FileStatus> {
        let path = normalize(path)?;
        self.transaction(move |_, ns| Ok(entry(ns, &path)?.status.clone())).await
    }
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>> {
        let path = normalize(path)?;
        self.transaction(move |_, ns| {
            if !entry(ns, &path)?.status.is_dir {
                return Err(Error::WrongKind {
                    path: path.into(),
                    actual: "file",
                    expected: "directory",
                });
            }
            Ok(ns
                .entries
                .values()
                .filter(|e| {
                    e.status.path != Path::new("/")
                        && e.status.path.parent() == Some(Path::new(&path))
                })
                .map(|e| e.status.clone())
                .collect())
        })
        .await
    }
    async fn mkdir(&self, path: &Path, parents: bool) -> Result<()> {
        let path = normalize(path)?;
        self.transaction(move |be, ns| {
            if let Some(e) = ns.entries.get(&path) {
                if parents && e.status.is_dir {
                    return Ok(());
                }
                return Err(Error::AlreadyExists(path.into()));
            }
            Self::parents(ns, &path, parents)?;
            ns.entries.insert(path.clone(), directory(&path));
            be.save(ns)
        })
        .await
    }
    async fn write(&self, path: &Path, mut data: ByteStream) -> Result<()> {
        self.validate()?;
        let path = normalize(path)?;
        // Stage the request before acquiring the transaction lock. A failed or
        // cancelled upload cannot replace the currently committed entry.
        let staging_dir = self.root.join("staging");
        let staging = tokio::task::spawn_blocking(move || tempfile::tempfile_in(staging_dir))
            .await
            .map_err(invalid)??;
        let mut staging = tokio::fs::File::from_std(staging);
        while let Some(chunk) = data.next().await {
            staging.write_all(&chunk?).await?;
        }
        staging.flush().await?;
        let mut staging = staging.into_std().await;
        self.transaction(move |be, ns| {
            if ns.entries.get(&path).is_some_and(|e| e.status.is_dir) {
                return Err(Error::WrongKind {
                    path: path.into(),
                    actual: "directory",
                    expected: "file",
                });
            }
            Self::parents(ns, &path, true)?;
            let len = staging.metadata()?.len();
            staging.seek(SeekFrom::Start(0))?;
            let mut blocks = Vec::new();
            let mut crc = 0;
            let mut md5 = Md5::new();
            let inline = if len <= be.inline_threshold {
                let mut bytes = Vec::new();
                staging.read_to_end(&mut bytes)?;
                crc = crc32c::crc32c(&bytes);
                md5.update(&bytes);
                Some(bytes)
            } else {
                let count = len.div_ceil(be.block_size);
                if count > u32::MAX as u64 {
                    return Err(Error::InvalidInput("file has too many blocks".into()));
                }
                let first = ns.next_block;
                ns.next_block =
                    first.checked_add(count).ok_or_else(|| invalid("block IDs exhausted"))?;
                // Reserve IDs durably; an interrupted transaction never reuses
                // the identity of a partially published block.
                be.save(ns)?;
                let mut remaining = len;
                for index in 0..count {
                    let id = BlockId(first + index);
                    let mut bytes = vec![0; remaining.min(be.block_size) as usize];
                    staging.read_exact(&mut bytes)?;
                    remaining -= bytes.len() as u64;
                    crc = crc32c::crc32c_append(crc, &bytes);
                    md5.update(&bytes);
                    let replicas = be.placements(id, be.replication);
                    for r in &replicas {
                        be.store(&r.node.0)?.put(id.0, &bytes)?;
                    }
                    blocks.push(BlockPlacement {
                        id,
                        index: index as u32,
                        len: bytes.len() as u64,
                        replicas,
                    });
                }
                None
            };
            let old = ns.entries.get(&path).cloned();
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
            ns.entries.insert(
                path,
                Entry { status, inline, blocks, etag: format!("{:x}", md5.finalize()) },
            );
            be.save(ns)?;
            // Cleanup is best effort after a successful commit. `gc` retries.
            if let Some(old) = old {
                let _ = be.clean(&old);
            }
            Ok(())
        })
        .await
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
        let staged = self
            .transaction(move |be, ns| {
                let e = file(ns, &path)?;
                let mut output = tempfile::tempfile_in(be.root.join("staging"))?;
                let start = range.start.min(e.status.len);
                let end = range.end.min(e.status.len);
                if let Some(bytes) = &e.inline {
                    let actual = format!("crc32c:{:08x}", crc32c::crc32c(bytes));
                    if e.status.checksum.as_ref() != Some(&actual)
                        || bytes.len() as u64 != e.status.len
                    {
                        return Err(Error::ChecksumMismatch {
                            path: path.into(),
                            expected: e.status.checksum.clone().unwrap_or_default(),
                            actual,
                        });
                    }
                    output.write_all(&bytes[start as usize..end as usize])?;
                } else {
                    let mut offset = 0;
                    for block in &e.blocks {
                        let block_end = offset + block.len;
                        if offset < end && block_end > start {
                            let bytes = be.read_block(block)?;
                            let from = start.saturating_sub(offset) as usize;
                            let to = (end.min(block_end) - offset) as usize;
                            output.write_all(&bytes[from..to])?;
                        }
                        offset = block_end;
                    }
                    if offset != e.status.len {
                        return Err(invalid("file length does not match its blocks"));
                    }
                }
                output.seek(SeekFrom::Start(0))?;
                Ok((e.status.clone(), e.etag.clone(), start..end, output))
            })
            .await?;
        let (status, etag, range, file) = staged;
        Ok(mammoth_core::backend::ReadSnapshot {
            status,
            etag,
            range,
            data: Box::pin(ReaderStream::new(tokio::fs::File::from_std(file)).map_err(Error::from)),
        })
    }
    async fn remove(&self, path: &Path, recursive: bool) -> Result<()> {
        let path = normalize(path)?;
        if path == "/" {
            return Err(Error::InvalidInput("cannot remove the namespace root".into()));
        }
        self.transaction(move |be, ns| {
            entry(ns, &path)?;
            let prefix = format!("{path}/");
            let keys: Vec<_> = ns
                .entries
                .keys()
                .filter(|p| **p == path || p.starts_with(&prefix))
                .cloned()
                .collect();
            if keys.len() > 1 && !recursive {
                return Err(Error::WrongKind {
                    path: path.into(),
                    actual: "non-empty directory",
                    expected: "empty directory (pass --recursive)",
                });
            }
            let old: Vec<_> = keys.iter().filter_map(|p| ns.entries.remove(p)).collect();
            be.save(ns)?;
            for e in old {
                let _ = be.clean(&e);
            }
            Ok(())
        })
        .await
    }
    async fn rename(&self, from: &Path, to: &Path) -> Result<()> {
        let from = normalize(from)?;
        let to = normalize(to)?;
        self.transaction(move |be, ns| {
            entry(ns, &from)?;
            if from == to {
                return Ok(());
            }
            if from == "/" || to.starts_with(&format!("{from}/")) {
                return Err(Error::InvalidInput(
                    "cannot move root or move a directory into itself".into(),
                ));
            }
            if ns.entries.contains_key(&to) {
                return Err(Error::AlreadyExists(to.into()));
            }
            Self::parents(ns, &to, false)?;
            let keys: Vec<_> = ns
                .entries
                .keys()
                .filter(|p| **p == from || p.starts_with(&format!("{from}/")))
                .cloned()
                .collect();
            for key in keys {
                let mut e = ns.entries.remove(&key).expect("existing key");
                let new = format!("{to}{}", &key[from.len()..]);
                e.status.path = new.clone().into();
                e.status.modified = now();
                ns.entries.insert(new, e);
            }
            be.save(ns)
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
        self.transaction(move |be, ns| {
            let e = ns.entries.get_mut(&path).ok_or_else(|| Error::NotFound(path.into()))?;
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
            be.save(ns)
        })
        .await
    }
    async fn set_replication(&self, path: &Path, replication: u8) -> Result<()> {
        self.check_replication(replication)?;
        let path = normalize(path)?;
        self.transaction(move |be, ns| {
            let old = file(ns, &path)?.clone();
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
            ns.entries.insert(path, new.clone());
            be.save(ns)?;
            for (old_b, new_b) in old.blocks.iter().zip(&new.blocks) {
                for r in &old_b.replicas {
                    if !new_b.replicas.iter().any(|n| n.node == r.node) {
                        let _ = be.store(&r.node.0)?.remove(old_b.id.0);
                    }
                }
            }
            Ok(())
        })
        .await
    }
    async fn block_layout(&self, path: &Path) -> Result<Vec<BlockPlacement>> {
        let path = normalize(path)?;
        self.transaction(move |be, ns| {
            let mut blocks = file(ns, &path)?.blocks.clone();
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
        self.transaction(|be, ns| {
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
            for e in ns.entries.values() {
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
        self.transaction(|be, ns| {
            let mut repaired = 0;
            for e in ns.entries.values() {
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
    }
    async fn etag(&self, path: &Path) -> Result<String> {
        let path = normalize(path)?;
        self.transaction(move |be, ns| {
            let e = file(ns, &path)?;
            if !e.etag.is_empty() {
                return Ok(e.etag.clone());
            }
            let mut digest = Md5::new();
            if let Some(bytes) = &e.inline {
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
        self.transaction(|be, ns| {
            let refs: BTreeSet<_> = ns
                .entries
                .values()
                .flat_map(|e| &e.blocks)
                .flat_map(|b| b.replicas.iter().map(move |r| (r.node.0.clone(), b.id.0)))
                .collect();
            let mut removed = 0;
            for (id, _) in WORKERS {
                let store = be.store(id)?;
                for (block, _) in store.blocks()? {
                    if !refs.contains(&(id.to_string(), block)) {
                        store.remove(block)?;
                        removed += 1;
                    }
                }
            }
            Ok(removed)
        })
        .await
    }
}

/// A byte stream containing one owned buffer, useful for SDK callers.
pub fn body(data: impl Into<Bytes>) -> ByteStream {
    let data = data.into();
    Box::pin(futures_util::stream::once(async move { Ok(data) }))
}
