# Choose the right branch

`main` is the team's manual build track. `AI_coded` is a separate working
reference implementation. A feature working in the reference branch does not
mean it is implemented in `main`.

| Track | Use it for | What to expect |
| --- | --- | --- |
| `main` | Learn, divide work, implement and review each milestone | Shared interfaces, CLI scaffolding, runnable teaching examples, demo dashboard and build chapters |
| `AI_coded` | Run the existing local service; inspect one implementation after attempting a task | Durable local storage, a live dashboard, local jobs and benchmark evidence; distributed workers, HA and authentication remain incomplete |

For the manual build, start explicitly from `main`:

```bash
git clone --branch main https://github.com/ProjectOrcha/Mammoth.git
cd Mammoth
git switch -c feat/your-small-task
```

With an existing clean clone, `git switch main` selects the learning track.
Commit or stash your own edits first; do not reset a teammate's work. Follow
[chapter 3](03-team-workflow.md) for reviews and protected-branch rules.

To inspect the reference without changing your manual checkout:

```bash
git fetch origin
git worktree add --detach ../Mammoth-reference origin/AI_coded
```

Keep a separate local data directory for each checkout. The guide's teaching
layout and the reference engine's SQLite/block format are not interchangeable.
Do not point old binaries at a reference store. Never copy a live SQLite file as
though it were a complete storage backup.

## Learn from a reference without skipping the work

1. Agree on observable behavior and tests using the guide and API contract.
2. Implement the smallest useful slice on a feature branch from `main`.
3. Run its success, failure and concurrency cases; ask the assigned reviewer.
4. Compare the relevant reference files and explain any differences in the PR.
5. Bring across a specific fix only after reviewing its dependencies and tests.

Do not merge all of `AI_coded` into `main` to make a milestone look complete.
Do not copy its benchmark scores into claims about your manually built engine.
Benchmark the exact implementation and record its source revision or hashes.

The [four-person plan](TEAM-PLAN.md) assigns the initial work. The
[readiness chapter](14-readiness-and-benchmarks.md) adds the checks needed after
integration. For the reference's actual boundaries, inspect its own
[`IMPLEMENTATION-STATUS.md`](https://github.com/ProjectOrcha/Mammoth/blob/AI_coded/docs/IMPLEMENTATION-STATUS.md).
