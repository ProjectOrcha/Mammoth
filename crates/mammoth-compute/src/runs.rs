//! Sorted scratch runs with bounded merge fan-in and per-record checksums.
use super::{failure, JobKind};
use bytes::Bytes;
use mammoth_core::{backend::ByteStream, Error, Result};
use std::{
    cmp::Reverse,
    collections::BinaryHeap,
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(super) struct Record {
    pub value: Bytes,
    pub count: u64,
}
#[derive(Default)]
pub(super) struct Counters {
    pub bytes: AtomicU64,
    pub records: AtomicU64,
}

pub(super) struct RunSet {
    directory: Option<PathBuf>,
    guard: Option<Arc<tempfile::TempDir>>,
    paths: Vec<PathBuf>,
    next: u64,
    kind: JobKind,
    pub spill_runs: u64,
    pub spilled_bytes: u64,
    pub merge_passes: u64,
}
impl RunSet {
    pub fn new(directory: Option<PathBuf>, kind: JobKind) -> Self {
        Self {
            directory,
            guard: None,
            paths: vec![],
            next: 0,
            kind,
            spill_runs: 0,
            spilled_bytes: 0,
            merge_passes: 0,
        }
    }
    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }
    fn destination(&mut self) -> Result<(PathBuf, Arc<tempfile::TempDir>)> {
        if self.guard.is_none() {
            let mut builder = tempfile::Builder::new();
            builder.prefix("mammoth-compute-");
            self.guard = Some(Arc::new(match &self.directory {
                Some(dir) => builder.tempdir_in(dir)?,
                None => builder.tempdir()?,
            }));
        }
        let guard = self.guard.as_ref().unwrap().clone();
        self.next += 1;
        Ok((guard.path().join(format!("run-{}", self.next)), guard))
    }
    pub async fn spill(&mut self, records: Vec<Record>) -> Result<()> {
        let (path, guard) = self.destination()?;
        let destination = path.clone();
        let kind = self.kind;
        let bytes = tokio::task::spawn_blocking(move || {
            let _guard = guard;
            write_run(&destination, records.into_iter().map(Ok), kind)
        })
        .await
        .map_err(failure)??;
        self.spill_runs += 1;
        self.spilled_bytes += bytes;
        self.paths.push(path);
        Ok(())
    }
    pub async fn finish(&mut self, budget: u64, max_record: usize) -> Result<Output> {
        // Head records and per-run input buffers fit within half the target,
        // excluding the fixed output buffer. The file-descriptor cap is independent.
        let read_buffer = (budget / 16).min(65536) as usize;
        let fan_in =
            ((budget / 2) / (max_record as u64 + read_buffer as u64 + 128)).clamp(2, 32) as usize;
        while self.paths.len() > fan_in {
            let old = std::mem::take(&mut self.paths);
            let mut next = vec![];
            for group in old.chunks(fan_in) {
                if group.len() == 1 {
                    next.push(group[0].clone());
                    continue;
                }
                let (path, guard) = self.destination()?;
                let destination = path.clone();
                let sources = group.to_vec();
                let kind = self.kind;
                let bytes = tokio::task::spawn_blocking(move || {
                    let _guard = guard;
                    let reader = Merge::open(&sources, kind, max_record, read_buffer)?;
                    let bytes = write_run(&destination, reader, kind)?;
                    for path in sources {
                        fs::remove_file(path)?;
                    }
                    Ok::<_, Error>(bytes)
                })
                .await
                .map_err(failure)??;
                self.spilled_bytes += bytes;
                next.push(path);
            }
            self.paths = next;
            self.merge_passes += 1;
        }
        let paths = self.paths.clone();
        let guard = self.guard.as_ref().expect("spilled runs").clone();
        let kind = self.kind;
        let output = tokio::task::spawn_blocking(move || {
            let reader = Merge::open(&paths, kind, max_record, read_buffer)?;
            Ok::<_, Error>(Output {
                source: Source::Disk(reader),
                kind,
                current: None,
                offset: 0,
                tail: vec![],
                tail_offset: 0,
                counters: Arc::new(Counters::default()),
                _guard: Some(guard),
            })
        })
        .await
        .map_err(failure)??;
        self.merge_passes += 1;
        Ok(output)
    }
}
fn magic(kind: JobKind) -> [u8; 5] {
    [b'M', b'M', b'R', 1, if kind == JobKind::Sort { 0 } else { 1 }]
}
fn checksum(value: &[u8], count: u64) -> u32 {
    crc32c::crc32c_append(crc32c::crc32c(&count.to_le_bytes()), value)
}
fn write_run(
    path: &std::path::Path,
    records: impl Iterator<Item = Result<Record>>,
    kind: JobKind,
) -> Result<u64> {
    let mut file = BufWriter::with_capacity(65536, File::create(path)?);
    file.write_all(&magic(kind))?;
    file.write_all(&0u64.to_le_bytes())?;
    let mut records_written = 0u64;
    let mut bytes = 13u64;
    for record in records {
        let record = record?;
        records_written =
            records_written.checked_add(1).ok_or_else(|| failure("spill record count overflow"))?;
        let len = u32::try_from(record.value.len()).map_err(failure)?;
        file.write_all(&len.to_le_bytes())?;
        file.write_all(&record.count.to_le_bytes())?;
        file.write_all(&checksum(&record.value, record.count).to_le_bytes())?;
        file.write_all(&record.value)?;
        bytes = bytes
            .checked_add(16 + len as u64)
            .ok_or_else(|| failure("spill byte count overflow"))?;
    }
    file.flush()?;
    file.seek(SeekFrom::Start(5))?;
    file.write_all(&records_written.to_le_bytes())?;
    file.flush()?;
    Ok(bytes)
}
struct RunReader {
    reader: BufReader<File>,
    remaining: u64,
    max_record: usize,
    kind: JobKind,
}
impl RunReader {
    fn open(
        path: &std::path::Path,
        kind: JobKind,
        max_record: usize,
        read_buffer: usize,
    ) -> Result<Self> {
        let mut reader = BufReader::with_capacity(read_buffer, File::open(path)?);
        let mut header = [0u8; 5];
        reader.read_exact(&mut header)?;
        if header != magic(kind) {
            return Err(failure("invalid spill format"));
        }
        let mut count = [0; 8];
        reader.read_exact(&mut count)?;
        Ok(Self { reader, remaining: u64::from_le_bytes(count), max_record, kind })
    }
    fn next(&mut self) -> Result<Option<Record>> {
        let mut header = [0u8; 16];
        // The header count also detects truncation on a record boundary.
        if self.remaining == 0 {
            if self.reader.read(&mut header[..1])? != 0 {
                return Err(failure("unexpected trailing spill data"));
            }
            return Ok(None);
        }
        self.reader.read_exact(&mut header)?;
        self.remaining -= 1;
        let len = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        let count = u64::from_le_bytes(header[4..12].try_into().unwrap());
        let crc = u32::from_le_bytes(header[12..].try_into().unwrap());
        if len > self.max_record || count == 0 || (self.kind == JobKind::Sort && count != 1) {
            return Err(failure("invalid spill record"));
        }
        let mut value = vec![0; len];
        self.reader.read_exact(&mut value)?;
        if checksum(&value, count) != crc {
            return Err(failure("spill checksum mismatch"));
        }
        Ok(Some(Record { value: Bytes::from(value), count }))
    }
}
struct Merge {
    readers: Vec<RunReader>,
    heap: BinaryHeap<Reverse<(Record, usize)>>,
    kind: JobKind,
    failed: bool,
}
impl Merge {
    fn open(
        paths: &[PathBuf],
        kind: JobKind,
        max_record: usize,
        read_buffer: usize,
    ) -> Result<Self> {
        let mut readers = Vec::new();
        let mut heap = BinaryHeap::new();
        for (index, path) in paths.iter().enumerate() {
            let mut reader = RunReader::open(path, kind, max_record, read_buffer)?;
            if let Some(record) = reader.next()? {
                heap.push(Reverse((record, index)));
            }
            readers.push(reader);
        }
        Ok(Self { readers, heap, kind, failed: false })
    }
    fn pop(&mut self) -> Result<Option<Record>> {
        let Some(Reverse((mut record, index))) = self.heap.pop() else {
            return Ok(None);
        };
        if let Some(next) = self.readers[index].next()? {
            self.heap.push(Reverse((next, index)));
        }
        if self.kind == JobKind::Wordcount {
            while self.heap.peek().is_some_and(|Reverse((next, _))| next.value == record.value) {
                let Reverse((next, index)) = self.heap.pop().unwrap();
                record.count = record
                    .count
                    .checked_add(next.count)
                    .ok_or_else(|| failure("word count overflow"))?;
                if let Some(next) = self.readers[index].next()? {
                    self.heap.push(Reverse((next, index)));
                }
            }
        }
        Ok(Some(record))
    }
}
impl Iterator for Merge {
    type Item = Result<Record>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.failed {
            return None;
        }
        match self.pop() {
            Ok(value) => value.map(Ok),
            Err(error) => {
                self.failed = true;
                Some(Err(error))
            }
        }
    }
}
enum Source {
    Memory(std::vec::IntoIter<Record>),
    Disk(Merge),
}
pub(super) struct Output {
    source: Source,
    kind: JobKind,
    current: Option<Record>,
    offset: usize,
    tail: Vec<u8>,
    tail_offset: usize,
    pub counters: Arc<Counters>,
    _guard: Option<Arc<tempfile::TempDir>>,
}
impl Output {
    pub fn memory(records: Vec<Record>, kind: JobKind) -> Self {
        Self {
            source: Source::Memory(records.into_iter()),
            kind,
            current: None,
            offset: 0,
            tail: vec![],
            tail_offset: 0,
            counters: Arc::new(Counters::default()),
            _guard: None,
        }
    }
    fn next_chunk(&mut self) -> Result<Option<Bytes>> {
        let mut output = Vec::with_capacity(65536);
        while output.len() < 65536 {
            if self.current.is_none() {
                self.current = match &mut self.source {
                    Source::Memory(rows) => rows.next(),
                    Source::Disk(rows) => rows.next().transpose()?,
                };
                let Some(record) = &self.current else {
                    break;
                };
                self.offset = 0;
                self.tail_offset = 0;
                self.tail = if self.kind == JobKind::Wordcount {
                    format!("\t{}\n", record.count).into_bytes()
                } else {
                    vec![b'\n']
                };
                self.counters.records.fetch_add(1, Ordering::Relaxed);
            }
            let record = self.current.as_ref().unwrap();
            let count = (65536 - output.len()).min(record.value.len() - self.offset);
            output.extend_from_slice(&record.value[self.offset..self.offset + count]);
            self.offset += count;
            if self.offset == record.value.len() {
                let count = (65536 - output.len()).min(self.tail.len() - self.tail_offset);
                output.extend_from_slice(&self.tail[self.tail_offset..self.tail_offset + count]);
                self.tail_offset += count;
                if self.tail_offset == self.tail.len() {
                    self.current = None;
                }
            }
        }
        self.counters.bytes.fetch_add(output.len() as u64, Ordering::Relaxed);
        if output.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Bytes::from(output)))
        }
    }
    pub fn stream(self) -> ByteStream {
        Box::pin(futures_util::stream::try_unfold(self, |mut reader| async move {
            if matches!(reader.source, Source::Memory(_)) {
                return Ok(reader.next_chunk()?.map(|bytes| (bytes, reader)));
            }
            // The scratch-directory guard stays with detached blocking work on
            // cancellation, so cleanup never races a spill read or merge.
            tokio::task::spawn_blocking(move || {
                Ok(reader.next_chunk()?.map(|bytes| (bytes, reader)))
            })
            .await
            .map_err(failure)?
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_corruption_truncation_and_invalid_counts_before_emitting_record() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run");
        write_run(
            &path,
            [Ok(Record { value: Bytes::from_static(b"hello"), count: 7 })].into_iter(),
            JobKind::Wordcount,
        )
        .unwrap();
        let original = fs::read(&path).unwrap();
        for offset in [13, 17, 25, 29] {
            let mut damaged = original.clone();
            damaged[offset] ^= 255;
            fs::write(&path, damaged).unwrap();
            let mut reader = RunReader::open(&path, JobKind::Wordcount, 10, 4096).unwrap();
            assert!(reader.next().is_err());
        }
        for length in 13..original.len() {
            fs::write(&path, &original[..length]).unwrap();
            assert!(RunReader::open(&path, JobKind::Wordcount, 10, 4096).unwrap().next().is_err());
        }
        fs::write(&path, &original).unwrap();
        assert!(RunReader::open(&path, JobKind::Sort, 10, 4096).is_err());
        assert!(RunReader::open(&path, JobKind::Wordcount, 4, 4096).unwrap().next().is_err());
        let mut reader = RunReader::open(&path, JobKind::Wordcount, 10, 4096).unwrap();
        assert_eq!(reader.next().unwrap().unwrap().count, 7);
        assert!(reader.next().unwrap().is_none());
    }

    #[tokio::test]
    async fn dropped_output_stream_removes_all_scratch_runs() {
        let dir = tempfile::tempdir().unwrap();
        let mut runs = RunSet::new(Some(dir.path().into()), JobKind::Sort);
        runs.spill(vec![Record { value: Bytes::from_static(b"hello"), count: 1 }]).await.unwrap();
        let output = runs.finish(65536, 5).await.unwrap().stream();
        drop(runs);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        drop(output);
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
