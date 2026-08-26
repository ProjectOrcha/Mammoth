---
title: Operations
description: Lifecycle, node and cluster management, admin, jobs, migration, benchmarks, config and tokens — every command with its flags and its output.
sidebar:
  order: 4
---

Everything that is not a file operation or a chart.

## Lifecycle

### `init` — create a cluster

```
mammoth init [--name NAME] [--dir PATH] [--masters A,B,C] [--rack RACK] [--force]
```

Writes `mammoth.toml`, generates a cluster ID and a self-signed CA, and prints
what to run next. It does **not** start anything.

```console
$ mammoth init --name prod-01 --masters m1:7000,m2:7000,m3:7000
  ✔ config      /etc/mammoth/mammoth.toml
  ✔ cluster id  01J8K2QP4X7M
  ✔ CA + certs  /etc/mammoth/pki/
  next:  mammoth serve --role master     (on m1, m2, m3)
         mammoth serve --role worker     (on every storage machine)
```

### `quickstart` — a whole cluster in one command

```
mammoth quickstart [--workers N] [--no-sample-data] [--no-open] [--port PORT]
```

Everything on one machine, for learning and demos: config, a master, N simulated
workers, a gateway, sample data, and the browser opened at the UI. Nothing here
is production — it is one process pretending to be a cluster.

```console
$ mammoth quickstart
  ✔ config written        ~/.mammoth/mammoth.toml
  ✔ started master        127.0.0.1:7000
  ✔ started 3 workers     w1 w2 w3  (simulated, single machine)
  ✔ started gateway       S3 :9000 · UI :8080
  ✔ sample data loaded    /sample/nyc-taxi.parquet (120 MB)

  Web UI  →  http://localhost:8080
```

Stop it with `mammoth serve stop`, which leaves the data and the config in
place so the next `quickstart` picks up where you left off.
`mammoth quickstart --clean` goes further and removes everything it created.

### `serve` — run a node

```
mammoth serve --role master|worker|gateway|all [--listen ADDR] [--rack RACK] [--join ADDR]
```

One binary, four roles. `--role all` runs every role in one process, which is
what `quickstart` uses and what you want on a laptop.

```bash
mammoth serve --role master                       # on each of the three masters
mammoth serve --role worker --rack /dc1/rack-a    # on every storage machine
mammoth serve --role gateway                      # S3 on :9000, UI on :8080
```

`--join` points a new node at an existing cluster instead of the config file.

### `ui` — open the dashboard

```
mammoth ui [--port PORT] [--no-open]
```

Starts the gateway if it is not already running and opens a browser. Under it is
just `serve --role gateway`; this exists so nobody has to remember the port.

### `doctor` — check what beginners get wrong

```
mammoth doctor [--node ID] [--fix] [--json]
```

Run it **before** filing an issue, and again after any change that surprised
you. It checks config validity, port availability, disk space and permissions
on every volume, clock skew against the masters, `ulimit -n`, and reachability
of every peer.

```console
$ mammoth doctor
  ✔ config           valid, 0 unknown keys
  ✔ ports            7000, 8080, 9000 free
  ✔ volumes          3 mounted, 10 GiB reserved each, all writable
  ⚠ clock skew       +9.2s vs m1  — leases assume a shared clock
       fix: enable NTP (timedatectl set-ntp true)
  ✕ ulimit -n        1024, want 1048576
       fix: mammoth systemd install --role worker   (sets LimitNOFILE)
  ✔ peers            m1 m2 m3 reachable, median RTT 0.4 ms

  1 error, 1 warning — run with --fix to apply the 1 safe fix
```

`--fix` applies only changes that cannot lose data. It will raise a ulimit; it
will never reformat a volume.

**Clock skew is not cosmetic.** A worker whose clock runs fast believes leases
have expired that have not, and two writers to one file is how you corrupt it.

## `node` — the workers

```
mammoth node list   [--rack RACK] [--state STATE] [--sort COLUMN] [--json]
mammoth node show   <ID>
mammoth node decommission <ID> [--wait] [--timeout D]
mammoth node maintenance  <ID> [--duration D]
mammoth node remove <ID> [--force]
```

**`decommission` is the safe way to remove a machine.** It marks the node as
draining, so placement stops choosing it, then rebuilds elsewhere every fragment
it uniquely holds — and only then does it report finished. No redundancy is lost
at any point.

```console
$ mammoth node decommission w8 --wait
  w8 draining · 4.9M fragments to relocate · 11 nodes participating
  ▓▓▓▓▓▓▓▓▓░░░░░  68%   ETA 1h 12m
```

**`maintenance` is for a reboot.** It suppresses the repair that a disappearance
would otherwise trigger, for a bounded window:

```bash
mammoth node maintenance w5 --duration 30m
```

Without it, a ten-minute reboot spends `repair.delay` and then starts copying a
hundred terabytes that were never actually lost.

`remove` deletes the node from the topology. Only do it after `decommission`
reports finished — `--force` skips that check and will lose data if the node
still held the last copy of anything.

## `cluster` — the masters

```
mammoth cluster members
mammoth cluster leader
mammoth cluster health
mammoth cluster join  <ADDR>
mammoth cluster leave <ID>
mammoth cluster transfer-leadership [--to ID]
```

```console
$ mammoth cluster members
 MEMBER  ADDRESS            ROLE      APPLIED    LAG  LAST CONTACT
 m1      192.168.1.5:7000   leader    8,413,350    0  —
 m2      192.168.1.6:7000   follower  8,413,347    3  41 ms
 m3      192.168.1.7:7000   follower  8,413,349    1  38 ms
 w1      192.168.1.11:7001  learner   8,413,310   40  120 ms
```

**Learners are workers**, holding a read-only replica of the namespace so a
client with no location lease can resolve `path + range` at the nearest worker
in a single round trip. They never vote and they are allowed to lag — see
[the one-shot read](/concepts/fast-paths/#1--the-one-shot-read).

`transfer-leadership` moves the leader deliberately, which is what you want
before rebooting the machine that currently holds it.

## `admin` — cluster administration

```
mammoth admin report                     # everything, as JSON
mammoth admin safemode status|enter|leave
mammoth admin fsck  <PATH> [--repair] [--delete-corrupt]
mammoth admin balancer start|status|stop [--threshold PCT] [--bandwidth B] [--scope PATH]
mammoth admin snapshot create|list|restore|delete <PATH>
mammoth admin quota set|clear|list <PATH> [--bytes B] [--files N]
mammoth admin ec convert|status <PATH> --policy P
mammoth admin repair status [--live]
mammoth admin upgrade check|start|status|finalize
mammoth admin metadata backup|restore
```

### `safemode`

Read-only until the index is trustworthy. In Mammoth it is **per namespace
shard**, so `status` names the shard that is not ready rather than reporting one
cluster-wide percentage:

```console
$ mammoth admin safemode status
  shard-0  /warehouse    ready        2.18s
  shard-1  /logs         ready        3.01s
  shard-2  /data         reconciling  bucket 617 of w7 still streaming
  shard-3  /tmp · /user  ready        6.24s
  reads are served from the mapped snapshot throughout
```

You should almost never have to `enter` or `leave` it by hand. If a restart is
taking longer than seconds, that is the thing to investigate — see
[warm start](/concepts/fast-paths/#4--warm-start).

### `fsck`

Block-level integrity. Re-derives expected placement, compares it with reality,
and verifies checksums.

```console
$ mammoth admin fsck /warehouse
  5,452,037 blocks checked
  ✔ healthy        1,270,998
  ◐ degraded       4,181,027   already queued for repair
  ✕ corrupt                0
  ⊘ lost                   0
```

`--repair` queues what it finds. `--delete-corrupt` throws away blocks that
cannot be reconstructed — it is destructive and it will make you confirm.

### `balancer`

Evens out per-node usage. `--threshold` is the percentage-point spread it will
tolerate; the default is 10.

```bash
mammoth admin balancer start --threshold 10 --bandwidth 1Gbps
mammoth admin balancer status
mammoth admin balancer stop
```

**Always set `--bandwidth`.** An unthrottled balancer competes with client
traffic and turns a cosmetic imbalance into a latency incident.

### `quota`

```bash
mammoth admin quota set /user/dana --bytes 10TB --files 1000000
mammoth admin quota list
```

Both limits are enforced at write time, and hitting one is a normal error with a
normal message, not a crash.

### `ec` — change encoding in bulk

```console
$ mammoth admin ec convert /warehouse/archive --policy rs-6-3
  ✔ 412 TB queued  ·  will free ~69 TB  ·  ETA 9h 40m
  note: rs-6-3 repairs one loss from 6 fragments; lrc-6-2-2 needs 3.
        cheaper on disk, twice as expensive during an incident.
```

### `repair status`

```console
$ mammoth admin repair status --live
  queued        4,181,027 of 4,747,510 blocks    11.9% rebuilt
  rate          295 blk/s · 6.6 GB/s             11 of 11 healthy nodes
  budget        40% of measured idle bandwidth
  eta           3h 56m                           (49h from a single source)
  worst block   7 of 10 fragments
  cause         w12 stopped answering 42m ago
```

## `job` — compute

```
mammoth job submit <SPEC> [--name NAME] [--priority P]
mammoth job list   [--state running|succeeded|failed] [--user U]
mammoth job status <ID> [--watch]
mammoth job logs   <ID> [--task T] [-f]
mammoth job kill   <ID>
```

```console
$ mammoth job status job-2026-0826-0041
  daily-active-users   running   3m 34s   62%
  locality  88%   ← share of tasks reading a replica on their own machine
  scan      240/240   ████████████████████
  filter    218/240   ██████████████████░░
  shuffle    22/64    ██████░░░░░░░░░░░░░░
  aggregate   0/64    ░░░░░░░░░░░░░░░░░░░░
  ⚠ t59 on w7 has run 1m 28s — 8× the median. w7 disk p99 is 340 ms.
```

**Locality is the number to watch.** Below about 80%, tasks are pulling their
input across the network instead of reading a local replica, and the job is
paying for it.

## `migrate` — get data in

```
mammoth migrate plan|run|resume|verify|sync|cutover|metastore
```

Six steps, plan first, resumable throughout. Fully worked in
[Migration](/migration/).

## `bench` — built-in benchmarks

```
mammoth bench dfsio    [--write|--read] [--size S] [--files N]
mammoth bench terasort  [--size S]
mammoth bench metadata  [--ops N] [--concurrency C]
mammoth bench --report bench.json
```

`dfsio` is throughput, `terasort` is an end-to-end shuffle, `metadata` is
namespace operations per second. `--report` writes a machine-readable file with
the hardware, the flags and the raw numbers.

**Publish the harness with the result or do not publish the result.** A
repeatable benchmark is this project's best argument; a cherry-picked one is its
worst.

## `config` — inspect and validate

```
mammoth config show     [--source] [--json]
mammoth config validate [--file PATH]
mammoth config set      <KEY> <VALUE>
mammoth config diff     [--against defaults|FILE]
```

`--source` is the useful one: it prints the resolved value **and which layer
set it**, which answers "why is this not what I put in the file".

```console
$ mammoth config show --source
  storage.replication      3            /etc/mammoth/mammoth.toml:16
  storage.placement        rendezvous   default
  write.mode               disperse     default
  write.ec_policy          lrc-6-2-2    /etc/mammoth/mammoth.toml:26
  read.lease_ttl           60s          default
  repair.bytes_per_sec     auto         default
  master.block_map         mmap         default
  master.safemode          per-range    MAMMOTH_MASTER__SAFEMODE
```

`config diff --against defaults` shows only what you have changed — usually a
much shorter and more interesting list than the file.

## `token` — authentication

```
mammoth token create --name NAME [--expires D] [--scope read|write|admin] [--path P]
mammoth token list
mammoth token revoke <ID>
```

```bash
mammoth token create --name spark-etl --scope write --path /warehouse --expires 90d
```

The secret is printed **once**, at creation. It is not recoverable — issue a new
one and revoke the old.

## `compat` — run an old Hadoop command

```
mammoth compat hdfs dfs -ls /data
```

Translates the invocation, runs the Mammoth equivalent, and prints what the new
command would have been — so a migrating team can keep its scripts working while
it learns the new verbs.

## `completions`

```bash
mammoth completions zsh  > "${fpath[1]}/_mammoth"
mammoth completions bash > /etc/bash_completion.d/mammoth
mammoth completions fish > ~/.config/fish/completions/mammoth.fish
```
