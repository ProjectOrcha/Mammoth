# Chapter 3 — A small Git and review workflow

**What you will practice:** make a change on a branch, check it, open a pull
request, respond to review, and update your local copy after merge.

A **commit** is a saved change. A **branch** is a line of commits. A **remote** is
a named repository URL. A **pull request (PR)** asks others to review a branch
before it joins the shared `main` branch. A PR is not the same as `git pull`:
`git pull` updates your local checkout.

Your four-person team assigns work using [TEAM-PLAN.md](TEAM-PLAN.md).
Contributors without repository write access use
[EXTERNAL-CONTRIBUTORS.md](EXTERNAL-CONTRIBUTORS.md) for the fork/remotes setup.

## 1. Start from a clean, current main

Run at the repository root. First check for unfinished changes:

```bash
git status
git switch main
git pull --ff-only origin main
git switch -c docs/explain-file-browser
```

If `git status` shows work, commit it on its own branch or deliberately stash
it before switching. Do not discard changes just to follow these instructions.
`--ff-only` stops if your local and remote histories diverge; ask for help
instead of resetting them blindly.

Use `docs/` for documentation, `fix/` for bugs, `feat/` for features, and
`chore/` for tooling. Example: `fix/file-browser-race`.

## 2. Make one small change

For a practice PR, clarify one sentence in a guide and verify it by running the
command it describes. For code, include the symptom, expected result and
regression check. Avoid everyone editing the authors line in `Cargo.toml` as a
first task: it creates needless shared-file conflicts.

## 3. Run checks for the area you changed

Rust, from the root:

```bash
cargo fmt --all
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --locked
```

Dashboard, from `ui/`:

```bash
npm run check
npm test
npm run build
```

Public docs, from `web/`:

```bash
npm run build
```

If you changed CLI arguments/help, run `cargo xtask docs` from the root and
include the generated reference in the PR. If you only changed a guide, verify
its commands and links; CI still runs the repository checks.

The [contribution guide](../../CONTRIBUTING.md) lists CI's additional checks.
Check failures are useful information; a passing build alone does not test
storage correctness or browser behavior.

## 4. Inspect, commit and push

For this example, assume you edited `docs/guide/CODE-MAP.md`:

```bash
git diff
git add docs/guide/CODE-MAP.md
git diff --cached
git commit -m "docs(guide): explain file browser request flow"
git push -u origin docs/explain-file-browser
```

Stage only intended files. `git diff --cached` shows exactly what the commit
will contain. Never commit credentials, `.env` secrets or generated build
folders. A descriptive commit message explains the result of the change.

Open a PR targeting `ProjectOrcha/Mammoth:main`. Include:

```markdown
What changed: explained how Browse chooses a directory or file view.
Why: new contributors could not find where the request starts.
Checked: followed the links and opened the demo Files page.
Issue: #123 (only if there is a real issue to link).
Limitations: gateway integration is still planned.
```

Use a draft PR while the work is incomplete. Ask your assigned reviewer, and
respond to questions with either a code change or a short explanation. Push
follow-up commits to the same branch; the PR updates automatically.

## 5. Bring in main and resolve conflicts

For beginners, merging `main` into a shared feature branch avoids rewriting
history. Commit your own changes first:

```bash
git fetch origin
git merge origin/main
```

If Git reports a conflict, open each reported file. Conflict markers look like:

```text
  <<<<<<< HEAD
  Your branch's text
  =======
  Main's text
  >>>>>>> origin/main
```

Edit the final content you want to keep, remove the markers, run the relevant
checks, then `git add path/to/resolved-file` and `git commit`. If you are unsure,
`git merge --abort` cancels this merge; it does not delete your earlier commits.
Ask the other author about intent before choosing one side.

## 6. Review and merge

At least one teammate checks:

- The requested behavior and its error cases make sense.
- They can explain the code and any new types.
- Tests exercise the bug or feature, not merely the implementation details.
- UI changes work with a keyboard and at a narrow viewport.
- Docs say whether the behavior works now or is planned.
- Required CI checks pass; no secret or unrelated file is included.

The author does not approve their own work. Merge using the repository's chosen
method. Do not push directly to a protected `main`.

After a confirmed merge:

```bash
git switch main
git pull --ff-only origin main
```

You can then delete the feature branch if it has been merged. After a squash
merge, Git may refuse `git branch -d` because the commit IDs changed. Keeping
the branch is harmless; check that all work is on main before removing it.

## Recover from a bad merge

Create a new branch from updated main, then `git revert COMMIT_SHA` using the
actual bad commit ID. This creates an undo commit. Open a repair PR and ask for
quick review. Merge commits require choosing the correct parent, so ask an
experienced reviewer before reverting one. Never reset or force-push main.

## Done when

- [ ] I can name my working directory and current branch.
- [ ] I understand the difference between commit, push, pull and PR.
- [ ] My small PR includes verification and has an independent reviewer.
- [ ] I know how to update my checkout after merge.

Next: [chapter 4 — the Backend trait](04-the-backend-trait.md).
