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
    LocalBackend::open(d.path()).unwrap().with_block_size(100).with_inline_threshold(10)
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
    assert_eq!(be.gc().await.unwrap(), 0);
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
    for path in ["/../escape", "a/../../escape", "a\\b"] {
        assert!(be.write(Path::new(path), body(vec![1])).await.is_err());
    }
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
