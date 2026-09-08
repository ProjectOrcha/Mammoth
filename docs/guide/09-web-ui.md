# Chapter 9 — Understand and improve the dashboard

**What you can do now:** run the Svelte dashboard, trace a request, make a small
change, and test it. **Later:** implement the Rust gateway and connect real data.
The dashboard exists; embedding and HTTP serving in Rust are not implemented yet.

Person C owns the dashboard and person D owns the gateway in the
[four-person plan](TEAM-PLAN.md). Work together on the API contract.

## Before you start

- Install Node 22.x and run `npm ci` in `ui/`.
- Read [the code map](CODE-MAP.md) and [the current API contract](API-CONTRACT.md).
- Create a branch using [chapter 3](03-team-workflow.md).
- Run `npm run check` and `npm test` once to establish the baseline.

## 1. Run the existing frontend

From `ui/`:

```bash
npm ci
npm run dev
```

Open <http://localhost:5173>. The initial `/api` proxy request may fail because
there is no server at port 8080. Development mode then uses `demo.ts` and shows
**Demo workspace** (all values are simulated). A Node proxy error at this stage does
not mean you need to implement Rust before improving the UI.

| Section | Try this | Relevant source |
| --- | --- | --- |
| Overview `/` | Read capacity and throughput, pause updates | `routes/+page.svelte`, `lib/live.svelte.ts` |
| Nodes `/nodes` | Search a worker, sort usage, open its details | `routes/nodes/+page.svelte` |
| Files `/files` | Open `/data`, then a file | `lib/components/Browse.svelte` |
| Distribution `/distribution` | Change metric/file and move the history slider | `routes/distribution/+page.svelte` |
| Jobs `/jobs` | Select a job and inspect its stages | `routes/jobs/+page.svelte` |
| Cluster `/cluster` | Inspect master and startup information | `routes/cluster/+page.svelte` |

The route `/files/[...path]` handles all nested paths with the same Browse
component. A catch-all parameter (`...path`) can include more than one directory.

For the overview, start with `ui/src/routes/+page.svelte`. It lays out the page
and reads the shared cluster report. `ui/src/lib/overview.ts` calculates health
totals and worker priority; `ui/src/lib/components/OverviewMetric.svelte` draws
each summary tile. For example, change a tile's `note` to improve its explanation
without changing how the report is fetched.

The public homepage is separate: `web/src/content/docs/index.mdx` holds its setup
tabs, `web/src/components/LandingHero.astro` draws the introduction and preview,
and `web/src/components/Homepage.astro` contains the guide links and remaining
sections. Its styles live in `web/src/styles/homepage.css`. Use
`web/src/lib/links.ts` for links inside Astro components so GitHub Pages URLs
keep the `/Mammoth/` prefix.

## 2. Read a Svelte component

A `.svelte` file usually has three parts: TypeScript logic, HTML-like markup,
and styles scoped to that component. This complete teaching component can be
saved as `ui/src/lib/components/Counter.svelte` on your learning branch:

```svelte
<script lang="ts">
  let clicks = $state(0);
  const doubled = $derived(clicks * 2);
</script>

<button onclick={() => clicks += 1}>Clicked {clicks} times</button>
<p>Twice that number: {doubled}</p>

<style>
  p { color: var(--fg-dim); }
</style>
```

To display it, add `import Counter from '$lib/components/Counter.svelte';` inside
a page's existing `<script>` block and `<Counter />` in that page's markup.
Click once: it shows `Clicked 1 times` and `Twice that number: 2`.
Remove the learning component before opening an unrelated production PR.

| Syntax | Meaning |
| --- | --- |
| `lang="ts"` | Check the script as TypeScript |
| `$state(0)` | A reactive value; changing it updates the page |
| `$derived(...)` | Recalculate a value when its inputs change |
| `$props()` | Read inputs passed by a parent component |
| `{#if ...}` / `{#each ...}` | Conditional markup / a repeated list |
| `$effect(() => ...)` | Run a browser side effect and optionally return cleanup |
| `onMount(...)` | Run once when a component appears in the browser |
| `$lib/...` | Alias for `ui/src/lib/...` |

Use existing tokens such as `--fg-dim`, `--bg-panel` and `--danger` so both themes
remain readable. Reuse `Panel`, `Meter` and formatting helpers before adding new
versions. ECharts registration lives in `lib/charts/echarts.ts`.

## 3. Trace and safely change a file request

Open `Browse.svelte`. It reads a path, calls `api.stat`, then calls `api.list`
for a directory or `api.blocks` for a file. It tracks loading, data and errors.

An asynchronous request may finish after the user navigates elsewhere. This
**pattern fragment** shows why cleanup is necessary:

```ts
$effect(() => {
  const wanted = path; // read the dependency before starting async work
  let active = true;

  void api.stat(wanted).then((result) => {
    if (active) status = result;
  }).catch((error) => {
    if (active) message = String(error);
  });

  return () => { active = false; };
});
```

The cleanup runs before the effect reruns and when the component disappears.
The old request may still finish, but it cannot write into the new view. Guard
**every** result, error and loading update, including a second request after
`stat`. Comparing only path strings fails when navigation goes A → B → A.
See the [Svelte effect documentation](https://svelte.dev/docs/svelte/$effect)
for the lifecycle rules. Keep the effect callback synchronous; start the async
work inside it so it can return cleanup immediately.

Use `fileHref(path)` when linking to a file page. A raw path such as
`/data/a#b.txt` cannot be inserted directly into `href`, because `#` starts a URL
fragment. Svelte escapes text in normal markup, but ECharts HTML tooltips need
our separate `escapeHtml` helper for values received from the API.

## 4. Write and run a regression test

Tests use Vitest and a small browser-like DOM (jsdom). They live next to the
behavior they exercise:

```text
lib/format.test.ts                     units, rounding and encoded paths
lib/api.test.ts                        source modes, failures and retry
lib/components/Browse.test.svelte.ts   navigation races and missing layouts
```

A basic test has arrange, act and assert steps. This is a complete test example
for a separate `file-link.test.ts` beside `format.ts`:

```ts
import { expect, it } from 'vitest';
import { fileHref } from './format';

it('keeps # inside the filename', () => {
  const result = fileHref('/data/a#b.txt');
  expect(result).toBe('/files/data/a%23b.txt');
});
```

Do not add that duplicate test to a PR; equivalent coverage already exists.
Instead, add a new case for the behavior you are changing. A `.test.svelte.ts`
file can use Svelte runes, useful when testing reactive prop changes.

From `ui/`:

```bash
npm test
npm test -- src/lib/components/Browse.test.svelte.ts
npm run check
npm run build
```

`check` catches type and template issues; tests catch behavior; a build confirms
the app can be bundled. None replaces trying the actual browser: navigate
quickly, use the keyboard, resize to phone width, and test both themes.
Vitest's version is kept compatible with this project's Vite version; see its
[versioned guide](https://v3.vitest.dev/guide/) before upgrading either one.

## 5. Understand demo and production builds

Development defaults to `auto`; production defaults to `gateway`. In gateway
mode a failure is visible rather than replaced with fake cluster numbers.

For an intentional standalone demo, create `ui/.env.local` with:

```dotenv
VITE_DATA_SOURCE=demo
```

Then run `npm run build` and `npm run preview` in `ui/`. Open the URL Vite prints.
Remove the setting and rebuild when testing a real gateway. `.env.local` is
ignored by Git. Vite variables are embedded into browser code: do not put secrets
in them. [API-CONTRACT.md](API-CONTRACT.md#build-and-source-modes) lists all modes.

`cargo xtask build-ui`, run from the repository root, also runs `npm ci` and
`npm run build`. The output is `ui/build/`. It is not automatically embedded in
the Rust binary yet, and building does not deploy anything.

## 6. Plan the gateway integration in small steps

The earlier storage exercises return smaller Rust records than the dashboard
needs. **Returning `Json(core_report)` directly is not sufficient.** Agree on
[endpoint fixtures and adapters](API-CONTRACT.md) before wiring pages together.

Create these files as part of the gateway implementation (they do not exist yet):

```text
crates/mammoth-gateway/src/
├── lib.rs     app/router construction and shared Backend state
├── api.rs     route handlers, query parsing, status/error responses
├── dto.rs     dashboard response types and explicit core-to-UI adapters
└── ui.rs      static assets and SPA fallback
```

A safe sequence for C and D:

1. Add a server with a health endpoint and a test. This proves HTTP works.
2. Implement one real filesystem endpoint and its error cases against a tested
   LocalBackend. Validate paths at the backend boundary.
3. Define DTOs for the matching dashboard views. Add fixtures for missing fields,
   zero values and special filenames. Represent unavailable metrics honestly.
4. Add the rest of the endpoints required by those views. `/fs/stat` is required
   before Browse can decide whether a path is a directory or a file.
5. Connect in `VITE_DATA_SOURCE=gateway` mode. An incompatible report must fail
   visibly; do not use fallback demo data as evidence of integration.
6. Implement SSE and its disconnect/reconnect behavior, or explicitly support a
   tested polling alternative. Then test events against the shared live store.
7. Serve `ui/build/` and add SPA fallback for nested **page** routes only.
   Unknown `/api/v1/...` paths must return API errors, never `index.html`.
8. Embed the assets and connect `mammoth serve --role gateway` only after these
   checks pass. Update the roadmap and status docs to reflect the implementation.

Do not begin with every distributed metric. First agree on a small supported
page with honest unavailable states, then expand the contract and implementation
together. The rich demo illustrates the target product, not completed storage.

## Done when

For a frontend PR:

- [ ] I can explain the route, component, API call and state update I changed.
- [ ] Demo data remains visibly labelled; gateway failures are visible.
- [ ] Loading, empty, error and stale-response cases are handled.
- [ ] Keyboard and narrow-screen behavior work in both themes.
- [ ] Relevant regression tests, `npm run check` and `npm run build` pass.

For gateway integration, also complete the acceptance checklist in
[API-CONTRACT.md](API-CONTRACT.md#integration-acceptance-checklist).

Next: [chapter 10 — documentation website](10-github-pages.md).
