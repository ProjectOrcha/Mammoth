//! Bounded local text jobs. Distributed DAG execution and shuffle remain roadmap work.
#![forbid(unsafe_code)]
use futures_util::StreamExt;
use mammoth_core::{Backend, Error, Result};
use serde::Serialize;
use std::{collections::BTreeMap, path::PathBuf};
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Wordcount,
    Sort,
}
#[derive(Debug, Serialize)]
pub struct JobResult {
    pub input: PathBuf,
    pub output: PathBuf,
    pub kind: JobKind,
    pub state: &'static str,
    pub execution: &'static str,
}
pub async fn run_local(
    be: &dyn Backend,
    input: PathBuf,
    output_path: PathBuf,
    kind: JobKind,
) -> Result<JobResult> {
    let sort = matches!(kind, JobKind::Sort);
    if mammoth_core::path::normalize(&input)? == mammoth_core::path::normalize(&output_path)? {
        return Err(Error::InvalidInput("job output must differ from input".into()));
    }
    let s = be.stat(&input).await?;
    if s.len > 64 * 1024 * 1024 {
        return Err(Error::InvalidInput("local text jobs support inputs up to 64 MiB".into()));
    }
    let mut stream = be.read(&input, 0..u64::MAX).await?;
    let mut bytes = vec![];
    while let Some(c) = stream.next().await {
        bytes.extend_from_slice(&c?);
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(Error::InvalidInput("job input exceeded 64 MiB".into()));
        }
    }
    let text = String::from_utf8(bytes)
        .map_err(|_| Error::InvalidInput("text jobs require UTF-8 input".into()))?;
    let result = tokio::task::spawn_blocking(move || {
        if sort {
            let mut lines: Vec<_> = text.lines().collect();
            lines.sort_unstable();
            if lines.is_empty() {
                String::new()
            } else {
                format!("{}\n", lines.join("\n"))
            }
        } else {
            let mut counts = BTreeMap::<String, u64>::new();
            for word in text.split_whitespace() {
                *counts.entry(word.to_string()).or_default() += 1;
            }
            counts.into_iter().map(|(word, count)| format!("{word}\t{count}\n")).collect::<String>()
        }
    })
    .await
    .map_err(|e| Error::Io(std::io::Error::other(e)))?;
    be.write(
        &output_path,
        Box::pin(futures_util::stream::once(async move { Ok(bytes::Bytes::from(result)) })),
    )
    .await?;
    Ok(JobResult { input, output: output_path, kind, state: "succeeded", execution: "local" })
}
