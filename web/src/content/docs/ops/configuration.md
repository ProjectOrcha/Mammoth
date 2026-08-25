---
title: Configuration
description: One file, sane defaults. No XML, no six files.
---

`/etc/mammoth/mammoth.toml` — the whole thing. No XML, no six files.

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

[storage.scrub]
enabled       = true
bytes_per_sec = "50MiB"

[write]
packet_size = "64KiB"
ack_policy  = "quorum"           # all | quorum

[read]
short_circuit = true
hedged_after  = "50ms"
cache_size    = "4GiB"

[master]
listen             = "0.0.0.0:7000"
data_dir           = "/var/lib/mammoth/meta"
heartbeat_ms       = 3000
dead_after         = "10m"
safemode_threshold = 0.999

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
