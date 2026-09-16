//! Memory-first parallel text jobs with bounded batches and sorted disk runs.
//! Execution stays on one host; these are not distributed Spark/TeraSort jobs.
#![forbid(unsafe_code)]
mod runs;

use bytes::Bytes;
use futures_util::StreamExt;
use mammoth_core::{Backend, Error, Result};
use rayon::prelude::*;
use runs::{Output, Record, RunSet};
use serde::Serialize;
use std::{collections::HashMap, path::PathBuf, sync::OnceLock, time::Instant};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Wordcount,
    Sort,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// Working-set target, including a conservative per-record charge. Not RSS.
    pub memory_budget: u64,
    /// Recomputable scratch files; no fsync is required for these intermediates.
    pub spill_directory: Option<PathBuf>,
}
impl Default for Options {
    fn default() -> Self {
        Self { memory_budget: 128 * 1024 * 1024, spill_directory: None }
    }
}
impl Options {
    pub fn from_config(config: &mammoth_core::config::Compute) -> Result<Self> {
        let options = Self {
            memory_budget: mammoth_core::config::parse_size(&config.memory_budget)?,
            spill_directory: if config.spill_directory.is_empty() {
                None
            } else {
                Some(config.spill_directory.clone().into())
            },
        };
        options.validate()?;
        Ok(options)
    }

    pub fn validate(&self) -> Result<()> {
        if !(64 * 1024..=4 * 1024 * 1024 * 1024).contains(&self.memory_budget) {
            return Err(Error::InvalidInput("compute memory budget must be 64 KiB–4 GiB".into()));
        }
        Ok(())
    }
}
#[derive(Debug, Default, Serialize)]
pub struct Metrics {
    pub input_bytes: u64,
    /// Lines for sort, whitespace-delimited tokens for word count.
    pub input_records: u64,
    pub output_bytes: u64,
    pub output_records: u64,
    pub spill_runs: u64,
    pub spilled_bytes: u64,
    pub merge_passes: u64,
    pub worker_threads: usize,
    pub memory_budget: u64,
    pub elapsed_seconds: f64,
    pub mode: &'static str,
}
#[derive(Debug, Serialize)]
pub struct JobResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub kind: JobKind,
    pub state: &'static str,
    pub execution: &'static str,
    pub metrics: Metrics,
}
fn failure(error: impl std::fmt::Display) -> Error {
    Error::Io(std::io::Error::other(error.to_string()))
}
fn pool() -> Result<&'static rayon::ThreadPool> {
    static POOL: OnceLock<std::result::Result<rayon::ThreadPool, String>> = OnceLock::new();
    POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(std::thread::available_parallelism().map_or(1, |n| n.get()).min(32))
            .thread_name(|n| format!("mammoth-compute-{n}"))
            .build()
            .map_err(|e| e.to_string())
    })
    .as_ref()
    .map_err(failure)
}
#[derive(Default)]
struct Batch {
    data: Vec<u8>,
    ranges: Vec<std::ops::Range<usize>>,
    cost: u64,
}
impl Batch {
    fn push(&mut self, record: &[u8]) {
        let start = self.data.len();
        self.data.extend_from_slice(record);
        self.ranges.push(start..self.data.len());
        self.cost += record.len() as u64 + 128;
    }
    fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}
async fn process(batch: Batch, kind: JobKind) -> Result<Vec<Record>> {
    tokio::task::spawn_blocking(move || {
        pool()?.install(|| {
            let data = Bytes::from(batch.data);
            let mut records: Vec<Record> = match kind {
                JobKind::Sort => batch
                    .ranges
                    .into_par_iter()
                    .map(|range| Record { value: data.slice(range), count: 1 })
                    .collect(),
                JobKind::Wordcount => {
                    // Borrow keys from packed input buffers. Allocate no String per
                    // token and combine partition-local counts before merging.
                    let counts = batch
                        .ranges
                        .par_iter()
                        .fold(HashMap::<&[u8], (usize, u64)>::new, |mut counts, range| {
                            let entry =
                                counts.entry(&data[range.clone()]).or_insert((range.start, 0));
                            entry.1 += 1;
                            counts
                        })
                        .reduce(HashMap::new, |mut left, right| {
                            for (key, (start, count)) in right {
                                left.entry(key).or_insert((start, 0)).1 += count;
                            }
                            left
                        });
                    counts
                        .into_iter()
                        .map(|(key, (start, count))| Record {
                            value: data.slice(start..start + key.len()),
                            count,
                        })
                        .collect()
                }
            };
            records.par_sort_unstable_by(|a, b| a.value.cmp(&b.value));
            Ok(records)
        })
    })
    .await
    .map_err(failure)?
}

struct Builder {
    batch: Batch,
    runs: RunSet,
    kind: JobKind,
    target: u64,
    metrics: Metrics,
    max_record: usize,
}
impl Builder {
    async fn record(&mut self, record: &[u8]) -> Result<()> {
        if !self.batch.is_empty() && self.batch.cost + record.len() as u64 + 128 > self.target {
            self.flush().await?;
        }
        self.max_record = self.max_record.max(record.len());
        self.batch.push(record);
        self.metrics.input_records += 1;
        Ok(())
    }
    async fn line(&mut self, line: &[u8]) -> Result<()> {
        let text = std::str::from_utf8(line)
            .map_err(|_| Error::InvalidInput("text jobs require UTF-8 input".into()))?;
        match self.kind {
            JobKind::Sort => self.record(text.as_bytes()).await?,
            JobKind::Wordcount => {
                for word in text.split_whitespace() {
                    self.record(word.as_bytes()).await?;
                }
            }
        }
        Ok(())
    }
    async fn flush(&mut self) -> Result<()> {
        if self.batch.is_empty() {
            return Ok(());
        }
        let records = process(std::mem::take(&mut self.batch), self.kind).await?;
        self.runs.spill(records).await?;
        Ok(())
    }
}

pub async fn run_local(
    be: &dyn Backend,
    input: PathBuf,
    output: PathBuf,
    kind: JobKind,
) -> Result<JobResult> {
    run_with_options(be, input, output, kind, Options::default()).await
}
pub async fn run_with_options(
    be: &dyn Backend,
    input: PathBuf,
    output: PathBuf,
    kind: JobKind,
    options: Options,
) -> Result<JobResult> {
    run_with_output_policy(be, input, output, kind, options, true).await
}

/// Run a job with atomic destination publication. With overwrite=false, a file
/// created by another client during processing is preserved.
pub async fn run_with_output_policy(
    be: &dyn Backend,
    input: PathBuf,
    output: PathBuf,
    kind: JobKind,
    options: Options,
    overwrite: bool,
) -> Result<JobResult> {
    options.validate()?;
    if mammoth_core::path::normalize(&input)? == mammoth_core::path::normalize(&output)? {
        return Err(Error::InvalidInput("job output must differ from input".into()));
    }
    let started = Instant::now();
    let max_line = (options.memory_budget / 8).min(16 * 1024 * 1024) as usize;
    let mut builder = Builder {
        batch: Batch::default(),
        runs: RunSet::new(options.spill_directory.clone(), kind),
        kind,
        target: options.memory_budget / 4,
        max_record: 0,
        metrics: Metrics {
            memory_budget: options.memory_budget,
            worker_threads: pool()?.current_num_threads(),
            ..Metrics::default()
        },
    };
    // One committed generation, even if the input path is overwritten mid-job.
    let mut stream = be.read(&input, 0..u64::MAX).await?;
    let mut line = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        builder.metrics.input_bytes = builder
            .metrics
            .input_bytes
            .checked_add(chunk.len() as u64)
            .ok_or_else(|| failure("input size overflow"))?;
        let mut start = 0;
        for (index, byte) in chunk.iter().enumerate() {
            if *byte == b'\n' {
                append_line(&mut line, &chunk[start..index], max_line)?;
                if line.last() == Some(&b'\r') {
                    line.pop();
                }
                builder.line(&line).await?;
                line.clear();
                start = index + 1;
            }
        }
        append_line(&mut line, &chunk[start..], max_line)?;
    }
    if !line.is_empty() {
        builder.line(&line).await?;
    }
    drop(line);
    drop(stream);
    let output_reader = if builder.runs.is_empty() {
        let rows = process(builder.batch, kind).await?;
        builder.metrics.mode = "memory";
        Output::memory(rows, kind)
    } else {
        builder.flush().await?;
        builder.metrics.mode = "spill";
        let reader = builder.runs.finish(options.memory_budget, builder.max_record).await?;
        builder.metrics.spill_runs = builder.runs.spill_runs;
        builder.metrics.spilled_bytes = builder.runs.spilled_bytes;
        builder.metrics.merge_passes = builder.runs.merge_passes;
        reader
    };
    let counters = output_reader.counters.clone();
    if overwrite {
        be.write(&output, output_reader.stream()).await?;
    } else {
        be.create(&output, output_reader.stream()).await?;
    }
    builder.metrics.output_bytes = counters.bytes.load(std::sync::atomic::Ordering::Relaxed);
    builder.metrics.output_records = counters.records.load(std::sync::atomic::Ordering::Relaxed);
    builder.metrics.elapsed_seconds = started.elapsed().as_secs_f64();
    Ok(JobResult {
        input,
        output,
        kind,
        state: "succeeded",
        execution: "local",
        metrics: builder.metrics,
    })
}
fn append_line(line: &mut Vec<u8>, bytes: &[u8], limit: usize) -> Result<()> {
    if line.len().saturating_add(bytes.len()) > limit {
        return Err(Error::InvalidInput(format!("text record exceeds {limit} bytes; raise compute memory budget or split the line (maximum 16 MiB per line)")));
    }
    line.extend_from_slice(bytes);
    Ok(())
}
