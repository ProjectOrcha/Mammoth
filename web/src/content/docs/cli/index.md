---
title: CLI overview
description: POSIX verbs, JSON everywhere, and errors that teach.
---

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
│   ├── health              replication health
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
└── completions bash|zsh|fish|powershell
```

## Design principles

1. **Verbs are POSIX, not Hadoop.** `mammoth ls /data`, not `hdfs dfs -ls /data`.
   `mammoth compat hdfs dfs -ls /` translates old scripts.
2. **Everything has `--json`.** Human tables on a TTY, JSON when piped.
3. **Errors teach.** Never a stack trace — what broke, why, and the next command.
4. **Progress bars on anything over one second**, auto-disabled when piped.
5. **`mammoth doctor` exists** and checks what beginners get wrong.

The full command reference at [`cli/reference`](/Mammoth/cli/reference/) is
generated from the `clap` tree by `cargo xtask docs` and verified in CI, so it
can never drift from the binary.
