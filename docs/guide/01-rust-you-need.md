# Chapter 1 — The 30-minute Rust you actually need

**What you'll build:** enough Rust to read and write the code in chapters 4–8.

**Time:** 30 minutes.

---

## Before you start

```markdown
- [ ] Chapter 0 is finished — `cargo build --workspace` succeeds
```

**How to read this chapter.** Do not try to memorise it. Read it once at normal
speed, do the warm-up at the end, and then *come back to it* whenever a later
chapter uses something you do not recognise. It is a reference, not an exam.

**Two companions, for when this is not enough.** Chapter 1 is deliberately the
short version:

- [**The Rust reference for this codebase**](RUST-REFERENCE.md) — everything
  here in far more depth, plus modules, collections, iterators, formatting,
  testing, cargo, and a decoder table for the compiler errors you will actually
  hit. Do not read it end to end; `Ctrl-F` it when something confuses you.
- [**`examples/parts/`**](../../examples/parts/) — sixteen small programs, one
  idea each. Every section below names one. **Run them.** Watching a borrow
  error happen teaches more in two minutes than any paragraph can.

**Nothing here needs to be committed.** The only code you write is a scratch
project outside the repository, which you delete at the end.

---

This is not a Rust course. It is the six things that appear on nearly every line
of Mammoth, explained once so the later chapters do not have to stop and explain
them. If you already know Rust, skim the last two sections and move on.

For anything deeper, [the Rust Book](https://doc.rust-lang.org/book/) is free
and genuinely excellent.

## 1 · Ownership, in one paragraph

Every value has exactly one owner. When the owner goes out of scope, the value
is freed. That is why Rust needs no garbage collector — and why a Mammoth master
never pauses for seconds the way a Java NameNode does.

You *lend* values instead of copying them. `&thing` is a read-only loan.
`&mut thing` is an exclusive loan. You can have many readers **or** one writer,
never both. The compiler enforces it.

```rust
let name = String::from("w1");
let borrowed = &name;      // lend it out, read-only
println!("{name} {borrowed}");   // both still usable
```

When you see `&self` on a method, it means "this method reads the struct". When
you see `&mut self`, it means "this method changes it".

▶ Run [`01-ownership`](../../examples/parts/examples/01-ownership.rs). It has
lines marked `BREAK ME` — uncomment one, rebuild, and read the error. You will
meet that exact message in chapter 5, and you will recognise it.

```bash
cargo run -q -p mammoth-parts --example 01-ownership
```

## 2 · `Result` — errors are values, not exceptions

Rust has no exceptions. A function that can fail returns `Result<T, E>`: either
`Ok(value)` or `Err(problem)`.

```rust
fn read_config() -> Result<Config, Error> {
    let text = std::fs::read_to_string("mammoth.toml")?;   // ← note the ?
    let config = parse(&text)?;
    Ok(config)
}
```

**The `?` operator is the one to learn.** It means: if this is `Err`, stop and
return that error to my caller; if it is `Ok`, unwrap it and carry on. Without
it, that function would be a pile of nested `match` blocks.

Mammoth defines its own alias so you rarely write the error type:

```rust
// crates/mammoth-core/src/error.rs
pub type Result<T, E = Error> = std::result::Result<T, E>;
```

So `Result<Vec<FileStatus>>` means `Result<Vec<FileStatus>, mammoth_core::Error>`.

### `.unwrap()` and when not to use it

`.unwrap()` says "I am certain this is `Ok`; crash if I am wrong." It is fine in
tests and in code that genuinely cannot fail. It is **not** fine on a path a
user can reach — crashing on bad input is exactly the "prints a stack trace"
behaviour Mammoth exists to avoid.

▶ [`02-result-and-errors`](../../examples/parts/examples/02-result-and-errors.rs)
builds a miniature version of Mammoth's `Error` — variants, stable codes, and
the hints that make an error *teach*. It is forty lines and it is the shape of
`crates/mammoth-core/src/error.rs`.

## 3 · `Option` — a value that might not be there

```rust
pub replication: Option<u8>,     // None for directories, Some(3) for files
```

`Option<T>` is `Some(value)` or `None`. Rust has no `null`, so anything that can
be absent says so in its type. Useful methods:

```rust
let repl = status.replication.unwrap_or(3);        // default if None
if let Some(r) = status.replication { /* ... */ }  // run only if present
```

More of them, and the same list for `Result`, are in
[the reference](RUST-REFERENCE.md#optiont-and-resultt-e).

## 4 · Structs, enums, and `match`

A **struct** groups data:

```rust
pub struct Replica {
    pub node: NodeId,
    pub rack: String,
    pub state: ReplicaState,
}
```

An **enum** is one of several possibilities — and unlike enums in most
languages, each variant can carry data:

```rust
pub enum ReplicaState {
    Primary,
    Replica,
    Corrupt,
}
```

`match` handles every case, and the compiler **fails the build** if you forget
one. This is a large part of why Rust code tends not to have surprise crashes:

```rust
let symbol = match replica.state {
    ReplicaState::Primary => "●",
    ReplicaState::Replica => "◐",
    ReplicaState::Corrupt => "✕",
};
```

▶ [`03-structs-enums-match`](../../examples/parts/examples/03-structs-enums-match.rs)
adds the two things this section skips and chapters 5–8 use constantly: enum
variants that *carry data*, and iterator chains — `.iter().filter().count()`,
`.map().sum()`, `.collect()`.

## 5 · Traits — the shape of a thing

A trait is a set of methods a type promises to provide. Other languages call
this an interface.

```rust
pub trait Backend {
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>>;
}
```

Any type can *implement* it:

```rust
impl Backend for LocalBackend {
    async fn list(&self, path: &Path) -> Result<Vec<FileStatus>> {
        // the real code
    }
}
```

**This is the single most important idea in Mammoth's architecture.** The CLI is
written against `Backend`, not against `LocalBackend`. So when you later write
`ClusterBackend` that talks to real machines over the network, every CLI command
keeps working without a single line changing. Chapter 4 is entirely about this.

▶ [`04-traits-and-dyn`](../../examples/parts/examples/04-traits-and-dyn.rs) is
that idea in one runnable file: one trait, two implementations, one function
called against both. If chapter 4 does not land the first time, run this and
read it again.

## 6 · `async` and `.await`

Reading from a disk or a network is slow. `async` lets one thread do other work
while waiting instead of sitting idle.

```rust
let status = backend.stat(path).await?;
//                              ^^^^^^ wait for it, then ? the Result
```

Three rules that cover everything in this guide:

1. `async fn` returns a *future* — a description of work, not the work itself.
2. Nothing happens until you `.await` it.
3. You can only `.await` inside another `async fn`.

`#[tokio::main]` on `main` starts the runtime that actually drives all this.

### Why `#[async_trait]`?

Rust's support for `async fn` directly inside traits is still limited for the
kind of dynamic dispatch Mammoth needs (`&dyn Backend`). The `async_trait`
macro works around it. You will see it on the trait and on every `impl`:

```rust
#[async_trait::async_trait]
impl Backend for LocalBackend { /* ... */ }
```

Treat it as a required incantation. Forgetting it produces a confusing error;
that is the answer to it.

▶ [`05-async-and-streams`](../../examples/parts/examples/05-async-and-streams.rs)
shows the three rules, `tokio::join!` turning three 100 ms reads into 100 ms,
and — the part chapter 6 needs — a `ByteStream` handing out chunks one at a
time. It prints timings, so you can see the concurrency rather than take it on
trust.

## Things you'll see and can safely skim past

| Thing | What it means |
| --- | --- |
| `Vec<T>` | a growable array |
| `String` vs `&str` | owned text vs borrowed text. `.to_string()` converts |
| `PathBuf` vs `&Path` | the same distinction, for filesystem paths |
| `impl Into<PathBuf>` | "anything that can become a PathBuf" — lets callers pass `&str` |
| `#[derive(Debug, Clone)]` | auto-generate boilerplate: printing, copying |
| `#[derive(Serialize, Deserialize)]` | auto-generate JSON conversion (serde) |
| `Box<dyn Trait>` | a value on the heap whose exact type is decided at runtime |
| `Arc<T>` | a shared, reference-counted value; cloning it is cheap |
| `pub` | visible outside this module. Without it, private |
| `//!` vs `///` | docs for the *containing* module vs docs for the *next* item |
| `#[cfg(test)]` | only compile this for `cargo test` |
| `matches!(x, P)` | "is `x` this shape?", without writing a whole `match` |
| `'static` | lives for the whole program — usually a string literal |
| `Send` / `Sync` | safe to move between threads / safe to share between them |

All of these are expanded, with examples, in
[the reference](RUST-REFERENCE.md).

## Check it works

Make a scratch project outside the repo and prove the pieces work together:

```bash
cd ~
cargo new rust-warmup
cd rust-warmup
```

Put this in `src/main.rs`:

```rust
#[derive(Debug)]
enum State {
    Healthy,
    Dead,
}

#[derive(Debug)]
struct Node {
    id: String,
    used: u64,
    capacity: u64,
    state: State,
}

impl Node {
    fn percent_used(&self) -> f64 {
        if self.capacity == 0 {
            return 0.0;
        }
        (self.used as f64 / self.capacity as f64) * 100.0
    }

    fn bar(&self) -> String {
        let filled = (self.percent_used() / 6.25).round() as usize;
        format!("{}{}", "█".repeat(filled), "░".repeat(16 - filled))
    }
}

fn main() {
    let nodes = vec![
        Node { id: "w1".into(), used: 114, capacity: 160, state: State::Healthy },
        Node { id: "w2".into(), used: 93,  capacity: 160, state: State::Healthy },
        Node { id: "w3".into(), used: 0,   capacity: 160, state: State::Dead },
    ];

    for n in &nodes {
        let marker = match n.state {
            State::Healthy => "●",
            State::Dead => "✕",
        };
        println!("  {} {}  {} {:.0}%", n.id, marker, n.bar(), n.percent_used());
    }
}
```

```bash
cargo run
```

```
  w1 ●  ███████████░░░░░ 71%
  w2 ●  █████████░░░░░░░ 58%
  w3 ✕  ░░░░░░░░░░░░░░░░ 0%
```

That is a struct, an enum, a `match`, a method taking `&self`, borrowing with
`&nodes`, and string formatting — most of what chapters 5–8 use. You have
also just written a crude version of `mammoth viz cluster`.

Delete the scratch project when you are done:

```bash
cd ~ && rm -rf rust-warmup
```

## Done when

You do not need to have *memorised* any of this. You need to recognise it when
you see it.

```markdown
- [ ] The `rust-warmup` project compiled and printed the three-node bar chart
- [ ] I can say what `?` does in one sentence
- [ ] I can say what the difference between `&self` and `&mut self` is
- [ ] I know that `Option<T>` is Rust's replacement for `null`
- [ ] I know what a trait is, in the "it is like an interface" sense
- [ ] I know that nothing in an `async fn` runs until it is `.await`ed
- [ ] I know where to look up the rest — [the reference](RUST-REFERENCE.md),
      [the glossary](GLOSSARY.md), [the Rust Book](https://doc.rust-lang.org/book/)
- [ ] I ran at least examples 01 and 04 from
      [`examples/parts/`](../../examples/parts/)
- [ ] I deleted the scratch project
```

Six of those boxes are enough to read every line of chapters 4–8. If one is
still fuzzy, re-read just that section — do not re-read the chapter.

## If it went wrong

**`cannot borrow as mutable`** — you tried to change something you only borrowed
read-only. Either take `&mut`, or make your own copy with `.clone()`.

**`value borrowed here after move`** — you gave a value away and then used it.
Pass `&thing` instead of `thing`, or `.clone()` it. Cloning is not a crime while
you are learning; make it fast later, once it shows up in a profile.

**`expected String, found &str`** — add `.to_string()` or `.into()`.

**`` `?` couldn't convert the error ``** — the error type from the inner call
does not convert into your function's error type. Mammoth's `Error` has
`#[from] std::io::Error`, so I/O errors convert automatically; others need a
`.map_err(...)`.

**Anything else** — there is a
[decoder table](RUST-REFERENCE.md#the-compiler-error-decoder) of every compiler
error this codebase produces, with the fix for each. And `rustc --explain E0502`
prints the long-form explanation of any error code, offline.

---

**Next:** [Chapter 2 — Your first change](02-first-change.md)
