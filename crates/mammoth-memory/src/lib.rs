//! Durable project context. All updates and history records commit in one transaction.
#![forbid(unsafe_code)]

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub type Result<T> = std::result::Result<T, Error>;
const SCHEMA_VERSION: i64 = 2;
const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("memory not found: {0}")]
    NotFound(String),
    #[error(
        "revision conflict: read the current memory and supply its revision before changing it"
    )]
    Conflict,
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Note,
    Decision,
    Convention,
    Handoff,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Remember {
    /// Stable key within this project, such as auth-decision or current-handoff.
    pub key: String,
    pub title: String,
    /// A concise fact or handoff. Never save credentials or secrets.
    pub content: String,
    pub kind: Kind,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional provenance: file path, commit, issue URL, or session identifier.
    pub source: Option<String>,
    /// Omit (or use 0) to create. Updating requires the current revision from get/recall.
    pub expected_revision: Option<i64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
pub struct Memory {
    pub project: String,
    pub key: String,
    pub title: String,
    pub content: String,
    pub kind: Kind,
    pub tags: Vec<String>,
    pub source: Option<String>,
    pub revision: i64,
    pub created_at: i64,
    pub updated_at: i64,
}
#[derive(Debug, Serialize)]
pub struct Recall {
    pub memories: Vec<Memory>,
    /// True when content or matching entries were omitted by the byte or count budget.
    pub truncated: bool,
    /// Serialized bytes in the memories array, excluding this envelope. Not a token count.
    pub used_bytes: usize,
    pub max_bytes: usize,
}

pub struct Store {
    conn: Connection,
}

pub fn validate_project(project: &str) -> Result<()> {
    validate_text("project", project, 200)
}
fn validate_text(name: &str, value: &str, max: usize) -> Result<()> {
    if value.trim().is_empty() || value.len() > max || value.contains('\0') {
        return Err(Error::Invalid(format!(
            "{name} must be nonempty, contain no NUL, and fit in {max} UTF-8 bytes"
        )));
    }
    Ok(())
}
fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis() as i64
}
fn parse_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<String> {
    row.get(0)
}

fn schema_version(conn: &Connection) -> Result<i64> {
    let version = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version > SCHEMA_VERSION {
        return Err(Error::Invalid(
            "memory database was created by a newer Mammoth version".into(),
        ));
    }
    Ok(version)
}

impl Store {
    /// Opens agent-memory.sqlite3 beneath the selected Mammoth root.
    /// WAL + FULL synchronous commits survive process restarts on a healthy local filesystem.
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        std::fs::create_dir_all(root.as_ref())?;
        let path = root.as_ref().join("agent-memory.sqlite3");
        let deadline = Instant::now() + LOCK_TIMEOUT;
        loop {
            match Self::open_connection(&path, deadline.saturating_duration_since(Instant::now())) {
                // Switching a new database to WAL can return BUSY without invoking
                // SQLite's busy handler. Retry initialization on a fresh connection;
                // failed schema transactions have rolled back before this point.
                Err(Error::Sql(error))
                    if error.sqlite_error_code() == Some(rusqlite::ErrorCode::DatabaseBusy)
                        && Instant::now() < deadline =>
                {
                    std::thread::sleep(
                        Duration::from_millis(10)
                            .min(deadline.saturating_duration_since(Instant::now())),
                    );
                }
                result => return result,
            }
        }
    }

    fn open_connection(path: &Path, timeout: Duration) -> Result<Self> {
        let mut conn = Connection::open(path)?;
        conn.busy_timeout(timeout)?;
        let version = schema_version(&conn)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        // Reads open independent connections. Only initialize/migrate once, so
        // opening an established store neither scans all rows nor takes a write lock.
        if version < SCHEMA_VERSION {
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            // Another process may have completed the migration while we waited.
            if schema_version(&tx)? < SCHEMA_VERSION {
                tx.execute_batch("CREATE TABLE IF NOT EXISTS memories (
            id INTEGER PRIMARY KEY, project TEXT NOT NULL, key TEXT NOT NULL,
            title TEXT NOT NULL, content TEXT NOT NULL, tags TEXT NOT NULL,
            revision INTEGER NOT NULL, updated_at INTEGER NOT NULL, document TEXT NOT NULL,
            UNIQUE(project, key));
            CREATE TABLE IF NOT EXISTS history (
                project TEXT NOT NULL, key TEXT NOT NULL, revision INTEGER NOT NULL,
                document TEXT NOT NULL, PRIMARY KEY(project,key,revision));
            CREATE TABLE IF NOT EXISTS revision_heads (
                project TEXT NOT NULL, key TEXT NOT NULL, revision INTEGER NOT NULL,
                PRIMARY KEY(project,key));
            INSERT OR IGNORE INTO revision_heads SELECT project,key,revision FROM memories;
            CREATE INDEX IF NOT EXISTS memories_recent ON memories(project, updated_at DESC);
            CREATE VIRTUAL TABLE IF NOT EXISTS memory_search USING fts5(title, content, tags, content='memories', content_rowid='id');
            CREATE TRIGGER IF NOT EXISTS memory_insert AFTER INSERT ON memories BEGIN
                INSERT INTO memory_search(rowid,title,content,tags) VALUES(new.id,new.title,new.content,new.tags); END;
            CREATE TRIGGER IF NOT EXISTS memory_delete AFTER DELETE ON memories BEGIN
                INSERT INTO memory_search(memory_search,rowid,title,content,tags) VALUES('delete',old.id,old.title,old.content,old.tags); END;
            CREATE TRIGGER IF NOT EXISTS memory_update AFTER UPDATE ON memories BEGIN
                INSERT INTO memory_search(memory_search,rowid,title,content,tags) VALUES('delete',old.id,old.title,old.content,old.tags);
                INSERT INTO memory_search(rowid,title,content,tags) VALUES(new.id,new.title,new.content,new.tags); END;
            PRAGMA user_version=2;")?;
            }
            tx.commit()?;
        }
        conn.busy_timeout(LOCK_TIMEOUT)?;
        Ok(Self { conn })
    }
    pub fn remember(&mut self, project: &str, input: Remember) -> Result<Memory> {
        validate_project(project)?;
        validate_text("key", &input.key, 200)?;
        validate_text("title", &input.title, 300)?;
        validate_text("content", &input.content, 64 * 1024)?;
        if input.tags.len() > 20 {
            return Err(Error::Invalid("at most 20 tags are allowed".into()));
        }
        for tag in &input.tags {
            validate_text("tag", tag, 64)?;
        }
        if let Some(source) = &input.source {
            validate_text("source", source, 1000)?;
        }
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let old: Option<String> = tx
            .query_row(
                "SELECT document FROM memories WHERE project=? AND key=?",
                params![project, input.key],
                parse_row,
            )
            .optional()?;
        let old: Option<Memory> = old.map(|s| serde_json::from_str(&s)).transpose()?;
        if input.expected_revision.unwrap_or(0) != old.as_ref().map_or(0, |m| m.revision) {
            return Err(Error::Conflict);
        }
        // Retain a content-free revision head after deletion so a stale agent
        // cannot overwrite a newly recreated entry with an old revision (ABA).
        let revision = tx.query_row(
            "INSERT INTO revision_heads(project,key,revision) VALUES(?,?,1)
             ON CONFLICT(project,key) DO UPDATE SET revision=revision+1 RETURNING revision",
            params![project, input.key],
            |r| r.get(0),
        )?;
        let stamp = now();
        let memory = Memory {
            project: project.into(),
            key: input.key,
            title: input.title,
            content: input.content,
            kind: input.kind,
            tags: input.tags,
            source: input.source,
            revision,
            created_at: old.as_ref().map_or(stamp, |m| m.created_at),
            updated_at: stamp,
        };
        let document = serde_json::to_string(&memory)?;
        tx.execute("INSERT INTO memories(project,key,title,content,tags,revision,updated_at,document) VALUES(?,?,?,?,?,?,?,?)
            ON CONFLICT(project,key) DO UPDATE SET title=excluded.title,content=excluded.content,tags=excluded.tags,revision=excluded.revision,updated_at=excluded.updated_at,document=excluded.document",
            params![project,memory.key,memory.title,memory.content,serde_json::to_string(&memory.tags)?,memory.revision,stamp,document])?;
        tx.execute(
            "INSERT INTO history(project,key,revision,document) VALUES(?,?,?,?)",
            params![project, memory.key, memory.revision, document],
        )?;
        tx.commit()?;
        Ok(memory)
    }
    pub fn get(&self, project: &str, key: &str) -> Result<Memory> {
        validate_project(project)?;
        validate_text("key", key, 200)?;
        let document = self
            .conn
            .query_row(
                "SELECT document FROM memories WHERE project=? AND key=?",
                params![project, key],
                parse_row,
            )
            .optional()?
            .ok_or_else(|| Error::NotFound(key.into()))?;
        Ok(serde_json::from_str(&document)?)
    }
    pub fn history(&self, project: &str, key: &str, limit: usize) -> Result<Vec<Memory>> {
        validate_project(project)?;
        validate_text("key", key, 200)?;
        validate_limit(limit)?;
        let mut stmt = self.conn.prepare(
            "SELECT document FROM history WHERE project=? AND key=? ORDER BY revision DESC LIMIT ?",
        )?;
        let rows = stmt.query_map(params![project, key, limit as i64], parse_row)?;
        rows.map(|r| Ok(serde_json::from_str(&r?)?)).collect()
    }
    /// Literal full-text terms, AND matching with relevance ranking. Empty query returns recent entries.
    pub fn recall(
        &self,
        project: &str,
        query: &str,
        limit: usize,
        max_bytes: usize,
    ) -> Result<Recall> {
        validate_project(project)?;
        validate_limit(limit)?;
        if !(512..=65536).contains(&max_bytes) {
            return Err(Error::Invalid("max_bytes must be between 512 and 65536".into()));
        }
        if query.len() > 1000 {
            return Err(Error::Invalid("query must fit in 1000 UTF-8 bytes".into()));
        }
        let terms = query
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|s| !s.is_empty())
            .map(|s| format!("\"{s}\""))
            .collect::<Vec<_>>()
            .join(" AND ");
        if !query.trim().is_empty() && terms.is_empty() {
            return Err(Error::Invalid("query needs at least one word".into()));
        }
        let documents: Vec<String> = if terms.is_empty() {
            let mut stmt = self.conn.prepare("SELECT document FROM memories WHERE project=? ORDER BY updated_at DESC,id DESC LIMIT ?")?;
            let rows = stmt.query_map(params![project, (limit + 1) as i64], parse_row)?;
            rows.collect::<std::result::Result<_, _>>()?
        } else {
            let mut stmt = self.conn.prepare("SELECT m.document FROM memory_search JOIN memories m ON m.id=memory_search.rowid WHERE memory_search MATCH ? AND m.project=? ORDER BY bm25(memory_search,4.0,1.0,2.0),m.updated_at DESC,m.id DESC LIMIT ?")?;
            let rows = stmt.query_map(params![terms, project, (limit + 1) as i64], parse_row)?;
            rows.collect::<std::result::Result<_, _>>()?
        };
        let mut result = Recall {
            memories: vec![],
            truncated: documents.len() > limit,
            used_bytes: 2,
            max_bytes,
        };
        for document in documents.into_iter().take(limit) {
            let mut memory: Memory = serde_json::from_str(&document)?;
            let separator = usize::from(!result.memories.is_empty());
            let available = max_bytes.saturating_sub(result.used_bytes + separator);
            let mut encoded = serde_json::to_vec(&memory)?.len();
            if encoded > available {
                result.truncated = true;
                let original = std::mem::take(&mut memory.content);
                // Binary search UTF-8 boundaries against actual JSON size, including escapes.
                let boundaries = original
                    .char_indices()
                    .map(|(i, _)| i)
                    .chain(std::iter::once(original.len()))
                    .collect::<Vec<_>>();
                let (mut low, mut high) = (0, boundaries.len());
                while low < high {
                    let mid = (low + high) / 2;
                    memory.content =
                        format!("{}… [truncated; use memory_get]", &original[..boundaries[mid]]);
                    if serde_json::to_vec(&memory)?.len() <= available {
                        low = mid + 1;
                    } else {
                        high = mid;
                    }
                }
                if low == 0 {
                    break;
                }
                memory.content =
                    format!("{}… [truncated; use memory_get]", &original[..boundaries[low - 1]]);
                encoded = serde_json::to_vec(&memory)?.len();
            }
            result.used_bytes += encoded + separator;
            result.memories.push(memory);
        }
        Ok(result)
    }
    /// Remove current entry and retained revisions. This is logical deletion, not secure disk erasure.
    pub fn forget(&mut self, project: &str, key: &str, expected_revision: i64) -> Result<()> {
        validate_project(project)?;
        validate_text("key", key, 200)?;
        let tx = self.conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let revision: Option<i64> = tx
            .query_row(
                "SELECT revision FROM memories WHERE project=? AND key=?",
                params![project, key],
                |r| r.get(0),
            )
            .optional()?;
        if revision.is_none() {
            return Err(Error::NotFound(key.into()));
        }
        if revision != Some(expected_revision) {
            return Err(Error::Conflict);
        }
        tx.execute("DELETE FROM memories WHERE project=? AND key=?", params![project, key])?;
        tx.execute("DELETE FROM history WHERE project=? AND key=?", params![project, key])?;
        tx.commit()?;
        Ok(())
    }
}
fn validate_limit(limit: usize) -> Result<()> {
    if !(1..=50).contains(&limit) {
        return Err(Error::Invalid("limit must be between 1 and 50".into()));
    }
    Ok(())
}
