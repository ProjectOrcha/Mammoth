# The Mammoth web UI

The admin dashboard, served by the `mammoth` binary itself. SvelteKit, built to
static files and embedded with `rust-embed`, so there is one artifact to ship
and no separate web server.

## Run it

```bash
npm install
npm run dev            # http://localhost:5173
```

**It works before the gateway does.** `src/lib/api.ts` probes
`/api/v1/cluster/report` on first load; if nothing answers it falls back to the
simulated cluster in `src/lib/demo.ts` — twelve workers, one dead, a repair in
flight, a namespace with real skew — and says so in a banner. So the front end
can be built and reviewed while the Rust half is still being written.

To point it at a real cluster, start the gateway in another terminal; `vite`
proxies `/api` to port 8080 and the banner disappears.

```bash
cargo run -p mammoth-cli -- serve --role gateway
```

| Script | Does |
| --- | --- |
| `npm run dev` | dev server with hot reload |
| `npm run check` | `svelte-check` over every file — keep it at zero |
| `npm run build` | static output to `ui/build/`, which is what `rust-embed` reads |
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

**Read from `live`, not from `api`.** `live.svelte.ts` holds one subscription
with a reference count — the first component to `attach()` starts it, the last
to detach stops it, and every page reads the same `$state`. Six pages polling
the same endpoint independently is how a dashboard becomes the cluster's busiest
client.

**Colour comes from the value, not from the caller.** `Meter` picks its own
colour from its fraction, so 94% is the same red everywhere it appears. Heat
ramps pick their *label* colour from the tile's luminance, because white text on
the gold middle of a heat scale is not readable.

**State uses `charts/colors.ts`, not the theme tokens.** Semantic tokens are
right for state and wrong for category: `--accent` and `--info` are gold and
pale blue in the dark theme and two shades of navy in the light one, so a legend
keyed off them stops working the moment somebody flips the toggle.

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
