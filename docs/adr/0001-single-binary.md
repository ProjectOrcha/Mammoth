# ADR 0001 — One binary, one config file

- **Status:** accepted
- **Date:** 2026-08-25

## Context

Hadoop ships `hadoop`, `hdfs`, `yarn` and `mapred` as separate scripts, and
configuration across `core-site.xml`, `hdfs-site.xml`, `yarn-site.xml` and
`mapred-site.xml` with more than a thousand tunable properties. High
availability requires ZooKeeper, JournalNodes and ZKFC — three additional
distributed systems to fail over one process.

The cost is not mainly runtime. It is that a newcomer cannot get to a working
cluster in an afternoon, and an operator cannot answer "what is this setting
right now, and who set it" without reading four files and a shell script.

## Decision

Ship **one binary** with a `--role` flag:

```
mammoth serve --role master|worker|gateway|all
```

and **one config file**, `mammoth.toml`, with every key overridable by
environment variable (`MAMMOTH_STORAGE__REPLICATION=2`).

Consensus is built in via `openraft`. No external coordination service.

## Consequences

**Good**

- `mammoth quickstart` can bring up a whole cluster in under ten seconds, which
  is the entire adoption funnel.
- One artifact to build, sign, ship and version. `cargo-dist` gives us shell,
  PowerShell, Homebrew and MSI installers from one config.
- `--role all` means the single-machine development story and the production
  story are the same code path, so the dev path cannot silently rot.
- No skew between a master's config and a worker's.

**Bad**

- The binary is larger than a role-specific one would be, and a worker links
  code it never runs. Acceptable: tens of MB against terabytes of storage.
- Rolling upgrades update every role at once, so on-disk and wire formats must
  stay compatible across one minor version. `layout_version` in `VERSION` plus
  hardlinked old layouts until `mammoth admin upgrade finalize` handles this.
- Embedding Raft means we own consensus bugs rather than delegating them to
  ZooKeeper. Mitigated by deterministic simulation testing from M5 — every such
  bug must be reproducible from a seed.

## Alternatives considered

- **Separate binaries per role.** Smaller artifacts, but four things to version
  and a worse first-run experience. The thing we are optimizing is the first ten
  minutes.
- **External etcd or ZooKeeper for consensus.** Less code to own, but it
  reintroduces exactly the operational burden this project exists to remove.
