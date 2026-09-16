//! Bounded, verified reads of immutable blocks. A shared store lifetime lock
//! defers physical reclamation until the stream (including cancellation) ends.
use super::{cache::Key, invalid, Entry, LocalBackend};
use bytes::Bytes;
use mammoth_core::{backend::ReadSnapshot, Error, Result};
use mammoth_storage::BlockReader;
use std::{fs::File, ops::Range, sync::Arc};

struct Reader {
    backend: LocalBackend,
    entry: Entry,
    position: u64,
    end: u64,
    block_index: usize,
    block_start: u64,
    current: Option<(usize, BlockReader)>,
    _guard: Arc<File>,
    inline: Option<Bytes>,
    resident_chunks: usize,
}
impl Reader {
    fn next_resident(&mut self) -> Result<Option<Bytes>> {
        if self.inline.is_some() || self.position >= self.end {
            return self.next(false);
        }
        while self.block_index < self.entry.blocks.len() {
            let b = &self.entry.blocks[self.block_index];
            if self.position < self.block_start + b.len {
                break;
            }
            self.block_start += b.len;
            self.block_index += 1;
            self.current = None;
        }
        let block =
            self.entry.blocks.get(self.block_index).ok_or_else(|| invalid("missing file block"))?;
        let start = self.position - self.block_start;
        let end = (start + 64 * 1024).min(block.len).min(self.end - self.block_start);
        let bytes = self.backend.cache.get(Key { block: block.id.0, start, end });
        if let Some(bytes) = &bytes {
            self.position += bytes.len() as u64;
        }
        Ok(bytes)
    }
    fn next(&mut self, lookup: bool) -> Result<Option<Bytes>> {
        if self.position >= self.end {
            return Ok(None);
        }
        if let Some(bytes) = &self.inline {
            let end = (self.position + 64 * 1024).min(self.end);
            let out = bytes.slice(self.position as usize..end as usize);
            self.position = end;
            return Ok(Some(out));
        }
        while self.block_index < self.entry.blocks.len() {
            let block = &self.entry.blocks[self.block_index];
            if self.position < self.block_start + block.len {
                break;
            }
            self.block_start += block.len;
            self.block_index += 1;
            self.current = None;
        }
        let block =
            self.entry.blocks.get(self.block_index).ok_or_else(|| invalid("missing file block"))?;
        let from = self.position - self.block_start;
        let to = (from + 64 * 1024).min(block.len).min(self.end - self.block_start);
        let key = Key { block: block.id.0, start: from, end: to };
        if let Some(bytes) = lookup.then(|| self.backend.cache.get(key)).flatten() {
            self.position += bytes.len() as u64;
            return Ok(Some(bytes));
        }
        let mut last = invalid("no readable replica");
        let first = self.current.as_ref().map_or(0, |(index, _)| *index);
        // Start with the last healthy copy, but try every replica for this
        // chunk: a copy damaged earlier may still have this chunk intact.
        for attempt_index in 0..block.replicas.len() {
            let index = (first + attempt_index) % block.replicas.len();
            let attempt = (|| {
                if self.current.as_ref().is_none_or(|(current, _)| *current != index) {
                    let reader = self
                        .backend
                        .store(&block.replicas[index].node.0)?
                        .open_reader(block.id.0)?;
                    if reader.len() != block.len {
                        return Err(invalid("block length does not match namespace"));
                    }
                    self.current = Some((index, reader));
                }
                self.current.as_mut().expect("opened replica").1.read_range(from..to)
            })();
            match attempt {
                Ok(bytes) => {
                    self.position += bytes.len() as u64;
                    let bytes = Bytes::from(bytes);
                    self.backend.cache.insert(key, bytes.clone());
                    return Ok(Some(bytes));
                }
                Err(error) => {
                    last = error;
                    self.current = None;
                }
            }
        }
        Err(last)
    }
}
pub(super) fn open(
    backend: LocalBackend,
    mut entry: Entry,
    range: Range<u64>,
    guard: Arc<File>,
) -> Result<ReadSnapshot> {
    let start = range.start.min(entry.status.len);
    let end = range.end.min(entry.status.len);
    if let Some(bytes) = &entry.inline {
        let actual = format!("crc32c:{:08x}", crc32c::crc32c(bytes));
        if entry.status.checksum.as_ref() != Some(&actual) || bytes.len() as u64 != entry.status.len
        {
            return Err(Error::ChecksumMismatch {
                path: entry.status.path.clone(),
                expected: entry.status.checksum.clone().unwrap_or_default(),
                actual,
            });
        }
    } else {
        let total = entry
            .blocks
            .iter()
            .try_fold(0u64, |sum, b| sum.checked_add(b.len))
            .ok_or_else(|| invalid("block length overflow"))?;
        if total != entry.status.len {
            return Err(invalid("file length does not match its blocks"));
        }
    }
    let status = entry.status.clone();
    let etag = entry.etag.clone();
    let inline = entry.inline.take().map(Bytes::from);
    let mut reader = Reader {
        inline,
        resident_chunks: 0,
        backend,
        entry,
        position: start,
        end,
        block_index: 0,
        block_start: 0,
        current: None,
        _guard: guard,
    };
    // Surface a missing first requested block before sending HTTP success. Later
    // corruption terminates the stream, and is never delivered as file data.
    let first = reader.next(true)?;
    let data =
        futures_util::stream::try_unfold((Some(reader), first), |(reader, first)| async move {
            let Some(mut reader) = reader else {
                return Ok(None);
            };
            if let Some(first) = first {
                return Ok(Some((first, (Some(reader), None))));
            }
            if reader.position >= reader.end {
                return Ok(None);
            }
            if let Some(bytes) = reader.next_resident()? {
                reader.resident_chunks += 1;
                if reader.resident_chunks % 16 == 0 {
                    tokio::task::yield_now().await;
                }
                return Ok(Some((bytes, (Some(reader), None))));
            }
            tokio::task::spawn_blocking(move || {
                Ok(reader.next(false)?.map(|bytes| (bytes, (Some(reader), None))))
            })
            .await
            .map_err(invalid)?
        });
    Ok(ReadSnapshot { status, etag, range: start..end, data: Box::pin(data) })
}
