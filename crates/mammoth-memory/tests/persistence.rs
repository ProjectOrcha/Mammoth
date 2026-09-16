use mammoth_memory::{Error, Kind, Remember, Store};
fn note(key: &str, content: &str) -> Remember {
    Remember {
        key: key.into(),
        title: "Pagination decision".into(),
        content: content.into(),
        kind: Kind::Decision,
        tags: vec!["api".into()],
        source: Some("src/api.rs".into()),
        expected_revision: None,
    }
}
#[test]
fn restart_isolation_history_and_forget() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = Store::open(dir.path()).unwrap();
    a.remember("one", note("cursor", "Use cursor pagination")).unwrap();
    a.remember("two", note("cursor", "Private other project")).unwrap();
    drop(a);
    let mut b = Store::open(dir.path()).unwrap();
    assert_eq!(b.get("one", "cursor").unwrap().revision, 1);
    assert!(b.recall("one", "private", 10, 16000).unwrap().memories.is_empty());
    assert_eq!(b.recall("one", "cursor pagination", 10, 16000).unwrap().memories.len(), 1);
    assert!(matches!(b.remember("one", note("cursor", "Overwrite")), Err(Error::Conflict)));
    let mut update = note("cursor", "Use keyset pagination");
    update.expected_revision = Some(1);
    assert_eq!(b.remember("one", update).unwrap().revision, 2);
    assert!(b.recall("one", "cursor", 10, 16000).unwrap().memories.is_empty());
    assert_eq!(b.history("one", "cursor", 10).unwrap().len(), 2);
    assert!(matches!(b.forget("one", "cursor", 1), Err(Error::Conflict)));
    b.forget("one", "cursor", 2).unwrap();
    assert!(b.history("one", "cursor", 10).unwrap().is_empty());
    assert!(b.recall("one", "", 10, 16000).unwrap().memories.is_empty());
    assert!(b.get("two", "cursor").is_ok());
    assert_eq!(b.remember("one", note("cursor", "Recreated")).unwrap().revision, 3);
    let mut stale = note("cursor", "Stale agent");
    stale.expected_revision = Some(1);
    assert!(matches!(b.remember("one", stale), Err(Error::Conflict)));
}
#[test]
fn concurrent_agents_cannot_lose_updates() {
    let dir = tempfile::tempdir().unwrap();
    Store::open(dir.path()).unwrap().remember("app", note("key", "Initial")).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let handles = (0..2)
        .map(|_| {
            let root = dir.path().to_owned();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = Store::open(root).unwrap();
                barrier.wait();
                let mut n = note("key", "Changed");
                n.expected_revision = Some(1);
                store.remember("app", n)
            })
        })
        .collect::<Vec<_>>();
    let results = handles.into_iter().map(|h| h.join().unwrap()).collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(results.iter().filter(|r| matches!(r, Err(Error::Conflict))).count(), 1);
}
#[test]
fn context_budget_covers_unicode_and_json_escaping() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    store.remember("app", note("one", &"🦣\n\"\\".repeat(4000))).unwrap();
    store.remember("app", note("two", "Second entry")).unwrap();
    for budget in [512, 1024, 16000] {
        let result = store.recall("app", "", 10, budget).unwrap();
        assert!(result.truncated);
        assert_eq!(result.used_bytes, serde_json::to_vec(&result.memories).unwrap().len());
        assert!(result.used_bytes <= budget);
    }
    assert!(store.recall("app", "", 0, 1000).is_err());
    assert!(store.recall("app", "", 10, 1).is_err());
    assert!(store.recall("app", "\"***", 10, 1000).is_err());
    assert!(store.recall("app", "\" OR project:other", 10, 1000).is_ok());
    assert!(store.remember(" ", note("key", "text")).is_err());
    assert!(store.remember("app", note("large", &"a".repeat(65537))).is_err());
    assert!(store.recall("app", "", 1, 65536).unwrap().truncated);
}
#[test]
fn future_schema_fails_closed() {
    let dir = tempfile::tempdir().unwrap();
    let conn = rusqlite::Connection::open(dir.path().join("agent-memory.sqlite3")).unwrap();
    conn.pragma_update(None, "user_version", 99).unwrap();
    assert!(Store::open(dir.path()).is_err());
    assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 99);
    assert_eq!(
        conn.pragma_query_value(None, "journal_mode", |r| r.get::<_, String>(0)).unwrap(),
        "delete"
    );
}

#[test]
fn opening_an_initialized_store_does_not_wait_for_a_writer() {
    let dir = tempfile::tempdir().unwrap();
    Store::open(dir.path()).unwrap().remember("app", note("key", "Committed")).unwrap();
    let mut writer = rusqlite::Connection::open(dir.path().join("agent-memory.sqlite3")).unwrap();
    let _transaction =
        writer.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate).unwrap();
    // A new CLI/MCP connection must still read committed context during a write.
    let reader = Store::open(dir.path()).unwrap();
    assert_eq!(reader.get("app", "key").unwrap().content, "Committed");
}

#[test]
fn version_one_upgrade_preserves_history_and_deleted_key_revisions() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = Store::open(dir.path()).unwrap();
    store.remember("app", note("kept", "Original")).unwrap();
    let mut update = note("kept", "Updated");
    update.expected_revision = Some(1);
    store.remember("app", update).unwrap();
    store.remember("app", note("removed", "Old content")).unwrap();
    store.forget("app", "removed", 1).unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(dir.path().join("agent-memory.sqlite3")).unwrap();
    // Version 1 predates the one-time migration marker; early v1 stores also
    // need their active revision heads backfilled.
    conn.execute("DELETE FROM revision_heads WHERE key='kept'", []).unwrap();
    conn.pragma_update(None, "user_version", 1).unwrap();
    let mut upgraded = Store::open(dir.path()).unwrap();
    assert_eq!(upgraded.get("app", "kept").unwrap().revision, 2);
    assert_eq!(upgraded.history("app", "kept", 10).unwrap().len(), 2);
    assert_eq!(upgraded.recall("app", "updated", 10, 16000).unwrap().memories.len(), 1);
    assert_eq!(upgraded.remember("app", note("removed", "New content")).unwrap().revision, 2);
    let mut update = note("kept", "Next update");
    update.expected_revision = Some(2);
    assert_eq!(upgraded.remember("app", update).unwrap().revision, 3);
    assert_eq!(conn.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0)).unwrap(), 2);
}

#[test]
fn concurrent_first_open_initializes_one_store() {
    let dir = tempfile::tempdir().unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(4));
    let handles = (0..4)
        .map(|i| {
            let root = dir.path().to_owned();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                Store::open(root).unwrap().remember("app", note(&i.to_string(), "Concurrent"))
            })
        })
        .collect::<Vec<_>>();
    for handle in handles {
        assert_eq!(handle.join().unwrap().unwrap().revision, 1);
    }
    assert_eq!(
        Store::open(dir.path()).unwrap().recall("app", "", 10, 16000).unwrap().memories.len(),
        4
    );
}
