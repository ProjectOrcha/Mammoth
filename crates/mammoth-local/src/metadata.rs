//! Indexed namespace transactions. WAL + FULL synchronization retain durable
//! acknowledgements while readers use independent committed snapshots.
use super::{directory, invalid, Entry, Namespace};
use mammoth_core::{Error, FileStatus, Result};
use mammoth_storage::{atomic_write, sync_dir};
use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
    time::Duration,
};

pub(crate) struct Metadata {
    path: PathBuf,
    idle: Mutex<Vec<Connection>>,
}
fn sql(error: rusqlite::Error) -> Error {
    invalid(error)
}

impl Metadata {
    /// Called with the legacy store.lock held. The format marker fences old
    /// binaries before any new-format mutation can run. An interrupted migration
    /// is retried from the retained JSON snapshot, not from a partial database.
    pub fn open(root: &Path) -> Result<Self> {
        let manifest = root.join("ns/namespace.json");
        let marker = if manifest.exists() {
            Some(
                serde_json::from_slice::<serde_json::Value>(&fs::read(&manifest)?)
                    .map_err(invalid)?,
            )
        } else {
            None
        };
        let version = marker.as_ref().and_then(|v| v["version"].as_u64()).unwrap_or(1);
        if version != 1 && version != 2 {
            return Err(invalid("unsupported namespace version"));
        }
        let path = root.join("ns/namespace.sqlite3");
        if marker.is_none() && path.exists() {
            return Err(invalid("namespace format marker is missing; restore a complete store backup instead of reinitializing the database"));
        }
        if version == 2 && !path.is_file() {
            return Err(invalid("namespace database is missing; restore a complete store backup"));
        }
        let metadata = Self { path, idle: Mutex::new(vec![]) };
        let mut conn = metadata.connect()?;
        let mode: String =
            conn.query_row("PRAGMA journal_mode=WAL", [], |r| r.get(0)).map_err(sql)?;
        if mode != "wal" {
            return Err(invalid("namespace requires SQLite WAL on a local filesystem"));
        }
        if version == 1 {
            let legacy: Namespace = match marker {
                Some(value) => serde_json::from_value(value).map_err(invalid)?,
                None => Namespace {
                    version: 1,
                    next_block: 1001,
                    entries: std::collections::BTreeMap::from([("/".into(), directory("/"))]),
                },
            };
            if !legacy.entries.get("/").is_some_and(|e| e.status.is_dir) {
                return Err(invalid("invalid legacy namespace root"));
            }
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate).map_err(sql)?;
            tx.execute_batch("DROP TABLE IF EXISTS entries; DROP TABLE IF EXISTS counters;
                CREATE TABLE entries(path TEXT PRIMARY KEY, parent TEXT NOT NULL, status BLOB NOT NULL, etag TEXT NOT NULL, inline BLOB, blocks BLOB NOT NULL) WITHOUT ROWID;
                CREATE INDEX entries_parent ON entries(parent,path);
                CREATE TABLE counters(name TEXT PRIMARY KEY, value INTEGER NOT NULL) WITHOUT ROWID;
                PRAGMA user_version=2;").map_err(sql)?;
            for (path, entry) in &legacy.entries {
                put(&tx, path, entry)?;
            }
            let next = i64::try_from(legacy.next_block).map_err(invalid)?;
            if next <= 0
                || legacy
                    .entries
                    .values()
                    .flat_map(|e| &e.blocks)
                    .any(|block| block.id.0 >= legacy.next_block)
            {
                return Err(invalid("invalid legacy block ID counter"));
            }
            tx.execute("INSERT INTO counters VALUES ('next_block',?1)", [next]).map_err(sql)?;
            tx.commit().map_err(sql)?;
            if manifest.exists() {
                atomic_write(&root.join("ns/namespace.legacy.json"), &fs::read(&manifest)?)?;
            }
            sync_dir(&root.join("ns"))?;
            atomic_write(&manifest, br#"{"version":2,"database":"namespace.sqlite3"}"#)?;
        }
        let version: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(sql)?;
        if version != 2 || !stat(&conn, "/")?.is_dir {
            return Err(invalid("invalid namespace database version or root"));
        }
        metadata.idle.lock().map_err(invalid)?.push(conn);
        Ok(metadata)
    }
    fn connect(&self) -> Result<Connection> {
        let conn = Connection::open(&self.path).map_err(sql)?;
        conn.busy_timeout(Duration::from_secs(30)).map_err(sql)?;
        conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA trusted_schema=OFF; PRAGMA foreign_keys=ON; PRAGMA wal_autocheckpoint=1000;").map_err(sql)?;
        Ok(conn)
    }
    pub fn transaction<T>(
        &self,
        write: bool,
        f: impl FnOnce(&Transaction<'_>) -> Result<T>,
    ) -> Result<T> {
        let cached = self.idle.lock().map_err(invalid)?.pop();
        let mut conn = match cached {
            Some(c) => c,
            None => self.connect()?,
        };
        let result = (|| {
            let tx = conn
                .transaction_with_behavior(if write {
                    TransactionBehavior::Immediate
                } else {
                    TransactionBehavior::Deferred
                })
                .map_err(sql)?;
            let value = f(&tx)?;
            tx.commit().map_err(sql)?;
            Ok(value)
        })();
        let mut idle = self.idle.lock().map_err(invalid)?;
        if idle.len() < 8 {
            idle.push(conn);
        }
        result
    }
}

pub(crate) fn stat(conn: &Connection, path: &str) -> Result<FileStatus> {
    let bytes: Option<Vec<u8>> = conn
        .prepare_cached("SELECT status FROM entries WHERE path=?1")
        .map_err(sql)?
        .query_row([path], |r| r.get(0))
        .optional()
        .map_err(sql)?;
    serde_json::from_slice(&bytes.ok_or_else(|| Error::NotFound(path.into()))?).map_err(invalid)
}
pub(crate) fn get(conn: &Connection, path: &str) -> Result<Entry> {
    type RawEntry = (Vec<u8>, String, Option<Vec<u8>>, Vec<u8>);
    let raw: Option<RawEntry> = conn
        .prepare_cached("SELECT status,etag,inline,blocks FROM entries WHERE path=?1")
        .map_err(sql)?
        .query_row([path], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
        .optional()
        .map_err(sql)?;
    let (status, etag, inline, blocks) = raw.ok_or_else(|| Error::NotFound(path.into()))?;
    Ok(Entry {
        status: serde_json::from_slice(&status).map_err(invalid)?,
        etag,
        inline,
        blocks: serde_json::from_slice(&blocks).map_err(invalid)?,
    })
}
pub(crate) fn file(conn: &Connection, path: &str) -> Result<Entry> {
    let entry = get(conn, path)?;
    if entry.status.is_dir {
        return Err(Error::WrongKind { path: path.into(), actual: "directory", expected: "file" });
    }
    Ok(entry)
}
pub(crate) fn put(conn: &Connection, path: &str, entry: &Entry) -> Result<()> {
    let parent = Path::new(path).parent().unwrap_or(Path::new("")).to_string_lossy();
    conn.prepare_cached("INSERT INTO entries(path,parent,status,etag,inline,blocks) VALUES (?1,?2,?3,?4,?5,?6)
        ON CONFLICT(path) DO UPDATE SET parent=excluded.parent,status=excluded.status,etag=excluded.etag,inline=excluded.inline,blocks=excluded.blocks").map_err(sql)?
        .execute(params![path,parent,serde_json::to_vec(&entry.status).map_err(invalid)?,entry.etag,entry.inline,serde_json::to_vec(&entry.blocks).map_err(invalid)?]).map_err(sql)?;
    Ok(())
}
pub(crate) fn delete(conn: &Connection, path: &str) -> Result<()> {
    conn.prepare_cached("DELETE FROM entries WHERE path=?1")
        .map_err(sql)?
        .execute([path])
        .map_err(sql)?;
    Ok(())
}
pub(crate) fn list(conn: &Connection, path: &str) -> Result<Vec<FileStatus>> {
    let mut query = conn
        .prepare_cached("SELECT status FROM entries WHERE parent=?1 ORDER BY path")
        .map_err(sql)?;
    let rows = query.query_map([path], |r| r.get::<_, Vec<u8>>(0)).map_err(sql)?;
    rows.map(|row| serde_json::from_slice(&row.map_err(sql)?).map_err(invalid)).collect()
}
pub(crate) fn subtree(conn: &Connection, path: &str) -> Result<Vec<String>> {
    let prefix = if path == "/" { "/".to_string() } else { format!("{path}/") };
    // Binary prefix bounds use the next ASCII character after '/'. Unlike LIKE,
    // paths containing '%' or '_' remain literal and case remains significant.
    let end = if path == "/" { "0".to_string() } else { format!("{path}0") };
    let mut query = conn
        .prepare_cached(
            "SELECT path FROM entries WHERE path=?1 OR (path>=?2 AND path<?3) ORDER BY path",
        )
        .map_err(sql)?;
    let rows = query.query_map(params![path, prefix, end], |r| r.get(0)).map_err(sql)?;
    rows.map(|r| r.map_err(sql)).collect()
}
pub(crate) fn all(conn: &Connection) -> Result<Vec<Entry>> {
    subtree(conn, "/")?.into_iter().map(|path| get(conn, &path)).collect()
}
pub(crate) fn parents(conn: &Connection, path: &str, create: bool) -> Result<()> {
    let mut parent = String::new();
    let parts: Vec<_> = path.trim_start_matches('/').split('/').collect();
    for part in parts.iter().take(parts.len().saturating_sub(1)) {
        parent.push('/');
        parent.push_str(part);
        match stat(conn, &parent) {
            Ok(e) if !e.is_dir => {
                return Err(Error::WrongKind {
                    path: parent.into(),
                    actual: "file",
                    expected: "directory",
                })
            }
            Ok(_) => {}
            Err(Error::NotFound(_)) if create => put(conn, &parent, &directory(&parent))?,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
pub(crate) fn reserve(conn: &Connection, count: u64) -> Result<u64> {
    let first: i64 = conn
        .query_row("SELECT value FROM counters WHERE name='next_block'", [], |r| r.get(0))
        .map_err(sql)?;
    let next = first
        .checked_add(i64::try_from(count).map_err(invalid)?)
        .filter(|_| first > 0)
        .ok_or_else(|| invalid("block IDs exhausted"))?;
    conn.execute("UPDATE counters SET value=?1 WHERE name='next_block'", [next]).map_err(sql)?;
    Ok(first as u64)
}
