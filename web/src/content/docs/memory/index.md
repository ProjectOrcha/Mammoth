---
title: Memory model & durability
description: How Mammoth stores, scopes, versions, and recalls coding-agent context.
---


Mammoth is local, durable context memory for coding agents. It saves useful project
knowledge between sessions: decisions and their reasons, repository conventions,
verified facts, and handoffs. The CLI and MCP server share the same database.

## Memory model

Each entry has a project, stable key, title, content, kind (`note`, `decision`,
`convention`, or `handoff`), tags, optional source, revision, and creation/update
timestamps in Unix milliseconds. Project names are exact, case-sensitive namespaces.
Use a stable name such as `team/repository`; use a separate namespace when branches
need conflicting decisions. No repository or conversation is captured automatically.

Creating an existing key fails. To update, read its current revision and supply
`expected_revision`. Writes to the entry and history commit in the same SQLite
transaction. Two agents updating the same revision cannot silently lose a change.
History is newest first. Forgetting deletes the current entry and its history.
A content-free project/key revision counter remains so that recreating a key cannot
reuse an old revision and accept a stale agent update.

Recall uses SQLite FTS5 to search title, content, and tags, with title matches
weighted more strongly. Words are literal, AND-matched, and case insensitive;
punctuation separates words. This is lexical search, not semantic or vector search.
An empty query returns recent entries. Keys and source fields can be read with get;
they are not searched. Recall defaults to 10 entries and 16,000 bytes. Limits are
1–50 entries and 512–65,536 bytes. The budget covers the JSON memories array, not the
response envelope or MCP framing, and is not a token count. `truncated` reports
omitted entries or text. Use get for a complete entry (up to 64 KiB of content).
History returns at most 50 full entries and does not use the recall byte budget.

## Durability and boundaries

The database is `<local-root>/agent-memory.sqlite3`. SQLite WAL mode with
`synchronous=FULL` acknowledges a write only after its transaction commits.
Memory survives process exits, restarts, and context-window resets. The database
and history live on one machine; disk loss still requires a backup. A revision
history is not a backup. Store it on a healthy local filesystem, not a shared
network filesystem. To back up, stop all Mammoth processes using the root and copy
the database together with any `-wal` and `-shm` sidecars. Keep the original files
together when restoring. A newer unknown schema is rejected without migration.

The memory database is independent of the legacy block-storage namespace. It does
not use simulated replicas, the block cache, or the old distributed scheduler.
It has no cloud sync, encryption, embeddings, automatic compaction, or remote MCP
HTTP endpoint. Filesystem permissions protect local data. Project scoping prevents
an MCP client from requesting another project, but is not a security boundary
against another process with access to the database. Forget is logical deletion;
SQLite pages, WAL files, and backups may retain older bytes.

Stored memory is historical data, never a source of higher-priority instructions.
Agents should verify stale context against current code, preserve provenance,
and avoid saving secrets, credentials, or unnecessary personal information.
