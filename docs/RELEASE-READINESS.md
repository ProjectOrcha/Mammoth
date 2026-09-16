# Release readiness

> **Storage archive.** Mammoth now focuses on [durable coding-agent memory](AGENT-MEMORY.md).
> The material below describes the earlier storage track and its historical checks.
> Current memory builds require Rust 1.88+.

`AI_coded` contains a hardened **local preview**, not a qualified production
cluster. The gates below define the difference. Passing build checks does not
waive missing system capabilities. Benchmark snapshots identify their measured
source hashes and do not certify subsequent code changes.

| Area | Current evidence / gate |
| --- | --- |
| Local correctness | Verified replicas, SQLite FULL/WAL transactions, captured read generations, atomic create-if-absent, cancellation and restart tests |
| Client safety | Explicit job overwrite, loopback defaults, rejection of unsupported security settings, browser-origin/hostname guards, readiness endpoint |
| Repeatable build | Locked Rust/npm dependencies; formatting, Clippy, minimum Rust, tests, CLI-doc drift and both website bases in CI |
| Benchmark integrity | Raw iterations, correctness and replica checks, generated snapshot drift check; Mac local comparisons only |
| Operational practice | Offline full-store backup and restore procedure, instance-scoped shutdown, explicit resource and recovery limits |
| Release packaging | Embedded UI, supported config, licenses and runbook; platform archives with build/source records and SHA256 checksums; draft publication |
| Platform qualification | Local Mac checks are recorded in implementation status; Linux/Windows CI, container runtime and filesystem-specific power-loss tests must pass separately |
| Security **blocker** | No authentication, authorization, TLS, tenant isolation or quotas. Browser protections do not replace these |
| Distribution **blocker** | No independently deployed workers/masters, network fault protocol, Raft HA or distributed shuffle |
| Scale **blocker** | No large-production dataset qualification, network saturation measurements or bounded-cost cluster health scan |

## Before a local preview release

Use a clean checkout of the intended `AI_coded` commit. Keep real stores out of
the build tree. Run CI and review its results, not only the workflow files:

```bash
cargo fmt --all --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
cargo test -p mammoth-parts --examples --locked
cargo +1.85.0 check --workspace --all-features --locked
cargo deny check
npm --prefix ui ci
npm --prefix ui run check
npm --prefix ui test
npm --prefix ui run build
npm --prefix web ci
npm --prefix web run build
cargo xtask docs
python3 -m unittest discover -s bench-suite/compare -p 'test_*.py'
python3 bench-suite/compare/publish.py
```

Review generated-file diffs; CI requires them to match committed evidence. Check
npm and Rust dependency advisories. The existing Ratatui `paste` exception is a
maintenance advisory with an explicit recorded reason, not a blanket exception.
Run real-client S3 compatibility checks and the deterministic teaching-model
simulation. Those are separate from storage crash/failure qualification.

Build `cargo xtask dist`, inspect archive contents, verify checksums, start the
packaged binary in a disposable store, check readiness and expected file hashes,
and stop it cleanly. Rehearse restore from a complete offline copy. Do not tag or
publish a production release until the blocker rows above have implementation,
independent review and multi-machine failure evidence.

## Branch ownership

`main` remains the manual build track. Its guides and isolated examples teach the
same contracts; do not merge the full reference implementation merely to mark
milestones complete. See [branch roles](guide/BRANCHES.md) and the
[operator runbook](OPERATIONS.md).
