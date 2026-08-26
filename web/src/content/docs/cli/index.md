---
title: CLI overview
description: POSIX verbs, JSON everywhere, and errors that teach — plus the global flags that apply to every command.
sidebar:
  order: 1
---

If you know `ls`, `cp` and `df`, you already know most of this CLI. The verbs
are the POSIX ones, they take the flags you expect, and everything prints a
table on a terminal and JSON in a pipe.

```bash
mammoth ls /data                  # not: hdfs dfs -ls /data
mammoth put ./big.parquet /data/  # not: hdfs dfs -put ./big.parquet /data/
mammoth df                        # not: hdfs dfsadmin -report
```

Old scripts do not have to be rewritten on day one:

```bash
mammoth compat hdfs dfs -ls /data     # runs the Mammoth equivalent and tells
                                      # you what it would have been
```

## Where to look

| Page | Commands |
| --- | --- |
| [Files](/Mammoth/cli/filesystem/) | `ls` `put` `get` `cat` `head` `tail` `mkdir` `rm` `mv` `cp` `stat` `du` `df` `find` `chmod` `chown` `setrep` `checksum` |
| [Visualization](/Mammoth/cli/viz/) | `viz blocks` `viz cluster` `viz topology` `viz skew` `viz treemap` `viz health` `viz flow` `top` |
| [Operations](/Mammoth/cli/operations/) | `init` `quickstart` `serve` `ui` `doctor` `node` `cluster` `admin` `job` `migrate` `bench` `config` `token` `completions` |
| [Reference](/Mammoth/cli/reference/) | every flag, generated from the binary |

## The command tree

```
mammoth
├── init                    Create a new cluster (config, IDs, certs)
├── quickstart              One-command demo cluster + sample data + open UI
├── serve --role R          Run a node
├── ui                      Launch/open the web GUI
├── doctor                  Diagnose config, ports, disks, clock, ulimits
│
├── ls | put | get | cat | tail | head | mkdir | rm | mv | cp
├── stat | du | df | find | chmod | chown | setrep | checksum
│
├── viz                     data distribution visualization
│   ├── blocks <path>       where this file's blocks live
│   ├── cluster             per-node capacity heatmap
│   ├── topology            rack/zone tree
│   ├── skew [path]         hotspots and imbalance
│   ├── treemap [path]      which directories eat the space
│   ├── health              redundancy health
│   └── flow                live data movement
├── top                     live TUI dashboard (htop for your cluster)
│
├── node    list | show | decommission | maintenance | remove
├── cluster members | leader | health | join | leave | transfer-leadership
├── admin   report | safemode | fsck | balancer | snapshot | quota | ec | upgrade
├── job     submit | list | status | logs | kill
├── migrate plan | run | resume | verify | sync | cutover
├── bench   dfsio | terasort | metadata
├── config  show | validate | set | diff
├── token   create | list | revoke
├── compat  hdfs …         translate an old Hadoop invocation
└── completions bash|zsh|fish|powershell
```

## Global flags

These work on every command.

| Flag | Env | Default | What it does |
| --- | --- | --- | --- |
| `-c`, `--config <PATH>` | `MAMMOTH_CONFIG` | `/etc/mammoth/mammoth.toml`, then `~/.mammoth/mammoth.toml` | Which `mammoth.toml` to read. |
| `--masters <A,B,C>` | `MAMMOTH_MASTERS` | from the config | Talk to a different cluster without editing a file. |
| `--output <FORMAT>` | — | `auto` | `auto` · `table` · `json` · `yaml` · `csv`. |
| `--json` | — | off | Shorthand for `--output json`. |
| `-v`, `-vv`, `-vvv` | — | off | More detail. `-vvv` includes per-RPC timing. |

Any config key can be overridden by environment variable — uppercase the path
and join it with `__`:

```bash
MAMMOTH_STORAGE__REPLICATION=2 mammoth put ./scratch.bin /tmp/scratch.bin
```

## Output: tables for you, JSON for scripts

`--output auto` is the default, and it does the right thing without being told:
**a table when stdout is a terminal, JSON when it is a pipe.** So the same
command is readable by hand and parseable in a script.

```console
$ mammoth node list
 NODE  RACK         STATE      USED             FRAGMENTS  DISK p99
 w1    /dc1/rack-a  ● healthy  113 TB / 160 TB       5.1M       8 ms
 w3    /dc1/rack-a  ⚠ warn     150 TB / 160 TB       6.7M      12 ms
 w12   /dc1/rack-c  ✕ dead     —                        —         —
```

```console
$ mammoth node list | jq -r '.[] | select(.state != "healthy") | .id'
w3
w12
```

The JSON field names are a **public API**. They will not be renamed without a
major version bump, so it is safe to build on them.

Colour is emitted only when stdout is a terminal, so redirected output never
contains escape sequences. `--output table` forces the human form anyway (handy
for `less -R`), and `--no-color` drops the colour but keeps the table.

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Success. |
| `1` | The operation failed — the message says why. |
| `2` | Bad usage: an unknown flag, a missing argument. |
| `3` | Could not reach a master. Network, address, or nothing running. |
| `4` | Refused on purpose: safe mode, a lease held elsewhere, a quota. |

```bash
mammoth stat /data/report.parquet >/dev/null 2>&1 || echo "not there yet"
```

## Errors that teach

A Mammoth error never prints a stack trace. It names what broke, why, what to
do about it, and where to read more:

```console
$ mammoth put ./big.bin /data/big.bin

error[E0301]: not enough healthy workers for replication 3

  only 2 workers are accepting writes, and this file asks for 3 copies.
  w3 is full (94%) and w12 has not sent a heartbeat in 42m.

  what you can do:
    · lower replication:   mammoth put ./big.bin /data/big.bin --replication 2
    · check node health:   mammoth node list
    · why is a node down:  mammoth doctor --node w12

  docs: https://projectorcha.github.io/Mammoth/errors/E0301
```

Codes are stable and greppable. The common ones:

| Code | Means | Usually fixed by |
| --- | --- | --- |
| `E0001` | The config file is wrong. | `mammoth config validate` |
| `E0101` | No such path. | check the path; `mammoth ls` its parent |
| `E0102` | Wrong kind — a directory where a file was expected, or the reverse. | add `--recursive`, or fix the path |
| `E0201` | Someone else holds the write lease. | wait for it to expire, or `mammoth admin lease list` |
| `E0301` | Not enough healthy workers for the replication asked for. | `mammoth node list`, lower `--replication` |
| `E0302` | The cluster is in safe mode. | `mammoth admin safemode status` — it names the shard |
| `E0401` | Checksum mismatch. | `mammoth admin fsck <path>` |
| `E0500` | Local I/O error. | disk, permissions, `ulimit -n` |

## Design principles

1. **Verbs are POSIX, not Hadoop.** `mammoth ls /data`, not `hdfs dfs -ls /data`.
2. **Everything has `--json`.** Human tables on a TTY, JSON when piped.
3. **Errors teach.** Never a stack trace — what broke, why, and the next command.
4. **Progress bars on anything over one second**, auto-disabled when piped.
5. **`mammoth doctor` exists** and checks what beginners get wrong.

The full flag-by-flag reference at [`cli/reference`](/Mammoth/cli/reference/) is
generated from the `clap` tree by `cargo xtask docs` and verified in CI, so it
can never drift from the binary. The pages here are the hand-written half: what
each command is *for*, and what its output is telling you.
