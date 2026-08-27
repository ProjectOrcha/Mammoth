# Chapter 2 — Your first change

**What you'll build:** `mammoth version`, a real working command, from scratch.

**Time:** 20 minutes.

---

## Before you start

```markdown
- [ ] Chapter 0 is finished — `cargo build --workspace` succeeds
- [ ] Chapter 1 is read — you know what a trait is and what `?` does
```

### Run the examples first

Two small programs that do this chapter's two ideas in isolation — clap, and the
`Render` trait — with nothing else around them:

```bash
cargo run -q -p mammoth-parts --example 06-cli-hello -- --help
```

```bash
cargo run -q -p mammoth-parts --example 08-table-or-json
```

Run that second one again through `| cat` and watch it switch to JSON. That is
the behaviour you are about to build.

### Files you will touch

Four files, all in one crate. Two exist and you edit them; one exists and you
add a line; one you create.

```
crates/mammoth-cli/
└── src/
    ├── cli.rs              EDIT    add the Version variant to the command tree
    ├── main.rs             EDIT    dispatch to it
    ├── output.rs           read    the Render trait — do not change it
    └── commands/
        ├── mod.rs          EDIT    one line: pub mod version;
        └── version.rs      NEW     the command itself
```

Open all four in your editor before you start. Being able to see `output.rs`
while writing `version.rs` is most of what makes this chapter click.

---

## Why this one

It is the smallest change that touches **every layer you will use for the rest
of the project**:

1. adding a command to the command tree (`cli.rs`)
2. dispatching to it (`main.rs`)
3. writing the command itself (`commands/version.rs`)
4. making its output work as both a table *and* JSON (`output.rs`)

Once you have done this, chapters 7 and 8 are the same four steps with more code
in step 3.

## Step 1 · Add the command to the tree

Open [`crates/mammoth-cli/src/cli.rs`](../../crates/mammoth-cli/src/cli.rs) and
find the `Command` enum. Look for `Doctor`, and add a `Version` variant just
above it:

```rust
    /// Print version and build information.
    Version,
    /// Diagnose config, ports, disks, clock and ulimits.
    Doctor {
```

That is it. The doc comment (`///`) becomes the help text automatically — that
is `clap` reading your code.

Check it:

```bash
cargo build -p mammoth-cli && ./target/debug/mammoth --help | grep version
```

```
  version      Print version and build information
```

**The command exists in the help output before you have written any of it.**
The build even still compiles, because `main.rs` has a catch-all. Try running it
and you get the `not implemented` panic. Let us fix that.

## Step 2 · Create the command module

Open [`crates/mammoth-cli/src/commands/mod.rs`](../../crates/mammoth-cli/src/commands/mod.rs).
It is a list of commented-out modules. Add a real one at the top:

```rust
pub mod version;
```

Now create the file `crates/mammoth-cli/src/commands/version.rs`:

```rust
//! `mammoth version` — the smallest complete command.

use crate::cli::OutputFormat;
use crate::output::{emit, Render};

/// What `mammoth version` reports.
pub struct VersionInfo {
    name: &'static str,
    version: &'static str,
    rust_version: &'static str,
}

impl VersionInfo {
    fn gather() -> Self {
        Self {
            name: "mammoth",
            version: env!("CARGO_PKG_VERSION"),
            rust_version: env!("CARGO_PKG_RUST_VERSION"),
        }
    }
}

impl Render for VersionInfo {
    fn to_table(&self) -> comfy_table::Table {
        let mut t = comfy_table::Table::new();
        t.load_preset(comfy_table::presets::NOTHING);
        t.add_row(vec!["name", self.name]);
        t.add_row(vec!["version", self.version]);
        t.add_row(vec!["rust", self.rust_version]);
        t
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "name": self.name,
            "version": self.version,
            "rust_version": self.rust_version,
        })
    }
}

/// Run the command.
pub fn run(format: OutputFormat) -> mammoth_core::Result<()> {
    emit(&VersionInfo::gather(), format);
    Ok(())
}
```

### What is going on here

- **`env!("CARGO_PKG_VERSION")`** reads `version` out of `Cargo.toml` *at compile
  time* and pastes it in as a string. So the version can never drift from the
  manifest.
- **`impl Render for VersionInfo`** is the trait idea from chapter 1. You are
  promising: "this type knows how to become a table, and how to become JSON."
- **`emit`** picks between them. You never write `if json { ... } else { ... }`
  in a command — that decision lives in exactly one place, `output.rs`.

## Step 3 · Dispatch to it

Open [`crates/mammoth-cli/src/main.rs`](../../crates/mammoth-cli/src/main.rs).
Two edits.

First, tell Rust the `commands` module exists — find the `mod` lines near the
top and make sure `mod commands;` is there (it already is in the scaffold).

Then replace the whole `run` function:

```rust
fn run(cli: cli::Cli) -> mammoth_core::Result<()> {
    match cli.command {
        cli::Command::Version => commands::version::run(cli.output),
        _ => unimplemented!("command dispatch — see docs/ROADMAP.md, milestone M1"),
    }
}
```

The `_ =>` arm is a catch-all for every command you have not built yet. As you
build them in later chapters, you add an arm and the list of unimplemented ones
shrinks.

## Check it works

```bash
cargo build -p mammoth-cli
```

```bash
./target/debug/mammoth version --output table
```

```
 name     mammoth 
 version  0.1.0   
 rust     1.82    
```

```bash
./target/debug/mammoth version --output json
```

```json
{
  "name": "mammoth",
  "rust_version": "1.82",
  "version": "0.1.0"
}
```

Now the part worth pausing on. Run it with **no** `--output` flag, first plain
and then piped into another command:

```bash
./target/debug/mammoth version
```

```
 name     mammoth 
 version  0.1.0   
 rust     1.82    
```

```bash
./target/debug/mammoth version | cat
```

```json
{
  "name": "mammoth",
  "rust_version": "1.82",
  "version": "0.1.0"
}
```

**Same command, different output.** It printed a table to your terminal and JSON
into the pipe, because `OutputFormat::Auto` checks `stdout().is_terminal()`.
That is design principle 2 from the CLI spec, and you just got it for free by
implementing `Render` instead of calling `println!` directly.

This is why later chapters never format output by hand.

## Step 4 · Commit it

First, the checks. Run all of these:

```bash
cargo fmt --all
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo test --workspace
```

Then commit. Mammoth uses
[Conventional Commits](https://www.conventionalcommits.org/) — a type, a scope
in parentheses, and a short imperative description:

```bash
git checkout -b feat/version-command
```

```bash
git add -A && git commit -m "feat(cli): add mammoth version command"
```

```bash
git push -u origin feat/version-command
```

Then open a pull request on GitHub. Chapter 3 covers what happens next.

## Done when

```markdown
- [ ] `mammoth version --output table` prints the three-row table
- [ ] `mammoth version --output json` prints JSON
- [ ] `mammoth version` alone prints a table
- [ ] `mammoth version | cat` prints JSON — **and I understand why**
- [ ] `mmcheck` (or fmt + clippy + test) passes
- [ ] Committed on a branch as `feat(cli): add mammoth version command`
- [ ] Pushed, PR opened
```

That fourth box is the one that matters. The same binary printed a table to your
terminal and JSON into a pipe, and no code in `version.rs` decided that — the
`Render` trait did. **Every command in chapters 7 and 8 gets that behaviour for
free by doing exactly what you just did.**

If you cannot yet explain why it happened, read `output.rs` once more before
moving on. It is short.

## If it went wrong

**`` error[E0433]: failed to resolve: use of undeclared crate or module `version` ``**
— you forgot `pub mod version;` in `commands/mod.rs`, or you spelled the file
name differently from the module name. They must match exactly:
`pub mod version;` needs `commands/version.rs`.

**`` error[E0004]: non-exhaustive patterns ``** — you removed the `_ =>` arm.
Put it back; you have not implemented the other 35 commands yet.

**`` warning: unused variable: `cli` ``** — you renamed `_cli` to `cli` in the
signature but did not actually use it. Make sure your `run` body says
`match cli.command`, not `match _cli.command`.

**`cargo clippy` fails on code you did not write** — the scaffold has
`#![allow(dead_code)]` at the top of `main.rs` precisely so this stays green.
Do not remove it until every command is implemented.

**The table has no borders and you expected some** — that is `NOTHING` preset
doing its job. Try `comfy_table::presets::UTF8_FULL` to see the difference, then
put it back; Mammoth's tables are deliberately quiet.

---

**Next:** [Chapter 3 — How the team works together](03-team-workflow.md)
