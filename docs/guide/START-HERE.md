# Your first hour in Mammoth

This page is for someone who has never contributed to a Rust project. You can
make a useful change without understanding the whole distributed system.

## 1. Know what works today

On `AI_coded`, the local application works with persistent files, replicated
blocks, a CLI, a live dashboard, and a development S3 endpoint. Use the
[current README](../../README.md) to build and run it, and
[implementation status](../IMPLEMENTATION-STATUS.md) for boundaries and checks.

The numbered guide chapters explain how the pieces fit together. The later
multi-machine architecture, Raft, distributed compute and HDFS/EC milestones
remain roadmap work. The GFS teaching simulation remains separate from the service.

## 2. Set up once

Follow [chapter 0](00-setup.md) to install Git, current stable Rust, and Node.js
**22.x**. Rust **1.85** is the minimum supported version for the locked
dependencies. `.nvmrc` records the Node version if you use nvm.

A **terminal** runs commands. A **working directory** is the folder those
commands run inside. These examples assume a Bash-like terminal (macOS Terminal,
Linux, or Git Bash/WSL on Windows). `cargo` and `npm` commands also work in
PowerShell; shell-specific commands such as `export` do not.

From the folder where you want to keep projects:

```bash
git clone --branch AI_coded https://github.com/ProjectOrcha/Mammoth.git
cd Mammoth
cargo build --workspace --locked
cargo run -p mammoth-cli -- --help
cargo run -p mammoth-cli -- --version
```

Expected: a successful build, a command list, then `mammoth 0.1.0`.

Read `cargo run -p mammoth-cli -- --help` as:

| Part | Meaning |
| --- | --- |
| `cargo run` | Compile and run a Rust program |
| `-p mammoth-cli` | Select this package in the workspace |
| `--` | Stop Cargo's arguments; pass the rest to the program |
| `--help` | Ask Mammoth to show its help |

Use `--help` to explore. Build the UI, rebuild the binary, then run `quickstart`
to start the local filesystem and dashboard. Separate `master` and `worker` roles
still report an explicit unsupported-operation error.

## 3. See something working

Run a small Rust example from the repository root:

```bash
cargo run -q -p mammoth-parts --example 01-ownership
cargo run -q -p mammoth-parts --example 13-block-matrix
```

The first explains ownership. The second prints a block placement table.
Open their source in [the examples folder](../../examples/parts/examples/) and
compare each line of output with the code that produced it.

For the dashboard, open a second terminal:

```bash
cd /path/to/Mammoth/ui
npm ci
npm run dev
```

Replace `/path/to/Mammoth` with your checkout location. Open
<http://localhost:5173>. Expect a **Demo workspace** banner and simulated workers.
Click Files → data → a file to inspect its blocks. These are generated examples,
not files on your disk. Stop the dev server with Ctrl+C.

`npm ci` installs exactly the versions in `package-lock.json`. `npm run dev`
starts the development server. `npm run build` creates static files for serving;
it does not publish a website.

The public docs are a different app:

```bash
cd /path/to/Mammoth/web
npm ci
npm run dev
```

Open <http://localhost:4321>. `ui/` is the dashboard; `web/` is its documentation.

## 4. Learn five Rust ideas in small steps

Read [chapter 1](01-rust-you-need.md), but run one example at a time:

| Idea | Plain meaning | Run from the root |
| --- | --- | --- |
| Ownership and borrowing | One owner; other code can temporarily borrow a value | `cargo run -p mammoth-parts --example 01-ownership` |
| `Result` and `?` | A function can succeed or return an error to its caller | `cargo run -p mammoth-parts --example 02-result-and-errors` |
| Struct / enum / match | A record, a set of alternatives, and handling each alternative | `cargo run -p mammoth-parts --example 03-structs-enums-match` |
| Trait | A promise about which methods a type provides | `cargo run -p mammoth-parts --example 04-traits-and-dyn` |
| Async | Let other work proceed while waiting for I/O | `cargo run -p mammoth-parts --example 05-async-and-streams` |

This complete Rust example can be pasted into the Rust Playground or into
`src/main.rs` of a separate `cargo new rust-warmup` project:

```rust
fn label(name: &str) -> String {
    format!("worker: {name}")
}

fn main() {
    let name = String::from("w1"); // name owns the allocated text
    println!("{}", label(&name)); // &name borrows it
    println!("{name}");           // the owner can still use it
}
```

Expected output:

```text
worker: w1
w1
```

`&str` means borrowed text; `String` means owned text. `-> String` declares the
return type. The last expression without a semicolon is the return value.
Do not paste this `main` into `mammoth-core`: a library is not an executable.

## 5. Trace one real feature before editing

Use [the code map](CODE-MAP.md) to follow a CLI request and a dashboard request.
Jump to a definition in your editor, then find its callers. Ask: which value
enters this function, what comes out, and who handles an error?

For a first frontend change, edit the Files page's descriptive sentence in
`ui/src/routes/files/+page.svelte`, check the browser, then revert or refine the
wording before making a PR. For manual Rust exercises, use `main` as described in the [branch guide](BRANCHES.md);
chapter 2’s `version` command is already implemented in this reference branch.
For a docs change, clarify one confusing paragraph and try every command in it.

## 6. Choose the right contribution workflow

- **You and your three teammates:** [the four-person team plan](TEAM-PLAN.md).
- **Outside contributors anywhere in the world:** [the external contributor guide](EXTERNAL-CONTRIBUTORS.md).
- **The Git commands both groups use:** [chapter 3](03-team-workflow.md).

## When something fails

| Symptom | First thing to do |
| --- | --- |
| `cargo: command not found` | Reopen the terminal after installing rustup |
| `linker ... not found` | Install the platform build tools from chapter 0 |
| `npm ... package.json ... ENOENT` | Run `pwd`; enter `ui/` or `web/` before npm commands |
| Wrong Node version | Use Node 22; with nvm run `nvm install` then `nvm use` at the root |
| `not implemented yet` / `E0002` | Check the status table; choose a working example or implement that chapter |
| Vite logs a proxy connection error | In development with no gateway, one initial failed probe is expected; look for the demo banner |
| Production dashboard says API error | Connect a compatible gateway; for an intentional demo build set `VITE_DATA_SOURCE=demo` before building |
| Rust reports a moved value | Read the first compiler error and run example 01 |
| A command appears to hang | Dev servers run until Ctrl+C; compiler downloads may take several minutes |

When asking for help, include the command, working directory, full error text,
OS, `rustc --version`, and `node --version`. Remove tokens or passwords from logs.

The docs website requires Node **22.12.0 or newer within the 22.x line**.
Use the latest Node 22 patch release to satisfy both frontend apps.
