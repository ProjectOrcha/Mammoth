---
title: Data distribution visualization
description: Every `mammoth viz` view, and the six charts on the web UI distribution page.
---

This is the feature that makes Mammoth feel different from Hadoop. Hadoop shows you
tables of numbers. Mammoth shows you where your data actually is.

This is the feature that makes Mammoth _feel_ different from Hadoop. Hadoop's web UI shows you tables of numbers. Yours shows you where your data actually is.

## 7.1 CLI: `mammoth viz`

### `viz blocks` — where did my file land?

```console
$ mammoth viz blocks /data/sales-2026.csv

  /data/sales-2026.csv   350 MB · 3 blocks · replication 3

           w1    w2    w3    w4    w5    w6
  blk 1    ●     ●     ●     ·     ·     ·
  blk 2    ·     ●     ●     ●     ·     ·
  blk 3    ●     ·     ●     ·     ●     ·

  ● primary   ◐ replica   ✕ corrupt   · absent

  racks:   w1 w2 ∈ rack-a    w3 w4 ∈ rack-b    w5 w6 ∈ rack-c
  ⚠ blk 1 has all 3 replicas in racks a,b — rack-c unused
    this file survives a rack failure, but placement is unbalanced
    fix: mammoth admin balancer start --scope /data/sales-2026.csv
```

### `viz cluster` — capacity heatmap

```console
$ mammoth viz cluster

  CLUSTER STORAGE  ·  1.24 PB / 2.00 PB used (62%)

  rack-a   w1 ████████████░░░░ 71%   w2 █████████░░░░░░░ 58%
           w3 ███████████████░ 94% ⚠ w4 ██████░░░░░░░░░░ 38%
  rack-b   w5 ██████████░░░░░░ 63%   w6 ██████████░░░░░░ 64%
           w7 ███████████░░░░░ 69%   w8 ████░░░░░░░░░░░░ 24% ⚠
  rack-c   w9 ██████████░░░░░░ 61%  w10 ███████████░░░░░ 68%
          w11 ██████████░░░░░░ 62%  w12 ░░░░░░░░░░░░░░░░  0% ✕ dead

  imbalance  σ = 21.4%   (healthy < 10%)
  ⚠ w3 is nearly full — new writes will skip it
  ⚠ w8 is under-used — the balancer would move ~180 GB onto it

  → mammoth admin balancer start --threshold 10
```

### `viz topology` — the rack tree

```console
$ mammoth viz topology

  cluster prod-01                                    1.24 PB / 2.00 PB
  │
  ├── /dc1/rack-a                                     412 TB / 640 TB  64%
  │   ├── w1   192.168.1.11   ● healthy    112 TB / 160 TB   4 vols
  │   ├── w2   192.168.1.12   ● healthy     93 TB / 160 TB   4 vols
  │   ├── w3   192.168.1.13   ⚠ near-full  150 TB / 160 TB   4 vols
  │   └── w4   192.168.1.14   ● healthy     57 TB / 160 TB   4 vols
  │
  ├── /dc1/rack-b                                     440 TB / 640 TB  69%
  │   └── ...
  └── /dc1/rack-c                                     388 TB / 720 TB  54%
      └── w12  192.168.1.32   ✕ dead       last seen 12m ago
```

### `viz skew` — find your hotspots

The most useful command for anyone debugging a slow job.

```console
$ mammoth viz skew /warehouse/events

  PARTITION SIZE DISTRIBUTION           1,024 files · 4.2 TB

    dt=2026-08-01  ██                    1.2 GB
    dt=2026-08-02  ██                    1.3 GB
    dt=2026-08-03  ████████████████████ 89.0 GB  ⚠ 68× median
    dt=2026-08-04  ██                    1.1 GB
    dt=2026-08-05  █                     0.9 GB
    ... 1,019 more

  median 1.3 GB   p99 4.1 GB   max 89.0 GB
  ⚠ severe skew — one task will process 89 GB while 1,023 process ~1 GB.
    your job's runtime is set by that one task.

  ACCESS HEAT (last 7d)                READ  WRITE
    dt=2026-08-03  ███████████████████  8.2k   12
    dt=2026-08-25  ████████             3.1k  842
    everything else ▁                    <100   <10

  → 12% of your data serves 79% of reads.
    consider: mammoth admin tier set /warehouse/events/dt=2026-08-03 --tier ssd
```

### `viz treemap` — what's eating my disk?

```console
$ mammoth viz treemap / --depth 2

  /                                                          1.24 PB
  ├── /warehouse ████████████████████████████████░░░░░░░░░░   842 TB  68%
  │   ├── events    ████████████████████████░░░░░░░░░░░░░░░   612 TB
  │   ├── sales     ██████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   148 TB
  │   └── dim       ███░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░    82 TB
  ├── /logs      ████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░   310 TB  25%
  ├── /tmp       ██░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░    71 TB   6%  ⚠
  └── /user      ▏░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░    17 TB   1%

  ⚠ /tmp holds 71 TB, 94% older than 30 days
    → mammoth rm /tmp --older-than 30d --dry-run
```

### `viz health` — replication status, live

```console
$ mammoth viz health --live

  BLOCK HEALTH                                    refreshing every 2s

  ● healthy (3/3)      ████████████████████████████  4,201,882   99.97%
  ◐ under-repl (2/3)   ▎                                 1,204    0.03%
  ◐ critical  (1/3)    ▏                                    12  ← urgent
  ○ over-repl (4/3)                                          88
  ✕ corrupt                                                   0
  ⊘ missing   (0/3)                                           0

  recovery queue   1,216 blocks    ▓▓▓▓▓▓▓░░░░░░░  52%   ETA 4m 12s
  recovery rate    284 blk/s · 3.1 GB/s
  cause            w12 went dead 12m ago

  press q to quit
```

### `viz flow` — live data movement

```console
$ mammoth viz flow

  DATA MOVEMENT  ·  last 60s

  clients  ──────── 2.1 GB/s ────────▶  w1 w2 w4 w5 w7 w9
  replication  ──── 3.1 GB/s ────────▶  w8 w10 w11   (recovering from w12)
  balancer   ────── 180 MB/s ─────────▶  w8
  shuffle    ────── 890 MB/s ─────────▶  cross-rack

  cross-rack traffic  4.0 GB/s / 10 GB/s link   ▓▓▓▓▓▓░░░░  40%
```

### `mammoth top` — the TUI dashboard

Built with **`ratatui`**. One screen, live, works over SSH:

```
┌ mammoth top ─ prod-01 ─────────────────────── 12 nodes ─ leader m1 ─ 14:22:07 ┐
│ CAPACITY  1.24/2.00 PB  ████████████░░░░ 62%   READ 2.1 GB/s  WRITE 680 MB/s │
├───────────────────────────────────────────────────────────────────────────────┤
│ NODE  RACK    STATE      USED         BLOCKS   READ      WRITE   DISK p99     │
│ w1    rack-a  ● healthy  ███████░ 71%  1.2M   412 MB/s  120 MB/s   8ms       │
│ w2    rack-a  ● healthy  █████░░░ 58%  0.9M   380 MB/s   98 MB/s   7ms       │
│ w3    rack-a  ⚠ full     ████████ 94%  1.6M   201 MB/s    0 B/s   12ms       │
│ w7    rack-b  ⚠ slow     ██████░░ 69%  1.1M   140 MB/s   40 MB/s  340ms  ⚠   │
│ w12   rack-c  ✕ dead     ░░░░░░░░  0%     —        —         —      —        │
├───────────────────────────────────────────────────────────────────────────────┤
│ ⚠ 1,204 blocks under-replicated · recovering · ETA 4m                        │
│ [1]nodes [2]blocks [3]jobs [4]flow  [b]alance [d]ecommission [q]uit           │
└───────────────────────────────────────────────────────────────────────────────┘
```

**How to render these in the terminal:**

|Visual|Crate / technique|
|---|---|
|Bars|Unicode blocks `█▉▊▋▌▍▎▏` — 1/8 resolution per cell|
|Sparklines|Braille `⣀⣤⣶⣿` or blocks `▁▂▃▄▅▆▇█`|
|Tables|`comfy-table`|
|Color|`owo-colors`, gated on `is_terminal()` — never emit ANSI when piped|
|Interactive|`ratatui` + `crossterm`|
|Fallback|`--no-color --ascii` for CI logs and dumb terminals|

## 7.2 Web UI: the `/distribution` page

Six visuals, all fed by `/api/v1/distribution/*` and live-updated over SSE:

**1 · Node heat grid** — one tile per node, arranged by rack. Color = chosen metric (usage / block count / read QPS / write QPS / disk latency). Hover for detail, click to drill into the node. This is the one people will screenshot.

```
rack-a  ▢▢▩▣   rack-b  ▩▢▢▤   rack-c  ▢▢▢✕
        └ w3 94%                └ w8 24%          └ w12 dead
```

**2 · Block placement matrix** — for a selected file, a D3 grid of blocks (rows) × nodes (columns), cells colored by replica state. Instantly reveals bad placement.

**3 · Namespace treemap** — ECharts `treemap`, area = bytes, color = age or access frequency. Click to zoom into a directory. Answers "what's eating my disk" in one glance.

**4 · Rack topology** — ECharts `graph` (force-directed) or a simple tree. Nodes sized by capacity, colored by health, edges show cross-rack traffic volume.

**5 · Skew scatter** — one point per partition/file, x = size, y = access count. Outliers in the top-right are your hot spots. Brush-select to get the file list.

**6 · Flow sankey** — ECharts `sankey`, showing bytes moving between source categories (clients / replication / balancer / shuffle) and destination nodes over the last N minutes.

Plus a **time machine slider**: replay the last 24h of distribution state. Watching blocks redistribute after a node failure is both genuinely useful and a great demo.

**Small ECharts example (heat grid):**

```ts
// ui/src/lib/charts/HeatGrid.svelte
const option = {
  tooltip: { formatter: (p) => `${p.data.node}<br/>${p.data.pct}% used` },
  visualMap: { min: 0, max: 100, inRange: { color: ['#1b5e20','#f9a825','#b71c1c'] } },
  series: [{
    type: 'heatmap',
    data: nodes.map(n => [n.col, n.row, n.pct]),
    label: { show: true, formatter: (p) => nodes[p.dataIndex].node },
  }],
  xAxis: { type: 'category', show: false },
  yAxis: { type: 'category', data: racks },
};
```
