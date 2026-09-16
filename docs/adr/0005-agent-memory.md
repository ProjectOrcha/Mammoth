# ADR 0005: Durable coding-agent context as the product

Status: accepted

Mammoth's primary use case is durable project context for coding agents. Both
branches expose the same memory workflow and MCP tools. Existing storage and
compute work remains supporting/legacy engineering material.

Use a dedicated SQLite database under the configured local root. Entries and
history commit atomically in WAL mode with FULL synchronous writes. Project/key
pairs identify memories; revision preconditions protect updates and deletion.
FTS5 supplies literal lexical retrieval without an embedding service. Recall
budgets count serialized UTF-8 bytes, not tokens.

Use the official Rust MCP SDK over stdio. A server connection has one immutable
project scope; a read-only configuration omits and rejects mutation tools. The
combined CLI and standalone MCP binary use the same library. Memory is not an
instruction authority and is not automatically captured from conversations.

This avoids tying useful local memory to the incomplete distributed storage
track on main. The dedicated database is independent of block storage, cached
reads, and simulated worker replicas. Remote sharing and semantic search require
separate decisions and evaluation. Rust 1.88 is now the minimum for the current SDK.
