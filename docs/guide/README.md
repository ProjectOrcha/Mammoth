# The Mammoth build guide

A step-by-step guide to building Mammoth from the scaffold in this repository,
written for people who have not built a distributed system before.

You do not need to know Rust well. You do not need to know Hadoop at all. You do
need to be willing to type things and read error messages.

## How this guide works

Each chapter is a **complete, working step**. You start it with a repository
that builds, and you end it with a repository that still builds and does one
more thing than it did before. Nothing is left half-finished between chapters.

Every chapter has the same shape:

- **What you'll build** — one sentence
- **Why it matters** — so you are not just typing
- **The code** — complete, not fragments
- **Check it works** — an exact command and its exact expected output
- **If it went wrong** — the three errors people actually hit
- **Commit it** — the commit message to use

> **Verified code.** Every Rust block in chapters 1, 2 and 4–8 was assembled
> exactly as written here, compiled, and run — `cargo clippy -- -D warnings`
> clean, tests passing — before this guide was published. The terminal output
> shown is real output, captured from those runs, not illustration. Chapter 10's
> build steps and expected output were verified the same way.
>
> The exception is **chapter 9**, whose Rust and Svelte are written to the same
> standard but were not machine-verified end to end. Treat its code as a solid
> starting point rather than a guarantee.
>
> So if you type something in and it does not work, the likely cause is a typo
> or a skipped step. Read the "If it went wrong" section at the end of each
> chapter — it lists the errors people actually hit.

## The chapters

### Part 1 — Getting started

| # | Chapter | Time |
| --- | --- | --- |
| 0 | [Set up your machine](00-setup.md) | 30 min |
| 1 | [The 30-minute Rust you actually need](01-rust-you-need.md) | 30 min |
| 2 | [Your first change](02-first-change.md) | 20 min |
| 3 | [How the team works together](03-team-workflow.md) | 20 min |

### Part 2 — Building the storage engine

| # | Chapter | Time |
| --- | --- | --- |
| 4 | [Understanding the Backend trait](04-the-backend-trait.md) | 30 min |
| 5 | [LocalBackend, part 1 — layout, list, stat](05-localbackend-part-1.md) | 2 h |
| 6 | [LocalBackend, part 2 — write, read, blocks](06-localbackend-part-2.md) | 3 h |
| 7 | [Wiring up the CLI](07-wiring-the-cli.md) | 2 h |
| 8 | [`viz blocks` — seeing your data](08-viz-blocks.md) | 2 h |

### Part 3 — Shipping it

| # | Chapter | Time |
| --- | --- | --- |
| 9 | [The web UI and the gateway](09-web-ui.md) | 4 h |
| 10 | [Publishing the docs to GitHub Pages](10-github-pages.md) | 45 min |
| 11 | [Where to go next](11-what-next.md) | 15 min |

Chapters 0–8 get you to **milestone M1–M2** in the [roadmap](../ROADMAP.md):
a working single-machine filesystem with the visualization that makes this
project worth building. Chapter 10 stands alone — you can do it on day one, and
you probably should, because a live docs site makes the project feel real.

## The one rule

**Never commit code that does not build.** Before every commit:

```bash
cargo build --workspace
cargo test --workspace
```

If either fails, fix it before committing. A broken `main` branch blocks
everyone on the team, and un-breaking it is always harder than not breaking it.

## If you get stuck

1. **Read the error message.** All of it. Rust's compiler errors are unusually
   good — they usually tell you the fix.
2. **Run `cargo build` again.** Sometimes you fixed it and did not notice.
3. **Check you are in the right directory.** `pwd` should end in `/Mammoth`.
4. **Ask.** Open a [discussion](https://github.com/ProjectOrcha/Mammoth/discussions)
   or an issue. Paste the *full* error, not a screenshot of part of it.
