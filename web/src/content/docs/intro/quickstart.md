---
title: Your first durable memory
description: Save a decision, close the process, and recall it in a new session.
---

[Install Mammoth](/intro/install/), then choose a stable project name:

```bash
mammoth memory --project my-app remember pagination \
  --title "API pagination" --kind decision \
  --content "Use cursor pagination for the event log." \
  --tag api --source src/events.rs
```

The response includes `revision: 1`. That process can exit; the entry is on disk.
In another terminal or session, recall it from the same local root:

```bash
mammoth memory --project my-app recall "pagination"
mammoth memory --project my-app get pagination
```

Save a handoff when you finish working:

```bash
mammoth memory --project my-app remember current-handoff \
  --title "Next session" --kind handoff \
  --content "Pagination decision saved. Next: implement cursor validation and tests."
```

[Connect your coding agent through MCP](/memory/mcp/) to use the same store.
The agent must call remember and recall; Mammoth does not automatically capture
conversations. Read the [workflow](/memory/workflow/) for suggested instructions.

To revise an entry, first get its revision and pass `--expected-revision` when
remembering it again. This prevents two agents overwriting one another's work.
See the [CLI guide](/memory/cli/) for history and deletion.

## Optional memory dashboard

On `AI_coded`, build the UI before the CLI, then start the local service using
the same root as your MCP connection:

```bash
npm --prefix ui ci
npm --prefix ui run build
cargo build --locked -p mammoth-cli
./target/debug/mammoth --local-root /absolute/path/to/memory-store quickstart --no-sample
```

Open [localhost:8080](http://localhost:8080) to save, search, read, and edit project
memories. The dashboard never substitutes simulated memories. The HTTP adapter is
on AI_coded; CLI and MCP memory work on both branches without the dashboard.
