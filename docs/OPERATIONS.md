# Operating the local service on AI_coded

> **Storage archive.** Mammoth now focuses on [durable coding-agent memory](AGENT-MEMORY.md).
> The material below describes the earlier storage track and its historical checks.
> Current memory builds require Rust 1.88+.

This runbook covers the durable single-host application. It is suitable for
controlled local evaluation and development. It is not a production distributed
storage service: authentication, authorization, TLS, quotas, real worker
processes and HA remain release blockers. See [release readiness](RELEASE-READINESS.md).

## Build and start

Use a supported Rust toolchain (minimum 1.85) and Node 22.12+ within 22.x.
From the repository root:

```bash
npm --prefix ui ci
npm --prefix ui run check
npm --prefix ui test
npm --prefix ui run build
cargo build --release --locked -p mammoth-cli
./target/release/mammoth config template > /path/to/mammoth.toml
./target/release/mammoth --config /path/to/mammoth.toml config validate
./target/release/mammoth --config /path/to/mammoth.toml --local-root /path/to/store serve --role all
```

Choose real paths before running. `serve` creates an empty store if needed;
`quickstart` additionally creates sample files unless `--no-sample` is supplied.
The dashboard is embedded at build time: rebuild the binary after changing UI
assets. Keep store data outside source/distribution directories. SQLite WAL
requires a local filesystem; do not use a network filesystem for this layout.

Defaults bind to `127.0.0.1:8080` and `127.0.0.1:9000`. Explicit remote listeners
require `--allow-remote`, which provides no authentication. Browser access uses
localhost or literal IP addresses. Foreign-origin mutations and browser DNS
rebinding hostnames are rejected; arbitrary native clients with network access
still have full access. Ownership and mode fields do not enforce ACLs.

The service refuses configurations claiming TLS/authentication. Use `off`/`none`
only in the controlled local scope above. A proxy alone does not qualify the
service for multi-tenant or public production use.

## Observe and stop

```bash
./target/release/mammoth --local-root /path/to/store status --json
curl --fail http://127.0.0.1:8080/healthz
curl --fail http://127.0.0.1:8080/readyz
./target/release/mammoth --local-root /path/to/store doctor --json
./target/release/mammoth --local-root /path/to/store stop --timeout 60
```

`healthz` checks HTTP liveness. `readyz` checks that the namespace root can be
read; it does not scan replicas or prove write capacity. `doctor`/health inspection
verifies actual replicas and can be expensive for large stores. Worker capacities
are reference values for six directories sharing one disk, not available space.
Monitor real free space externally. The service does not enforce quotas.

Stop requests target a locked service instance, not an arbitrary PID. Ctrl-C and
SIGTERM drain HTTP requests, SSE and accepted local jobs/benchmarks. A slow client
or long job can delay draining; `stop --timeout` reports that it has not stopped
rather than killing it. Do not start a backup until it actually reports stopped.

Local jobs support `--overwrite` explicitly; without it, a destination created
during computation is preserved. Dashboard job history is bounded and lasts
only for the service session. Stored output files survive restarts.

## Backup, restore and upgrade

1. Stop the service. Stop all CLI writers and clients using the same store.
2. Copy the **entire store** into a new backup directory while it is idle. Include
   the namespace marker, SQLite database and any WAL/SHM files, worker blocks and
   metadata. Do not take a database-file-only copy.
3. Record the binary version, configuration, backup timestamp and known file
   hashes. Keep backups outside the original root and off its physical disk
   when protection against disk loss is required.
4. Restore to a new directory, leaving the original intact. Use the same binary
   first, run `doctor`, list expected files and compare downloaded hashes.
5. For an upgrade, test the newer binary against a separate restored copy before
   switching the running service. Keep the old complete backup and old binary.

SQLite WAL can contain committed transactions absent from the main database;
see [SQLite's WAL documentation](https://sqlite.org/wal.html). Opening a restored
copy can update/checkpoint it. Keep one untouched backup. Do not copy a live store
unless a consistent snapshot procedure has been implemented and tested.

Version-1 JSON stores migrate automatically to namespace version 2 when opened.
The retained legacy JSON is an archive, not a rollback mechanism for subsequent
writes. Never run an older engine on a migrated store; see
[format and upgrade details](STORAGE-ENGINE.md).

## Recovery and resource limits

Normal errors and cancelled jobs remove their temporary compute runs. Forced
termination can leave `mammoth-compute-*` scratch directories or benchmark
`scratch-*` stores. Remove only identified scratch directories belonging to a
stopped process. Never delete live worker block directories manually.

Interrupted writes may leave unreferenced replicas. Once all reads/writes and
jobs are idle, inspect health and use `mammoth admin gc` against the intended local
root. GC may reject maintenance while another operation retains a read snapshot.
Repair restores from verified replicas; it cannot recreate data when every copy
is lost. Preserve a copy before investigating corruption.

Plan memory for the verified read cache, up to eight upload buffers, concurrent
jobs, spill merges and runtime overhead. Targets are not RSS caps. Plan disk for
replicas, staging and spill output in addition to logical input. Failed writes
must be treated as failures even if some unreferenced bytes reached disk.

## Packaging and deployment boundaries

`cargo xtask dist` builds the embedded dashboard and native archive. The archive
contains the binary, supported config, licenses, README and operational/readiness
notes. CI tests platforms separately and publishes only draft releases on tags.
A locally successful Mac build does not verify Linux/Windows or container runtime.

The [Compose layout](../deploy/compose/docker-compose.yml) is one container with
a persistent volume and host ports restricted to loopback. Back up that volume
while stopped. The [systemd example](../deploy/systemd/mammoth.service) needs an
existing `mammoth` user and writable data directory. Kubernetes/Helm installation
is unavailable; the misleading placeholder chart has been removed.
