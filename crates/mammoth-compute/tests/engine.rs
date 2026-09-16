use futures_util::{stream, StreamExt};
use mammoth_compute::{run_with_options, JobKind, Options};
use mammoth_core::{Backend, Error};
use mammoth_local::{body, LocalBackend};
use std::{collections::BTreeMap, path::Path};
async fn data(be: &LocalBackend, path: &str) -> Vec<u8> {
    let mut stream = be.read(Path::new(path), 0..u64::MAX).await.unwrap();
    let mut out = vec![];
    while let Some(chunk) = stream.next().await {
        out.extend_from_slice(&chunk.unwrap());
    }
    out
}
fn oracle(text: &str, kind: JobKind) -> Vec<u8> {
    match kind {
        JobKind::Sort => {
            let mut lines: Vec<_> = text.lines().collect();
            lines.sort_unstable();
            if lines.is_empty() {
                vec![]
            } else {
                format!("{}\n", lines.join("\n")).into_bytes()
            }
        }
        JobKind::Wordcount => {
            let mut counts = BTreeMap::new();
            for word in text.split_whitespace() {
                *counts.entry(word).or_insert(0u64) += 1;
            }
            counts
                .into_iter()
                .map(|(word, count)| format!("{word}\t{count}\n"))
                .collect::<String>()
                .into_bytes()
        }
    }
}
#[tokio::test]
async fn memory_and_multi_pass_spill_match_sequential_unicode_oracle() {
    let store = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(store.path())
        .unwrap()
        .with_block_size(4096)
        .with_inline_threshold(0)
        .with_replication(1);
    let text = (0..12000)
        .rev()
        .map(|i| format!("{:04} 雪\tβéta mammoth\u{2003}rust\r\n", i % 119))
        .collect::<String>();
    be.write(Path::new("/in"), body(text.clone())).await.unwrap();
    for kind in [JobKind::Sort, JobKind::Wordcount] {
        let expected = oracle(&text, kind);
        for budget in [64 * 1024, 64 * 1024 * 1024] {
            let result = run_with_options(
                &be,
                "/in".into(),
                "/out".into(),
                kind,
                Options { memory_budget: budget, spill_directory: Some(scratch.path().into()) },
            )
            .await
            .unwrap();
            assert_eq!(data(&be, "/out").await, expected);
            assert_eq!(result.metrics.output_bytes, expected.len() as u64);
            assert_eq!(result.metrics.input_bytes, text.len() as u64);
            if budget == 64 * 1024 {
                assert_eq!(result.metrics.mode, "spill");
                assert!(result.metrics.spill_runs > 2);
                assert!(result.metrics.merge_passes > 1);
            } else {
                assert_eq!(result.metrics.mode, "memory");
                assert_eq!(result.metrics.spill_runs, 0);
            }
            assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
        }
    }
}
#[tokio::test]
async fn empty_crlf_final_line_duplicates_and_split_utf8_match_text_semantics() {
    let dir = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(dir.path()).unwrap();
    for text in ["", "\n", "\r\n", "a\r", "b\na\r\n\na\n", "雪 β\u{a0}雪\nlast"] {
        let chunks: Vec<_> =
            text.as_bytes().iter().map(|b| Ok(bytes::Bytes::from(vec![*b]))).collect();
        be.write(Path::new("/in"), Box::pin(stream::iter(chunks))).await.unwrap();
        for kind in [JobKind::Sort, JobKind::Wordcount] {
            let result =
                run_with_options(&be, "/in".into(), "/out".into(), kind, Options::default())
                    .await
                    .unwrap();
            assert_eq!(data(&be, "/out").await, oracle(text, kind), "{text:?} {kind:?}");
            assert_eq!(result.metrics.mode, "memory");
        }
    }
}
#[tokio::test]
async fn invalid_input_and_record_limit_preserve_output_and_clean_spills() {
    let dir = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(dir.path()).unwrap();
    be.write(Path::new("/out"), body("keep")).await.unwrap();
    for input in [vec![b'a'; 10000], [b"many words\n".repeat(2000), vec![255]].concat()] {
        be.write(Path::new("/in"), body(input)).await.unwrap();
        assert!(run_with_options(
            &be,
            "/in".into(),
            "/out".into(),
            JobKind::Wordcount,
            Options { memory_budget: 65536, spill_directory: Some(scratch.path().into()) }
        )
        .await
        .is_err());
        assert_eq!(data(&be, "/out").await, b"keep");
        assert_eq!(std::fs::read_dir(scratch.path()).unwrap().count(), 0);
    }
    assert!(matches!(
        run_with_options(&be, "/in".into(), "/in".into(), JobKind::Sort, Options::default()).await,
        Err(Error::InvalidInput(_))
    ));
}

// Generate a large logical input without storing or timing a large disk fixture.
struct GeneratedInput {
    chunks: Vec<bytes::Bytes>,
    pending: bool,
    output: std::sync::Mutex<Vec<u8>>,
}
#[async_trait::async_trait]
impl Backend for GeneratedInput {
    async fn read(
        &self,
        _: &Path,
        _: std::ops::Range<u64>,
    ) -> mammoth_core::Result<mammoth_core::backend::ByteStream> {
        let chunks = stream::iter(self.chunks.clone().into_iter().map(Ok));
        if self.pending {
            Ok(Box::pin(chunks.chain(stream::pending())))
        } else {
            Ok(Box::pin(chunks))
        }
    }
    async fn write(
        &self,
        _: &Path,
        mut input: mammoth_core::backend::ByteStream,
    ) -> mammoth_core::Result<()> {
        let mut output = vec![];
        while let Some(chunk) = input.next().await {
            output.extend_from_slice(&chunk?);
        }
        *self.output.lock().unwrap() = output;
        Ok(())
    }
    async fn list(&self, _: &Path) -> mammoth_core::Result<Vec<mammoth_core::types::FileStatus>> {
        unreachable!()
    }
    async fn stat(&self, _: &Path) -> mammoth_core::Result<mammoth_core::types::FileStatus> {
        unreachable!()
    }
    async fn remove(&self, _: &Path, _: bool) -> mammoth_core::Result<()> {
        unreachable!()
    }
    async fn block_layout(
        &self,
        _: &Path,
    ) -> mammoth_core::Result<Vec<mammoth_core::types::BlockPlacement>> {
        unreachable!()
    }
    async fn cluster_report(&self) -> mammoth_core::Result<mammoth_core::types::ClusterReport> {
        unreachable!()
    }
}
#[tokio::test]
async fn streams_more_than_the_former_64_mib_limit() {
    let mut line = vec![b' '; 1024 * 1024];
    *line.last_mut().unwrap() = b'\n';
    let mut chunks = vec![bytes::Bytes::from(line); 65];
    chunks.push(bytes::Bytes::from_static(b"done\n"));
    let be = GeneratedInput { chunks, pending: false, output: Default::default() };
    let result =
        run_with_options(&be, "/in".into(), "/out".into(), JobKind::Wordcount, Options::default())
            .await
            .unwrap();
    assert_eq!(result.metrics.input_bytes, 65 * 1024 * 1024 + 5);
    assert_eq!(result.metrics.input_records, 1);
    assert_eq!(*be.output.lock().unwrap(), b"done\t1\n");
}
#[tokio::test]
async fn cancelled_input_releases_spills_without_replacing_output() {
    let scratch = tempfile::tempdir().unwrap();
    let be = std::sync::Arc::new(GeneratedInput {
        chunks: vec![bytes::Bytes::from(b"many words\n".repeat(2000))],
        pending: true,
        output: std::sync::Mutex::new(b"keep".to_vec()),
    });
    let options = Options { memory_budget: 65536, spill_directory: Some(scratch.path().into()) };
    let task_be = be.clone();
    let task = tokio::spawn(async move {
        run_with_options(task_be.as_ref(), "/in".into(), "/out".into(), JobKind::Wordcount, options)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while std::fs::read_dir(scratch.path()).unwrap().count() == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    // Detached blocking work owns the final scratch guard until it finishes.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        while std::fs::read_dir(scratch.path()).unwrap().count() != 0 {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(*be.output.lock().unwrap(), b"keep");
}

#[tokio::test]
async fn job_publication_requires_explicit_overwrite() {
    use mammoth_compute::run_with_output_policy;
    let dir = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(dir.path()).unwrap();
    be.write(Path::new("/in"), body("b\na\n")).await.unwrap();
    be.write(Path::new("/out"), body("keep")).await.unwrap();
    assert!(matches!(
        run_with_output_policy(
            &be,
            "/in".into(),
            "/out".into(),
            JobKind::Sort,
            Options::default(),
            false
        )
        .await,
        Err(Error::AlreadyExists(_))
    ));
    assert_eq!(data(&be, "/out").await, b"keep");
    run_with_output_policy(
        &be,
        "/in".into(),
        "/out".into(),
        JobKind::Sort,
        Options::default(),
        true,
    )
    .await
    .unwrap();
    assert_eq!(data(&be, "/out").await, b"a\nb\n");
}
