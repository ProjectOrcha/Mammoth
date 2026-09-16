use futures_util::{stream, StreamExt};
use mammoth_core::{Backend, Error};
use mammoth_local::{body, LocalBackend};
use mammoth_storage::BlockStore;
use std::{collections::HashSet, path::Path};

async fn bytes(be: &dyn Backend, path: &str, range: std::ops::Range<u64>) -> Vec<u8> {
    let mut s = be.read(Path::new(path), range).await.unwrap();
    let mut out = vec![];
    while let Some(c) = s.next().await {
        out.extend_from_slice(&c.unwrap());
    }
    out
}
fn open(d: &tempfile::TempDir) -> LocalBackend {
    LocalBackend::open(d.path())
        .unwrap()
        .with_cache_size(0)
        .with_block_size(100)
        .with_inline_threshold(10)
}

#[tokio::test]
async fn roundtrip_inline_blocks_ranges_and_restart() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    for (name, data) in [
        ("empty", vec![]),
        ("notes #?% 雪.mmeta", b"hello".to_vec()),
        ("big", (0..350).map(|n| (n % 251) as u8).collect()),
    ] {
        let path = format!("/data/{name}");
        be.write(Path::new(&path), body(data.clone())).await.unwrap();
        assert_eq!(bytes(&be, &path, 0..u64::MAX).await, data);
        assert_eq!(bytes(&open(&dir), &path, 0..u64::MAX).await, data);
    }
    let layout = be.block_layout(Path::new("/data/big")).await.unwrap();
    assert_eq!(layout.len(), 4);
    assert_eq!(layout[3].len, 50);
    for b in layout {
        assert_eq!(b.replicas.iter().map(|r| &r.rack).collect::<HashSet<_>>().len(), 3);
    }
    assert_eq!(bytes(&be, "/data/big", 95..105).await, (95..105).collect::<Vec<u8>>());
    assert!(bytes(&be, "/data/big", 999..u64::MAX).await.is_empty());
    assert!(be.read(Path::new("/data/big"), std::ops::Range { start: 20, end: 10 }).await.is_err());
    assert_eq!(be.cluster_report().await.unwrap().used, 1050);
}

#[tokio::test]
async fn failed_overwrite_and_concurrent_writes_preserve_committed_files() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    be.write(Path::new("/keep"), body(b"original".to_vec())).await.unwrap();
    let failed = Box::pin(stream::iter(vec![
        Ok(bytes::Bytes::from(vec![1; 200])),
        Err(Error::InvalidInput("interrupted".into())),
    ]));
    assert!(be.write(Path::new("/keep"), failed).await.is_err());
    assert_eq!(bytes(&be, "/keep", 0..u64::MAX).await, b"original");
    let mut tasks = vec![];
    for n in 0..20 {
        let b = open(&dir);
        tasks.push(tokio::spawn(async move {
            b.write(Path::new(&format!("/parallel/{n}")), body(vec![n; 350])).await.unwrap();
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(be.list(Path::new("/parallel")).await.unwrap().len(), 20);
    // Streaming uploads may leave unpublished blocks after a failed input.
    // GC must reclaim them without touching committed concurrent files.
    assert!(be.gc().await.unwrap() > 0);
    assert_eq!(be.gc().await.unwrap(), 0);
    for n in 0..20 {
        assert_eq!(bytes(&be, &format!("/parallel/{n}"), 0..u64::MAX).await, vec![n; 350]);
    }
}

#[tokio::test]
async fn corrupted_and_missing_replicas_fall_back_then_repair() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    let payload = vec![42; 350];
    be.write(Path::new("/big"), body(payload.clone())).await.unwrap();
    let layout = be.block_layout(Path::new("/big")).await.unwrap();
    let b = &layout[0];
    let first = BlockStore::open(dir.path().join("workers").join(&b.replicas[0].node.0)).unwrap();
    std::fs::write(first.path(b.id.0).join("data"), vec![0; 100]).unwrap();
    let second = BlockStore::open(dir.path().join("workers").join(&b.replicas[1].node.0)).unwrap();
    second.remove(b.id.0).unwrap();
    assert_eq!(bytes(&be, "/big", 0..u64::MAX).await, payload);
    assert_eq!(be.cluster_report().await.unwrap().health.critical, 1);
    assert_eq!(be.repair().await.unwrap(), 2);
    assert_eq!(be.cluster_report().await.unwrap().health.healthy, 4);
    for r in &b.replicas {
        BlockStore::open(dir.path().join("workers").join(&r.node.0))
            .unwrap()
            .remove(b.id.0)
            .unwrap();
    }
    assert!(be.read(Path::new("/big"), 0..100).await.is_err());
    assert!(be.repair().await.is_err());
}

#[tokio::test]
async fn namespace_conflicts_rename_and_recursive_cleanup() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    be.mkdir(Path::new("/empty"), false).await.unwrap();
    assert!(be.list(Path::new("/empty")).await.unwrap().is_empty());
    for path in ["/../escape", "a/../../escape", "a\0b"] {
        assert!(be.write(Path::new(path), body(vec![1])).await.is_err());
    }
    #[cfg(not(windows))]
    assert!(be.write(Path::new("a\\b"), body(vec![1])).await.is_err());
    be.write(Path::new("/data/sub/a"), body(vec![9; 350])).await.unwrap();
    assert!(be.write(Path::new("/data/sub/a/b"), body(vec![1])).await.is_err());
    assert!(be.write(Path::new("/data/sub"), body(vec![1])).await.is_err());
    assert!(be.remove(Path::new("/data"), false).await.is_err());
    assert!(be.rename(Path::new("/data"), Path::new("/data/sub/z")).await.is_err());
    be.rename(Path::new("/data"), Path::new("/moved")).await.unwrap();
    assert_eq!(bytes(&be, "/moved/sub/a", 0..u64::MAX).await, vec![9; 350]);
    be.set_replication(Path::new("/moved/sub/a"), 2).await.unwrap();
    assert_eq!(be.cluster_report().await.unwrap().used, 700);
    be.remove(Path::new("/moved"), true).await.unwrap();
    assert_eq!(be.cluster_report().await.unwrap().used, 0);
    assert_eq!(be.stat(Path::new("/missing")).await.unwrap_err().code(), "E0101");
}

#[tokio::test]
async fn read_snapshot_keeps_the_original_generation_after_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    be.write(Path::new("/file"), body(vec![1; 350])).await.unwrap();
    let mut snapshot = be.open_read(Path::new("/file"), 95..105).await.unwrap();
    let etag = snapshot.etag.clone();
    be.write(Path::new("/file"), body(vec![2; 40])).await.unwrap();
    assert_eq!(snapshot.status.len, 350);
    assert_eq!(snapshot.range, 95..105);
    assert_ne!(etag, be.etag(Path::new("/file")).await.unwrap());
    let mut got = vec![];
    while let Some(chunk) = snapshot.data.next().await {
        got.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(got, vec![1; 10]);
}

#[tokio::test]
async fn metadata_progresses_during_paused_upload_and_snapshots_defer_gc() {
    use std::time::Duration;
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    be.write(Path::new("/keep"), body(vec![1; 350])).await.unwrap();
    let snapshot = be.open_read(Path::new("/keep"), 0..u64::MAX).await.unwrap();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    let waiting = stream::once(async move {
        receiver
            .await
            .map(|_| bytes::Bytes::from_static(b"done"))
            .map_err(|e| Error::InvalidInput(e.to_string()))
    });
    let upload = {
        let be = be.clone();
        tokio::spawn(async move { be.write(Path::new("/waiting"), Box::pin(waiting)).await })
    };
    tokio::time::timeout(Duration::from_secs(5), async {
        be.mkdir(Path::new("/concurrent"), false).await.unwrap();
        assert_eq!(be.stat(Path::new("/keep")).await.unwrap().len, 350);
        be.remove(Path::new("/keep"), false).await.unwrap();
        assert_eq!(be.gc().await.unwrap(), 0);
        assert!(be.repair().await.is_err());
    })
    .await
    .expect("metadata and deletion must not wait for readers or uploads");
    sender.send(()).unwrap();
    upload.await.unwrap().unwrap();
    let mut data = snapshot.data;
    let mut got = vec![];
    while let Some(chunk) = data.next().await {
        got.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(got, vec![1; 350]);
    drop(data);
    assert_eq!(be.gc().await.unwrap(), 12);
    assert_eq!(bytes(&be, "/waiting", 0..u64::MAX).await, b"done");
}

#[tokio::test]
async fn ranges_verify_only_requested_chunks_and_retry_late_corruption() {
    let dir = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(dir.path())
        .unwrap()
        .with_block_size(256 * 1024)
        .with_inline_threshold(0);
    let payload: Vec<_> = (0..200_000).map(|i| (i % 251) as u8).collect();
    be.write(Path::new("/file"), body(payload.clone())).await.unwrap();
    let block = be.block_layout(Path::new("/file")).await.unwrap().remove(0);
    use std::io::{Seek, SeekFrom, Write};
    for replica in &block.replicas {
        let store = BlockStore::open(dir.path().join("workers").join(&replica.node.0)).unwrap();
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(store.path(block.id.0).join("data"))
            .unwrap();
        f.seek(SeekFrom::Start(100)).unwrap();
        f.write_all(&[255]).unwrap();
    }
    // A damaged prefix must not cause a suffix read to scan or reject that prefix.
    assert_eq!(bytes(&be, "/file", 199_900..200_000).await, &payload[199_900..]);
    assert!(be.read(Path::new("/file"), 0..1).await.is_err());
    // Restore all, then corrupt a later chunk on only the preferred replica.
    for replica in &block.replicas {
        let store = BlockStore::open(dir.path().join("workers").join(&replica.node.0)).unwrap();
        store.remove(block.id.0).unwrap();
        store.put(block.id.0, &payload).unwrap();
    }
    let store =
        BlockStore::open(dir.path().join("workers").join(&block.replicas[0].node.0)).unwrap();
    let mut f =
        std::fs::OpenOptions::new().write(true).open(store.path(block.id.0).join("data")).unwrap();
    f.seek(SeekFrom::Start(100_000)).unwrap();
    f.write_all(&[255]).unwrap();
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, payload);
    assert_eq!(std::fs::read_dir(dir.path().join("staging")).unwrap().count(), 0);
}

#[tokio::test]
async fn literal_namespace_ranges_and_cancelled_uploads_preserve_data() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    for path in ["/a%_/one", "/aXX/two", "/A%_/three"] {
        be.write(Path::new(path), body(vec![7; 350])).await.unwrap();
    }
    be.rename(Path::new("/a%_"), Path::new("/moved")).await.unwrap();
    assert_eq!(be.list(Path::new("/moved")).await.unwrap().len(), 1);
    be.remove(Path::new("/moved"), true).await.unwrap();
    assert_eq!(bytes(&be, "/aXX/two", 0..u64::MAX).await, vec![7; 350]);
    assert_eq!(bytes(&be, "/A%_/three", 0..u64::MAX).await, vec![7; 350]);
    let (started, signal) = tokio::sync::oneshot::channel();
    let input = stream::once(async { Ok(bytes::Bytes::from(vec![9; 350])) }).chain(stream::once(
        async move {
            started.send(()).unwrap();
            std::future::pending::<mammoth_core::Result<bytes::Bytes>>().await
        },
    ));
    let upload = {
        let be = be.clone();
        tokio::spawn(async move { be.write(Path::new("/aXX/two"), Box::pin(input)).await })
    };
    signal.await.unwrap();
    upload.abort();
    assert!(upload.await.unwrap_err().is_cancelled());
    assert_eq!(bytes(&be, "/aXX/two", 0..u64::MAX).await, vec![7; 350]);
    assert!(be.gc().await.unwrap() > 0);
    assert_eq!(bytes(&open(&dir), "/A%_/three", 0..u64::MAX).await, vec![7; 350]);
}

#[tokio::test]
async fn migrates_legacy_namespace_and_rejects_missing_database() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    be.write(Path::new("/small"), body(b"hello".to_vec())).await.unwrap();
    be.write(Path::new("/large"), body(vec![3; 350])).await.unwrap();
    let root = be.stat(Path::new("/")).await.unwrap();
    let small = be.stat(Path::new("/small")).await.unwrap();
    let large = be.stat(Path::new("/large")).await.unwrap();
    let blocks = be.block_layout(Path::new("/large")).await.unwrap();
    let large_etag = be.etag(Path::new("/large")).await.unwrap();
    let next = blocks.iter().map(|b| b.id.0).max().unwrap() + 1;
    let legacy = serde_json::json!({"version":1,"next_block":next,"entries":{
        "/":{"status":root,"inline":null,"blocks":[]},
        "/small":{"status":small,"inline":b"hello","blocks":[]},
        "/large":{"status":large,"inline":null,"blocks":blocks,"etag":large_etag}
    }});
    drop(be);
    let manifest = dir.path().join("ns/namespace.json");
    std::fs::write(&manifest, serde_json::to_vec(&legacy).unwrap()).unwrap();
    // A leftover new-format database emulates interruption before marker publication.
    let migrated = open(&dir);
    assert_eq!(bytes(&migrated, "/small", 0..u64::MAX).await, b"hello");
    assert_eq!(bytes(&migrated, "/large", 0..u64::MAX).await, vec![3; 350]);
    assert_eq!(migrated.etag(Path::new("/large")).await.unwrap(), large_etag);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &std::fs::read(dir.path().join("ns/namespace.legacy.json")).unwrap()
        )
        .unwrap(),
        legacy
    );
    migrated.write(Path::new("/next"), body(vec![4; 350])).await.unwrap();
    assert!(migrated.block_layout(Path::new("/next")).await.unwrap()[0].id.0 >= next);
    drop(migrated);
    assert_eq!(bytes(&open(&dir), "/next", 0..u64::MAX).await, vec![4; 350]);
    std::fs::remove_file(dir.path().join("ns/namespace.sqlite3")).unwrap();
    assert!(LocalBackend::open(dir.path()).is_err());
}

#[tokio::test]
async fn process_writer() {
    let Ok(root) = std::env::var("MAMMOTH_TEST_PROCESS_STORE") else {
        return;
    };
    let id = std::env::var("MAMMOTH_TEST_PROCESS_ID").unwrap();
    let be = LocalBackend::open(root).unwrap().with_block_size(100).with_inline_threshold(10);
    for index in 0..5 {
        be.write(Path::new(&format!("/{id}/{index}")), body(vec![index; 350])).await.unwrap();
    }
}
#[tokio::test]
async fn independent_processes_share_transactions_and_block_ids() {
    let dir = tempfile::tempdir().unwrap();
    let be = open(&dir);
    let mut children = vec![];
    for id in 0..3 {
        children.push(
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "process_writer", "--nocapture"])
                .env("MAMMOTH_TEST_PROCESS_STORE", dir.path())
                .env("MAMMOTH_TEST_PROCESS_ID", id.to_string())
                .stdout(std::process::Stdio::null())
                .spawn()
                .unwrap(),
        );
    }
    for mut child in children {
        assert!(child.wait().unwrap().success());
    }
    let mut ids = HashSet::new();
    for id in 0..3 {
        for index in 0..5 {
            let path = format!("/{id}/{index}");
            assert_eq!(bytes(&be, &path, 0..u64::MAX).await, vec![index; 350]);
            for b in be.block_layout(Path::new(&path)).await.unwrap() {
                assert!(ids.insert(b.id.0));
            }
        }
    }
    assert_eq!(ids.len(), 60);
}

#[tokio::test]
async fn process_exit_writer() {
    let Ok(root) = std::env::var("MAMMOTH_TEST_EXIT_STORE") else {
        return;
    };
    let be = LocalBackend::open(root).unwrap().with_block_size(100).with_inline_threshold(10);
    if std::env::var("MAMMOTH_TEST_EXIT_PHASE").unwrap() == "committed" {
        be.write(Path::new("/file"), body(vec![5; 350])).await.unwrap();
    } else {
        let input = stream::once(async { Ok(bytes::Bytes::from(vec![9; 350])) }).chain(
            stream::once(async {
                std::process::exit(0);
                #[allow(unreachable_code)]
                Ok(bytes::Bytes::new())
            }),
        );
        be.write(Path::new("/file"), Box::pin(input)).await.unwrap();
    }
    // Skip destructors/checkpointing to exercise WAL recovery after process death.
    std::process::exit(0);
}
#[tokio::test]
async fn abrupt_exit_recovers_committed_wal_and_discards_unpublished_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    for phase in ["committed", "interrupted"] {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "process_exit_writer", "--nocapture"])
            .env("MAMMOTH_TEST_EXIT_STORE", dir.path())
            .env("MAMMOTH_TEST_EXIT_PHASE", phase)
            .stdout(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success());
        assert_eq!(bytes(&open(&dir), "/file", 0..u64::MAX).await, vec![5; 350]);
    }
    let be = open(&dir);
    assert!(be.gc().await.unwrap() > 0);
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, vec![5; 350]);
    drop(be);
    let marker = dir.path().join("ns/namespace.json");
    let saved = std::fs::read(&marker).unwrap();
    std::fs::remove_file(&marker).unwrap();
    assert!(LocalBackend::open(dir.path()).is_err());
    std::fs::write(&marker, saved).unwrap();
    assert_eq!(bytes(&open(&dir), "/file", 0..u64::MAX).await, vec![5; 350]);
}

#[tokio::test]
async fn streaming_retries_every_replica_for_each_checksum_chunk() {
    use std::io::{Seek, SeekFrom, Write};
    let dir = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(dir.path())
        .unwrap()
        .with_block_size(256 * 1024)
        .with_inline_threshold(0)
        .with_replication(2);
    let payload = vec![42; 200_000];
    be.write(Path::new("/file"), body(payload.clone())).await.unwrap();
    let block = be.block_layout(Path::new("/file")).await.unwrap().remove(0);
    for (replica, offset) in block.replicas.iter().zip([100_000, 180_000]) {
        let store = BlockStore::open(dir.path().join("workers").join(&replica.node.0)).unwrap();
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .open(store.path(block.id.0).join("data"))
            .unwrap();
        file.seek(SeekFrom::Start(offset)).unwrap();
        file.write_all(&[0]).unwrap();
    }
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, payload);
}

#[tokio::test]
async fn memory_cache_is_bounded_generation_safe_and_independent_of_health() {
    let dir = tempfile::tempdir().unwrap();
    let be = LocalBackend::open(dir.path())
        .unwrap()
        .with_inline_threshold(0)
        .with_block_size(65536)
        .with_replication(1)
        .with_cache_size(200_000);
    be.write(Path::new("/file"), body(vec![7; 131072])).await.unwrap();
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, vec![7; 131072]);
    let cold = be.cache_stats().unwrap();
    assert_eq!(cold.hits, 0);
    assert_eq!(cold.misses, 2);
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, vec![7; 131072]);
    assert_eq!(be.cache_stats().unwrap().hits, 2);
    let block = be.block_layout(Path::new("/file")).await.unwrap().remove(0);
    let store =
        BlockStore::open(dir.path().join("workers").join(&block.replicas[0].node.0)).unwrap();
    std::fs::write(store.path(block.id.0).join("data"), vec![0; 65536]).unwrap();
    // Healthy cached bytes are usable, but replica health must still inspect disk.
    assert_eq!(bytes(&be, "/file", 0..65536).await, vec![7; 65536]);
    assert!(be.cluster_report().await.unwrap().health.missing > 0);
    let other =
        LocalBackend::open(dir.path()).unwrap().with_inline_threshold(0).with_replication(1);
    other.write(Path::new("/file"), body(vec![8; 150000])).await.unwrap();
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, vec![8; 150000]);
    let stats = be.cache_stats().unwrap();
    assert!(stats.resident_bytes <= stats.capacity_bytes);
    assert!(stats.evictions > 0);
    be.clear_read_cache();
    assert_eq!(be.cache_stats().unwrap().resident_bytes, 0);
    assert_eq!(bytes(&be, "/file", 0..u64::MAX).await, vec![8; 150000]);
    be.remove(Path::new("/file"), false).await.unwrap();
    assert!(be.read(Path::new("/file"), 0..u64::MAX).await.is_err());
}

#[tokio::test]
async fn create_checks_absence_at_commit_and_preserves_a_concurrent_writer() {
    for inline in [0, 1024] {
        let dir = tempfile::tempdir().unwrap();
        let be = LocalBackend::open(dir.path())
            .unwrap()
            .with_inline_threshold(inline)
            .with_block_size(4)
            .with_replication(1);
        let other = LocalBackend::open(dir.path()).unwrap();
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release_tx, release_rx) = tokio::sync::oneshot::channel();
        let task_be = be.clone();
        let task = tokio::spawn(async move {
            let input = stream::once(async move {
                started_tx.send(()).unwrap();
                release_rx.await.unwrap();
                Ok(bytes::Bytes::from_static(b"job result"))
            });
            task_be.create(Path::new("/result"), Box::pin(input)).await
        });
        started_rx.await.unwrap();
        other.write(Path::new("/result"), body("someone else's file")).await.unwrap();
        release_tx.send(()).unwrap();
        assert!(matches!(task.await.unwrap(), Err(Error::AlreadyExists(_))));
        assert_eq!(bytes(&be, "/result", 0..u64::MAX).await, b"someone else's file");
        be.create(Path::new("/new"), body("new data")).await.unwrap();
        assert_eq!(bytes(&other, "/new", 0..u64::MAX).await, b"new data");
        be.gc().await.unwrap();
        assert_eq!(bytes(&other, "/result", 0..u64::MAX).await, b"someone else's file");
    }
}
