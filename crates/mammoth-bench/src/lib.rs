//! DFSIO-inspired and namespace benchmarks for the implemented local backend.
//! No network, distributed shuffle or Hadoop speedup is inferred from these runs.
#![forbid(unsafe_code)]
mod compute;
use futures_util::{stream, StreamExt};
use mammoth_core::{Backend, Error, Result};
use mammoth_local::LocalBackend;
use serde::{Deserialize, Serialize};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::{
        atomic::{AtomicU32, Ordering},
        Arc,
    },
    time::{Instant, SystemTime, UNIX_EPOCH},
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Workload {
    Suite,
    Dfsio,
    Metadata,
    Compute,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Options {
    pub workload: Workload,
    pub file_size: u64,
    pub files: usize,
    pub concurrency: usize,
    /// Number of files in EACH metadata phase (create, stat, rename, delete).
    pub operations: usize,
    pub iterations: usize,
    pub warmups: usize,
    pub replications: Vec<u8>,
    pub block_size: u64,
    pub seed: u64,
    pub read_cache_size: u64,
    pub compute_memory_budget: u64,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            workload: Workload::Suite,
            file_size: 8 * 1024 * 1024,
            files: 8,
            concurrency: 4,
            operations: 200,
            iterations: 3,
            warmups: 1,
            replications: vec![1, 3],
            block_size: 4 * 1024 * 1024,
            seed: 42,
            read_cache_size: 256 * 1024 * 1024,
            compute_memory_budget: 32 * 1024 * 1024,
        }
    }
}
impl Options {
    pub fn validate(&self) -> Result<()> {
        if self.read_cache_size > 1024 * 1024 * 1024
            || !(64 * 1024..=256 * 1024 * 1024).contains(&self.compute_memory_budget)
        {
            return Err(Error::InvalidInput("benchmark memory limits: read cache 0–1 GiB, compute target 64 KiB–256 MiB per job".into()));
        }
        if matches!(self.workload, Workload::Suite | Workload::Compute)
            && (self.file_size < 25
                || self.compute_memory_budget.saturating_mul(self.concurrency as u64)
                    > 1024 * 1024 * 1024)
        {
            return Err(Error::InvalidInput("compute benchmarks need at least 25 bytes/file and at most 1 GiB of aggregate job memory targets".into()));
        }
        if !(1..=32).contains(&self.concurrency)
            || !(1..=1024).contains(&self.files)
            || !(1..=100_000).contains(&self.operations)
            || !(1..=10).contains(&self.iterations)
            || self.warmups > 3
            || self.file_size == 0
            || self.file_size > 1024 * 1024 * 1024
            || self.block_size == 0
            || self.block_size > 256 * 1024 * 1024
            || self.seed == 0
            || self.replications.is_empty()
            || self.replications.len() > 6
            || self.replications.iter().any(|n| !(1..=6).contains(n))
            || self.replications.iter().enumerate().any(|(i, n)| self.replications[..i].contains(n))
        {
            return Err(Error::InvalidInput("benchmark limits: 1–32 clients, 1–1024 files, 1 B–1 GiB/file, 1–100000 metadata files, 1–10 iterations, 0–3 warmups, distinct replicas 1–6, block size 1 B–256 MiB, nonzero seed".into()));
        }
        let replicated = self.file_size
            * self.files as u64
            * u64::from(*self.replications.iter().max().unwrap());
        if self.workload != Workload::Metadata && replicated > 8 * 1024 * 1024 * 1024 {
            return Err(Error::InvalidInput(
                "benchmark data is limited to 8 GiB including replicas per iteration".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Sample {
    pub phase: String,
    pub replication: u8,
    pub iteration: usize,
    pub operations: usize,
    pub bytes: u64,
    pub wall_seconds: f64,
    pub ops_per_second: f64,
    /// Logical bytes / wall time, NOT TestDFSIO's sum-of-task-times metric.
    pub aggregate_mib_s: Option<f64>,
    pub task_time_mib_s: Option<f64>,
    pub mean_task_mib_s: Option<f64>,
    pub latency_p50_ms: f64,
    pub latency_p95_ms: f64,
    pub latency_p99_ms: f64,
    pub latency_max_ms: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute: Option<serde_json::Value>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Summary {
    pub phase: String,
    pub replication: u8,
    pub unit: String,
    pub median: f64,
    pub min: f64,
    pub max: f64,
    /// Median of each measured iteration's p99, not a pooled percentile.
    pub median_p99_ms: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema_version: u32,
    pub id: String,
    pub timestamp_ms: u64,
    pub scope: String,
    pub environment: serde_json::Value,
    pub options: Options,
    pub samples: Vec<Sample>,
    pub summary: Vec<Summary>,
    pub verified: bool,
    pub cleanup_complete: bool,
    pub notes: Vec<String>,
}

/// An OS lock also prevents CLI and dashboard runs from distorting one another.
pub fn acquire(parent: &Path) -> Result<File> {
    std::fs::create_dir_all(parent)?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(parent.join("run.lock"))?;
    fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
        if error.raw_os_error() == fs2::lock_contended_error().raw_os_error() {
            Error::InvalidInput("a benchmark is already running for this store".into())
        } else {
            Error::Io(error)
        }
    })?;
    Ok(file)
}

fn percentile(values: &mut [f64], percent: usize) -> f64 {
    values.sort_by(f64::total_cmp);
    values[(values.len() * percent).div_ceil(100).saturating_sub(1).min(values.len() - 1)]
}
fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    if values.len().is_multiple_of(2) {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    }
}
fn summarize(samples: &[Sample]) -> Vec<Summary> {
    let mut keys: Vec<_> = samples.iter().map(|s| (s.phase.clone(), s.replication)).collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .map(|(phase, replication)| {
            let rows: Vec<_> = samples
                .iter()
                .filter(|s| s.phase == phase && s.replication == replication)
                .collect();
            let mut rates: Vec<_> =
                rows.iter().map(|s| s.aggregate_mib_s.unwrap_or(s.ops_per_second)).collect();
            let mut tails: Vec<_> = rows.iter().map(|s| s.latency_p99_ms).collect();
            let mid = median(&mut rates);
            Summary {
                phase,
                replication,
                unit: if rows[0].aggregate_mib_s.is_some() { "MiB/s" } else { "ops/s" }.into(),
                median: mid,
                min: rates[0],
                max: rates[rates.len() - 1],
                median_p99_ms: median(&mut tails),
            }
        })
        .collect()
}

/// Drain every in-flight operation even on error before the isolated store is removed.
async fn measure<F, Fut>(
    phase: &str,
    replication: u8,
    iteration: usize,
    count: usize,
    concurrency: usize,
    bytes_per_op: u64,
    f: F,
) -> Result<Sample>
where
    F: Fn(usize) -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    let start = Instant::now();
    let timings: Vec<_> = stream::iter(0..count)
        .map(|i| {
            let future = f(i);
            async move {
                let start = Instant::now();
                future.await.map(|_| start.elapsed().as_secs_f64().max(f64::EPSILON))
            }
        })
        .buffer_unordered(concurrency)
        .collect()
        .await;
    let wall = start.elapsed().as_secs_f64().max(f64::EPSILON);
    let mut timings: Vec<f64> = timings.into_iter().collect::<Result<_>>()?;
    let bytes = bytes_per_op * count as u64;
    let task_time = timings.iter().sum::<f64>();
    let mean_task =
        timings.iter().map(|s| bytes_per_op as f64 / 1048576.0 / s).sum::<f64>() / count as f64;
    Ok(Sample {
        phase: phase.into(),
        replication,
        iteration,
        operations: count,
        bytes,
        wall_seconds: wall,
        ops_per_second: count as f64 / wall,
        aggregate_mib_s: (bytes > 0).then_some(bytes as f64 / 1048576.0 / wall),
        task_time_mib_s: (bytes > 0).then_some(bytes as f64 / 1048576.0 / task_time),
        mean_task_mib_s: (bytes > 0).then_some(mean_task),
        latency_p50_ms: percentile(&mut timings, 50) * 1000.0,
        latency_p95_ms: percentile(&mut timings, 95) * 1000.0,
        latency_p99_ms: percentile(&mut timings, 99) * 1000.0,
        cache: None,
        compute: None,
        latency_max_ms: timings[timings.len() - 1] * 1000.0,
    })
}

/// Deterministic xorshift64 stream; no compressible constant-filled files or full-file buffers.
fn payload(size: u64, seed: u64, checksum: Arc<AtomicU32>) -> mammoth_core::backend::ByteStream {
    Box::pin(stream::unfold((size, seed, 0u32), move |(remaining, mut rng, crc)| {
        let checksum = checksum.clone();
        async move {
            if remaining == 0 {
                return None;
            }
            let mut bytes = vec![0u8; remaining.min(64 * 1024) as usize];
            for chunk in bytes.chunks_mut(8) {
                rng ^= rng << 13;
                rng ^= rng >> 7;
                rng ^= rng << 17;
                chunk.copy_from_slice(&rng.to_le_bytes()[..chunk.len()]);
            }
            let crc = crc32c::crc32c_append(crc, &bytes);
            checksum.store(crc, Ordering::Relaxed);
            Some((Ok(bytes::Bytes::from(bytes)), (remaining - bytes_len(remaining), rng, crc)))
        }
    }))
}
fn bytes_len(remaining: u64) -> u64 {
    remaining.min(64 * 1024)
}

async fn verify(be: &LocalBackend, path: &Path, size: u64, expected: u32) -> Result<()> {
    let mut stream = be.read(path, 0..u64::MAX).await?;
    let (mut total, mut crc) = (0u64, 0);
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        total += chunk.len() as u64;
        crc = crc32c::crc32c_append(crc, &chunk);
    }
    if total != size || crc != expected {
        return Err(Error::InvalidInput(format!(
            "benchmark verification failed: {} (length or CRC32C mismatch)",
            path.display()
        )));
    }
    Ok(())
}

async fn iteration(
    parent: &Path,
    opts: &Options,
    replication: u8,
    iteration: usize,
) -> Result<Vec<Sample>> {
    let temp = tempfile::Builder::new().prefix("scratch-").tempdir_in(parent)?;
    let be = LocalBackend::open(temp.path())?
        .with_block_size(opts.block_size)
        .with_inline_threshold(0)
        .with_cache_size(opts.read_cache_size)
        .with_replication(replication);
    let mut samples = vec![];
    if matches!(opts.workload, Workload::Suite | Workload::Dfsio) {
        let checksums: Vec<_> = (0..opts.files).map(|_| Arc::new(AtomicU32::new(0))).collect();
        samples.push(
            measure(
                "write",
                replication,
                iteration,
                opts.files,
                opts.concurrency,
                opts.file_size,
                |i| {
                    let be = &be;
                    let crc = checksums[i].clone();
                    async move {
                        be.write(
                            Path::new(&format!("/io-{i}")),
                            payload(opts.file_size, opts.seed.wrapping_add(i as u64).max(1), crc),
                        )
                        .await
                    }
                },
            )
            .await?,
        );
        // Placement inspection is outside the timing window and ensures inline writes
        // cannot silently bypass the replication comparison.
        for i in 0..opts.files {
            let path = format!("/io-{i}");
            let stat = be.stat(Path::new(&path)).await?;
            let blocks = be.block_layout(Path::new(&path)).await?;
            if stat.len != opts.file_size
                || stat.inlined
                || blocks.is_empty()
                || blocks.iter().any(|b| b.replicas.len() != replication as usize)
            {
                return Err(Error::InvalidInput("benchmark placement verification failed".into()));
            }
        }
        be.clear_read_cache();
        for phase in ["read", "read_cached"] {
            let before = be.cache_stats().unwrap_or_default();
            let mut sample =
                measure(
                    phase,
                    replication,
                    iteration,
                    opts.files,
                    opts.concurrency,
                    opts.file_size,
                    |i| {
                        let be = &be;
                        let crc = checksums[i].load(Ordering::Relaxed);
                        async move {
                            verify(be, Path::new(&format!("/io-{i}")), opts.file_size, crc).await
                        }
                    },
                )
                .await?;
            let after = be.cache_stats().unwrap_or_default();
            sample.cache = Some(
                serde_json::json!({"hits":after.hits-before.hits,"misses":after.misses-before.misses,"evictions":after.evictions-before.evictions,"resident_bytes":after.resident_bytes,"capacity_bytes":after.capacity_bytes}),
            );
            samples.push(sample);
        }
        for i in 0..opts.files {
            be.remove(Path::new(&format!("/io-{i}")), false).await?;
        }
        be.gc().await?;
    }
    if matches!(opts.workload, Workload::Suite | Workload::Metadata) {
        for phase in ["create", "stat", "rename", "delete"] {
            samples.push(
                measure(phase, replication, iteration, opts.operations, opts.concurrency, 0, |i| {
                    let be = &be;
                    async move {
                        let path = format!("/meta-{i}");
                        let moved = format!("/moved-{i}");
                        match phase {
                            "create" => {
                                be.write(Path::new(&path), mammoth_local::body(vec![])).await
                            }
                            "stat" => be.stat(Path::new(&path)).await.map(|_| ()),
                            "rename" => be.rename(Path::new(&path), Path::new(&moved)).await,
                            _ => be.remove(Path::new(&moved), false).await,
                        }
                    }
                })
                .await?,
            );
        }
    }
    if matches!(opts.workload, Workload::Suite | Workload::Compute) {
        samples.extend(compute::iteration(&be, opts, replication, iteration).await?);
    }
    if !be.list(Path::new("/")).await?.is_empty() {
        return Err(Error::InvalidInput("benchmark namespace cleanup failed".into()));
    }
    // Close SQLite/WAL handles before deleting the store (also on Windows).
    drop(be);
    temp.close()?;
    Ok(samples)
}

pub async fn run(parent: &Path, opts: Options) -> Result<Report> {
    opts.validate()?;
    let lock = acquire(parent)?;
    run_locked(parent, opts, lock).await
}

/// Caller may acquire first to reject concurrent dashboard submissions immediately.
pub async fn run_locked(parent: &Path, opts: Options, _lock: File) -> Result<Report> {
    opts.validate()?;
    let timestamp_ms =
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
    let mut samples = vec![];
    // Rotate replication order between rounds to reduce systematic ordering bias.
    for round in 0..opts.warmups + opts.iterations {
        for offset in 0..opts.replications.len() {
            let replication = opts.replications[(round + offset) % opts.replications.len()];
            let rows =
                iteration(parent, &opts, replication, round.saturating_sub(opts.warmups) + 1)
                    .await?;
            if round >= opts.warmups {
                samples.extend(rows);
            }
        }
    }
    let report = Report {
        schema_version: 1, id: format!("bench-{timestamp_ms}-{}",std::process::id()),timestamp_ms,
        scope: "local / one host / six simulated worker directories / direct Backend calls".into(),
        environment: serde_json::json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "logical_cpus":std::thread::available_parallelism().map(|n|n.get()).unwrap_or(1),
            "profile":if cfg!(debug_assertions) {"debug"} else {"release"},
            "mammoth_version":env!("CARGO_PKG_VERSION"),"data_directory":parent.canonicalize()?,
            "inline_threshold":0,"physical_hosts":1,"simulated_workers":6,
            "engine":"local-memory-parallel-v3","namespace_format":2,"upload_slots":8,
            "metadata_durability":"SQLite WAL synchronous=FULL",
            "read_cache_size":opts.read_cache_size,"compute_memory_budget":opts.compute_memory_budget,
            "network_bytes":null,"network_saturation":null,"shuffle_bytes":null}),
        summary: summarize(&samples), samples, options: opts, verified: true, cleanup_complete: true,
        notes: vec![
            "DFSIO-inspired, not Hadoop TestDFSIO. Aggregate MiB/s = logical bytes / phase wall time; task_time_mib_s = bytes / sum of operation durations; mean_task_mib_s = mean per-file rate.".into(),
            "Write timing includes seeded pseudorandom generation, CRC32C, bounded streaming, concurrent durable replica writes and namespace commit. Read timing includes full byte-count and CRC32C verification.".into(),
            "Reads follow writes; OS caches are not flushed. The read phase starts with an empty Mammoth cache; read_cached reuses it. Cache counters reveal hits and evictions; neither phase claims cold-disk bandwidth.".into(),
            "Metadata creates empty files, stats, renames and deletes them; it measures the indexed SQLite WAL namespace, not NameNode RPC latency. Empty files do not exercise replication.".into(),
            "Warmups excluded; all measured iterations retained. Latencies exclude waiting for a client slot but include backend contention (eight upload slots per backend; one SQLite writer). Fresh isolated store per iteration; setup and cleanup excluded.".into(),
            "Compute uses fixed 25-byte UTF-8 records (requested size rounded down), parallel line sort and word count with verified outputs. Setup and validation are outside timing; job memory targets, spill bytes and run counts are recorded. This is local compute, not distributed TeraSort.".into(),
            "This report measures Mammoth only; external baselines are separate artifacts. No physical network, distributed shuffle or TeraSort result. Rust alone implies no performance advantage.".into(),
        ],
    };
    let reports = parent.join("reports");
    std::fs::create_dir_all(&reports)?;
    write_report(&reports.join(format!("{}.json", report.id)), &report)?;
    Ok(report)
}

pub fn write_report(path: &Path, report: &Report) -> Result<()> {
    let data = serde_json::to_vec_pretty(report).map_err(|e| Error::InvalidInput(e.to_string()))?;
    mammoth_storage::atomic_write(path, &data)
}

pub fn history(parent: &Path) -> Result<Vec<Report>> {
    let dir = parent.join("reports");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut paths = std::fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<_>>>()?;
    paths.retain(|p| p.extension().is_some_and(|e| e == "json"));
    paths.sort();
    paths.reverse();
    paths
        .into_iter()
        .take(20)
        .map(|p| {
            serde_json::from_slice(&std::fs::read(p)?)
                .map_err(|e| Error::InvalidInput(format!("invalid benchmark report: {e}")))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn suite_verifies_replicas_counts_repeats_and_cleans_up() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("keep"), b"user data").unwrap();
        let opts = Options {
            file_size: 131_079,
            files: 3,
            operations: 4,
            block_size: 65536,
            iterations: 2,
            warmups: 1,
            ..Options::default()
        };
        let report = run(dir.path(), opts).await.unwrap();
        assert_eq!(report.samples.len(), 36);
        assert_eq!(report.summary.len(), 18);
        assert!(report.verified && report.cleanup_complete);
        for row in &report.samples {
            assert!(
                row.latency_p50_ms <= row.latency_p99_ms
                    && row.latency_p99_ms <= row.latency_max_ms
            );
            assert_eq!(row.operations, if row.bytes > 0 { 3 } else { 4 });
            if let Some(rate) = row.aggregate_mib_s {
                assert!((rate - row.bytes as f64 / 1048576.0 / row.wall_seconds).abs() < 1e-6);
            }
        }
        assert_eq!(history(dir.path()).unwrap().len(), 1);
        assert_eq!(std::fs::read(dir.path().join("keep")).unwrap(), b"user data");
        assert!(!std::fs::read_dir(dir.path()).unwrap().any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with("scratch-")));
    }
    #[tokio::test]
    async fn detects_truncation_and_wrong_checksum() {
        let dir = tempfile::tempdir().unwrap();
        let be = LocalBackend::open(dir.path()).unwrap();
        be.write(Path::new("/a"), mammoth_local::body(b"abc".to_vec())).await.unwrap();
        assert!(verify(&be, Path::new("/a"), 4, crc32c::crc32c(b"abc")).await.is_err());
        assert!(verify(&be, Path::new("/a"), 3, 0).await.is_err());
    }
    #[test]
    fn rejects_unbounded_runs_and_overlapping_runs() {
        for opts in [
            Options { concurrency: 0, ..Options::default() },
            Options { replications: vec![3, 3], ..Options::default() },
            Options { file_size: u64::MAX, ..Options::default() },
        ] {
            assert!(opts.validate().is_err());
        }
        let dir = tempfile::tempdir().unwrap();
        let lock = acquire(dir.path()).unwrap();
        assert!(
            matches!(acquire(dir.path()), Err(Error::InvalidInput(message)) if message.contains("already running"))
        );
        drop(lock);
        assert!(acquire(dir.path()).is_ok());
        assert_eq!(percentile(&mut [1., 2., 3., 4.], 99), 4.);
        assert_eq!(median(&mut [4., 1., 2., 3.]), 2.5);
    }
}
