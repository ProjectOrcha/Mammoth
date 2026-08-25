# Chapter 7 — Wiring up the CLI

**What you'll build:** `mammoth ls`, `put`, `cat` and `stat`, working for real.

**Time:** about 2 hours.

---

You have a working filesystem. Nobody can use it yet. This chapter connects it
to the command tree you met in chapter 2.

It is mostly plumbing, and that is the point: **the hard thinking already
happened in chapters 5 and 6.** Once a `Backend` exists, adding commands is
cheap. That is the whole argument for [ADR 0002](../adr/0002-backend-trait.md).

## Step 1 · Give the commands arguments

In chapter 2 the commands were bare: `Ls,` with no fields. They need arguments
now. Open `crates/mammoth-cli/src/cli.rs` and change these variants:

```rust
    /// List a directory.
    Ls(LsArgs),
    /// Upload a local file.
    Put(PutArgs),
    /// Download to a local file.
    Get,
    /// Stream a file to stdout.
    Cat(PathArgs),
```

and

```rust
    /// Metadata for one path.
    Stat(PathArgs),
```

Then add the argument structs, just above the `VizCommand` enum:

```rust
/// Arguments for `mammoth ls`.
#[derive(clap::Args)]
pub struct LsArgs {
    /// Directory to list.
    #[arg(default_value = "/")]
    pub path: PathBuf,
    /// Long format: permissions, owner, size, replication, blocks.
    #[arg(short, long)]
    pub long: bool,
}

/// Arguments for `mammoth put`.
#[derive(clap::Args)]
pub struct PutArgs {
    /// Local file to upload.
    pub src: PathBuf,
    /// Destination path inside the cluster.
    pub dst: PathBuf,
    /// Override the block size for this file, e.g. `512MB`.
    #[arg(long)]
    pub block_size: Option<String>,
    /// Override the inline threshold for this file, e.g. `4KB`.
    #[arg(long)]
    pub inline_threshold: Option<String>,
}

/// A command that takes one cluster path.
#[derive(clap::Args)]
pub struct PathArgs {
    /// Path inside the cluster.
    pub path: PathBuf,
}
```

`#[derive(clap::Args)]` means "these fields are command-line arguments". Fields
without `#[arg(...)]` become positional, in declaration order. `bool` fields
with `#[arg(short, long)]` become flags: `-l` and `--long`.

## Step 2 · Open the backend

Every command needs a `Backend`. Replace
`crates/mammoth-cli/src/commands/mod.rs`:

```rust
//! One module per command group. Each takes `&dyn Backend` and returns a type
//! that implements [`crate::output::Render`], so a command never knows whether
//! it is talking to `LocalBackend` or a real cluster, and never decides how its
//! output is formatted.

pub mod fs;

use std::path::PathBuf;

use mammoth_core::Result;
use mammoth_local::LocalBackend;

/// Where LocalBackend keeps its store: `~/.mammoth/data`.
pub fn store_dir() -> PathBuf {
    std::env::var_os("MAMMOTH_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".mammoth")))
        .unwrap_or_else(|| PathBuf::from(".mammoth"))
        .join("data")
}

/// Open the backend every command runs against.
pub fn backend() -> Result<LocalBackend> {
    LocalBackend::open(store_dir())
}

/// Parse a human size: `1024`, `64KB`, `128MB`, `2GB`, `1MiB`.
pub fn parse_size(s: &str) -> Result<u64> {
    let s = s.trim();
    let (num, mult) = match s.to_ascii_uppercase() {
        v if v.ends_with("KIB") || v.ends_with("KB") => {
            (strip(s, 2 + usize::from(v.ends_with("KIB"))), 1024)
        }
        v if v.ends_with("MIB") || v.ends_with("MB") => {
            (strip(s, 2 + usize::from(v.ends_with("MIB"))), 1024 * 1024)
        }
        v if v.ends_with("GIB") || v.ends_with("GB") => {
            (strip(s, 2 + usize::from(v.ends_with("GIB"))), 1024 * 1024 * 1024)
        }
        v if v.ends_with('B') => (strip(s, 1), 1),
        _ => (s, 1),
    };
    num.trim()
        .parse::<u64>()
        .map(|n| n * mult)
        .map_err(|_| mammoth_core::Error::Config(format!("not a size: {s} (try 128MB)")))
}

fn strip(s: &str, n: usize) -> &str {
    &s[..s.len() - n]
}
```

**`MAMMOTH_HOME` exists so tests and demos do not scribble on your real store.**
You will use it in a moment.

## Step 3 · The filesystem commands

Create `crates/mammoth-cli/src/commands/fs.rs`:

```rust
//! `ls`, `put`, `cat`, `stat` — the filesystem verbs.

use std::path::Path;

use bytes::Bytes;
use futures_util::StreamExt;
use mammoth_core::backend::ByteStream;
use mammoth_core::types::FileStatus;
use mammoth_core::{Backend, Result};

use crate::cli::OutputFormat;
use crate::output::{emit, Render};

/// A directory listing.
pub struct Listing {
    entries: Vec<FileStatus>,
    long: bool,
}

impl Render for Listing {
    fn to_table(&self) -> comfy_table::Table {
        let mut t = comfy_table::Table::new();
        t.load_preset(comfy_table::presets::NOTHING);
        if self.long {
            t.set_header(vec!["PERM", "OWNER", "SIZE", "REPL", "BLOCKS", "NAME"]);
        }
        for e in &self.entries {
            let name = e.path.file_name().unwrap_or_default().to_string_lossy().to_string();
            let name = if e.is_dir { format!("{name}/") } else { name };
            if self.long {
                t.add_row(vec![
                    perms(e),
                    e.owner.clone(),
                    if e.is_dir { "—".into() } else { human(e.len) },
                    e.replication.map(|r| r.to_string()).unwrap_or_else(|| "—".into()),
                    if e.is_dir {
                        "—".into()
                    } else if e.inlined {
                        "inline".into()
                    } else {
                        e.blocks.to_string()
                    },
                    name,
                ]);
            } else {
                t.add_row(vec![name]);
            }
        }
        t
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.entries).unwrap_or(serde_json::Value::Null)
    }
}

/// One path's metadata.
pub struct Stat(FileStatus);

impl Render for Stat {
    fn to_table(&self) -> comfy_table::Table {
        let s = &self.0;
        let mut t = comfy_table::Table::new();
        t.load_preset(comfy_table::presets::NOTHING);
        t.add_row(vec!["path".to_string(), s.path.display().to_string()]);
        t.add_row(vec!["size".to_string(), format!("{} ({} bytes)", human(s.len), s.len)]);
        t.add_row(vec!["block size".to_string(), human(s.block_size)]);
        t.add_row(vec![
            "blocks".to_string(),
            if s.inlined { "0 (inlined)".into() } else { s.blocks.to_string() },
        ]);
        t.add_row(vec![
            "replication".to_string(),
            s.replication.map(|r| r.to_string()).unwrap_or_else(|| "—".into()),
        ]);
        t.add_row(vec!["owner".to_string(), format!("{}:{} {}", s.owner, s.group, perms(s))]);
        t
    }

    fn to_json(&self) -> serde_json::Value {
        serde_json::to_value(&self.0).unwrap_or(serde_json::Value::Null)
    }
}

/// `mammoth ls`
pub async fn ls(be: &dyn Backend, path: &Path, long: bool, fmt: OutputFormat) -> Result<()> {
    let entries = be.list(path).await?;
    emit(&Listing { entries, long }, fmt);
    Ok(())
}

/// `mammoth stat`
pub async fn stat(be: &dyn Backend, path: &Path, fmt: OutputFormat) -> Result<()> {
    let s = be.stat(path).await?;
    emit(&Stat(s), fmt);
    Ok(())
}

/// `mammoth put`
pub async fn put(be: &dyn Backend, src: &Path, dst: &Path) -> Result<()> {
    let bytes = std::fs::read(src)?;
    let len = bytes.len() as u64;
    let stream: ByteStream =
        Box::pin(futures_util::stream::once(async move { Ok(Bytes::from(bytes)) }));

    be.write(dst, stream).await?;

    let s = be.stat(dst).await?;
    let shape = if s.inlined {
        "inlined (no blocks allocated)".to_string()
    } else {
        format!("{} blocks · replication {}", s.blocks, s.replication.unwrap_or(0))
    };
    println!("  ✔ {}   {} · {}", dst.display(), human(len), shape);
    Ok(())
}

/// `mammoth cat`
pub async fn cat(be: &dyn Backend, path: &Path) -> Result<()> {
    use std::io::Write;
    let mut stream = be.read(path, 0..u64::MAX).await?;
    let mut out = std::io::stdout().lock();
    while let Some(chunk) = stream.next().await {
        out.write_all(&chunk?)?;
    }
    Ok(())
}

/// `-rw-r--r--` from mode bits.
fn perms(s: &FileStatus) -> String {
    let kind = if s.is_dir { 'd' } else { '-' };
    let bit = |shift: u32, c: char| if (s.mode >> shift) & 1 == 1 { c } else { '-' };
    format!(
        "{kind}{}{}{}{}{}{}{}{}{}",
        bit(8, 'r'),
        bit(7, 'w'),
        bit(6, 'x'),
        bit(5, 'r'),
        bit(4, 'w'),
        bit(3, 'x'),
        bit(2, 'r'),
        bit(1, 'w'),
        bit(0, 'x')
    )
}

/// 367001600 -> "350.0 MB"
pub fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut v = bytes as f64;
    let mut u = 0;
    while v >= 1024.0 && u < UNITS.len() - 1 {
        v /= 1024.0;
        u += 1;
    }
    format!("{v:.1} {}", UNITS[u])
}
```

### The one idea worth pausing on

Look at the signature of every command:

```rust
pub async fn ls(be: &dyn Backend, path: &Path, long: bool, fmt: OutputFormat) -> Result<()>
```

**`&dyn Backend`, not `&LocalBackend`.** These four functions will work
unchanged against a real thousand-node cluster. That is not a hope, it is a
compile-time guarantee: they literally cannot call anything outside the trait.

Notice too that `cat` writes to stdout in a loop over stream chunks rather than
collecting first. `LocalBackend` only ever hands back one chunk today, but the
command is already shaped correctly for a backend that streams 10 TB.

## Step 4 · Dispatch

`main.rs` has to become async, because `Backend` methods are async. Change the
top:

```rust
#[tokio::main]
async fn main() -> std::process::ExitCode {
    let cli = cli::Cli::parse();
    match run(cli).await {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            output::print_error(&e);
            std::process::ExitCode::FAILURE
        }
    }
}
```

and replace `run`:

```rust
async fn run(cli: cli::Cli) -> mammoth_core::Result<()> {
    let fmt = cli.output;
    let be = commands::backend()?;

    match cli.command {
        cli::Command::Ls(a) => commands::fs::ls(&be, &a.path, a.long, fmt).await,
        cli::Command::Stat(a) => commands::fs::stat(&be, &a.path, fmt).await,
        cli::Command::Put(a) => {
            // Per-file overrides rebuild the backend with different settings.
            let mut be = be;
            if let Some(bs) = &a.block_size {
                be = be.with_block_size(commands::parse_size(bs)?);
            }
            if let Some(it) = &a.inline_threshold {
                be = be.with_inline_threshold(commands::parse_size(it)?);
            }
            commands::fs::put(&be, &a.src, &a.dst).await
        }
        cli::Command::Cat(a) => commands::fs::cat(&be, &a.path).await,
        _ => unimplemented!("command dispatch — see docs/ROADMAP.md, milestone M1"),
    }
}
```

Finally, add the two new dependencies to `crates/mammoth-cli/Cargo.toml`:

```toml
bytes        = { workspace = true }
futures-util = { workspace = true }
```

```bash
cargo build -p mammoth-cli
```

## Check it works

Use a throwaway store so you do not litter your home directory:

```bash
export MAMMOTH_HOME=/tmp/mammoth-demo && rm -rf "$MAMMOTH_HOME"
```

```bash
echo "hello from mammoth" > /tmp/hello.txt
head -c 350000 /dev/urandom > /tmp/sales.csv
```

A small file gets inlined:

```bash
./target/debug/mammoth put /tmp/hello.txt /data/hello.txt
```

```
  ✔ /data/hello.txt   19 B · inlined (no blocks allocated)
```

A bigger one gets blocks. The defaults are 128 MB blocks and a 1 MiB inline
threshold, so a 350 KB file would *also* be inlined — override both to see the
block layer work without generating half a gigabyte of test data:

```bash
./target/debug/mammoth put /tmp/sales.csv /data/sales.csv --block-size 128KB --inline-threshold 4KB
```

```
  ✔ /data/sales.csv   341.8 KB · 3 blocks · replication 3
```

```bash
./target/debug/mammoth ls /data --long
```

```
 PERM        OWNER  SIZE      REPL  BLOCKS  NAME      
 -rw-r--r--  local  19 B      3     inline  hello.txt 
 -rw-r--r--  local  341.8 KB  3     3       sales.csv 
```

```bash
./target/debug/mammoth stat /data/sales.csv
```

```
 path         /data/sales.csv         
 size         341.8 KB (350000 bytes) 
 block size   128.0 KB                
 blocks       3                       
 replication  3                       
 owner        local:local -rw-r--r--  
```

```bash
./target/debug/mammoth cat /data/hello.txt
```

```
hello from mammoth
```

### And now the payoff

Everything above printed a table because your terminal is a terminal. Pipe it
somewhere and the *same command* emits JSON:

```bash
./target/debug/mammoth stat /data/sales.csv | python3 -c "import json,sys; d=json.load(sys.stdin); print(d['blocks'],'blocks,',d['len'],'bytes')"
```

```
3 blocks, 350000 bytes
```

You never wrote an `if json` branch. You implemented `Render` twice and `emit`
did the rest.

### Errors still teach

```bash
./target/debug/mammoth stat /data/nope.txt
```

```
  error[E0101]: no such path: /data/nope.txt

  docs: https://projectorcha.github.io/Mammoth/errors/E0101
```

```bash
./target/debug/mammoth put /tmp/hello.txt /x --block-size banana
```

```
  error[E0001]: config error: not a size: banana (try 128MB)

  what you can do:
    · validate config:     mammoth config validate

  docs: https://projectorcha.github.io/Mammoth/errors/E0001
```

Exit code is 1 in both cases, so shell scripts and CI can branch on it.

## Commit it

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
git add -A && git commit -m "feat(cli): wire ls, put, cat and stat to LocalBackend"
```

## Exercises

These are small and each one teaches something the chapters skipped.

1. **`mammoth mkdir`** — `LocalBackend` has no `mkdir`, because `write` creates
   parent directories on demand. Add one anyway. What should it do if the
   directory already exists?
2. **`mammoth rm`** — the backend method already exists. Wire it up, and make
   `--recursive` work. Try removing a non-empty directory without the flag and
   check the error reads well.
3. **`mammoth df`** — one call to `cluster_report`, one small `Render` impl.
4. **`mammoth head` / `tail`** — this is what the `range` argument on `read` was
   for. `head -c 100` should read exactly 100 bytes, not the whole file.

## If it went wrong

**`` `main` function is not allowed to be `async` ``** — you added `async` but
not `#[tokio::main]`. Both, in that order.

**`error[E0277]: LocalBackend cannot be shared between threads safely`** — you
put something non-`Sync` in the struct. Everything in chapter 5's version is
fine, so check what you added.

**`the size for values of type dyn Backend cannot be known`** — you wrote
`be: dyn Backend` instead of `be: &dyn Backend`. Trait objects always need a
reference or a `Box`.

**`ls` prints JSON when you expected a table** — you are piping, or your
terminal is not detected as one. Force it with `--output table`. This is the
`auto` behaviour working correctly; it is also why the outputs above use
explicit commands.

**Everything gets inlined and you never see blocks** — your file is under the
1 MiB inline threshold. Pass `--inline-threshold 4KB` as shown.

**`put` succeeds but `ls` shows nothing** — you wrote to a different store than
you are listing. Check `MAMMOTH_HOME` is exported in the *same* shell.

**`error: unexpected argument '--long' found`** — you added the field to
`LsArgs` but the variant still reads `Ls,` rather than `Ls(LsArgs)`.

---

**Next:** [Chapter 8 — `viz blocks`, seeing your data](08-viz-blocks.md)
