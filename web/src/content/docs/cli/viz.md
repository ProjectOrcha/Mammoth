---
title: Visualization
description: Inspect block placement, worker capacity, file-size skew, and replica health.
sidebar:
  order: 3
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


The terminal and dashboard both read the same stored files and replica layout.
Use the same local root or HTTP gateway as the service. These are the current
commands; the [generated reference](/cli/reference/) lists their options.
Run `mammoth commands` for the full command catalog, or `mammoth viz --help`
for all visualization commands. Interactive terminals select colored charts
automatically. Redirected output defaults to JSON; use `--output table` to
keep the human view when saving it or using a pager.

## `viz blocks`

```bash
mammoth viz blocks /sample/blocks.bin --output table
mammoth viz blocks /sample/blocks.bin --json
```

Rows are blocks and columns identify workers. Symbols distinguish the primary,
replicas, damaged copies and absent placements. Wide worker sets split into
column groups so every placement remains visible. An inline or empty file has no block
placement. In the dashboard, open a file or choose one on **Distribution**.
Hover, tap, or focus a matrix cell for its details. Large layouts are paginated.

## `viz cluster`

```bash
mammoth viz cluster --output table
mammoth viz heatmap
mammoth viz topology
```

Cluster output uses capacity bars, readable byte units and health colors.
`viz heatmap` is an alias for `viz cluster`. Topology draws a rack-and-worker
tree with health and replica counts. Local workers are directories on one machine; their
capacities are reference values, not separate physical disks or quotas.

## `viz skew`

```bash
mammoth viz skew /sample
mammoth viz skew /sample --by-partition
```

The terminal draws file-size bars sorted largest first, plus median, p99,
maximum and total bytes. A nonzero value always gets a visible marker; its size
is printed beside it. `--by-partition` totals
each direct child's subtree. The dashboard plots every file, including zero-byte
files. In local mode the vertical axis identifies files because read counts are
not collected.

## `viz treemap`

```bash
mammoth viz treemap / --depth 2
```

This draws a namespace tree with logical sizes and proportional bars up to the
chosen depth. Directory sizes include all descendants, including those below
the displayed depth. Percentages refer to the selected root. The dashboard shows
a treemap whose area represents bytes. Click a directory to inspect its contents.
Empty files and directories have no area.

## `viz health`

```bash
mammoth viz health
mammoth viz health --live
```

In a terminal, live mode refreshes the same screen every two seconds. Use the
arrow keys or **j/k** to scroll and **q**, Escape or Ctrl-C to exit. A single
snapshot uses labeled health bars, including zero-count categories. Inspect under-replicated,
critical, corrupt, or missing blocks before running `mammoth admin repair`.
The dashboard shows the same health categories on Overview, Distribution, and Cluster.

## `viz flow`

The local service has no network-flow measurements. `mammoth viz flow` returns
`available: false`. The local dashboard shows replica health in this space.
Distributed flow and historical replay are demonstrated only in the explicitly
labelled standalone demo.

## `top`

```bash
mammoth top
mammoth top --once
```

In a terminal, `top` refreshes the cluster dashboard. Use the arrow keys or
**j/k** to scroll; Home/End select the first/last row. Press **q**, Escape, or
Ctrl-C to exit. `--once` prints capacity and health charts. Explicit structured
output such as `top --json` prints a snapshot. The planned node-management keyboard
shortcuts are not available.

## Colour

Terminal charts, listings, errors and help support `--color auto|always|never`:

```bash
mammoth viz treemap / --color always --output table
mammoth viz health --color never
mammoth --color always --help
```

`auto` uses color in a terminal and respects `NO_COLOR` and `TERM=dumb`.
`always` forces color for human output, including redirects; `never` disables
it. Symbols and labels keep the views understandable without color. JSON, YAML
and CSV exports contain no styling, and `cat`, `head` and `tail` preserve file bytes.
