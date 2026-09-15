---
title: CLI overview
description: Commands and options supported by the current local service and HTTP gateway.
sidebar:
  order: 1
---

The CLI works with persistent local storage or one HTTP gateway. These pages
describe the commands available in the current build. For installation, follow
the [quickstart](/intro/quickstart/).

The examples use `mammoth` on your PATH. Follow the
[shortcut setup](/intro/install/#use-mammoth-from-any-folder) to use that command
from any folder. You can also run `./target/release/mammoth` directly after a
release build. Use the same storage root as the service:

```bash
export MAMMOTH_LOCAL_ROOT="$PWD/.mammoth"
mammoth ls /sample
mammoth put README.md /sample/readme.md
mammoth df
```

Specify the complete destination filename for uploads, copies, and downloads.
To connect to a running service:

```bash
mammoth --masters http://127.0.0.1:8080 ls /
```

## Where to look

`mammoth --help` lists top-level and nested commands with descriptions.
`mammoth commands` shows the complete catalog, including aliases;
`mammoth <command> --help` explains its arguments.

| Page | Commands |
| --- | --- |
| [Files](/cli/filesystem/) | `ls`, `put`, `get`, `cat`, `head`, `tail`, `mkdir`, `rm`, `mv`, `cp`, `stat`, `du`, `df`, `find`, `chmod`, `chown`, `setrep`, `checksum` |
| [Visualization](/cli/viz/) | block placement, node capacity, topology, skew, namespace sizes, health, `top` |
| [Operations](/cli/operations/) | lifecycle, health checks, repairs, local text jobs, transfers, benchmarks, configuration |
| [Generated reference](/cli/reference/) | complete command tree and accepted flags |

## Global flags

| Flag | Environment variable | Behavior |
| --- | --- | --- |
| `--local-root PATH` | `MAMMOTH_LOCAL_ROOT` | Local store; defaults to `~/.mammoth/local`. |
| `-c`, `--config PATH` | `MAMMOTH_CONFIG` | Read a configuration file. |
| `--masters URL` | `MAMMOTH_MASTERS` | Connect to one HTTP gateway. |
| `--output FORMAT` | — | `auto`, `table`, `json`, `yaml`, or `csv`. |
| `--json` | — | Shorthand for JSON output. |
| `--color MODE` | `NO_COLOR` for automatic mode | `auto`, `always`, or `never`; controls human output and help. |
| `-h`, `--help` | — | Show accepted arguments for a command. |

Configuration values can also be set through nested environment variables:

```bash
MAMMOTH_STORAGE__REPLICATION=2 mammoth put ./scratch.bin /tmp/scratch.bin
```

`--masters` does not provide multi-master discovery. Separate distributed
master and worker services, token management, and high availability remain
[roadmap work](https://github.com/ProjectOrcha/Mammoth/blob/AI_coded/docs/IMPLEMENTATION-STATUS.md).

## Output

Automatic output is a table in a terminal and JSON when piped. Choose an
explicit format for scripts. File content from `cat`, `head`, and `tail` remains
raw regardless of the output flag.

```bash
mammoth node list --json
mammoth ls /sample --output csv
mammoth config show --output yaml
```

## Exit codes and errors

| Exit | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | An operation failed; the error includes a stable code. |
| `2` | Invalid command-line usage, such as an unknown flag. |

Use `mammoth COMMAND --help` when a command is rejected. Earlier design examples
included flags that the local service does not implement. The
[generated reference](/cli/reference/) is generated from the command parser.

| Error | Action |
| --- | --- |
| `E0001` — configuration | Run `mammoth config validate`. |
| `E0101` — missing path | List the parent folder and check the filename. |
| `E0102` — wrong kind | Check whether the path is a file or directory. |
| `E0301` — insufficient workers | Inspect `mammoth node list` and the requested replication count. |
| `E0401` — checksum mismatch | Inspect the file's replicas and run `mammoth admin repair`. |
| `E0500` — local I/O | Check disk space and filesystem access. |
