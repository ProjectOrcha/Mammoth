---
title: Install Mammoth
description: Build the CLI and MCP server for durable coding-agent memory.
---

Requires **Rust 1.88+**, Cargo, Git, and a C compiler for bundled SQLite.
Node is only needed for building the website or legacy dashboard.

```bash
git clone https://github.com/ProjectOrcha/Mammoth.git
cd Mammoth
cargo build --release --locked -p mammoth-cli -p mammoth-mcp
./target/release/mammoth memory --help
./target/release/mammoth-mcp --help
```

The same commands work on `AI_coded`. On Windows, use the `.exe` binaries in
`target/release`. Build from source for this memory preview; older downloaded
releases may predate these commands.

## Use Mammoth from any folder

Install both binaries from the checkout:

```bash
cargo install --locked --path crates/mammoth-cli
cargo install --locked --path crates/mammoth-mcp
```

Ensure Cargo's binary directory is on your PATH. MCP clients should use an absolute
binary path because their environment may differ from your terminal.

Memory defaults to `~/.mammoth/local/agent-memory.sqlite3`. To select a store:

```bash
mammoth --local-root /absolute/path/to/memory-store memory --project my-app recall
```

Use the same absolute root in your [MCP configuration](/memory/mcp/).
