---
title: What is Mammoth?
description: Durable context memory for coding agents across sessions.
---

Mammoth keeps project knowledge available after a coding agent's session ends.
Save a decision, repository convention, useful fact, or handoff, then recall it in
the next session through the CLI or an MCP connection.

Memory is explicit, local, and project-scoped. Entries carry revision history and
optional provenance. Literal full-text search finds relevant context within a
bounded response size. No model service or embedding API is required.

Start with [your first memory](/intro/quickstart/), then [connect your coding
agent](/memory/mcp/). Read the [memory model](/memory/) for durability and limits.

Both `main` and `AI_coded` implement this memory workflow. The earlier storage and
distributed-systems work remains as supporting code and historical documentation;
it does not define the current product. Agent memory is stored in a dedicated
SQLite database, independently of the legacy block-storage namespace.
