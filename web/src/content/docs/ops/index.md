---
title: Operations
description: Running a Mammoth cluster.
---

:::note[Design reference]
This page describes the full project design, including future distributed
features and command options. For the working local application on `AI_coded`,
follow the [quickstart](/intro/quickstart/) and [generated CLI reference](/cli/reference/).
See the [implementation status](https://github.com/ProjectOrcha/Mammoth/blob/AI_coded/docs/IMPLEMENTATION-STATUS.md)
for supported behavior and remaining milestones.
:::


- [Configuration](/ops/configuration/) — the single `mammoth.toml`
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
