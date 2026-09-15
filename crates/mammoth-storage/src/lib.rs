//! Durable immutable blocks with CRC32C per 4 KiB chunk.
#![forbid(unsafe_code)]

use mammoth_core::{Error, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    io::{Read, Write},
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
        atomic_write(
            &temp.path().join("meta.json"),
            &serde_json::to_vec(&header).map_err(invalid_data)?,
        )?;
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
        let mut data = Vec::new();
        File::open(dir.join("data"))?.take(header.len.saturating_add(1)).read_to_end(&mut data)?;
        let checksums: Vec<_> = data.chunks(4096).map(crc32c::crc32c).collect();
        if header.version != 1 || header.len != data.len() as u64 || header.checksums != checksums {
            return Err(Error::ChecksumMismatch {
                path: dir,
                expected: format!("{} bytes with recorded CRC32C chunks", header.len),
                actual: format!("{} bytes or mismatched chunks", data.len()),
            });
        }
        Ok(data)
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
