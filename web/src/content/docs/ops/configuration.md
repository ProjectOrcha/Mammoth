---
title: Configuration
description: One file, sane defaults. No XML, no six files.
---

`/etc/mammoth/mammoth.toml` — the whole thing. No XML, no six files.

## The short version

Most of the file below is defaults you will never touch. A working cluster needs
**four** things set, and `mammoth init` writes three of them for you:

```toml
[cluster]
masters = ["m1:7000", "m2:7000", "m3:7000"]   # who the masters are

[node]
role = "worker"                               # what this machine is
rack = "/dc1/rack-a"                          # which failure domain it is in

[storage]
volumes = ["/data/d1", "/data/d2", "/data/d3"] # one entry per physical disk
```

**`node.rack` is the one people forget**, and forgetting it is expensive: a node
with no rack is in *no* failure domain as far as placement is concerned, so
every block it holds quietly loses its rack guarantee. Check it with:

```bash
mammoth viz topology --output json | jq -r '.nodes[] | select(.rack == "/default-rack") | .id'
```

Empty output is what you want.

**`storage.volumes` wants one entry per spindle or NVMe device**, not one entry
per mount point that happens to exist. Two volumes on one physical disk make the
cluster think it has twice the parallelism it has.

Verify before you start anything:

```bash
mammoth config validate     # syntax, unknown keys, impossible combinations
mammoth doctor              # ports, disks, clock skew, ulimits
```

## The whole file

```toml
[cluster]
name    = "prod-01"
masters = ["m1:7000", "m2:7000", "m3:7000"]

[node]
role = "worker"                  # master | worker | gateway | all
rack = "/dc1/rack-a"

[storage]
volumes          = ["/data/d1", "/data/d2", "/data/d3"]
block_size       = "128MiB"
replication      = 3
inline_threshold = "1MiB"        # small files skip the block layer entirely
reserved_space   = "10GiB"
placement        = "rendezvous"  # rendezvous | explicit — computed, not stored

[storage.scrub]
enabled       = true
bytes_per_sec = "50MiB"

[write]
mode        = "disperse"         # disperse | mirror | pipeline
ec_policy   = "lrc-6-2-2"        # 6 data · 2 local parities · 2 global
ack_policy  = "quorum"           # all | quorum — quorum acks at k+1 fragments
packet_size = "64KiB"
window      = "8MiB"             # per-fragment sliding window

[read]
short_circuit  = true
hedged_after   = "50ms"
cache_size     = "4GiB"
lease_ttl      = "60s"           # how long a location lease stays usable
inline_resolve = true            # workers resolve paths from their own learner

[repair]
delay         = "10m"            # grace period for an absent-but-not-dead node
parallelism   = "auto"           # auto = every healthy node participates
bytes_per_sec = "auto"           # token bucket; yields to client traffic
priority      = "redundancy"     # least-redundant blocks first

[master]
listen             = "0.0.0.0:7000"
data_dir           = "/var/lib/mammoth/meta"
heartbeat_ms       = 3000
dead_after         = "10m"
block_map          = "mmap"      # mmap | rebuild — rebuild is HDFS behaviour
merkle_fanout      = 1024        # leaves in the worker's block-ID Merkle tree
safemode           = "per-range" # per-range | global
safemode_threshold = 0.999       # only consulted when safemode = "global"

[gateway]
s3_listen = "0.0.0.0:9000"
ui_listen = "0.0.0.0:8080"

[security]
tls  = "auto"                    # auto | required | off (dev only)
auth = "token"                   # token | mtls | kerberos | none

[telemetry]
metrics_listen = "0.0.0.0:9100"
log_format     = "json"
```

Everything overridable by env: `MAMMOTH_STORAGE__REPLICATION=2`.

## The keys that change the shape of the system

Most of the file is sizing. These six change *how* the cluster works, and each
one is explained in [The four fast paths](/Mammoth/concepts/fast-paths/).

| Key | Default | What flipping it does |
| --- | --- | --- |
| `storage.placement` | `rendezvous` | `explicit` stores placement in the master, HDFS-style. Everything else on this list depends on `rendezvous`, so change it last and expect the rest to get slower. |
| `write.mode` | `disperse` | `mirror` sends whole copies down a 2-level tree — 1× uplink, 3× storage. `pipeline` is the HDFS chain, kept for migration comparisons. |
| `write.ec_policy` | `lrc-6-2-2` | `rs-6-3` is 1.5× storage but reads 6 fragments to fix 1. LRC costs 1.67× and reads 3, from inside one rack. |
| `read.lease_ttl` | `60s` | How long a client may read a file without asking the master anything. Longer = fewer round trips, slower reaction to a topology change. Leases are epoch-stamped, so a stale one fails safe. |
| `repair.bytes_per_sec` | `auto` | The single most dangerous key here. Uncapped repair on a big failure will take client traffic down with it. `auto` measures idle bandwidth; set a number only if you have a reason. |
| `master.block_map` | `mmap` | `rebuild` re-derives the map from block reports at every start — the 30-minute HDFS boot. Keep `mmap`; keep `rebuild` tested as the fallback. |
