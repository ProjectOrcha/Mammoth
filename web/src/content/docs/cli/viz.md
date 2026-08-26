---
title: Visualization
description: Every `viz` subcommand and `top` — the flags, and what each screen is actually telling you.
sidebar:
  order: 3
---

Seven commands that answer seven questions, all in the terminal, all over SSH,
none of them needing a browser.

| Command | Answers |
| --- | --- |
| [`viz blocks`](#viz-blocks) | Where did *this file* land? |
| [`viz cluster`](#viz-cluster) | Which node is full, and which is empty? |
| [`viz topology`](#viz-topology) | What is the shape of the cluster? |
| [`viz skew`](#viz-skew) | Why is my job slow? |
| [`viz treemap`](#viz-treemap) | What is eating my disk? |
| [`viz health`](#viz-health) | Is anything at risk right now? |
| [`viz flow`](#viz-flow) | What is moving, and where? |
| [`top`](#top) | All of it, on one live screen. |

Every one takes `--json`, so the same numbers a human reads are the numbers a
script gets. The [full gallery of sample output is on the
visualization page](/Mammoth/concepts/visualization/); this page is the
per-command detail.

## `viz blocks`

```
mammoth viz blocks <PATH> [--limit N] [--wide] [--json]
```

| Flag | Does |
| --- | --- |
| `--limit N` | Rows to draw. Default 20 — a 60,000-block file is not a chart. |
| `--wide` | One column per *volume* rather than per node. Finds a bad disk. |

Draws a block × node matrix. Each cell is one fragment, labelled by what it is:
`d0`–`d5` data, `l0`–`l1` local parity, `p0`–`p1` global parity.

**What to look for, in order:**

1. **A row bunched into one rack band.** That block does not survive that rack.
2. **A column that is empty.** That node is full, draining, or dead — writes are
   skipping it and it will drift further out of balance.
3. **`◌` cells.** Fragments being rebuilt. Reads still work; they reconstruct.

## `viz cluster`

```
mammoth viz cluster [--metric usage|fragments|read|write|latency] [--sort] [--json]
```

Per-node bars, grouped by rack, with the imbalance coefficient underneath.

**The number that matters is `σ`** — the standard deviation of per-node usage. A
healthy cluster sits under 10%. Above that, some nodes are doing more work than
others simply because they hold more data, and the balancer should run:

```bash
mammoth admin balancer start --threshold 10
```

`--metric latency` is the one people forget. A node at 340 ms p99 is worse than
a dead node: a dead node gets routed around in seconds, a slow one quietly makes
everything slow.

## `viz topology`

```
mammoth viz topology [--depth N] [--show-empty] [--json]
```

The rack tree with per-rack and per-node capacity. Use it to confirm that
`node.rack` is actually set on every machine — a node with the default rack is
in *no* failure domain as far as placement is concerned, and it will quietly
break the rack rule for every block it holds.

```console
$ mammoth viz topology --output json | jq -r '.nodes[] | select(.rack == "/default-rack") | .id'
w17
```

That empty output is what you want.

## `viz skew`

```
mammoth viz skew [PATH] [--by-partition] [--top N] [--metric size|reads|writes] [--json]
```

| Flag | Does |
| --- | --- |
| `--by-partition` | Group by partition directory (`dt=…`) rather than by file. |
| `--top N` | Show the N worst. Default 5. |
| `--metric` | What "worst" means. Default `size`. |

**The single most useful command in the tool for anyone debugging a slow job.**
A job's runtime is set by its slowest task, and its slowest task is whichever
one drew the biggest partition. This is how you find that partition in one
command:

```console
$ mammoth viz skew /warehouse/events --by-partition

  PARTITION SIZE DISTRIBUTION            1,024 files · 4.2 TB

    dt=2026-08-03  ████████████████████ 89.0 GB  ⚠ 68× median
    dt=2026-08-02  ██                    1.3 GB
    dt=2026-08-01  ██                    1.2 GB
    ... 1,021 more

  median 1.3 GB   p99 4.1 GB   max 89.0 GB
  ⚠ severe skew — one task processes 89 GB while 1,023 process ~1 GB.
    your job's runtime is set by that one task.
```

The fix is almost never in the cluster; it is in how the data was partitioned.
Re-partition on a higher-cardinality key, or split the hot partition.

## `viz treemap`

```
mammoth viz treemap [PATH] [--depth N] [--min-size S] [--by age|size|reads] [--json]
```

`--depth` defaults to 2. `--by age` is the one that finds money:

```console
$ mammoth viz treemap / --depth 2 --by age

  /tmp       ██░░░░░░░░░░░░░░░░░░░░░░░░    71 TB   6%  ⚠ 94% older than 30 days
```

## `viz health`

```
mammoth viz health [--live] [--refresh SECONDS] [--path PATH] [--json]
```

| Flag | Does |
| --- | --- |
| `--live` | Redraw until interrupted. `q` quits. |
| `--refresh N` | Seconds between redraws. Default 2. |
| `--path P` | Only blocks under this path. |

Counts every block by how much redundancy it has left. For `lrc-6-2-2` that is
fragments out of ten; for a mirrored file it is replicas out of three.

**Read `critical`, not `degraded`.** `9/10` means one fragment is gone and the
block is fine — it reads by reconstructing from its local group, and repair will
get to it. `7/10` is the last safe state for `lrc-6-2-2`: three losses are
survivable, a fourth is not. A non-zero `critical` count is the line that should
make you stop what you are doing.

The screen also prints the repair queue, its rate, how many nodes are
participating, and the ETA. If `participating` is much lower than your healthy
node count, repair is not declustering properly and the rebuild will take far
longer than it should.

[Full sample output, and the erasure-coding-width warning it prints →](/Mammoth/concepts/visualization/)

## `viz flow`

```
mammoth viz flow [--window SECONDS] [--live] [--json]
```

Bytes per second from each source category — clients, repair, balancer, shuffle
— to the nodes receiving them, plus cross-rack link utilisation.

**Watch the width of the repair row, not its rate.** Declustered repair should
reach *every* healthy node. A narrow repair fan means the surviving fragments
are not spread out, and the rebuild is bottlenecked on a few disks.

**Watch cross-rack too.** It is the expensive link and the one that saturates
first. `lrc-6-2-2` keeps single-fragment repair inside one rack precisely so
this row stays small during an incident.

## `top`

```
mammoth top [--refresh SECONDS] [--sort COLUMN] [--filter EXPR]
```

The live TUI. One screen, works over SSH, built with
[`ratatui`](https://ratatui.rs/).

| Key | Does |
| --- | --- |
| `1` `2` `3` `4` | nodes · blocks · jobs · flow |
| `b` | start the balancer |
| `d` | decommission the selected node |
| `/` | filter |
| `q` | quit |

```
┌ mammoth top ─ prod-01 ─────────────────────── 12 nodes ─ leader m1 ─ 14:22:07 ┐
│ CAPACITY  1.1/2.0 PB  ███████████░░░░░ 56%   READ 3.9 GB/s  WRITE 1.2 GB/s   │
├───────────────────────────────────────────────────────────────────────────────┤
│ NODE  RACK    STATE      USED         FRAGS   READ      WRITE   DISK p99      │
│ w1    rack-a  ● healthy  ███████░ 71%   5.1M  378 MB/s  120 MB/s    8ms       │
│ w3    rack-a  ⚠ full     ████████ 94%   6.7M  355 MB/s    0 B/s    12ms       │
│ w7    rack-b  ⚠ slow     ██████░░ 69%   4.9M  127 MB/s   40 MB/s   340ms  ⚠   │
│ w12   rack-c  ✕ dead     ░░░░░░░░  0%      —       —         —       —        │
├───────────────────────────────────────────────────────────────────────────────┤
│ ⚠ 4.2M blocks degraded · rebuilding on 11 nodes · ETA 3h 56m                  │
│ [1]nodes [2]blocks [3]jobs [4]flow  [b]alance [d]ecommission [q]uit           │
└───────────────────────────────────────────────────────────────────────────────┘
```

## Terminal rendering

| Visual | Crate / technique |
| --- | --- |
| Bars | Unicode blocks `█▉▊▋▌▍▎▏` — 1/8 resolution per cell |
| Sparklines | Braille `⣀⣤⣶⣿` or blocks `▁▂▃▄▅▆▇█` |
| Tables | `comfy-table` |
| Colour | `owo-colors`, gated on `is_terminal()` — never ANSI into a pipe |
| Interactive | `ratatui` + `crossterm` |
| Fallback | `--no-color --ascii` for CI logs and dumb terminals |
