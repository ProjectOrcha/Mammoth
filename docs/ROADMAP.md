# Mammoth roadmap

Mammoth's product focus is durable context memory for coding agents.

## Implemented on main and AI_coded

- Project-scoped entries: notes, decisions, conventions, and handoffs.
- SQLite WAL persistence, atomic entry/history commits, and revision conflict checks.
- Literal full-text search, recent recall, and a configurable response byte budget.
- Current-entry reads, retained revision history, and explicit deletion.
- CLI commands and a stdio MCP server using the official Rust SDK.
- Read-only MCP mode and fixed project scope per connection.
- Memory-focused website, setup documentation, and regression coverage.

## Next

- Consistent online backup/export and import tooling.
- Better memory curation, stale-entry review, and provenance workflows.
- Evaluation of retrieval relevance on real coding-agent tasks.
- Optional semantic retrieval only after measurable improvement over lexical search.

Remote hosting, shared multi-user access, authentication, cloud sync, and vector
search are not implemented. Distributed storage and Hadoop/Spark comparisons are
legacy engineering work, not milestones for the agent-memory product.
