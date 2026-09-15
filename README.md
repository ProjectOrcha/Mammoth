<p align="center"><img src="assets/logo/mammoth-logo.svg" alt="Mammoth" width="360"></p>

# Mammoth

A Rust storage engine with a persistent local filesystem, replica visualization,
a command-line interface, a live web dashboard and a development S3 endpoint.

**The guided local application now runs end to end on `AI_coded`.** Six worker
directories simulate three racks on one machine. Files are real, checksummed and
persistent. The distributed M5–M8 roadmap is still incomplete: this is not a
production multi-machine cluster or a Raft implementation.

Read [implementation status](docs/IMPLEMENTATION-STATUS.md) for the exact supported
surface and remaining work, or [the build guide](docs/guide/README.md) for the design.

## Build and start

Requires Rust 1.85+ and Node 22.x.

```bash
npm --prefix ui ci
npm --prefix ui run build
cargo build --locked -p mammoth-cli
./target/debug/mammoth --local-root .mammoth quickstart
```

Open [the dashboard](http://127.0.0.1:8080). The S3 endpoint is
`http://127.0.0.1:9000`. Quickstart runs in the foreground; Ctrl-C stops it.
From another terminal, inspect or stop that service with:

```bash
./target/debug/mammoth --local-root .mammoth status
./target/debug/mammoth --local-root .mammoth stop
```

`shutdown` is an alias for `stop`. Both listeners close after active requests and
dashboard jobs finish. `stop --timeout 60` allows a longer wait. Files stay on disk.
The selected local root identifies the service; lifecycle commands do not use `--masters`.
The logo appears with `mammoth`, `mammoth --help`, `mammoth logo`, `quickstart`, and
`serve`. Explicit structured service output (`--json`, YAML or CSV) omits the logo.

Restart with the same local root to keep your files. Omit `--local-root` to use
`~/.mammoth/local`; `MAMMOTH_LOCAL_ROOT` also selects the store.

The UI must be built **before** the binary to embed it. A Rust-only build still
serves the API and a landing page with dashboard build instructions.

## Use your filesystem

In another terminal, use the same root:

```bash
export MAMMOTH_LOCAL_ROOT="$PWD/.mammoth"
./target/debug/mammoth mkdir /data
./target/debug/mammoth put README.md /data/readme.md
./target/debug/mammoth ls /data
./target/debug/mammoth cat /data/readme.md
./target/debug/mammoth get /data/readme.md ./downloaded-readme.md
./target/debug/mammoth checksum /data/readme.md
./target/debug/mammoth viz blocks /data/readme.md
./target/debug/mammoth viz cluster --output table
./target/debug/mammoth top
```

Small files are inlined in metadata. To inspect multiple blocks:

```bash
./target/debug/mammoth put ./large-file.bin /data/large.bin --block-size 1MiB
./target/debug/mammoth viz blocks /data/large.bin --output table
```

Also implemented: `head`, `tail`, `cp -r`, `mv`, `rm -r`, `find`, `du`, `df`,
`chmod`, `chown`, `setrep`, `doctor`, `admin repair`, `admin gc`, topology,
skew and treemap views, shell completions and basic `hdfs dfs` translation.
POSIX ownership is descriptive in local mode; it is not an authorization system.

`--json` emits structured output. `--output table|json|yaml|csv` works for result
records; `cat`, `head`, `tail` and downloaded files preserve raw content.
Errors have stable codes and nonzero exits. Existing local downloads require
`get --force` before replacement.

Human output includes colored file listings, capacity and size bars, rack and
namespace trees, block placement matrices and replica-health charts. Use
`--color auto|always|never`; auto respects `NO_COLOR` and terminal detection.
Use `--output table` to keep charts when piping output. `top` and
`viz health --live` refresh in place, support arrow-key scrolling and exit with q.
`mammoth --help` includes nested commands; `mammoth commands` prints the full
catalog, matching the generated website reference.

## Remote CLI access

The HTTP client uses the same Backend trait:

```bash
./target/debug/mammoth --masters http://127.0.0.1:8080 ls /
./target/debug/mammoth --masters http://127.0.0.1:8080 put README.md /sample/remote.md
```

`--masters` currently selects an **HTTP gateway**, not the planned gRPC master.
Only one endpoint is accepted; there is no leader discovery or automatic failover.

## S3 and DuckDB

Use path-style requests against port 9000. The local endpoint does not authenticate
requests. It supports bucket create/list/head/delete, object put/get/head/delete,
copy, ListObjects v1/v2 with prefixes and pagination, single byte ranges, MD5
ETags and upload checksum verification. Multipart uploads, IAM, versions, ACLs,
and server-side encryption are not implemented; unsupported operations fail explicitly.

```bash
aws --endpoint-url http://127.0.0.1:9000 --no-sign-request s3api create-bucket --bucket warehouse
aws --endpoint-url http://127.0.0.1:9000 --no-sign-request s3api put-object --bucket warehouse --key sales.parquet --body ./sales.parquet
```

```sql
INSTALL httpfs;
LOAD httpfs;
CREATE SECRET mammoth (
  TYPE S3,
  ENDPOINT '127.0.0.1:9000',
  URL_STYLE 'path',
  USE_SSL false
);
SELECT count(*) FROM read_parquet('s3://warehouse/*.parquet');
```

The compatibility test runs real boto3 and DuckDB clients, using an isolated
store and checking exact results:

```bash
uv run --no-project --with boto3 --with duckdb python tests/compat/s3_clients.py
```

## Local processing and transfers

```bash
./target/debug/mammoth job wordcount /sample/words.txt /sample/counts.txt
./target/debug/mammoth job sort /sample/words.txt /sample/sorted.txt
./target/debug/mammoth migrate import ./dataset /dataset
./target/debug/mammoth migrate export /dataset ./exported-dataset
./target/debug/mammoth bench --size 8MiB
```

Text jobs run locally with a 64 MiB input limit. Tree transfers commit one file at
a time and refuse symlinks. These do not implement distributed shuffle or native
HDFS migration.

## Docker development environment

```bash
docker compose -f deploy/compose/docker-compose.yml up --build
```

This builds one persistent local-service container and publishes both ports only
on the host's loopback interface. The container explicitly enables its internal
network listeners with `--allow-remote`. Do not expose this unauthenticated
service to an untrusted network. Native CLI listeners default to loopback.

## Verification

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --locked
cargo +1.85.0 check --workspace --all-features --locked
npm --prefix ui run check
npm --prefix ui test
npm --prefix ui run build
npm --prefix web ci
npm --prefix web run build
cargo xtask docs
```

Tests cover restart persistence, interrupted writes, concurrent clients, atomic
read snapshots, partial blocks, replica corruption/fallback/repair, path validation,
CLI processes, dashboard HTTP contracts, S3 requests and streamed remote access.
The separate [GFS teaching simulation](docs/guide/13-gfs-reliability.md) retains
its deterministic failure and write-ordering tests.

CI runs on pushes to `main` and `AI_coded`, and pull requests. The release workflow
builds Linux, macOS ARM and Windows archives with the dashboard embedded; version
tags create a **draft** GitHub release. This checkout does not publish a release.

## Architecture and roadmap

- [Roadmap](docs/ROADMAP.md) — M1–M8, including the remaining production work
- [Durable local storage decision](docs/adr/0004-durable-local-service.md)
- [Backend trait](docs/adr/0002-backend-trait.md)
- [Guide and team workflow](docs/guide/README.md)
- [Dashboard API contract](docs/guide/API-CONTRACT.md)
- [Planned distributed fast paths](docs/guide/12-the-fast-paths.md)
- [GFS reliability coverage](docs/guide/GFS-COVERAGE.md)

## License

Apache-2.0 OR MIT. See [LICENSE-APACHE](LICENSE-APACHE) and [LICENSE-MIT](LICENSE-MIT).

## Package the local build

Run `cargo xtask dist` to build the dashboard, compile the release binary and
create a native archive under `target/dist/`. The archive includes licenses and
implementation status. This command does not publish a release.
