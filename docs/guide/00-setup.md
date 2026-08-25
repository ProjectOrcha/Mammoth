# Chapter 0 — Set up your machine

**What you'll build:** a working copy of Mammoth that compiles and runs.

**Time:** about 30 minutes, most of it waiting for downloads.

---

## What you need

| Tool | Why | Needed by |
| --- | --- | --- |
| **Rust** 1.82+ | the whole engine is written in it | everyone |
| **Git** | version control | everyone |
| **Node.js** 20+ | only for the web UI and the docs site | chapters 9–10 |
| A code editor | VS Code is the easiest start | everyone |

## 1 · Install Rust

Rust is installed through `rustup`, which manages Rust versions for you.

**macOS and Linux:**

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Press Enter to accept the defaults. Then **close and reopen your terminal**, or:

```bash
source "$HOME/.cargo/env"
```

**Windows:** download and run [rustup-init.exe](https://win.rustup.rs/x86_64).
When it asks about Visual Studio build tools, say yes — Rust needs a C linker
and that is where Windows keeps it.

Check it worked:

```bash
rustc --version
cargo --version
```

You should see something like `rustc 1.82.0` or newer. If you see
`command not found`, your terminal has not picked up the new `PATH` — close it
and open a new one.

### What are `rustc` and `cargo`?

- `rustc` is the compiler. You will almost never call it directly.
- `cargo` is the build tool, package manager, and test runner. This is the one
  you use. `cargo build`, `cargo test`, `cargo run`.

## 2 · Install Git

**macOS:** `git` comes with the Xcode command line tools:

```bash
xcode-select --install
```

**Linux (Debian/Ubuntu):**

```bash
sudo apt update && sudo apt install git
```

**Windows:** download [Git for Windows](https://git-scm.com/download/win).

Then tell Git who you are — this is what shows up on your commits:

```bash
git config --global user.name "Your Name"
git config --global user.email "you@example.com"
```

## 3 · Install Node.js (chapters 9–10 only)

Skip this for now if you like; you can come back to it.

Get the **LTS** version from [nodejs.org](https://nodejs.org/), or on macOS:

```bash
brew install node
```

```bash
node --version   # should be v20 or higher
npm --version
```

## 4 · Get the code

```bash
git clone https://github.com/ProjectOrcha/Mammoth.git
cd Mammoth
```

Everything from here on assumes you are inside the `Mammoth` directory. If a
command does not work, run `pwd` first and check.

## 5 · Build it

```bash
cargo build
```

**The first build takes 2–5 minutes** and prints a lot of lines. That is normal
— Cargo is downloading and compiling every dependency. Later builds take a few
seconds because it caches all of that.

You are done when you see:

```
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2m 14s
```

## 6 · Run it

```bash
./target/debug/mammoth --help
```

You should see the full command list — `ls`, `put`, `get`, `viz`, `top`, and the
rest. Try a subcommand:

```bash
./target/debug/mammoth viz --help
```

**Most of these commands do not do anything yet.** The command *tree* is built,
the command *bodies* are not. That is what you are here to build.

If you run one, you will get:

```
thread 'main' panicked at crates/mammoth-cli/src/main.rs:26:5:
not implemented: command dispatch — see docs/ROADMAP.md, milestone M1
```

That panic is deliberate. It is the project telling you where to start.

## 7 · Set up your editor

**VS Code** is the path of least resistance. Install it, then install one
extension: **rust-analyzer**. It gives you red squiggles as you type, jump-to-
definition, and inline type hints — which matter a lot when you are learning.

Two more that help:

- **Even Better TOML** — syntax highlighting for `Cargo.toml`
- **Svelte for VS Code** — for chapter 9

Open the project:

```bash
code .
```

The first time you open a Rust project, rust-analyzer spends a minute indexing.
The status bar tells you when it is done.

## Check it works

Run all four of these. All four must succeed.

```bash
cargo build --workspace
```

```bash
cargo test --workspace
```

```bash
cargo fmt --all --check
```

```bash
./target/debug/mammoth --version
```

The last one prints `mammoth 0.1.0`.

## If it went wrong

**`linker 'cc' not found`** (Linux) — you need a C toolchain:

```bash
sudo apt install build-essential
```

**`error: linking with 'link.exe' failed`** (Windows) — the Visual Studio build
tools did not install. Re-run `rustup-init.exe` and accept the prompt, or
install [Build Tools for Visual Studio](https://visualstudio.microsoft.com/downloads/)
with the "Desktop development with C++" workload.

**`cargo: command not found`** — your shell has not picked up `~/.cargo/bin`.
Close and reopen the terminal. If that fails, add it by hand:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc && source ~/.zshrc
```

**The build is extremely slow or fails to download** — you may be behind a proxy
or a firewall. Set `CARGO_HTTP_CHECK_REVOKE=false` on Windows corporate
networks, or configure a mirror in `~/.cargo/config.toml`.

**`cargo fmt --all --check` fails and you have not written any code** — someone
committed unformatted code. Fix it with `cargo fmt --all` and mention it.

---

**Next:** [Chapter 1 — The 30-minute Rust you actually need](01-rust-you-need.md)
