# Contributing from outside the core team

You do not need to join the four-person team or have write access to contribute.
Documentation fixes, reproducible bug reports, tests, accessibility improvements
and small features are welcome. The [first-hour guide](START-HERE.md) explains
what runs today; the [code map](CODE-MAP.md) helps you find the right file.

## 1. Choose a bounded contribution

Read the existing issues and pull requests before starting. For a bug, record
exact reproduction steps and expected behavior. For a new feature, discuss its
scope in an issue before a large implementation. A typo fix can go directly to
a PR. Leave a short issue comment if you plan to work on it so others know.

Good first tasks include clarifying a confusing setup instruction, adding an
empty-state message, reproducing a filename bug, or adding a regression test.
The roadmap contains designs that are not implemented; do not assume a missing
cluster feature is a regression in a working product.

## 2. Fork and clone

A **fork** is your copy of the repository on GitHub. Use GitHub's Fork action
on `ProjectOrcha/Mammoth`, then replace `YOUR_USERNAME` below with your account:

```bash
git clone https://github.com/YOUR_USERNAME/Mammoth.git
cd Mammoth
git remote add upstream https://github.com/ProjectOrcha/Mammoth.git
git remote -v
```

Expected remotes:

| Name | Points to | Purpose |
| --- | --- | --- |
| `origin` | `YOUR_USERNAME/Mammoth` | Push your branches here |
| `upstream` | `ProjectOrcha/Mammoth` | Get the project's latest changes |

Do not push to upstream; your fork is where you have write access. Use GitHub's
normal HTTPS credential helper or your own SSH setup. Never paste a token into
a committed config file or PR description.

## 3. Install and verify

Use stable Rust (minimum 1.85), Git, and Node 22.x for frontend work. Follow
[chapter 0](00-setup.md) for OS-specific prerequisites.

From the repository root:

```bash
cargo build --workspace --locked
cargo test --workspace --locked
cargo run -p mammoth-cli -- --help
```

For `ui/` changes, run in that directory:

```bash
npm ci
npm run check
npm test
npm run dev
```

Expect simulated data in the dev UI. `mammoth serve` is still unimplemented.
For public documentation changes, run `npm ci` and `npm run dev` in `web/`.

## 4. Create a feature branch from upstream

Check `git status` first and save existing work on its own branch. Then:

```bash
git fetch upstream
git switch -c fix/file-browser-message upstream/main
```

Use a descriptive name for your actual change. Keep one logical change per PR.
Avoid formatting unrelated files or upgrading dependencies as part of a small
bug fix. Read any local conventions near the file you are changing.

## 5. Make and verify the change

Explain the symptom first, then change the responsible code. Add a focused test
when fixing a behavior bug. Documentation should include a small example,
expected output, working directory, and whether code is a fragment or runnable.

Run the relevant commands from [CONTRIBUTING.md](../../CONTRIBUTING.md).
For UI changes, check both themes, keyboard operation, small screens, loading
and error states. A before/after screenshot in the PR can help explain layout
changes. For Rust changes, run formatting, Clippy and workspace tests. CLI help
changes also require `cargo xtask docs`.

## 6. Commit only the intended files

Example for a docs contribution:

```bash
git diff
git add docs/guide/START-HERE.md
git diff --cached
git commit -m "docs(guide): clarify initial dashboard setup"
git push -u origin fix/file-browser-message
```

Use your actual file and branch names. Check the staged diff for secrets,
generated build output and unrelated work. The repository ignores common build
folders but cannot recognize every secret automatically.

## 7. Open the pull request

On GitHub, choose **base repository `ProjectOrcha/Mammoth`, base `main`**, and
**head repository your fork, compare your feature branch**. The author pushes
to their fork; maintainers decide whether to merge into the project.

Use the PR template. A useful description could be:

```markdown
Problem: a missing layout left the file page blank.
Change: show an error naming the requested path.
Validation: regression test, npm run check, npm run build; checked both themes.
Related issue: link the real issue if one exists.
Limitations: tested against mocked API responses; Rust gateway is unfinished.
```

Mark unfinished work as a draft. CI checks may need maintainer approval for a
first-time fork. You do not need publishing secrets to contribute, and you
should not change CI permissions to make a fork run privileged jobs.

## 8. Respond to review and keep your branch current

Make follow-up commits on the same branch and push to `origin`. Answer review
questions or ask for clarification; you are not expected to know every system.

To include new upstream work without rewriting your branch:

```bash
git fetch upstream
git merge upstream/main
```

Resolve conflicts using [chapter 3](03-team-workflow.md#5-bring-in-main-and-resolve-conflicts),
rerun checks, then push. Coordinate with the reviewer before rebasing a shared
branch. Never force-push upstream/main.

If you become unavailable, leave a note describing what works and what remains.
For a security-sensitive bug, use the repository's private reporting route if
available; avoid posting credentials or exploit details in a public issue.

## 9. After merge

Update your local main, assuming it contains no independent commits:

```bash
git switch main
git fetch upstream
git merge --ff-only upstream/main
git push origin main
```

If fast-forward fails, stop and inspect your local work before changing history.
Delete the old feature branch only after confirming the contribution was merged.
Then choose another small issue or help review documentation.

## Contribution checklist

- [ ] The contribution is scoped and does not duplicate an open PR.
- [ ] Commands and examples are labelled “works now” or “planned” accurately.
- [ ] Relevant checks passed, or failures and environment limits are explained.
- [ ] The PR targets upstream/main from my fork branch.
- [ ] No secrets, generated build directories or unrelated changes are included.
- [ ] I can explain what I changed and why.

Contributions use the repository's dual Apache-2.0 / MIT licensing. Be respectful
in issues and reviews, give credit for others' work, and ask questions early.
