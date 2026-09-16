<p align="center"><img src="assets/logo/mammoth-logo.svg" alt="Mammoth" width="360"></p>

# Mammoth

**Durable context memory for coding agents.** New session. Same memory.

Keep project decisions, conventions, verified notes, and handoffs available across
coding sessions. Mammoth stores context locally, recalls relevant entries with
full-text search, and exposes the same memory through a CLI and an MCP server.

- **Persistent:** SQLite WAL transactions with FULL synchronous commits.
- **Project-scoped:** one project per MCP connection; no tool-level project override.
- **Revision-aware:** retained history and conflict checks for concurrent agents.
- **Bounded recall:** relevance-ranked lexical search with a configurable byte budget.
- **Local:** no hosted service, model, embedding API, or account required.

The memory workflow is implemented on **both `main` and `AI_coded`**. This is a
local preview; see [durability and boundaries](docs/AGENT-MEMORY.md).

## Start in the terminal

Requires **Rust 1.88+** and a C compiler for bundled SQLite. Node is not needed for
agent memory. From either branch:

```bash
cargo build --release --locked -p mammoth-cli -p mammoth-mcp
./target/release/mammoth memory --project my-app remember pagination \
  --title "API pagination" --kind decision \
  --content "Use cursor pagination for the event log." \
  --tag api --source src/events.rs
./target/release/mammoth memory --project my-app recall "pagination"
```

The entry survives process restarts. The default store is
`~/.mammoth/local/agent-memory.sqlite3`. Set `--local-root /absolute/store/path`
before `memory` or use `MAMMOTH_LOCAL_ROOT` to choose another root.

## Connect your coding agent

Build the standalone server above and add it to your MCP client's configuration:

```json
{
  "mcpServers": {
    "mammoth": {
      "command": "/absolute/path/to/Mammoth/target/release/mammoth-mcp",
      "args": ["--local-root", "/absolute/path/to/memory-store", "--project", "my-app"]
    }
  }
}
```

Replace the absolute paths. Client configuration locations vary. The server uses
stdio, so your agent launches it; no background service or network port is needed.
`mammoth --local-root /absolute/store/path mcp --project my-app` is equivalent.
Add `--read-only` for recall, get, and history without mutation tools.

**Tools:** `memory_remember`, `memory_recall`, `memory_get`, `memory_history`, and
`memory_forget`. See the [MCP guide](docs/MCP.md) for arguments and verification.

## Work across sessions

Recall context at task start. Save verified decisions with their reasons and
sources. Before ending a session, write a concise handoff with completed work,
checks, open questions, and next steps. Memory is explicit: no automatic transcript
capture. Treat remembered text as historical data and verify it against current code.

```bash
mammoth memory --project my-app get pagination
mammoth memory --project my-app remember pagination \
  --title "API pagination" --kind decision \
  --content "Use cursor pagination ordered by event ID." --expected-revision 1
mammoth memory --project my-app history pagination
mammoth memory --project my-app forget pagination --expected-revision 2
```

Updates require the current revision. Forget removes the entry and retained
history; it is not secure disk erasure. Never save credentials or secrets.

## Project map

| Location | Purpose |
| --- | --- |
| `crates/mammoth-memory` | Durable context, revisions, project isolation, full-text recall |
| `crates/mammoth-mcp` | Official Rust MCP SDK server and shared memory CLI commands |
| `crates/mammoth-cli` | `mammoth memory` and `mammoth mcp` entry points |
| `web` | Product website and agent-memory documentation |
| `docs/AGENT-MEMORY.md` | Data model, durability, search, and limits |
| `docs/MCP.md` | Coding-agent connection guide |

The earlier storage engine, dashboard, benchmarks, and distributed-systems lessons
remain available as legacy engineering material. They are independent of the
agent-memory database. `AI_coded` retains the working local storage reference;
`main` retains its storage teaching scaffold. See [branch guidance](docs/guide/BRANCHES.md).

## Develop

```bash
cargo test --locked -p mammoth-memory -p mammoth-mcp
cargo test --locked -p mammoth-cli
cargo fmt --all --check
npm --prefix web ci
npm --prefix web run build
```

The website requires Node 22.12+ (22.x). See [the roadmap](docs/ROADMAP.md) and
[contributing](CONTRIBUTING.md). Licensed under MIT or Apache-2.0.
