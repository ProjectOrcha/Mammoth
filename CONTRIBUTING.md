# Contributing to Mammoth

## Getting set up

```bash
git clone https://github.com/Sakib-Dalal/mammoth
cd mammoth
cargo build
cargo test
```

Optional, but what CI runs:

```bash
cargo install cargo-nextest cargo-deny cargo-dist cross
```

Node 20+ is needed only if you are touching `ui/` or `web/`.

## The layout

| Path | What lives there |
| --- | --- |
| `crates/` | the Rust workspace — see [docs/ROADMAP.md](docs/ROADMAP.md) for per-crate status |
| `ui/` | Svelte 5 admin GUI, embedded into the binary with `rust-embed` |
| `web/` | Astro Starlight public site and docs → GitHub Pages |
| `deploy/` | Dockerfile, Compose, systemd unit, Helm chart |
| `examples/` | numbered, runnable walkthroughs |
| `tests/` | `e2e/`, `sim/`, `compat/` — see [tests/README.md](tests/README.md) |
| `xtask/` | `cargo xtask build-ui \| docs \| assets \| dist` |
| `assets/logo/` | canonical logo files — see [assets/logo/README.md](assets/logo/README.md) |

## Before you open a PR

```bash
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo deny check
```

If you changed the CLI surface, regenerate the docs — CI fails if the committed
reference differs from the `clap` tree:

```bash
cargo xtask docs
```

## Conventions

- **Commits** follow [Conventional Commits](https://www.conventionalcommits.org/):
  `feat(cli): add viz skew --by-partition`.
- **Errors teach.** Every user-facing error gets a stable `E….` code, a
  one-line cause, and concrete next commands. Add the variant to
  `mammoth-core/src/error.rs` and a page under `web/src/content/docs/errors/`.
  Never print a stack trace.
- **Everything has `--json`.** JSON field names are a public API; changing one
  is a breaking change.
- **Architectural decisions get an ADR** in `docs/adr/`. Write it *before* the
  code — justifying a design in prose surfaces half the problems for free.
- **`unsafe` is denied** workspace-wide. If you genuinely need it, that is an
  ADR conversation.

## Distributed bugs

Anything involving more than one node needs a deterministic simulation test, not
just a unit test. If you found the bug from a nightly seed, put the seed in the
test:

```bash
MAMMOTH_SIM_SEED=8412337 cargo nextest run --test sim
```

## Licence

Contributions are dual-licensed under Apache-2.0 and MIT, matching the project.
