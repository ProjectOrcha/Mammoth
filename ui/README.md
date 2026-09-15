# The Mammoth web UI

The Svelte 5 admin dashboard for the persistent local storage service. Production
assets are embedded in the Rust binary by `mammoth-gateway`. Beginner walkthrough: [chapter 9](../docs/guide/09-web-ui.md).

## Run it

Use Node **22.x**. From `ui/`:

```bash
npm ci
npm run dev            # http://localhost:5173
```

Development probes `/api/v1/cluster/report` through Vite's port-8080 proxy. If
no gateway answers, it shows a labelled demo: twelve workers, one dead, and a
repair in progress. Run `mammoth serve --role all` to start the gateway.

Production builds connect to a gateway by default. Use the **Workspace** selector
in the header to switch between **My storage** and **Example cluster** without a
rebuild. The choice survives navigation and reloads in the same tab. Example data
is clearly labelled and cannot change real files. A connection error never
silently switches a production dashboard to example data.

`VITE_DATA_SOURCE=demo` can still set the initial build default; use
`VITE_DATA_SOURCE=gateway` for strict gateway integration in development.
Vite reads these settings at startup/build time; do not put secrets in them.

[API-CONTRACT.md](../docs/guide/API-CONTRACT.md) documents all modes and endpoints.
The core Rust types and dashboard types differ; they need an explicit adapter.

| Script | Does |
| --- | --- |
| `npm run dev` | dev server with hot reload |
| `npm run check` | `svelte-check` over every file — keep it at zero |
| `npm test` | regression tests for requests, navigation and formatting |
| `npm run build` | static output to `ui/build/`, embedded by the next Rust build |
| `npm run preview` | serve that build locally |

`cargo xtask build-ui` runs `npm ci && npm run build` for you.

## The pages

| Route | Shows |
| --- | --- |
| `/` | capacity, throughput, block health, alerts, and the four fast paths as live numbers |
| `/nodes` | sortable, rack-grouped worker table with per-node detail |
| `/files` · `/files/[...path]` | uploads, downloads, filtering, folder creation, rename/move, deletion, properties, bounded UTF-8 previews and paginated block placement |
| `/distribution` | six visualizations, replica bytes, and local session snapshots; example mode includes 24-hour historical replay |
| `/jobs` | submit local word-count/sort jobs, inspect results and measured execution timelines; example mode illustrates distributed stage DAGs |
| `/cluster` | rack and worker cards, replica repair and data lifecycle; example mode includes Raft and warm-start metrics |

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
`charts/echarts.svelte.ts` and nowhere else — the full bundle is about a megabyte and
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

Dashboard jobs are limited to two running operations and 100 recent submissions.
History lasts for the current service session; output files persist, and CLI jobs
are not included. Inputs must be UTF-8 files up to 64 MiB.

Local distribution history samples metadata every ten seconds for up to thirty
minutes. It stays in this tab’s memory and clears on reload. It is separate from
server-side historical replay. File previews request at most 64 KiB.

The namespace treemap retains its selected folder across live refreshes and theme
changes. Its path buttons navigate back; **View all entries** exposes tiny and
zero-byte entries that cannot occupy a clickable area. Folder navigation is held
in Svelte because replacing an ECharts tree resets ECharts' internal zoom.
