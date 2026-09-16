//! Local text workload with exact, bounded-memory validation outside timing.
use super::{measure, Options, Sample};
use bytes::Bytes;
use futures_util::StreamExt;
use mammoth_compute::{run_with_options, JobKind};
use mammoth_core::{Backend, Error, Result};
use mammoth_local::LocalBackend;
use std::{
    path::Path,
    sync::{Arc, Mutex},
};
const WIDTH: u64 = 25;
fn key(index: u64, seed: u64) -> usize {
    seed.wrapping_add(index.wrapping_mul(4051)) as usize % 4096
}
fn input(records: u64, seed: u64) -> mammoth_core::backend::ByteStream {
    Box::pin(futures_util::stream::unfold(0u64, move |mut index| async move {
        if index == records {
            return None;
        }
        let mut bytes = String::with_capacity(65536);
        while index < records && bytes.len() + WIDTH as usize <= 65536 {
            bytes.push_str(&format!("{:04} mammoth rust memory\n", key(index, seed)));
            index += 1;
        }
        Some((Ok(Bytes::from(bytes)), index))
    }))
}
fn counts(records: u64, seed: u64) -> Vec<u64> {
    let mut counts = vec![records / 4096; 4096];
    for index in 0..records % 4096 {
        counts[key(index, seed)] += 1;
    }
    counts
}
async fn validate(
    be: &LocalBackend,
    path: &str,
    kind: JobKind,
    records: u64,
    seed: u64,
) -> Result<()> {
    let mut expected = counts(records, seed);
    if kind == JobKind::Wordcount {
        let mut output = String::new();
        for (key, count) in expected.iter().enumerate() {
            if *count > 0 {
                output.push_str(&format!("{key:04}\t{count}\n"));
            }
        }
        output.push_str(&format!("mammoth\t{records}\nmemory\t{records}\nrust\t{records}\n"));
        let mut offset = 0;
        let mut stream = be.read(Path::new(path), 0..u64::MAX).await?;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            if output.as_bytes().get(offset..offset + chunk.len()) != Some(&chunk[..]) {
                return Err(invalid());
            }
            offset += chunk.len();
        }
        if offset != output.len() {
            return Err(invalid());
        }
        return Ok(());
    }
    let mut stream = be.read(Path::new(path), 0..u64::MAX).await?;
    let mut pending = Vec::new();
    let mut previous = 0;
    let mut seen = 0u64;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        pending.extend_from_slice(&chunk);
        let whole = pending.len() / WIDTH as usize * WIDTH as usize;
        for line in pending[..whole].chunks_exact(WIDTH as usize) {
            if &line[4..] != b" mammoth rust memory\n" {
                return Err(invalid());
            }
            let index = std::str::from_utf8(&line[..4])
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
                .filter(|n| *n < 4096)
                .ok_or_else(invalid)?;
            if index < previous || expected[index] == 0 {
                return Err(invalid());
            }
            previous = index;
            expected[index] -= 1;
            seen += 1;
        }
        pending.drain(..whole);
    }
    if !pending.is_empty() || seen != records || expected.iter().any(|n| *n != 0) {
        return Err(invalid());
    }
    Ok(())
}
fn invalid() -> Error {
    Error::InvalidInput("compute benchmark output failed exact validation".into())
}
pub(super) async fn iteration(
    be: &LocalBackend,
    opts: &Options,
    replication: u8,
    iteration: usize,
) -> Result<Vec<Sample>> {
    let records = opts.file_size / WIDTH;
    for i in 0..opts.files {
        be.write(
            Path::new(&format!("/compute-{i}")),
            input(records, opts.seed.wrapping_add(i as u64)),
        )
        .await?;
    }
    let mut samples = Vec::new();
    for (phase, kind) in [("sort", JobKind::Sort), ("wordcount", JobKind::Wordcount)] {
        be.clear_read_cache();
        let metrics = Arc::new(Mutex::new(Vec::new()));
        let before = be.cache_stats().unwrap_or_default();
        let mut sample = measure(
            phase,
            replication,
            iteration,
            opts.files,
            opts.concurrency,
            records * WIDTH,
            |i| {
                let metrics = metrics.clone();
                async move {
                    let result = run_with_options(
                        be,
                        format!("/compute-{i}").into(),
                        format!("/result-{i}").into(),
                        kind,
                        mammoth_compute::Options {
                            memory_budget: opts.compute_memory_budget,
                            spill_directory: Some(be.root().join("staging")),
                        },
                    )
                    .await?;
                    metrics.lock().unwrap().push(result.metrics);
                    Ok(())
                }
            },
        )
        .await?;
        let after = be.cache_stats().unwrap_or_default();
        sample.cache = Some(
            serde_json::json!({"hits":after.hits-before.hits,"misses":after.misses-before.misses,"resident_bytes":after.resident_bytes,"capacity_bytes":after.capacity_bytes}),
        );
        sample.compute = Some(
            serde_json::json!({"jobs":*metrics.lock().unwrap(),"record_bytes":WIDTH,"records_per_file":records}),
        );
        // Validate every record and frequency, not just sortedness or output size.
        for i in 0..opts.files {
            validate(be, &format!("/result-{i}"), kind, records, opts.seed.wrapping_add(i as u64))
                .await?;
            be.remove(Path::new(&format!("/result-{i}")), false).await?;
        }
        samples.push(sample);
    }
    for i in 0..opts.files {
        be.remove(Path::new(&format!("/compute-{i}")), false).await?;
    }
    be.gc().await?;
    Ok(samples)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn validation_rejects_missing_duplicate_unsorted_and_wrong_frequency_results() {
        let dir = tempfile::tempdir().unwrap();
        let be = LocalBackend::open(dir.path()).unwrap();
        for text in [
            "0001 mammoth rust memory\n",
            "0001 mammoth rust memory\n0001 mammoth rust memory\n",
            "4052 mammoth rust memory\n0001 mammoth rust memory\n",
            "0001 mammoth rust memory\n4052 mammoth rust memory\nx",
        ] {
            be.write(Path::new("/out"), mammoth_local::body(text)).await.unwrap();
            assert!(validate(&be, "/out", JobKind::Sort, 2, 1).await.is_err());
        }
        be.write(
            Path::new("/out"),
            mammoth_local::body("0001 mammoth rust memory\n4052 mammoth rust memory\n"),
        )
        .await
        .unwrap();
        validate(&be, "/out", JobKind::Sort, 2, 1).await.unwrap();
        be.write(
            Path::new("/out"),
            mammoth_local::body("0001\t1\n4052\t1\nmammoth\t1\nmemory\t2\nrust\t2\n"),
        )
        .await
        .unwrap();
        assert!(validate(&be, "/out", JobKind::Wordcount, 2, 1).await.is_err());
    }
}
