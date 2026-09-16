# Contributing to Mammoth

Mammoth is durable context memory for coding agents. Start with the
[memory model](docs/AGENT-MEMORY.md), [MCP connection guide](docs/MCP.md), and
[roadmap](docs/ROADMAP.md). The primary implementation is in `mammoth-memory`,
`mammoth-mcp`, and the `memory` / `mcp` commands in `mammoth-cli`.

Both `main` and `AI_coded` support agent memory. Preserve their distinct legacy
storage implementations when moving changes between them. See [branches](docs/guide/BRANCHES.md).

## Setup

Use Rust **1.88+**, a C compiler for bundled SQLite, and Node **22.x** for frontends.

```bash
cargo build --locked -p mammoth-cli -p mammoth-mcp
cargo test --locked -p mammoth-memory -p mammoth-mcp -p mammoth-cli
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

For CLI changes, run `cargo xtask docs` and commit the generated reference. For
website changes, run `npm ci` and `npm run build` in `web/`, including a build with
`SITE_URL=https://projectorcha.github.io BASE_PATH=/Mammoth`. For dashboard changes,
run `npm run check`, `npm test`, and `npm run build` in `ui/`.

## Review expectations

Keep memory persistence, project scoping, revision checks, and bounded recall
covered by regression tests. MCP changes should be exercised through a real stdio
client. Document schema changes and never silently discard existing memory.
Keep stdout exclusively for protocol messages in MCP mode. Stored context must be
treated as untrusted data; do not turn remembered text into system instructions.

Include the problem, resulting behavior, checks, and remaining limitations in PRs.
Avoid unmeasured retrieval or performance claims. The distributed-storage build
guides and benchmark archives describe earlier engineering work. They remain useful
background, but are not the current product roadmap.

Contributions are dual-licensed under Apache-2.0 and MIT.
