# Contributing to Mammoth

Mammoth is a pre-release scaffold. The dashboard and teaching examples run;
the Rust filesystem and gateway still need implementation. Start with the
[current status and first-hour guide](docs/guide/START-HERE.md).

Choose your path:

- **Four-person core team:** [roles, handoffs and review rotation](docs/guide/TEAM-PLAN.md).
- **Outside contributors:** [fork, branch, test and pull-request instructions](docs/guide/EXTERNAL-CONTRIBUTORS.md).
- **New to Rust or the repository:** [code structure](docs/guide/CODE-MAP.md),
  [Rust basics](docs/guide/01-rust-you-need.md), [small runnable examples](examples/parts/).
- **Frontend contributors:** [dashboard walkthrough](docs/guide/09-web-ui.md),
  [API contract](docs/guide/API-CONTRACT.md), [UI README](ui/README.md).

## Setup and working directories

Use current stable Rust (minimum **1.85**) and Node **22.x** (`.nvmrc`).

```bash
git clone https://github.com/ProjectOrcha/Mammoth.git
cd Mammoth
cargo build --workspace --locked
cargo run -p mammoth-cli -- --help
```

Rust commands run at the root. Run `npm ci` in `ui/` or `web/` before working on
that app. Each has its own `package.json` and lockfile. A successful build does
not imply that the commands listed in CLI help are implemented.

## Checks before a PR

| Changed area | Commands | Directory |
| --- | --- | --- |
| Rust | `cargo fmt --all --check` | Root |
| Rust | `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` | Root |
| Rust | `cargo test --workspace --locked` | Root |
| CLI arguments/help | `cargo xtask docs` and include the reference diff | Root |
| Dashboard | `npm run check`, `npm test`, `npm run build` | `ui/` |
| Public docs | `npm run build` | `web/` |
| Learning guides | Run changed examples and verify local links | As indicated by the guide |

Run `cargo fmt --all` to apply formatting before the check. CI also runs
nextest, dependency/license checks (`cargo-deny`), Rust 1.85, doc tests and
platform builds. These extra tools are not prerequisites for a first docs or UI
change. Install only the tool you need; release tools are not onboarding steps.

`cargo xtask` is configured in `.cargo/config.toml`. Its tasks are:

| Task | Result |
| --- | --- |
| `cargo xtask build-ui` | Installs the UI lockfile and builds `ui/build/`; does not embed it in the unfinished gateway |
| `cargo xtask docs` | Generates the committed CLI reference from the real clap tree |
| `cargo xtask assets` | Copies the canonical logo to `ui/static/` and `web/public/` |
| `cargo xtask dist` | Calls `cargo dist build`; requires cargo-dist and release readiness |

## Conventions

Keep PRs focused. Use commit messages such as `fix(ui): encode file links` or
`docs(guide): explain borrowed strings`. Include the problem, resulting behavior,
checks run and any limitations. [Chapter 3](docs/guide/03-team-workflow.md) shows
the exact Git loop and a conflict-resolution example.

Preserve JSON field names unless the team agrees on a contract change. Add
stable error codes and useful hints for new user-facing Rust errors. Keep
`unsafe` out of crates that forbid it. Record changes to shared architecture or
storage formats in `docs/adr/` before dependent implementations grow.

Use Mermaid fences for diagrams. Use plain code blocks for directory trees and
terminal output. Mark teaching snippets as fragments when they are not complete
programs, and distinguish planned capabilities from runnable commands.

The distributed simulation, fault-injection and compatibility harnesses in
`tests/` are planned. Add deterministic failure coverage when implementing those
milestones; the README descriptions are not evidence that the harnesses exist.

Contributions are dual-licensed under Apache-2.0 and MIT, matching the project.
