# The four-person team plan

**Manual build track: `main`.** Follow the [branch guide](BRANCHES.md) to keep
your implementation separate from the working `AI_coded` reference.

This guide is for **four people total: you and three teammates**. It assumes
everyone is learning Rust. Use the role labels below until you fill in real
names. Outside contributors should use the [external guide](EXTERNAL-CONTRIBUTORS.md).

## Start together

Each person completes [the first-hour guide](START-HERE.md), runs examples
01–05, and reads [the code map](CODE-MAP.md). Then read
[the Backend chapter](04-the-backend-trait.md) together. Each person explains
one of `list`, `read`, `write`, and `cluster_report` in their own words.

The dashboard works with demo data. Storage and the gateway remain exercises.
A screenshot of the dashboard is not proof that the Rust backend works.
Keep “demo works” and “real data works” as separate issue acceptance criteria.

## Who does what

| Person / fill in name | Primary area | First small task | Backup reviewer |
| --- | --- | --- | --- |
| A — Ana / ______ | Storage: `crates/mammoth-local/`, chapters 5–6 | Directory listing and missing-path test | D |
| B — Ben / ______ | CLI and terminal views: `mammoth-cli`, `mammoth-viz`, chapters 2, 7, 8–8b | Implement `version`; table and JSON output | A |
| C — Cai / ______ | Dashboard: `ui/`, frontend part of chapter 9 | Run demo, improve a page, add a regression test | B |
| D — Dev / ______ | Gateway and integration: `mammoth-gateway`, `xtask`, `web/`, chapters 9–10 | Check API shapes with C; improve one public doc | C |

The first reviewer is the next person in A → B → C → D → A. The backup in the
table is another person to ask if that reviewer is away. No self-approval.
For API changes, C and D review together; for storage layout changes, A and D
review together. Request a first response within one working day.

Primary ownership means coordinating changes and helping reviewers understand
the area. Everyone writes tests and updates docs for their own feature. D is
not the team's sole tester or writer. Rotate roles after a milestone so each
person learns both a Rust boundary and a user-facing part.

If everyone is equally new, choose by interest and pair on the first issue.
Nobody has to become the “Rust expert” before work can start.

## Work in small, reviewable rounds

Treat each round as a milestone, not a deadline. Begin the next round when its
checks pass. Each person keeps at most one active implementation issue.

| Round | A: storage | B: CLI | C: dashboard | D: integration |
| --- | --- | --- | --- | --- |
| 0: all four onboard | Run Rust examples | Run help and examples | Run UI and tests | Run docs and xtask |
| 1: independent changes | `list` / `stat`, temporary-directory tests | `version`, output formatting | File loading/errors, demo fixtures | Define endpoint fixtures with C, docs fixes |
| 2: local filesystem | Write/read/remove, partial-block tests | Wire `ls` / `stat`, then `put` / `cat` | Validate UI against agreed fixture shapes | HTTP routing, errors, storage-to-UI adapters |
| 3: integration | Verify round trips and placement | Block visualization, CLI smoke checks | Real API mode and failure states | Connect backend, static assets, live updates |
| 4: shared demo | Explain storage | Explain CLI | Explain UI | Explain integration |
| 5: qualify local behavior | Atomic publication, failure tests | Overwrite controls, lifecycle | Errors, labels, accessibility | Origin checks, recovery drill, CI |

At the end of round 4, each person runs someone else's instructions from a
fresh checkout. Record what is actually implemented in the roadmap and guide.
Distributed storage is later work; local directories do not test network failures.

## The handoff contracts

A handoff states exactly what the next person can rely on. Include a command,
expected result, and limitations in the PR. “It compiles” alone is insufficient.

### Handoff 1 — Ana → Ben, end of chapter 5

A delivers `LocalBackend`, `list` and `stat` plus tests for an empty directory
and a missing path. Other trait methods may still be unfinished. B can wire
`ls` and `stat`; B cannot assume `put` works. Record unfinished methods in the
issue. Prefer explicit unsupported-operation errors on user-facing paths.

### Handoff 2 — Ana → Ben, end of chapter 6

A delivers write/read/remove/block layout/report behavior and a test proving
that writing bytes then reading them returns identical bytes. B wires the
remaining CLI commands and checks stdout, stderr and exit status separately.

### Handoff 3 — Ana → Dev, end of chapter 6

D receives the same backend and wraps it in HTTP handlers. **The Rust core
records are not the dashboard JSON contract.** C and D use
[API-CONTRACT.md](API-CONTRACT.md) to agree on adapters, error responses and
unsupported metrics. Do not fabricate real performance data to satisfy a type.

### Handoff 4 — Cai ↔ Dev, dashboard integration

C supplies expected endpoint fixtures and failure cases. D supplies matching
responses and runs the server. Together check a file with spaces and `#` in its
name, a missing file, an empty directory, an HTTP failure, a reconnect, and a
slow response arriving after navigation. Use gateway mode so demo fallback
cannot hide integration errors.

## Nobody waits: work against fake data

C can work in `ui/src/lib/demo.ts` today. B can practice rendering with
`examples/parts/08-table-or-json` (run the example by its package command below).
D can review fixtures and build routing while A implements storage.

```bash
cargo run -p mammoth-parts --example 08-table-or-json
```

Use real project types in fixtures. Keep mock data clearly named and labelled.
When an upstream implementation lands, replace the dependency, then run the
same behavior checks against it. Mock success does not count as integration.

## One issue, one branch, one PR

Use [chapter 3](03-team-workflow.md) for exact Git commands. Copy this template
into each issue:

```markdown
Goal: show a useful message when a file layout is missing.
Owner: C
Reviewer: D (backup: B)
Files: ui/src/lib/components/Browse.svelte and its test
Depends on: no other issue; use the demo/mock API
Done when:
- Missing layout displays a message instead of a blank screen.
- Normal files and directories still open.
- Regression test, type check and build pass.
Guide update: mention the error behavior in the frontend walkthrough.
```

Before editing shared files (`Cargo.toml`, lockfiles, core types, UI types or CI),
note the planned change in the issue. One person should coordinate that edit;
the others rebase or merge the resulting commit before touching the same file.
Do not manually splice package lockfiles to resolve conflicts. Reapply the
intended dependency change with the package manager and inspect the result.

## Daily and weekly habits

Post a short update where the team already coordinates:

```text
Done: file paths containing # now open correctly; regression test added.
Next: test a slow directory response during navigation.
Blocked: need agreement on whether missing stat returns null or HTTP 404.
PR: link to the branch/PR.
```

Once a week, each person gives a five-minute demo: one working case, one error
case, and one code explanation. Keep a shared issue with four owner columns,
dependencies, PR links and the next milestone. Do not assign the same issue to
two people unless they explicitly agree to pair.

## If someone is stuck, away, or disagrees

After a focused attempt, share the smallest reproduction and the full error.
Pair for 20–30 minutes. If still blocked, write the open question in the issue
and take a task that does not depend on it.

Before an absence, push the branch and leave a draft PR with what works, what
fails, and the next command to run. A teammate can continue from that branch
by agreement; never overwrite another person's uncommitted work.

For a shared interface or storage format disagreement, write a small
[architecture decision](../adr/0002-backend-trait.md): problem, alternatives,
choice, consequences. Decide together before dependent code spreads.

If `main` breaks, create a repair or revert branch and PR. Prioritize review.
Do not bypass the same protected-branch process you rely on for normal changes.

## Day-one checklist for all four

- [ ] Four real names filled into the role table and reviewer ring.
- [ ] Each person built Rust, ran one example, and opened the demo UI.
- [ ] All four can explain a trait and a simulated backend.
- [ ] Four small issues with acceptance criteria; no duplicated work.
- [ ] `main` requires passing CI and at least one independent approval.
- [ ] Everyone knows the daily update location and weekly demo time.
- [ ] An outside contributor can follow the separate fork workflow without team access.

Back to [the guide](README.md) · [Git workflow](03-team-workflow.md) · [checklists](CHECKLISTS.md).

Round 5 is detailed in [the readiness chapter](14-readiness-and-benchmarks.md).
A successful local round does not complete the distributed milestones.
