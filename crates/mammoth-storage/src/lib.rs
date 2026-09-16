//! Durable immutable blocks with CRC32C per 4 KiB chunk.
#![forbid(unsafe_code)]

use mammoth_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Seek, SeekFrom, Write},
    ops::Range,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct BlockStore {
    root: PathBuf,
}
#[derive(Serialize, Deserialize)]
struct Header {
    version: u32,
    len: u64,
    checksums: Vec<u32>,
}

/// One immutable block generation, checked as data is requested. Only checksum
/// chunks overlapping the requested range are read; unverified bytes never escape.
pub struct BlockReader {
    file: File,
    header: Header,
    path: PathBuf,
}
impl BlockReader {
    pub fn len(&self) -> u64 {
        self.header.len
    }
    pub fn is_empty(&self) -> bool {
        self.header.len == 0
    }
    pub fn read_range(&mut self, range: Range<u64>) -> Result<Vec<u8>> {
        if range.start > range.end || range.end > self.header.len {
            return Err(Error::InvalidInput("invalid block range".into()));
        }
        if range.is_empty() {
            return Ok(vec![]);
        }
        let start = range.start / 4096 * 4096;
        let end = range.end.div_ceil(4096).saturating_mul(4096).min(self.header.len);
        let mut bytes = vec![0; (end - start) as usize];
        self.file.seek(SeekFrom::Start(start))?;
        self.file.read_exact(&mut bytes)?;
        for (index, chunk) in bytes.chunks(4096).enumerate() {
            if crc32c::crc32c(chunk) != self.header.checksums[start as usize / 4096 + index] {
                return Err(Error::ChecksumMismatch {
                    path: self.path.clone(),
                    expected: "recorded CRC32C chunk".into(),
                    actual: "mismatched chunk".into(),
                });
            }
        }
        Ok(bytes[(range.start - start) as usize..(range.end - start) as usize].to_vec())
    }
}

/// Sync directory entries after publication on platforms supporting directory fsync.
pub fn sync_dir(path: &Path) -> Result<()> {
    #[cfg(unix)]
    File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

/// Replace a file atomically with fully synced contents.
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().ok_or_else(|| Error::InvalidInput("missing parent".into()))?;
    fs::create_dir_all(parent)?;
    let mut tmp = tempfile::NamedTempFile::new_in(parent)?;
    tmp.write_all(bytes)?;
    tmp.as_file().sync_all()?;
    tmp.persist(path).map_err(|e| e.error)?;
    sync_dir(parent)
}

impl BlockStore {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_owned();
        fs::create_dir_all(root.join("blocks"))?;
        fs::create_dir_all(root.join("tmp"))?;
        Ok(Self { root })
    }
    pub fn path(&self, id: u64) -> PathBuf {
        self.root
            .join("blocks")
            .join(format!("{:02x}", (id >> 8) & 255))
            .join(format!("{:02x}", id & 255))
            .join(format!("blk_{id:016x}"))
    }
    pub fn put(&self, id: u64, data: &[u8]) -> Result<()> {
        if data.len() > 256 * 1024 * 1024 {
            return Err(Error::InvalidInput("blocks are limited to 256 MiB".into()));
        }
        let dst = self.path(id);
        if dst.exists() {
            if self.read(id)? == data {
                return Ok(());
            }
            return Err(Error::AlreadyExists(dst));
        }
        let temp = tempfile::tempdir_in(self.root.join("tmp"))?;
        let mut file = File::create(temp.path().join("data"))?;
        file.write_all(data)?;
        file.sync_all()?;
        let header = Header {
            version: 1,
            len: data.len() as u64,
            checksums: data.chunks(4096).map(crc32c::crc32c).collect(),
        };
        // This directory is unpublished. The header needs a durable write, but
        // does not need its own atomic rename and extra directory sync.
        let mut meta = File::create(temp.path().join("meta.json"))?;
        meta.write_all(&serde_json::to_vec(&header).map_err(invalid_data)?)?;
        meta.sync_all()?;
        sync_dir(temp.path())?;
        let parent = dst.parent().expect("block parent");
        fs::create_dir_all(parent)?;
        // Persist every new level, not only the final directory entry.
        sync_dir(parent.parent().expect("shard parent"))?;
        sync_dir(&self.root.join("blocks"))?;
        fs::rename(temp.path(), &dst)?;
        sync_dir(parent)
    }
    pub fn read(&self, id: u64) -> Result<Vec<u8>> {
        let mut reader = self.open_reader(id)?;
        reader.read_range(0..reader.len())
    }
    pub fn open_reader(&self, id: u64) -> Result<BlockReader> {
        let dir = self.path(id);
        let header: Header =
            serde_json::from_slice(&fs::read(dir.join("meta.json"))?).map_err(invalid_data)?;
        if header.version != 1
            || header.len > 256 * 1024 * 1024
            || header.checksums.len() as u64 != header.len.div_ceil(4096)
        {
            return Err(Error::Io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "invalid block header",
            )));
        }
        let file = File::open(dir.join("data"))?;
        if file.metadata()?.len() != header.len {
            return Err(Error::ChecksumMismatch {
                path: dir,
                expected: format!("{} bytes with recorded CRC32C chunks", header.len),
                actual: "incorrect block length".into(),
            });
        }
        Ok(BlockReader { file, header, path: dir })
    }
    pub fn remove(&self, id: u64) -> Result<()> {
        match fs::remove_dir_all(self.path(id)) {
            Ok(()) => sync_dir(self.path(id).parent().expect("block parent")),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
    pub fn blocks(&self) -> Result<Vec<(u64, u64)>> {
        let mut out = Vec::new();
        for first in fs::read_dir(self.root.join("blocks"))? {
            for second in fs::read_dir(first?.path())? {
                for entry in fs::read_dir(second?.path())? {
                    let entry = entry?;
                    if let Some(id) = entry
                        .file_name()
                        .to_str()
                        .and_then(|s| s.strip_prefix("blk_"))
                        .and_then(|s| u64::from_str_radix(s, 16).ok())
                    {
                        let len =
                            fs::metadata(entry.path().join("data")).map(|m| m.len()).unwrap_or(0);
                        out.push((id, len));
                    }
                }
            }
        }
        out.sort_unstable();
        Ok(out)
    }
}
fn invalid_data(e: serde_json::Error) -> Error {
    Error::Io(std::io::Error::new(std::io::ErrorKind::InvalidData, e))
}
