# The checklists

Everything in this file is meant to be **copied and ticked**. Paste a checklist
into a GitHub issue, a PR description, or a scratch file, and work down it.

Nothing here is new information — it is the rules from chapters 0, 3 and 4
turned into something you can actually run down at 6pm when you are tired and
about to push something broken.

**Jump to:**
[Day one](#day-one--each-person-once) ·
[Before every commit](#before-every-commit-the-30-second-one) ·
[PR author](#opening-a-pr) ·
[PR reviewer](#reviewing-someone-elses-pr) ·
[Chapter done](#is-this-chapter-actually-done) ·
[Whole guide](#the-whole-guide--progress-tracker) ·
[Stuck](#i-am-stuck)

---

## Day one — each person, once

Every one of the three of you does this on the same day, before anyone writes
real code. It takes a morning and it gets all the awkward parts — toolchains,
permissions, the merge button — out of the way while nothing is at stake.

```markdown
### Setup (chapter 0)
- [ ] Rust installed — `rustc --version` prints 1.82 or newer
- [ ] Git installed and `git config --global user.name` / `user.email` set
- [ ] Node.js 20+ installed (skip if you are not on chapters 9–10)
- [ ] Repo cloned, and `pwd` ends in `/Mammoth`
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` succeeds
- [ ] `cargo fmt --all --check` succeeds
- [ ] `./target/debug/mammoth --version` prints `mammoth 0.1.0`
- [ ] VS Code + rust-analyzer installed, project opened, indexing finished

### The concepts (before chapter 4)
- [ ] Read [CONCEPTS.md](CONCEPTS.md) — 40 minutes, no code
- [ ] Answered its eight "check you understand it" questions **out loud, as a team**
- [ ] Nobody is still fuzzy on why three replicas, or why they must span racks

### Rust warm-up (chapter 1)
- [ ] Read chapter 1's six sections
- [ ] Built and ran the `rust-warmup` scratch project
- [ ] Saw the three-node bar chart print
- [ ] Ran `cargo build -p mammoth-parts --examples` — all sixteen compile
- [ ] Ran examples 01 (ownership) and 04 (traits) and read the source
- [ ] Bookmarked [the Rust reference](RUST-REFERENCE.md) and its
      [error decoder](RUST-REFERENCE.md#the-compiler-error-decoder)
- [ ] Deleted the scratch project

### First real change (chapter 2)
- [ ] `mammoth version` works with `--output table`
- [ ] `mammoth version` works with `--output json`
- [ ] `mammoth version | cat` prints JSON — you understand *why*

### Process (chapter 3)
- [ ] `mmcheck` shell function added to `~/.zshrc` or `~/.bashrc`
- [ ] Pushed the `chore/add-me-to-authors` branch
- [ ] A teammate reviewed and approved it
- [ ] Merged it, pulled `main`, deleted the local branch
- [ ] I have push access and can open a PR without asking anyone

### Shared understanding (chapter 4)
- [ ] Read chapter 4 **together, out loud, as a team**
- [ ] Everyone can answer all five questions in "Check you understand it"
- [ ] We agree on the `Backend` trait as written — or we have opened an issue
      about the one thing we want to change, and settled it *before* chapter 5
```

If everyone finishes this list on day one, the rest of the project is downhill.

---

## Before every commit (the 30-second one)

This is the whole of "never break `main`", and it is three commands.

```markdown
- [ ] `cargo fmt --all`
- [ ] `cargo clippy --workspace --all-targets -- -D warnings`
- [ ] `cargo test --workspace`
```

Do not type them individually. Put this in `~/.zshrc` once:

```bash
mmcheck() {
  cargo fmt --all &&
  cargo clippy --workspace --all-targets -- -D warnings &&
  cargo test --workspace &&
  echo "✔ ready to commit"
}
```

Then it is one word: `mmcheck`. If it does not print `✔ ready to commit`, you
are not ready to commit.

### Make it impossible to forget

Better than remembering: have Git run it for you. Create
`.git/hooks/pre-commit`, make it executable, and it runs on every `git commit`.

```bash
cat > .git/hooks/pre-commit <<'EOF'
#!/usr/bin/env bash
set -e
echo "→ fmt"    && cargo fmt --all --check
echo "→ clippy" && cargo clippy --workspace --all-targets -- -D warnings
echo "→ tests"  && cargo test --workspace
EOF
chmod +x .git/hooks/pre-commit
```

Two things to know about it:

- Git hooks are **not** committed to the repo, so each of the three of you has
  to run those commands once, on your own machine.
- If you ever genuinely need to skip it, `git commit --no-verify`. Use that
  roughly never.

---

## Opening a PR

Paste this into the PR description. It tells your reviewer what you already
checked, so they can spend their attention on the code instead.

````markdown
### What this does
<one sentence>

### Chapter
Chapter <n> — <title>

### Checklist
- [ ] Branch is named `feat/…`, `fix/…`, `docs/…`, `refactor/…` or `chore/…`
- [ ] Commit messages are `type(scope): imperative description`
- [ ] `mmcheck` passes locally
- [ ] The chapter's "Check it works" commands produce the output the chapter shows
- [ ] New public items in `mammoth-core` have `///` doc comments
- [ ] No `.unwrap()` on any path a user's input can reach
- [ ] Tests cover the unhappy path (empty dir, missing file, zero bytes, spaces in path)
- [ ] One change only — this PR does not also sneak in a refactor
- [ ] CI is green

### How to check it yourself
```bash
<the exact commands the reviewer should run>
```
````

**Keep PRs small.** A PR that changes one file gets reviewed in ten minutes. A
PR that changes twelve files sits for a week, and on a three-person team a PR
sitting for a week is a third of the project stopped.

---

## Reviewing someone else's PR

You are all learning. Review for *understanding*, not style — `fmt` and `clippy`
already own style, so never spend a comment on it.

```markdown
- [ ] CI is green (do not review a red PR — ask them to fix it first)
- [ ] I pulled the branch and ran the chapter's "Check it works" commands myself
- [ ] I can explain, in my own words, what this code does
- [ ] I checked the unhappy path: empty input, missing file, zero bytes, weird path
- [ ] No `.unwrap()` / `.expect()` on a user-reachable path
- [ ] Public items in `mammoth-core` are documented
- [ ] I would be comfortable debugging this at 2am
```

That last one is the actual bar. If you would not be, say so — kindly, and with
a specific reason.

**How to phrase a finding.** Not "this is wrong". Say what you tried and what
happened:

> I ran `mammoth ls /nope` and got a panic rather than an error message —
> line 84's `.unwrap()` looks like the cause. Could that be a `?` instead?

**Turnaround.** With three people, agree on a rule: **every PR gets a first
response within one working day.** Not necessarily an approval — a response.
Blocking a teammate for two days costs the project more than any bug in the PR.

---

## Is this chapter actually done?

Run this before you tell the team you have finished a chapter.

```markdown
- [ ] Every "Check it works" command in the chapter runs
- [ ] Its output matches what the chapter shows (not just "something printed")
- [ ] `mmcheck` passes
- [ ] I committed with the message the chapter suggests
- [ ] The PR is merged and `main` is green
- [ ] I deleted the branch, locally and on GitHub
- [ ] I can explain the chapter's one core idea to a teammate without notes
- [ ] Anything that surprised me is written down — in the chapter's issue,
      the team notes, or a `docs/` fix
```

That last box matters more than it looks. You are the last person who will ever
read this guide as a beginner. What confused you will confuse the next person,
and right now you are the only one who can say so.

---

## The whole guide — progress tracker

Copy this into a pinned GitHub issue called **"Guide progress"** and let all
three of you tick your own rows. It is the cheapest project-management tool you
will ever set up.

```markdown
## Part 0 — Before any code  (everyone)
- [ ] CONCEPTS · Distributed storage, from zero   ← read together

## Part 1 — Getting started  (everyone, day one)
- [ ] 0 · Set up your machine
- [ ] 1 · The 30-minute Rust you actually need
- [ ] 2 · Your first change
- [ ] 3 · How the team works together
- [ ] 4 · Understanding the Backend trait   ← read together, do not split up

## Part 2 — Building the storage engine
- [ ] 5 · LocalBackend, part 1 — layout, list, stat      (owner: ______)
- [ ] 6 · LocalBackend, part 2 — write, read, blocks     (owner: ______)
- [ ] 7 · Wiring up the CLI                              (owner: ______)
- [ ] 8 · viz blocks — seeing your data                  (owner: ______)
- [ ] 8a · Colour, done properly                         (owner: ______)
- [ ] 8b · mammoth top — the live TUI                     (owner: ______)

## Part 3 — Shipping it
- [ ] 9 · The web UI and the gateway                     (owner: ______)
- [ ] 10 · Publishing the docs to GitHub Pages           (owner: ______)
- [ ] 11 · Where to go next                              (read together)

## Part 4 — Going faster than Hadoop
- [ ] 12 · The four fast paths   ← read together before anyone writes mammoth-master

## Milestones
- [ ] **M1** — `mammoth version` works, dispatch is real       (end of ch. 2)
- [ ] **M2** — a file can be put, listed, read back, drawn     (end of ch. 8)
- [ ] **Presentable** — colour, progress bars, `mammoth top`   (end of ch. 8b)
- [ ] **Demo-able** — someone outside the team can be shown it (end of ch. 9)
```

---

## I am stuck

Work down this list *before* asking. Most of the time you will not get to the
bottom of it.

```markdown
- [ ] I read the **whole** error message, including the `help:` and `note:` lines
- [ ] I checked the chapter's "If it went wrong" section
- [ ] I ran `cargo build` again (sometimes you fixed it and did not notice)
- [ ] `pwd` ends in `/Mammoth`
- [ ] I searched the glossary for the word I did not recognise
- [ ] I re-read the chapter step I am on, from its first line
- [ ] I ran `git diff` and looked at what I actually changed
- [ ] I tried `cargo clean && cargo build` (slow, but it fixes stale-state weirdness)
```

Still stuck after all eight? **Ask, and ask early.** On a three-person team,
someone quietly stuck for four hours is a real cost. When you ask, include:

1. What you were trying to do
2. The exact command you ran
3. The **full** error text — copy-pasted, not a screenshot, not the last line
4. What you already tried from the list above

That format usually gets an answer in minutes, and about a third of the time you
solve it yourself while writing it out.

---

**Back to:** [the guide index](README.md) · [Glossary](GLOSSARY.md) · [Team plan](TEAM-PLAN.md)
