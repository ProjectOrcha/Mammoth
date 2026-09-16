---
title: Contributing
description: Build durable coding-agent memory with Mammoth.
---

Mammoth focuses on durable context memory for coding agents. Good starting points
include memory curation, retrieval evaluation, provenance, MCP interoperability,
and clear setup documentation. Read the [memory model](/memory/) before changing
persistence or retrieval behavior.

Use Rust 1.88+ and Node 22.x for frontends. Both main and AI_coded implement memory.

```bash
cargo test --locked -p mammoth-memory -p mammoth-mcp -p mammoth-cli
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

The memory crate owns persistence and revisions. The MCP crate owns protocol
integration and shared CLI commands. The public website lives in web/; the local
memory dashboard on AI_coded lives in ui/. Keep both branches' homepage and memory
experience aligned while preserving their different legacy storage implementations.

See [contributor guidance](https://github.com/ProjectOrcha/Mammoth/blob/main/CONTRIBUTING.md)
and the [roadmap](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/ROADMAP.md).
