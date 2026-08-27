# Glossary — every word this guide uses

Read this once, skim it now, come back to it whenever a chapter uses a word you
do not recognise. Nothing here assumes you have used Hadoop, or Rust, or a
distributed system before.

Terms are grouped by *when you first meet them*, not alphabetically — so the
first group is the one to read on day one.

---

## Part 1 — Words you need before chapter 4

### Distributed filesystem

A filesystem whose files are too big for one computer, so the files are cut into
pieces and the pieces are spread across many computers. You still say
`ls /data/logs` and it still looks like one directory tree; the fact that the
bytes are on thirty machines is hidden from you.

Mammoth is one of these. So is HDFS (the Hadoop one), and S3 is a cousin.

### Block

**The unit a file is cut into.** A 300 MB file with a 128 MB block size becomes
three blocks: 128 MB, 128 MB, 44 MB. The last block is *partial* — it is not
padded out to full size.

Blocks are why a big file can live on many machines at once, and why reading one
can use many disks in parallel.

```
/data/big.log   (300 MB, block size 128 MB)
   block 0  ██████████████████  128 MB
   block 1  ██████████████████  128 MB
   block 2  ██████░░░░░░░░░░░░   44 MB   ← partial, and that is correct
```

In the code: `BlockId(u64)`, rendered as `blk_0000000000012345`.

### Replica

**A copy of one block, on one machine.** Disks die. So each block is stored
`replication` times — three by default — on three different machines. Lose a
machine, the other two copies still serve the data, and the system quietly makes
a third copy somewhere else.

In the code: `Replica { node, rack, state }`, where `state` is `Primary`,
`Replica`, or `Corrupt`.

### Replication factor

**How many copies of each block you want.** Written `replication: 3`. Directories
do not have one, which is why the field is `Option<u8>` and is `None` for them.

### Node / worker

**One machine that stores blocks.** Called a *worker* in Mammoth. In chapters
5–8 the "workers" are six directories on your laptop (`workers/w1`, `w2`, …) so
you can build and see the whole system without owning six computers.

In the code: `NodeId(String)` — `"w1"`, `"w2"`, …

### Rack

**A group of machines that fail together.** Physically: one cabinet, sharing one
power supply and one top-of-rack switch. Pull the wrong plug and the whole rack
goes.

This matters because putting all three replicas in one rack means one unplugged
cable loses your data. The placement rule in chapter 5 exists entirely to avoid
that.

In the code: a string like `"/dc1/rack-a"`.

### Master

**The machine that remembers where everything is.** It holds the namespace (the
directory tree) and the block map (which blocks live on which workers). It does
not hold your actual bytes — those are on workers.

Hadoop calls this the NameNode. Mammoth's version is `mammoth-master`, and it
does not exist yet — chapter 12 is its design.

### Namespace

**The directory tree, as a thing separate from the data.** `/data/logs/a.txt`
is a namespace path. The bytes behind it are somewhere else entirely, in blocks
on workers. Chapter 5 makes the namespace real directories on your disk, which
is a shortcut that works surprisingly well.

### Heartbeat

**A worker saying "I am still alive" every few seconds.** When the master stops
hearing from a worker for long enough (`master.dead_after`), it marks the worker
`Dead` and starts rebuilding that worker's blocks from their surviving replicas.

---

## Part 2 — Rust and Cargo words

### Crate

**A Rust package** — one library or one program. Mammoth has 16 of them under
`crates/`. `mammoth-core` is a library; `mammoth-cli` builds the actual
`mammoth` binary you run.

Roughly: a crate is what other languages call a module, a package, or a jar.

### Workspace

**A set of crates built together as one project.** The root `Cargo.toml`
declares it with `members = ["crates/*", "xtask"]`. That is why
`cargo build --workspace` builds all sixteen at once, and why crates can depend
on each other by path.

### `cargo`

The build tool, test runner, and package manager, all one command. The four you
will actually type:

| Command | What it does |
| --- | --- |
| `cargo build` | compile |
| `cargo test` | compile and run tests |
| `cargo run` | compile and run the binary |
| `cargo fmt` | reformat the code to the standard style |

### `clippy`

**The linter.** It catches things that compile but are a bad idea. Mammoth runs
it with `-D warnings`, which means "treat every warning as an error" — so CI
fails on lint, not just on broken code. That is deliberate.

### Trait

**A list of methods a type promises to provide.** Other languages call this an
interface or a protocol.

The whole architecture of Mammoth rests on one trait, `Backend`. See chapter 4.

### `impl`

"Implementation". `impl Backend for LocalBackend` means "here is how
`LocalBackend` keeps the promises `Backend` makes".

### `async` / `.await`

**A way to wait for slow things without blocking the whole program.** Disks and
networks are slow; `async` lets one thread go do something else while waiting.

`async fn` describes work. `.await` actually runs it. Nothing happens until you
`.await`.

### `Result<T, E>`

**How Rust reports failure.** A function returns either `Ok(value)` or
`Err(problem)`. There are no exceptions. The `?` operator means "if this failed,
give up and return the error to my caller".

### `Option<T>`

**A value that might not be there.** Either `Some(value)` or `None`. Rust has no
`null`, so anything absent must say so in its type.

### Borrowing (`&`)

**Lending a value instead of giving it away.** `&thing` is a read-only loan;
`&mut thing` is an exclusive one. Many readers *or* one writer, never both. The
compiler enforces this at build time, which is why Rust needs no garbage
collector.

---

## Part 3 — Words from chapters 7 onward

### Backend

**The trait every storage implementation satisfies**, and the single most
important idea in the codebase. The CLI, the visualizations, and the web UI all
talk to `&dyn Backend` — never to a concrete type. Swap `LocalBackend` for a
real `ClusterBackend` later and none of them change.

### `LocalBackend`

**The laptop implementation.** Namespace = real directories. Workers = six
subdirectories. Blocks = real files. You build it in chapters 5 and 6, and it is
what makes everything demoable in week 8 instead of week 30.

### Gateway

**The HTTP server that puts the filesystem behind a web API**, so the browser UI
can read it. Chapter 9. It is `mammoth-gateway`, served by `mammoth serve`.

### `viz`

Mammoth's visualization commands — `viz blocks`, `viz cluster`, `viz health`.
They draw the state of the system as terminal graphics. Chapter 8.

This is the part of the project people will remember, which is why it is built
early rather than last.

### Inlining

**Storing a very small file's bytes directly in its metadata** rather than
allocating a whole block for it. A 12-byte file does not deserve a 128 MB block.
Files under the inline threshold have `blocks: 0` and `inlined: true`.

### Fan-out write / one-shot read / declustered repair / warm start

The four ideas in chapter 12 that make Mammoth faster than Hadoop. Do not worry
about them until you get there — but do read chapter 12 *before* writing
`mammoth-master`, because retrofitting them is far more expensive than building
them in.

---

## Part 4 — Git and process words

### Branch

**A private line of work.** You make one, you change things, nobody else is
affected until you merge. `git checkout -b feat/my-thing`.

### PR (pull request)

**A request to merge your branch into `main`**, plus a place to review it. One
teammate reads it, approves, and it merges.

### `main`

**The branch everyone shares.** Keeping it green (building, tests passing) is
the single most important team rule — a broken `main` blocks all three of you.

### Conventional Commits

The commit message format this project uses: `type(scope): description`.

```
feat(cli): add mammoth version command
fix(local): last block was padded instead of left partial
```

Written as a command — "add X", not "added X".

### CI

**Continuous integration** — GitHub runs the build, tests, and clippy on every
PR automatically. The green tick means it passed. Do not merge without it.

---

**Back to:** [the guide index](README.md)
