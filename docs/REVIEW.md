# Project review — 8 September 2026

This review improves the existing scaffold and contributor experience. It does
not implement a distributed filesystem. Start learning with
[Your first hour](guide/START-HERE.md).

## Documentation changes

- Added a beginner onboarding path with working directories, command explanations,
  expected results, small Rust examples and troubleshooting.
- Added a code map with repository trees and CLI/dashboard request flows.
- Replaced the three-person plan with a four-person plan: storage, CLI, dashboard,
  and gateway/integration. Each role has review coverage and concrete handoffs.
- Added a separate external contributor guide covering forks, remotes, PRs,
  feedback, conflicts and syncing after merge.
- Rewrote the frontend chapter around the actual Svelte app and regression tests.
  Added an API contract explaining why the core Rust records cannot be sent
  directly to the current rich dashboard.
- Corrected onboarding, landing-page and quickstart claims to distinguish
  runnable features from planned ones. Added a contributor hub to the public site.

## Fixed behavior

| Problem | Change |
| --- | --- |
| A late file-list response could overwrite a newer route, including its errors | Effect cleanup invalidates all results, not just the first `stat` request |
| `#`, `?`, `%` and spaces in file paths could change URL meaning | Shared per-segment file URL encoding |
| A missing block layout could leave a blank file page | Explicit error state; empty fragment lists do not dereference a missing preferred node |
| Production gateway failures could silently become simulated cluster data | Production defaults to gateway mode; demo builds must be intentional |
| Failed probes leaked timers or could not be retried | Bounded requests, `finally` cleanup, retryable probe state |
| Auth failures, malformed events and connection errors were not handled consistently | Visible errors; 401/403 never select demo data |
| Event bursts could issue overlapping report reads | Shared in-flight request; reference-count cleanup is idempotent |
| History/file selection could display old responses under a new selection | Selection-scoped loaders, stale-result guards and error states |
| Jobs never refreshed and rejected requests were unhandled | Refresh with live report updates, coalescing and failure/empty states |
| Chart tooltip strings interpreted API values as HTML | Explicit escaping and patched ECharts dependency |
| Sub-byte values used an undefined unit; rounded times could contain 60 seconds | Clamped unit index and normalized duration rounding |
| Node details required a pointer; narrow screens lost the theme control | Keyboard buttons, linked node selection, mobile theme control and layout containment |
| Mermaid failures could leave diagrams hidden; rapid themes could overlap renders | Source-text fallback and serialized theme rendering |

## Tooling and dependency corrections

`cargo xtask` now has an alias and working UI build, CLI reference generation,
asset copy and release-tool delegation. Unsupported CLI operations return
`E0002` instead of panicking, and have a real documentation page.

The supported Rust floor and CI now agree on 1.85. `comfy-table` is pinned to
7.1.4 because its newer 7.2 line uses language features beyond that floor.
The nightly simulation job detects whether a `sim` target exists before running
seeds, avoiding bogus failures while the harness is still planned.

Dashboard dependencies include Vite 6.4.3 and ECharts 6.1.0. The cookie override
selects the patched 0.7 line required to clear the remaining SvelteKit dependency
advisory. The docs site now uses Astro 7.3.1/Starlight 0.42, the Content Layer
loader, the current route-data interface and an explicit unified Markdown
processor. Use current Node 22 (at least 22.12.0).

Both lockfiles reported **zero known npm audit vulnerabilities** after updates.
That is the registry's result on the review date, not a guarantee about future
advisories or all possible vulnerabilities.

## Verification completed locally

- Rust formatting, Clippy with warnings denied, workspace tests (including two
  new CLI smoke tests and the generated-reference test), and a locked Rust 1.85
  workspace check passed.
- The ownership and block-matrix teaching examples ran, as did CLI help/version.
- `cargo xtask build-ui` completed a clean install and production build.
- Svelte check: zero errors/warnings. All 17 frontend regression tests passed.
- Docs builds passed both at `/` and with `BASE_PATH=/Mammoth`.
- CLI reference regeneration produced no changes; local Markdown links and
  anchors resolved across 29 guides/READMEs. Built Pages asset/page URLs resolved.
- Browser checks covered file browsing, block details, history/file controls,
  jobs, keyboard node selection, both themes and a 390-pixel viewport with no
  whole-page horizontal overflow on Distribution.
- Production preview without a gateway showed an API error and no simulated
  cluster. The docs contributor page, landing links and Mermaid diagrams rendered
  under the Pages base path; theme switching worked.

CI now also checks frontend tests and both docs build configurations. Windows,
Linux, cargo-nextest and cargo-deny were not run locally in this macOS review.

## Remaining project boundaries

Storage, real gateway serving, S3, distributed execution and the simulation
harness are still roadmap work. Gateway behavior was exercised with frontend
mocks, not a real Rust cluster. The file browser still caps listings at 200 and
labels that limit; pagination needs a future API contract and UI.

Builds still emit informational warnings about large chart bundles and
Starlight's absent optional i18n/custom-404 content. These do not fail the builds.

## Homepage and overview UI refresh

The public homepage now uses a dedicated hero, an illustrated cluster preview,
three contributor entry points, keyboard-accessible setup tabs with copy buttons,
and a plain-language architecture overview. Available demo functionality and
planned backend roles are labelled separately. Its theme selector is available
on phones as well as desktops, and Astro component links preserve the hosting
base path through `web/src/lib/links.ts`.

The dashboard overview now prioritizes cluster status, readable metric tiles,
severity-ordered alerts, repair progress, and unhealthy workers. Suggested
commands expand on demand; demo commands are identified as examples. Refresh,
pause/resume, and connection-error recovery have clear controls. Block-health
percentages include all six categories, and worker sorting handles zero capacity
while prioritizing unhealthy states. Secondary text and light-theme status colors
have stronger contrast.

Validation for this refresh:

- All **24 frontend tests** pass, including seven new overview regression tests.
- Svelte check reports zero errors and zero warnings; the production UI builds.
- Website production builds pass at `/` and `/Mammoth/` (22 pages each).
- Browser checks cover dark/light themes, desktop and 390/320-pixel layouts,
  expandable alerts and commands, keyboard pause/resume and setup tabs, copy
  feedback, and the built homepage's contributor link and image paths.
- Both homepages have no whole-page horizontal overflow at the checked phone
  widths; long code samples and the worker table scroll within their containers.
- The default production build still requires a gateway and displays a retry
  screen when it cannot connect. Local review previews may explicitly use demo
  data; they do not change that production default.
