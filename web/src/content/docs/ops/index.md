---
title: Operations
description: Running a Mammoth cluster.
---

- [Configuration](/mammoth/ops/configuration/) — the single `mammoth.toml`
- `mammoth doctor` — config, ports, disks, clock skew, ulimits
- `mammoth admin fsck` — block-level integrity
- `mammoth admin balancer` — even out per-node usage
- `mammoth admin safemode` — why the cluster is read-only after a restart
- `mammoth top` — live TUI dashboard, works over SSH

## Deploying

`deploy/` in the repository carries a Dockerfile, a Compose file
(1 master + 3 workers + gateway), a systemd unit and a Helm chart.

```bash
mammoth systemd install --role worker
```

writes the unit, creates the user, sets `LimitNOFILE=1048576`, and runs
`daemon-reload`. Do not hand-write units.
