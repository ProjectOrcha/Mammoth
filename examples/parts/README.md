# The parts

Sixteen small programs. Each one takes a **single idea** out of Mammoth, puts it
in one file with nothing else around it, and lets you run it.

They exist because reading chapter 6 is much easier once you have watched a
`ByteStream` hand out chunks on your own screen, and because "add colour to the
CLI" is a much smaller job when there is a working palette to copy.

Nothing here is a toy: the types come from `mammoth-core`, the patterns are the
ones the real crates use, and every file compiles under
`cargo clippy -- -D warnings`.

---

## Running them

```bash
cargo run -p mammoth-parts --example 01-ownership
```

Some take arguments. Everything after the bare `--` goes to the program rather
than to cargo — this catches everyone once:

```bash
cargo run -p mammoth-parts --example 07-cli-subcommands -- ls /data --long
```

Add `-q` to drop cargo's `Compiling…` chatter, which matters for the ones whose
output you are meant to look at:

```bash
cargo run -q -p mammoth-parts --example 13-block-matrix
```

Build all sixteen at once, to check your toolchain:

```bash
cargo build -p mammoth-parts --examples
```

---

## The list

### Rust, in the shapes Mammoth uses it

| # | Example | It answers | Read with |
| --- | --- | --- | --- |
| 01 | [`01-ownership`](examples/01-ownership.rs) | Why does the compiler keep saying "moved value"? | [ch 1 §1](../../docs/guide/01-rust-you-need.md) |
| 02 | [`02-result-and-errors`](examples/02-result-and-errors.rs) | What is `?`, and what makes an error message good? | [ch 1 §2](../../docs/guide/01-rust-you-need.md) |
| 03 | [`03-structs-enums-match`](examples/03-structs-enums-match.rs) | Structs, enums that carry data, exhaustive `match`, iterators | [ch 1 §4](../../docs/guide/01-rust-you-need.md) |
| 04 | [`04-traits-and-dyn`](examples/04-traits-and-dyn.rs) | **The `Backend` idea in one file.** One function, two implementations | [ch 4](../../docs/guide/04-the-backend-trait.md) |
| 05 | [`05-async-and-streams`](examples/05-async-and-streams.rs) | `async`, `.await`, `join!`, `#[async_trait]`, and what a `ByteStream` is | [ch 1 §6](../../docs/guide/01-rust-you-need.md), [ch 6](../../docs/guide/06-localbackend-part-2.md) |

### The command line

| # | Example | It answers | Read with |
| --- | --- | --- | --- |
| 06 | [`06-cli-hello`](examples/06-cli-hello.rs) | The smallest clap program that is worth having | [ch 2](../../docs/guide/02-first-change.md) |
| 07 | [`07-cli-subcommands`](examples/07-cli-subcommands.rs) | A real command tree: global flags, args structs, nested subcommands | [ch 7](../../docs/guide/07-wiring-the-cli.md) |
| 08 | [`08-table-or-json`](examples/08-table-or-json.rs) | How one `Render` trait gives every command four output formats | [ch 2](../../docs/guide/02-first-change.md), [ch 7](../../docs/guide/07-wiring-the-cli.md) |
| 09 | [`09-cli-errors`](examples/09-cli-errors.rs) | stderr, exit codes, and errors that suggest the next command | [ch 7](../../docs/guide/07-wiring-the-cli.md) |

### Colour and terminal graphics

| # | Example | It answers | Read with |
| --- | --- | --- | --- |
| 10 | [`10-colour-basics`](examples/10-colour-basics.rs) | What an ANSI escape is, and the three questions before you emit one | [ch 8a](../../docs/guide/08a-colour-in-the-terminal.md) |
| 11 | [`11-colour-palette`](examples/11-colour-palette.rs) | **The palette module.** Colour by meaning, shared with the TUI | [ch 8a](../../docs/guide/08a-colour-in-the-terminal.md) |
| 12 | [`12-bars-and-heatmap`](examples/12-bars-and-heatmap.rs) | Eighth-block bars, sparklines, and why they are pure functions | [ch 8](../../docs/guide/08-viz-blocks.md) |
| 13 | [`13-block-matrix`](examples/13-block-matrix.rs) | The block × node matrix, in colour, with the rack-safety warning | [ch 8](../../docs/guide/08-viz-blocks.md) |

### Progress and the live dashboard

| # | Example | It answers | Read with |
| --- | --- | --- | --- |
| 14 | [`14-progress`](examples/14-progress.rs) | Progress bars that go to stderr and vanish when piped | [ch 8a](../../docs/guide/08a-colour-in-the-terminal.md) |
| 15 | [`15-tui-hello`](examples/15-tui-hello.rs) | Raw mode, the alternate screen, the frame loop, and getting out safely | [ch 8b](../../docs/guide/08b-the-live-tui.md) |
| 16 | [`16-tui-dashboard`](examples/16-tui-dashboard.rs) | `mammoth top`: layout, tables, gauges, sparklines, tabs | [ch 8b](../../docs/guide/08b-the-live-tui.md) |

---

## Three things to actually do with them

**1 · Break them on purpose.** Example 01 has lines marked `BREAK ME`.
Uncomment one, run it, read the compiler error. You will meet that exact error
in chapter 5 and recognise it.

**2 · Run everything through `| cat`.** Examples 08, 10, 11, 12, 13 and 14 all
behave differently when stdout is not a terminal. That is not a trick — it is
the behaviour chapters 2, 7 and 8a make you build, and seeing it before you
build it saves an hour.

```bash
cargo run -q -p mammoth-parts --example 11-colour-palette | cat
```

**3 · Copy from them.** `mod palette` in example 11 is written to be lifted
straight into `crates/mammoth-viz/src/style.rs`. `App` / `tick` / `draw` in
example 16 is the structure `mammoth top` should have. That is the point.

---

## No terminal? No problem

Examples 15 and 16 take over the screen, which makes them awkward in CI, over a
flaky SSH session, or inside an editor's output pane. Both take `--check`, which
renders their frames into an in-memory buffer and prints them as plain text:

```bash
cargo run -q -p mammoth-parts --example 16-tui-dashboard -- --check
```

That is `ratatui::backend::TestBackend`, and it is also how you write a test for
a TUI — see the bottom of either file.

---

## Where these fit

The five numbered directories next to this one (`01-hello-mammoth` and friends)
are **product demos**: what Mammoth looks like to someone using it. This
directory is the opposite — what Mammoth looks like to someone building it.

Start here if you are on the team. Start there if you are evaluating the thing.
