# Mammoth memory dashboard

The primary page saves, recalls, and edits durable project context. It uses the
same agent-memory.sqlite3 database as CLI and MCP and never substitutes demo
memories when the service is unavailable. The original storage overview is at
`/storage`; its engineering screens remain as an archive.

The local HTTP adapter is implemented on AI_coded. To run the complete dashboard
on that branch, build the UI before the binary and select your memory root:

```bash
npm --prefix ui ci
npm --prefix ui run build
cargo build --locked -p mammoth-cli
./target/debug/mammoth --local-root /absolute/path/to/memory-store quickstart --no-sample
```

Open http://127.0.0.1:8080 and use the same project name as your MCP connection.
The main branch's frontend can connect to this API, but its legacy gateway remains
a scaffold. CLI and MCP memory work independently on both branches.

For development, run `npm run dev` in ui/ with the AI_coded service at port 8080.
`npm run check`, `npm test`, and `npm run build` validate the frontend.

## Legacy storage dashboard notes


The Svelte 5 admin dashboard. It runs independently with simulated data today.
Serving and embedding it in the Rust binary is planned; `mammoth-gateway` is
still a scaffold. Beginner walkthrough: [chapter 9](../docs/guide/09-web-ui.md).

## Run it

Use Node **22.x**. From `ui/`:

```bash
npm ci
npm run dev            # http://localhost:5173
```

Development probes `/api/v1/cluster/report` through Vite's port-8080 proxy. If
no gateway answers, it shows a labelled demo: twelve workers, one dead, and a
repair in progress. The Rust `serve` command does not start a gateway yet.

Production builds require a compatible gateway by default. To preview a
standalone demo, put `VITE_DATA_SOURCE=demo` in `ui/.env.local`, then build and
preview. This local file is ignored by Git. Remove it and rebuild for real API
integration. To test integration in dev, use `VITE_DATA_SOURCE=gateway`.
Vite reads settings at startup/build time; do not put secrets in them.

[API-CONTRACT.md](../docs/guide/API-CONTRACT.md) documents all modes and endpoints.
The core Rust types and dashboard types differ; they need an explicit adapter.

| Script | Does |
| --- | --- |
| `npm run dev` | dev server with hot reload |
| `npm run check` | `svelte-check` over every file — keep it at zero |
| `npm test` | regression tests for requests, navigation and formatting |
| `npm run build` | static output to `ui/build/`, for future embedding |
| `npm run preview` | serve that build locally |

`cargo xtask build-ui` runs `npm ci && npm run build` for you.

## The pages

| Route | Shows |
| --- | --- |
| `/` | capacity, throughput, block health, alerts, and the four fast paths as live numbers |
| `/nodes` | sortable, rack-grouped worker table with per-node detail |
| `/files` · `/files/[...path]` | namespace browser; per-file block placement, read plan and EC layout |
| `/distribution` | six visualizations, the repair fan, and a 24-hour time machine |
| `/jobs` | stage DAG, task Gantt, and the straggler setting the job's runtime |
| `/cluster` | Raft members, and what the last start actually cost |

## Layout

```
src/
├── app.html            theme bootstrap, before first paint
├── app.css             the --mm-* ramp, shared with the docs site
├── lib/
│   ├── types.ts        every shape the gateway serves
│   ├── api.ts          the typed client, with the demo fallback
│   ├── demo.ts         the simulated cluster
│   ├── live.svelte.ts  one shared, ref-counted cluster subscription
│   ├── format.ts       bytes, rates, durations — one place
│   ├── components/     Panel, Stat, Meter, StateDot, Sparkline, Browse, FastPaths
│   └── charts/         BlockMatrix, HeatGrid (SVG) · Treemap, RackTopology,
│                       SkewScatter, FlowSankey (ECharts) · colors, echarts setup
└── routes/             the six routes above
```

## Conventions worth keeping

**Read shared cluster reports from `live`.** Page-specific data such as files
and history uses `api`; do not make a second cluster subscription for it.
 `live.svelte.ts` holds one subscription
with a reference count — the first component to `attach()` starts it, the last
to detach stops it, and every page reads the same `$state`. Multiple pages polling
the same report endpoint independently is how a dashboard becomes the cluster's busiest
client.

**Colour comes from the value, not from the caller.** `Meter` picks its own
colour from its fraction, so 94% is the same red everywhere it appears. Heat
ramps pick their *label* colour from the tile's luminance, because white text on
the gold middle of a heat scale is not readable.

**State uses `charts/colors.ts`, not the theme tokens.** Semantic tokens are
right for state and wrong for category: `--accent` and `--info` are gold and
pale blue in the dark theme and two shades of navy in the light one, so a legend
keyed off them stops working the moment somebody flips the toggle.

**API text in HTML tooltips must use `escapeHtml`.** Svelte escapes ordinary
markup, but ECharts HTML formatter strings are outside that protection.

**ECharts is imported from `echarts/core`.** Only the four registered chart
types ship. Adding a fifth means adding it to the `use()` call in
`charts/echarts.ts` and nowhere else — the full bundle is about a megabyte and
this all ends up inside the binary.

## Design

The palette, the Roman-capital display face and the letterspaced monospace
micro-labels are the docs site's, in `src/app.css` as `--mm-*` tokens. No
webfont is linked: this UI is served from inside a cluster where
`fonts.googleapis.com` is usually unreachable and always slow, so every stack
names the faces we want first and a system fallback after.

The `cookie` override selects the patched 0.7 line while SvelteKit still requests
0.6. Recheck upstream's dependency before removing it. `npm audit` reports no
known advisories for the reviewed UI lockfile; keep reviewing dependency updates.
