---
title: Connect with MCP
description: Connect Mammoth’s durable project memory to a coding agent over stdio.
---


Build from either `main` or `AI_coded` with Rust 1.88+:

```bash
cargo build --release --locked -p mammoth-cli -p mammoth-mcp
```

The standalone `mammoth-mcp` binary and `mammoth mcp` expose the same server.
The transport is stdio: your coding agent launches the process. There is no port,
account, gateway, dashboard build, or API key to configure. stdout carries MCP
messages only; diagnostics go to stderr. The server uses the [official Rust MCP SDK](https://github.com/modelcontextprotocol/rust-sdk).

## Client configuration

For clients using an `mcpServers` configuration, add this entry. Replace both
absolute paths and choose a stable project name. Use the same root for CLI access.

```json
{
  "mcpServers": {
    "mammoth": {
      "command": "/absolute/path/to/Mammoth/target/release/mammoth-mcp",
      "args": [
        "--local-root", "/absolute/path/to/memory-store",
        "--project", "my-app"
      ]
    }
  }
}
```

Client configuration filenames and locations vary. Add the entry to your coding
agent's MCP settings, restart its connection, and confirm that the five tools
below appear. For clients configured with command/arguments fields, use the same
values. To use the combined binary, set command to the absolute `mammoth` path and
args to `["--local-root", "/absolute/path/to/memory-store", "mcp", "--project", "my-app"]`.

Append `--read-only` to expose only recall, get, and history. Writes are blocked
as well as hidden. One server connection is fixed to one project; tools have no
project override. Run a separate configuration for a different project. Do not
start a background service first; the host manages this child process.

## Tools

| Tool | Purpose | Required inputs |
| --- | --- | --- |
| `memory_remember` | Create or revise context | `key`, `title`, `content`, `kind`; `expected_revision` for updates |
| `memory_recall` | Search context or list recent entries | None; optional `query`, `limit`, `max_bytes` |
| `memory_get` | Read the complete current entry | `key` |
| `memory_history` | Inspect retained revisions | `key`; optional `limit` |
| `memory_forget` | Delete an entry and its history | `key`, `expected_revision` |

Remember also accepts `tags` and `source`. Keys are scoped to the configured
project. Tool failures report `isError` with an explanation; malformed requests
and unknown tools are rejected with tool or protocol errors. On a revision conflict, read the current
entry, reconcile your change, and retry using that revision. Never blindly retry
with a guessed revision. Successful mutations are committed before returning.

## Verify the connection

Ask your agent to save a short test note, then end the session and ask the next
session to recall it. The server does not automatically read transcripts or inject
context: the agent must call its tools. The repository tests also launch a real
stdio client, save and recall context, restart the server, and verify read-only
behavior. Run `cargo test --locked -p mammoth-memory -p mammoth-mcp`.

If startup fails, check the binary path, executable permission, project name, and
write access to the selected root. Do not paste memory database contents into a
public issue. MCP server mode waits for its client when launched in a terminal;
that is expected.
