---
title: CLI guide
description: Learn Mammoth in three commands, then update and organize your saved project context.
---

Use Mammoth from your terminal to **save something about a project and find it
again later**. Start with the three steps below. You do not need to run a server.

This guide assumes `mammoth` is installed. If your terminal says “command not
found,” follow the [installation guide](/intro/install/) first.

## 1. Save your first memory

Copy this whole command into your terminal. The `\` characters let one command
continue across several lines in macOS or Linux shells:

```bash
mammoth memory --project demo remember testing \
  --title "Running tests" \
  --content "Run cargo test before opening a pull request."
```

You have saved a memory called `testing` inside the `demo` project.

| Part of the command | What it means |
| --- | --- |
| `mammoth memory` | Work with saved project context. |
| `--project demo` | Keep this project's memories together. Replace `demo` with your own project name when you are ready. |
| `remember testing` | Save a memory with the key `testing`. A key is a short name you choose so you can read or change this memory later. |
| `--title "Running tests"` | Give the memory a readable heading. |
| `--content "…"` | Write the information you want to keep. |

The command prints the saved memory as **JSON**, a text format that coding agents
can also read. Braces and quoted field names are normal output, not an error.
Part of the result looks like this:

```json
{
  "project": "demo",
  "key": "testing",
  "title": "Running tests",
  "content": "Run cargo test before opening a pull request.",
  "revision": 1
}
```

This is an excerpt; the full result includes more fields. A key that has never
been used before starts at revision `1`. If `testing` already exists, use another
key or follow [Change a memory](#change-a-memory) below.

## 2. Find it again

Search for a word from the title or content:

```bash
mammoth memory --project demo recall "tests"
```

Look for your entry inside the `memories` list in the result. Search matches words
in titles, content, and tags. If you search for several words, all must match.

To see recent memories without searching, leave out the search text:

```bash
mammoth memory --project demo recall
```

## 3. Read the full memory

If you know the key, use `get`:

```bash
mammoth memory --project demo get testing
```

This prints the complete saved entry and its current `revision`. You can close
your terminal, open another one, and run the same command: the memory is saved on
disk. Keep using the same project name and storage folder.

**That is enough to get started.** Save useful facts with `remember`, search with
`recall`, and read an exact entry with `get`. To let your coding agent do this for
you, [connect it through MCP](/memory/mcp/).

## Commands at a glance

Every command below follows `mammoth memory --project demo`.

| Command | Use it to… |
| --- | --- |
| `remember KEY --title "…" --content "…"` | Save a new memory. |
| `recall "WORDS"` | Search saved memories. |
| `recall` | Show recent memories. |
| `get KEY` | Read one complete memory. |
| `history KEY` | See previous versions, newest first. |
| `forget KEY --expected-revision NUMBER` | Delete a memory and its history. |

## Change a memory

First, read the entry you want to change:

```bash
mammoth memory --project demo get testing
```

Find the `revision` number in the result. Then save the new text with the same
key and that number. **The example below assumes the result says `"revision": 1`.**
Replace `1` if yours is different.

```bash
mammoth memory --project demo remember testing \
  --title "Running tests" \
  --content "Run cargo test and cargo fmt --check before opening a pull request." \
  --expected-revision 1
```

Mammoth saves a new version and increases the revision number. The revision check
prevents you from overwriting a change another agent has just made. If you get a
revision conflict, run `get` again and review the latest text before retrying.

An update replaces the title, content, kind, tags, and source. Include any optional
fields you want to keep.

To read the saved versions:

```bash
mammoth memory --project demo history testing
```

## Optional: organize your memories

Add these options to a `remember` command when you need them:

| Option | Example | Purpose |
| --- | --- | --- |
| `--kind` | `--kind decision` | Choose `note` (the default), `decision`, `convention`, or `handoff`. |
| `--tag` | `--tag rust --tag tests` | Add searchable labels. Repeat the option for more than one tag. |
| `--source` | `--source CONTRIBUTING.md` | Record where the fact came from. This is a reference; Mammoth does not read or import the file. |

For example, save a separate decision:

```bash
mammoth memory --project demo remember pagination \
  --title "API pagination" \
  --content "Use cursor pagination for the event log." \
  --kind decision --tag api --source src/events.rs
```

## Optional: choose where memories are stored

Your **project name** groups related memories. Your **storage folder** is where
Mammoth keeps its local database. Project names do not automatically follow your
current directory or Git branch.

By default, the database is `~/.mammoth/local/agent-memory.sqlite3`. If you have set
the `MAMMOTH_LOCAL_ROOT` environment variable, Mammoth uses that folder instead.
You can always choose a folder explicitly:

```bash
mammoth --local-root ./mammoth-demo memory --project demo recall
```

Use the same folder and project in the CLI, dashboard, and
[MCP configuration](/memory/mcp/) to see the same memories. An absolute folder
path is best when switching between terminals or tools. Keep the database out of
Git; its contents stay on your machine unless you copy or sync them yourself.

## Delete a memory

**Deleting also removes the entry's saved history.** Read the current entry with
`get` first, then pass its revision number. Only run the example below if you want
to remove `testing` and its current revision is `2`:

```bash
mammoth memory --project demo forget testing --expected-revision 2
```

A successful result includes `"forgotten": "testing"`.

## If something does not work

| What you see | What to do |
| --- | --- |
| `command not found: mammoth` | Complete [installation](/intro/install/), including adding Mammoth to your PATH, or use the full path to the binary. |
| A revision conflict when saving | That key already exists or has changed. Run `get`, review its current revision, and use `--expected-revision` to update it. |
| An empty `memories` list | Try `recall` without search text. Check that the project name and storage folder match the ones you used to save. |
| `memory not found` | No memory with that exact key exists in this project and storage folder. Use `recall` to find the right key. |
| `"truncated": true` in recall output | Some text or matching entries were left out to fit the result limit. Use `get KEY` to read a complete entry. |

<details>
<summary>More options and built-in help</summary>

`recall` returns up to 10 entries by default. Use `--limit` to request 1–50 entries,
or `--max-bytes` to change the size budget for the returned memory list
(512–65,536 bytes; default 16,000):

```bash
mammoth memory --project demo recall "tests" --limit 5 --max-bytes 8000
```

`history` also accepts `--limit` (1–50; default 10).

To see the available memory commands:

```bash
mammoth memory --help
```

To see all options for one command:

```bash
mammoth memory --project demo remember --help
```

All memory commands return JSON. Output-format options for Mammoth's older
storage commands do not change memory output.

</details>
