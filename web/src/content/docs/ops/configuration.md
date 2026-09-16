---
title: Configuration
description: Supported local settings, validation and restart behavior on AI_coded.
---

> **Storage archive.** Mammoth now focuses on [durable context memory for coding agents](/memory/). This page describes the earlier storage project.


The `AI_coded` service uses the following settings today. Generate this starter
with `mammoth config template`, or download a validated draft from **Configure**.
Validation does not change a running service.

```toml
[storage]
block_size = "128MiB"
replication = 3
inline_threshold = "1MiB"

[read]
cache_size = "256MiB"

[compute]
memory_budget = "128MiB"
spill_directory = ""

[gateway]
ui_listen = "127.0.0.1:8080"
s3_listen = "127.0.0.1:9000"

[security]
tls = "off"
auth = "none"
```

## Apply a configuration

```bash
mammoth --config /path/to/mammoth.toml config validate
mammoth --local-root /path/to/store stop
mammoth --config /path/to/mammoth.toml --local-root /path/to/store serve --role all
```

Keep the same local root to preserve files. Block size, inline threshold and
replication affect new writes; existing files retain their layout. Existing
replication can be changed explicitly with `setrep`. Filesystem ownership/mode
fields are descriptive and do not enforce access control.

Configuration loads defaults, `/etc/mammoth/mammoth.toml`,
`~/.mammoth/mammoth.toml`, the explicit `--config` / `MAMMOTH_CONFIG` file, then
`MAMMOTH_` environment overrides using `__` for nesting. For example,
`MAMMOTH_STORAGE__REPLICATION=2`. `config show` prints resolved values; it does not
identify the source layer. Some resolved fields describe future distributed
features. Only the supported settings above are exported by the local template.

## Limits and memory

| Setting | Bound / behavior |
| --- | --- |
| Block size | 1 byte–256 MiB |
| Inline threshold | 0–16 MiB; 0 keeps non-empty files in blocks |
| Replication | 1–6 worker directories on this host |
| Read cache | 0–4 GiB; 0 disables application caching |
| Compute memory | 64 KiB–4 GiB target per job |
| Spill directory | Empty uses OS temp; otherwise choose an existing directory with free space |

The read cache contains verified immutable byte ranges and resets on restart.
Jobs use parallel batches and spill larger inputs into checksummed scratch runs.
A job memory target excludes the read cache, storage buffers, worker stacks and
allocator overhead; it is not an RSS ceiling. The dashboard permits two
concurrent jobs, so budget for both. Uploads have eight slots per local backend.
There are no disk quotas or admission checks based on remaining free space yet.

Scratch files are removed on normal completion, ordinary errors and cancellation.
After a forced exit, a `mammoth-compute-*` scratch directory can remain. Confirm
that its process is stopped before removing it. Never delete storage worker
blocks as though they were compute scratch files.

## Listener safety

The defaults bind to loopback. An explicitly configured non-loopback address
requires `--allow-remote`; it is never silently rewritten. This flag does not
provide authentication. Use this service only on a trusted local machine or an
isolated development network. TLS and authentication settings other than
`off` / `none` are rejected before startup, rather than silently ignored.

Browser access uses `localhost` or literal IP addresses. Cross-origin mutations,
opaque origins and browser requests using arbitrary DNS hostnames are rejected.
Framing and MIME sniffing are disabled. These protections do not authenticate
native clients. The development UI proxy preserves the original Host/Origin
pair so its same-origin requests continue to work.

## Benchmarks and planned features

Benchmark settings are independent of service defaults. The benchmark form
supports stricter bounds described in [Benchmarks](/ops/benchmarks/).

Separate workers/masters, Raft, distributed leases, automatic repair,
erasure coding, quotas, TLS and authentication remain future work. Their design
is explained in [the architecture](/concepts/architecture/) and the manual build
guide. They are not active capabilities of the configuration above.
