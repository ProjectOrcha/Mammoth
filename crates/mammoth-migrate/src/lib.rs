//! Local tree import/export with streaming transfers and atomic downloaded files.
#![forbid(unsafe_code)]
use futures_util::{StreamExt, TryStreamExt};
use mammoth_core::{backend::ByteStream, Backend, Error, Result};
use serde::Serialize;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;
pub enum Transfer {
    Import { source: PathBuf, destination: PathBuf },
    Export { source: PathBuf, destination: PathBuf },
}
#[derive(Debug, Serialize)]
pub struct TransferReport {
    pub files: u64,
    pub bytes: u64,
    pub status: &'static str,
}
pub async fn transfer(be: &dyn Backend, command: Transfer) -> Result<TransferReport> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    match command {
        Transfer::Import { source, destination } => {
            if tokio::fs::symlink_metadata(&source).await?.file_type().is_symlink() {
                return Err(Error::InvalidInput(format!(
                    "import does not follow symlinks: {}",
                    source.display()
                )));
            }
            let source = tokio::fs::canonicalize(source).await?;
            let metadata = tokio::fs::metadata(&source).await?;
            let mut queue = vec![(source.clone(), destination)];
            if !metadata.is_file() && !metadata.is_dir() {
                return Err(Error::InvalidInput(
                    "import source must be a regular file or directory".into(),
                ));
            }
            while let Some((local, remote)) = queue.pop() {
                let metadata = tokio::fs::symlink_metadata(&local).await?;
                if metadata.file_type().is_symlink() {
                    return Err(Error::InvalidInput(format!(
                        "import does not follow symlinks: {}",
                        local.display()
                    )));
                }
                if metadata.is_dir() {
                    be.mkdir(&remote, true).await?;
                    let mut entries = tokio::fs::read_dir(&local).await?;
                    while let Some(e) = entries.next_entry().await? {
                        queue.push((e.path(), remote.join(e.file_name())));
                    }
                } else if metadata.is_file() {
                    be.write(&remote, input(&local).await?).await?;
                    files += 1;
                    bytes += metadata.len();
                } else {
                    return Err(Error::InvalidInput(format!(
                        "unsupported import entry: {}",
                        local.display()
                    )));
                }
            }
        }
        Transfer::Export { source, destination } => {
            let source = PathBuf::from(mammoth_core::path::normalize(&source)?);
            if tokio::fs::try_exists(&destination).await? {
                return Err(Error::AlreadyExists(destination));
            }
            let entries = mammoth_core::backend::walk(be, &source).await?;
            for s in entries {
                let relative =
                    s.path.strip_prefix(&source).map_err(|e| Error::InvalidInput(e.to_string()))?;
                let target = if relative == Path::new("") {
                    destination.clone()
                } else {
                    destination.join(relative)
                };
                if s.is_dir {
                    tokio::fs::create_dir_all(&target).await?;
                } else {
                    if let Some(parent) = target.parent() {
                        tokio::fs::create_dir_all(parent).await?;
                    }
                    download(be, &s.path, &target, false).await?;
                    files += 1;
                    bytes += s.len;
                }
            }
        }
    }
    Ok(TransferReport { files, bytes, status: "complete" })
}
fn join_error(e: tokio::task::JoinError) -> Error {
    Error::Io(std::io::Error::other(e))
}
pub async fn input(path: &Path) -> Result<ByteStream> {
    if path == Path::new("-") {
        Ok(Box::pin(ReaderStream::new(tokio::io::stdin()).map_err(Error::from)))
    } else {
        Ok(Box::pin(ReaderStream::new(tokio::fs::File::open(path).await?).map_err(Error::from)))
    }
}
pub async fn download(be: &dyn Backend, src: &Path, dst: &Path, force: bool) -> Result<()> {
    let parent = dst.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let tmp = tempfile::NamedTempFile::new_in(parent)?;
    let mut file = tokio::fs::File::from_std(tmp.reopen()?);
    let mut stream = be.read(src, 0..u64::MAX).await?;
    while let Some(c) = stream.next().await {
        file.write_all(&c?).await?;
    }
    file.sync_all().await?;
    drop(file);
    let dst = dst.to_owned();
    tokio::task::spawn_blocking(move || {
        if force { tmp.persist(dst) } else { tmp.persist_noclobber(dst) }
            .map_err(|e| Error::Io(e.error))
            .map(|_| ())
    })
    .await
    .map_err(join_error)?
}
