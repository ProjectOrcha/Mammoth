---
title: Start contributing
description: Beginner setup, code structure and separate team and external workflows.
---

Mammoth is a learning scaffold. The Rust command tree, core types and teaching
examples exist; storage and gateway serving remain to be built. The Svelte
dashboard runs with clearly labelled simulated data.

## First steps

Use stable Rust (minimum 1.85) and Node 22.x. From your checkout root:

```bash
cargo build --workspace --locked
cargo run -p mammoth-cli -- --help
cargo run -p mammoth-parts --example 01-ownership
```

To run the dashboard, open another terminal in `ui/`:

```bash
npm ci
npm run dev
```

Open http://localhost:5173 and look for the Demo workspace banner. These are example
values, not a real storage cluster.

## Guides for your next step

| Need | Repository guide |
| --- | --- |
| Install tools, understand commands, run small examples | [Your first hour](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/START-HERE.md) |
| Understand the Rust and frontend folder structure | [Code map](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/CODE-MAP.md) |
| Coordinate four members, assign tasks and reviews | [Four-person team plan](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/TEAM-PLAN.md) |
| Contribute from outside the core team | [Fork-to-PR workflow](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/EXTERNAL-CONTRIBUTORS.md) |
| Learn Svelte and fix frontend bugs | [Frontend chapter](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/09-web-ui.md) |
| Connect a future gateway | [API contract](https://github.com/ProjectOrcha/Mammoth/blob/main/docs/guide/API-CONTRACT.md) |
| Check a PR before review | [Contributing](https://github.com/ProjectOrcha/Mammoth/blob/main/CONTRIBUTING.md) |

The guides live in the repository so contributors can improve them alongside
the code. Follow the appropriate guide for team membership or external access;
you do not need core-team permissions to submit a contribution.
