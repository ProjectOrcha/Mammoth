# The Rust reference for this codebase

[Chapter 1](01-rust-you-need.md) is the thirty minutes you need before you start.
**This page is what you come back to.** It covers everything chapter 1 skims,
in the order you meet it, with a runnable example for each idea and a decoder
for the compiler errors you will actually hit.

Do not read it end to end. Use the contents, or `Ctrl-F` for the symbol that
confused you.

**Contents**

[Reading a file](#part-1--reading-a-rust-file) ·
[Values and types](#part-2--values-and-types) ·
[Ownership](#part-3--ownership-in-practice) ·
[Errors](#part-4--errors) ·
[Traits](#part-5--traits) ·
[Async](#part-6--async) ·
[Collections and iterators](#part-7--collections-and-iterators) ·
[Formatting](#part-8--formatting-strings) ·
[Testing](#part-9--testing) ·
[Cargo](#part-10--cargo-and-the-workspace) ·
[**Error decoder**](#the-compiler-error-decoder) ·
[Cheat sheet](#the-one-page-cheat-sheet)

> **Everything here runs.** Sixteen small programs live in
> [`examples/parts/`](../../examples/parts/), one idea each:
>
> ```bash
> cargo run -q -p mammoth-parts --example 01-ownership
> ```
>
> Where a section names an example, run it. Watching ownership fail to compile
> teaches more in two minutes than any paragraph.

---

## Part 1 · Reading a Rust file

Every file in `crates/` has the same five parts, in this order. Recognising the
shape is most of the battle.

```rust
//! Module documentation. Describes the file you are in.
//! `mammoth-core/src/backend.rs` starts with three of these.

use std::path::Path;                 // ① what this file borrows from elsewhere
use crate::error::Result;

/// Item documentation. Describes the *next* thing down.
pub type ByteStream = /* … */;       // ② type aliases

pub trait Backend { /* … */ }        // ③ traits — the shapes

pub struct LocalBackend { /* … */ }  // ④ structs — the data

impl Backend for LocalBackend {      // ⑤ impls — the behaviour
    /* … */
}
```

### `//!` versus `///`

| | Documents | Goes |
| --- | --- | --- |
| `//!` | the *containing* module or crate | at the very top of the file |
| `///` | the *next item* | immediately above a fn, struct, enum, field |
| `//` | nothing; it is a plain comment | anywhere |

`mammoth-core` has `#![warn(missing_docs)]` at the top of `lib.rs`, so a
public item with no `///` is a warning. That is deliberate: the doc comments are
what `cargo doc` publishes and what `--help` prints.

### `use`, modules, and paths

`use` does not import code — everything in the crate is already compiled in. It
only introduces a **shorter name**:

```rust
use mammoth_core::types::FileStatus;   // now `FileStatus` instead of the full path
use mammoth_core::{Backend, Result};   // several at once
use std::collections::BTreeMap;
```

Three prefixes tell you where a name comes from:

| Prefix | Means |
| --- | --- |
| `std::` | the standard library |
| `crate::` | somewhere else in *this* crate |
| anything else | another crate, listed in this crate's `Cargo.toml` |

A module is either a `mod name { … }` block or a file. `pub mod fs;` in
`commands/mod.rs` means "there is a file `commands/fs.rs`, and it is public".
Without `pub`, a module or item is visible only inside its parent.

## Part 2 · Values and types

### The number types

```rust
u8   u16  u32  u64  usize    // unsigned
i8   i16  i32  i64  isize    // signed
f64  f32                     // floating point
bool  char                   // char is one Unicode scalar, 4 bytes
```

`usize` is "the size of a pointer on this machine" and is what indexes and
lengths use. Mammoth uses `u64` for byte counts (a file may exceed 4 GB),
`u32` for block counts, `u8` for the replication factor.

Rust does **no implicit numeric conversion**. You write it out:

```rust
let fraction = used as f64 / capacity as f64;   // `as` for numbers
let n: u64 = small_u32.into();                  // `.into()` when it cannot lose data
let n: u32 = big_u64.try_into()?;               // `.try_into()` when it might
```

That is why `as f64` appears on nearly every line of `mammoth-viz`.

### Text: `String` and `&str`

The distinction that trips up everyone coming from a garbage-collected language.

| | `String` | `&str` |
| --- | --- | --- |
| Owns its bytes | yes | no — it borrows |
| Can grow | yes | no |
| Written as | `String::from("w1")`, `"w1".to_string()` | `"w1"`, `&some_string` |
| Use it for | a field in a struct, anything you keep | a function parameter |

The rule of thumb: **take `&str`, store `String`.**

```rust
fn rack_of(node: &str) -> String {          // borrows in, owns out
    format!("/dc1/{node}")
}
```

`&'static str` — which you will see on `Error` variants and in `Tone::name()` —
means "a string slice that lives for the whole program", which in practice means
a literal baked into the binary. It costs nothing to return.

The same pair exists for paths (`PathBuf` / `&Path`) and for lists
(`Vec<T>` / `&[T]`), for the same reason.

### `Option<T>` and `Result<T, E>`

Rust has no `null` and no exceptions. Both absences are types.

```rust
pub replication: Option<u8>,   // None for a directory, Some(3) for a file
fn stat(&self, p: &Path) -> Result<FileStatus>   // Ok(status) or Err(problem)
```

The methods worth knowing, roughly in order of how often you want them:

```rust
opt.unwrap_or(3)                  // a default
opt.unwrap_or_else(|| expensive())// a default you only compute if needed
opt.map(|r| r.to_string())        // transform the inside, leave None alone
opt.is_some()  opt.is_none()
if let Some(r) = opt { … }        // run only if present
match opt { Some(r) => …, None => … }

res?                              // unwrap, or return the error to my caller
res.unwrap()                      // unwrap, or panic. tests only
res.expect("why this cannot fail")// unwrap, or panic with your message
res.map_err(|e| Error::Config(…)) // change the error type
res.ok()                          // Result<T, E> → Option<T>, discarding the error
```

> `.unwrap()` on a path a user can reach is the "prints a stack trace" behaviour
> Mammoth exists to avoid. In tests it is fine. In `main` it is a bug.

▶ [`02-result-and-errors`](../../examples/parts/examples/02-result-and-errors.rs)

## Part 3 · Ownership, in practice

The rules, stated once:

1. Every value has exactly one **owner**.
2. When the owner goes out of scope, the value is dropped.
3. You may have **many `&` borrows, or one `&mut` borrow. Never both.**

You will not think about this consciously for long. What you will do is meet
three errors, and each has a mechanical fix.

| Error | What happened | Fix |
| --- | --- | --- |
| `borrow of moved value` | you passed a value away, then used it | pass `&thing`, or `.clone()` |
| `cannot borrow as mutable` | you only have a `&` | take `&mut`, or clone |
| `cannot borrow as mutable more than once` | two live `&mut` | shorten one; often just a `{ }` block |

**Cloning to get past a borrow error is completely fine while you are learning.**
`.clone()` costs an allocation, and that matters in a hot loop and nowhere else.
Make it fast later, when a profiler tells you which one.

Where this shows up as syntax you read every day:

```rust
fn count(&self) -> usize        // reads the struct
fn add(&mut self, p: &str)      // changes the struct
fn into_parts(self) -> (A, B)   // consumes it; the value is gone afterwards
```

▶ [`01-ownership`](../../examples/parts/examples/01-ownership.rs) — including
`BREAK ME` lines that produce each error on purpose.

### Lifetimes, the ten percent you need

Sometimes a borrow needs a name so the compiler can check it outlives what holds
it. That name is a **lifetime**, written `'a`:

```rust
struct Listing<'a> {
    entries: &'a [FileStatus],   // this struct borrows; it must not outlive the data
}
```

You will rarely write one. When the compiler asks for one, the usual right
answer is *not* to add a lifetime but to **own the data instead** — store
`Vec<FileStatus>` rather than `&'a [FileStatus]`. Mammoth's types own their
data almost everywhere, precisely so that nobody has to think about this.

## Part 4 · Errors

Mammoth's error type lives in `crates/mammoth-core/src/error.rs` and is worth
reading in full — it is 120 lines and it defines the product's voice.

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no such path: {0}")]
    NotFound(PathBuf),

    #[error("not enough healthy workers for replication {wanted}: only {available} available")]
    NotEnoughWorkers { wanted: u8, available: u8 },

    #[error(transparent)]
    Io(#[from] std::io::Error),
}
```

Three things are happening:

- **`thiserror`** turns each `#[error("…")]` into the `Display` impl, so `{e}`
  prints that sentence. You never write `impl Display`.
- **`{0}` and `{wanted}`** interpolate the variant's own fields.
- **`#[from]`** generates `impl From<std::io::Error> for Error`, which is what
  makes `?` work on a `std::fs::read(...)` inside a function returning
  `mammoth_core::Result`. Without it you would need `.map_err(…)` on every I/O
  call.

The alias saves typing the error type everywhere:

```rust
pub type Result<T, E = Error> = std::result::Result<T, E>;
```

So `Result<Vec<FileStatus>>` means `Result<Vec<FileStatus>, mammoth_core::Error>`.

▶ [`02-result-and-errors`](../../examples/parts/examples/02-result-and-errors.rs) ·
▶ [`09-cli-errors`](../../examples/parts/examples/09-cli-errors.rs)

## Part 5 · Traits

A trait is a set of methods a type promises to provide. It is the single most
important construct in this codebase — [chapter 4](04-the-backend-trait.md) is
entirely about one of them.

```rust
pub trait Backend: Send + Sync {          // ← supertraits, see below
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>>;
}

impl Backend for LocalBackend {
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>> { /* … */ }
}
```

### `impl Trait` versus `dyn Trait`

Two ways to accept "something implementing a trait", and the difference matters:

```rust
fn a(be: &impl Backend)    // static dispatch: a separate copy compiled per type
fn b(be: &dyn Backend)     // dynamic dispatch: one copy, type known at run time
```

`impl` is faster (the calls can be inlined). `dyn` is more flexible: you can put
different implementations in one `Vec<Box<dyn Backend>>`, and the function is
compiled once. Mammoth's commands take `&dyn Backend` because the flexibility is
the entire point — and the cost, one pointer hop per call, is nothing next to a
disk read.

`dyn` always needs a `&` or a `Box`, because the compiler does not know how big
the concrete type is. `be: dyn Backend` is the error `the size for values of
type dyn Backend cannot be known at compilation time`.

### Supertraits: `Send`, `Sync`, `'static`

`pub trait Backend: Send + Sync` means "any Backend must also be `Send` and
`Sync`":

- **`Send`** — safe to move to another thread.
- **`Sync`** — safe to share (`&T`) between threads.

These are derived automatically for anything built from `Send + Sync` parts, so
you usually notice them only when something is not. `Rc<T>` is not `Send`
(use `Arc<T>`); `RefCell<T>` is not `Sync` (use `Mutex<T>`).

### Deriving

`#[derive(…)]` writes an impl for you:

| Derive | Gives you |
| --- | --- |
| `Debug` | `{:?}` and `{:#?}` printing, and `dbg!()` |
| `Clone` | `.clone()` |
| `Copy` | assignment copies instead of moving. Small, plain types only |
| `PartialEq, Eq` | `==` |
| `PartialOrd, Ord` | `<`, `.sort()` |
| `Hash` | usable as a `HashMap` key |
| `Default` | `Default::default()`, and `..Default::default()` in a literal |
| `Serialize, Deserialize` | JSON and every other serde format |

`FileStatus` derives `Debug, Clone, Serialize, Deserialize`, which is why
`emit()` can turn one into JSON without a line of conversion code.

▶ [`04-traits-and-dyn`](../../examples/parts/examples/04-traits-and-dyn.rs)

## Part 6 · Async

`async` lets one thread do other work while waiting for a disk or a network.
Three rules cover everything in this codebase:

1. Calling an `async fn` returns a **future** — a description of work.
2. **Nothing runs until you `.await` it.**
3. You may only `.await` inside another `async fn`.

`#[tokio::main]` on `main` starts the runtime that drives it all. Forgetting it
gives `` `main` function is not allowed to be `async` ``.

### Doing several things at once

```rust
let (a, b, c) = tokio::join!(f1(), f2(), f3());   // all three, wait for all

let winner = tokio::select! {                     // whichever answers first
    r = slow_replica() => r,
    r = fast_replica() => r,
};

let handle = tokio::spawn(async move { … });      // a genuinely separate task
let result = handle.await?;
```

`join!` is what turns three 100 ms replica reads into 100 ms rather than 300.
`select!` is the shape of a hedged read — chapter 12 §1.

### `#[async_trait]`

Rust's native `async fn` in traits does not yet support `&dyn`. The
`async_trait` macro rewrites each method to return a boxed future, which does.

**You need the attribute in two places** — on the trait and on every `impl`:

```rust
#[async_trait::async_trait]
impl Backend for LocalBackend { /* … */ }
```

Forgetting it on the impl produces a confusing lifetime error. That is the
answer to it.

### `ByteStream`

```rust
pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes>> + Send>>;
```

Read it inside out: a `Stream` of `Result<Bytes>` items — an async iterator —
that is `Send`, boxed because the concrete type varies, and `Pin`ned because a
future must not move in memory once it has been polled. You will type the alias,
not build it from scratch.

Consuming one is a loop:

```rust
use futures_util::StreamExt;
while let Some(chunk) = stream.next().await {
    out.write_all(&chunk?)?;
}
```

That loop is `mammoth cat`, and it streams a 10 TB file through 8 MB of memory
because the whole file is never held at once.

▶ [`05-async-and-streams`](../../examples/parts/examples/05-async-and-streams.rs)

## Part 7 · Collections and iterators

| Type | Use it when |
| --- | --- |
| `Vec<T>` | an ordered list. The default |
| `HashMap<K, V>` | key → value, fast, **order is random** |
| `BTreeMap<K, V>` | key → value, **sorted** — use this when you print it |
| `HashSet<T>` / `BTreeSet<T>` | membership, same distinction |
| `VecDeque<T>` | push one end, pop the other. A rolling window |

> **Use `BTreeMap` for anything you display.** `HashMap` iterates in an
> unspecified order that changes between runs, which makes output flicker and
> tests flake. Chapter 8's rack grouping uses `BTreeMap` for exactly this
> reason.

Iterator chains do most of the work in `mammoth-viz` and the CLI:

```rust
let used: u64 = nodes.iter().map(|n| n.used).sum();
let dead = nodes.iter().filter(|n| matches!(n.state, NodeState::Dead)).count();
let names: Vec<&str> = nodes.iter().map(|n| n.id.0.as_str()).collect();
let fullest = nodes.iter().max_by(|a, b| a.fill().total_cmp(&b.fill()));
```

Three things to know:

- **They are lazy.** Nothing runs until `.collect()`, `.sum()`, `.count()` or a
  `for` loop pulls. Chaining ten adaptors still makes one pass.
- **`.iter()` borrows, `.into_iter()` consumes, `.iter_mut()` lends mutably.**
  `for x in &vec` is `.iter()`; `for x in vec` moves the vector.
- **`|n| …` is a closure**, an anonymous function. `move |n| …` takes ownership
  of what it captures, which is what `tokio::spawn` needs.

`matches!(value, Pattern)` is the short way to ask "is it this shape?" without
writing a whole `match`.

▶ [`03-structs-enums-match`](../../examples/parts/examples/03-structs-enums-match.rs)

## Part 8 · Formatting strings

Alignment is not a nicety in this project — it is what keeps the block matrix
from shearing. Learn this mini-language properly:

```rust
format!("{}",        value)   // Display  — the human form
format!("{:?}",      value)   // Debug    — the developer form
format!("{:#?}",     value)   // Debug, pretty-printed over lines

format!("{:<8}",     s)       // left-aligned in 8 columns
format!("{:>8}",     s)       // right-aligned
format!("{:^6}",     s)       // centred — the block matrix
format!("{:08.3}",   f)       // zero-padded, 3 decimals
format!("{:.1}",     f)       // one decimal
format!("{:>3.0}%",  pct)     // right-aligned, no decimals, then a literal %

let name = "w1";
format!("{name} is up")       // captured directly from scope, since Rust 1.58
format!("{:>width$}", s, width = 12)   // width from a variable
```

Two traps, both of which will bite you in chapter 8:

**Bytes are not columns.** `"█".len()` is 3; `"█".chars().count()` is 1. Any
width calculation must count `chars()`, and even that is approximate for East
Asian text — that is what the `unicode-width` crate is for.

**Pad first, colour second.** `{:^6}` counts bytes, and an ANSI escape is five
of them. Centre the plain text, *then* colour the result, or the grid shears.

## Part 9 · Testing

Tests live in the file they test, at the bottom:

```rust
#[cfg(test)]                 // only compiled for `cargo test`
mod tests {
    use super::*;            // bring the parent module into scope

    #[test]
    fn bar_is_always_exactly_width_cells() {
        for pct in 0..=100 {
            assert_eq!(bar(pct as f64 / 100.0, 16).chars().count(), 16, "at {pct}%");
        }
    }

    #[tokio::test]           // for anything async
    async fn write_then_read_round_trips() { /* … */ }
}
```

```rust
assert!(condition, "message with {value}");
assert_eq!(left, right);
assert_ne!(left, right);
```

The third argument to `assert_eq!` is a message printed on failure. Use it —
`at 73%` turns a failing test into a fixed bug in ten seconds.

**Doctests.** A ```` ``` ```` block in a `///` comment is compiled and run by
`cargo test`:

```rust
/// ```
/// use mammoth_viz::bar;
/// assert_eq!(bar(1.0, 4), "████");
/// ```
pub fn bar(fraction: f64, width: usize) -> String { /* … */ }
```

Documentation that cannot go stale, because the build fails if it does.

```bash
cargo test --workspace        # everything
cargo test -p mammoth-viz     # one crate
cargo test bar_is             # tests whose name contains this
cargo test -- --nocapture     # show println! output from passing tests
```

## Part 10 · Cargo and the workspace

Mammoth is a **workspace**: many crates, one `Cargo.lock`, one `target/`.

```toml
# Cargo.toml at the root
[workspace]
members = ["crates/*", "examples/parts", "xtask"]

[workspace.dependencies]        # versions decided once, here
tokio = { version = "1", features = ["full"] }
```

```toml
# crates/mammoth-cli/Cargo.toml
[dependencies]
tokio = { workspace = true }    # "use whatever the workspace says"
```

**Always add dependencies this way.** Two crates depending on different versions
of the same library compiles, links both copies into the binary, and produces
type errors that make no sense — `expected Bytes, found Bytes`.

The commands you will actually type:

```bash
cargo build --workspace          # compile everything
cargo build -p mammoth-cli       # one crate, much faster
cargo check                      # type-check only; the fastest feedback loop
cargo run -p mammoth-cli -- ls /data      # note the bare --
cargo run -q -p mammoth-parts --example 01-ownership
cargo test --workspace
cargo fmt --all                  # format. Never argue about layout
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --open                 # the docs for every crate you depend on
```

`cargo clippy` is a second, much pickier compiler. It is not optional here — CI
runs it with `-D warnings`, which makes every lint an error. It is also a
genuinely good Rust teacher: most of its suggestions are idioms worth learning.

## The compiler error decoder

Rust's errors are unusually good; read the whole message including `help:`.
These are the ones you will actually meet in this codebase.

| Message | What it means | Usual fix |
| --- | --- | --- |
| `borrow of moved value: x` | you gave `x` away, then used it | pass `&x`, or `.clone()` |
| `cannot borrow x as mutable` | you only hold a `&` | take `&mut x`; or clone |
| `cannot borrow x as mutable more than once` | two live `&mut` | shorten one borrow's scope with `{ }` |
| `cannot borrow x as mutable because it is also borrowed as immutable` | a `&` is still alive | finish with the reader first |
| `expected String, found &str` | ownership mismatch | `.to_string()` or `.into()` |
| `expected &str, found String` | the other direction | `&s` or `s.as_str()` |
| `the trait bound X: Y is not satisfied` | a required trait is missing | `use` it, or `#[derive]` it |
| `the size for values of type dyn T cannot be known` | a bare `dyn` | `&dyn T` or `Box<dyn T>` |
| `` `?` couldn't convert the error `` | no `From` between the error types | add `#[from]`, or `.map_err(…)` |
| `` `main` is not allowed to be `async` `` | missing runtime | add `#[tokio::main]` |
| `future cannot be sent between threads safely` | you held a non-`Send` value across an `.await` | drop it before awaiting; use `Arc`/`Mutex` |
| `cannot return value referencing temporary value` | you returned a borrow of something local | return an owned value; for owo-colors, build a `Style` |
| `no method named X found` | trait not in scope | `use owo_colors::OwoColorize;` and friends |
| `unused variable: x` | exactly that | prefix with `_`: `_x` |
| `mismatched types: expected [T; 4], found Rc<[T]>` | ratatui `.split()` vs `.areas()` | use `.areas()` for the array form |

Two commands worth knowing when a message is opaque:

```bash
rustc --explain E0502     # the long-form explanation of any error code
cargo clippy --fix        # apply the mechanical suggestions
```

## The one-page cheat sheet

```rust
// ── bindings ───────────────────────────────────────────────────────────────
let x = 5;                  // immutable by default
let mut y = 5;              // mutable
const MAX: u64 = 128;       // compile-time constant
static NAME: &str = "w1";   // one instance, whole program

// ── functions ──────────────────────────────────────────────────────────────
fn add(a: u64, b: u64) -> u64 { a + b }     // no `return` needed on the last line
fn nothing() {}                             // returns ()

// ── data ───────────────────────────────────────────────────────────────────
struct Node { id: String, used: u64 }
enum State { Healthy, Warn(&'static str), Dead { minutes: u32 } }

impl Node {
    fn new(id: &str) -> Self { Self { id: id.into(), used: 0 } }   // constructor
    fn fill(&self) -> f64 { 0.0 }                                  // reads
    fn add(&mut self, n: u64) { self.used += n }                   // writes
}

// ── control flow ───────────────────────────────────────────────────────────
if x > 3 { … } else { … }
let label = if x > 3 { "big" } else { "small" };   // if is an expression
match state {
    State::Healthy => "●",
    State::Warn(why) => why,
    State::Dead { minutes } => "✕",
}
if let Some(v) = maybe { … }
while let Some(chunk) = stream.next().await { … }
for n in &nodes { … }               // borrow
for n in nodes { … }                // consume
loop { break; }

// ── errors ─────────────────────────────────────────────────────────────────
let text = std::fs::read_to_string(p)?;    // propagate
let n = maybe.ok_or(Error::NotFound(p))?;  // Option → Result → propagate

// ── async ──────────────────────────────────────────────────────────────────
let status = backend.stat(path).await?;
let (a, b) = tokio::join!(f1(), f2());
```

---

**See also:** [chapter 1](01-rust-you-need.md) for the thirty-minute version ·
[the glossary](GLOSSARY.md) for one-line definitions of everything, including
the Git and distributed-systems words ·
[`examples/parts/`](../../examples/parts/) for all of this, running ·
[the Rust Book](https://doc.rust-lang.org/book/), which is free and genuinely
excellent when you want the real thing.
