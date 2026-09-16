---
title: Memory CLI
description: Save, recall, revise, and remove project context from the terminal.
---

All memory commands return JSON. Global `--local-root` or `MAMMOTH_LOCAL_ROOT`
selects the store; default is `~/.mammoth/local`. `--project` is required on the
memory command. These commands run locally and do not use `--masters`.

```bash
mammoth memory --project my-app remember pagination \
  --title "API pagination" --kind decision \
  --content "Use cursor pagination for the event log." \
  --tag api --source src/events.rs
mammoth memory --project my-app recall "pagination" --limit 5 --max-bytes 8000
mammoth memory --project my-app get pagination
mammoth memory --project my-app remember pagination \
  --title "API pagination" --kind decision \
  --content "Use cursor pagination ordered by event ID." \
  --expected-revision 1 --tag api --source src/events.rs
mammoth memory --project my-app history pagination --limit 5
mammoth memory --project my-app forget pagination --expected-revision 2
```

Kinds are `note` (default), `decision`, `convention`, and `handoff`. Repeat `--tag`
for multiple tags. Updates replace all entry fields, so include any tags and source
you want to keep. A missing expected revision only creates a new key; it cannot
replace an existing entry. Forget deletes all revisions of that key.

Run `mammoth memory --help` or `mammoth mcp --help` for options. The legacy storage
commands remain for existing users; the agent-memory commands are the primary
product interface. Memory output is always JSON, independent of the storage
commands' table/YAML/CSV formatting options.
