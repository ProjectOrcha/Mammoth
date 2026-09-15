# Dashboard API: what the gateway must provide

**Status on AI_coded:** the local gateway and its dashboard adapters are
implemented. `GET /api/v1/cluster/report` returns `capabilities` with `local: true`,
`distributed_metrics: false`, `history: false`, and `jobs: true`. Unavailable
metric groups are JSON `null`; the frontend normalizes them to non-finite display
values and hides their panels. Node rates and latency are also `null`. They must
never be treated as measured zeroes. History requests return HTTP 501.

The filesystem API also supports PUT `/fs/data`, GET `/fs/data`, PUT
`/fs/directory`, DELETE `/fs`, POST `/fs/rename`, POST `/fs/attributes`, and
POST `/fs/replication`, with query parameters matching the CLI's backend operations.
GET `/fs/search` provides up to 1,000 file choices for the distribution view.
The `/core/*` endpoints use Rust records for the HTTP SDK; dashboard endpoints
use the enriched records below.

An **endpoint** is an HTTP URL and method. A **contract** describes the shape and
meaning of its input, success response and errors. A TypeScript interface checks
our source code; it does not automatically validate JSON received over a network.

## Two different sets of types

| Source | Purpose | Example difference |
| --- | --- | --- |
| `crates/mammoth-core/src/types.rs` | Small storage vocabulary for the Rust exercises | `block_layout` returns a list of placements with replicas |
| `ui/src/lib/types.ts` | Rich dashboard DTOs (data transfer objects) | `BlockLayout` wraps placements, nodes, racks, policy and warnings; placements use fragments |

The dashboard's `ClusterReport` additionally needs throughput, repair, read/write
path stats, startup details, alerts and Raft data. Serializing the core report
as-is will not make the dashboard work. The client rejects the smaller report
with an incompatible-contract error before rendering it.

Implement a gateway adapter explicitly, or simplify the relevant frontend
contract and views together. If a real metric is unavailable, agree on an
optional/null field and a visible “unavailable” state. Never label an invented
value as a real measurement just to make a TypeScript type compile.

## Endpoints used by ui/src/lib/api.ts

All are `GET` requests under `/api/v1`, except the SSE connection, which is a
long-lived GET. The client expects JSON for normal responses.

| Path | Query / input | Successful response type |
| --- | --- | --- |
| `/cluster/report` | Optional `minutes_ago` | `ClusterReport` |
| `/nodes` | None | `NodeReport[]` |
| `/nodes/:id` | Encoded node ID | `NodeReport`; use HTTP 404 if absent |
| `/fs` | `path`, `limit` (UI defaults to 200) | `FileStatus[]`, direct children |
| `/fs/stat` | `path` | `FileStatus`; client also handles `null` for absence |
| `/fs/blocks` | `path` | `BlockLayout`; client also handles `null` for absence |
| `/distribution/heat` | `metric`, `minutes_ago` | `HeatCell[]` |
| `/distribution/treemap` | `path`, `depth` | `TreemapNode` |
| `/distribution/skew` | `path` | `SkewReport` |
| `/distribution/topology` | `minutes_ago` | `TopologyReport` |
| `/distribution/flow` | `minutes_ago` | `FlowReport` |
| `/jobs` | None | `Job[]` |
| `/events` | None | SSE events described below |

The default listing limit is 200. `offset` selects subsequent pages; the file
browser has Previous/Next controls. Listings use stable lexicographic order.
Concurrent namespace changes may shift offsets; this is not a snapshot cursor.

Use HTTP 404 for missing resources in a new gateway. The current demo returns
null for missing stat/layout; both paths show a message. Use non-2xx statuses
for failures, including 401/403 for authentication/authorization errors. A
future structured error body should preserve `code`, `message`, `hints` and
`docs`; the current client displays the status and requested endpoint.

## Small filesystem response example

This is a **JSON fixture**, not a complete gateway implementation:

```json
{
  "path": "/data/notes #1.txt",
  "name": "notes #1.txt",
  "is_dir": false,
  "len": 12,
  "block_size": 134217728,
  "replication": 3,
  "policy": "replication-3",
  "blocks": 1,
  "inlined": false,
  "mode": 420,
  "owner": "demo",
  "group": "demo",
  "modified": 1788000000000,
  "checksum": null
}
```

`modified` and other timestamps are **Unix milliseconds**, matching JavaScript
`Date`. `mode` is numeric: decimal 420 is octal `0644`. Sizes and rates are
bytes and bytes/second. Ratios such as job progress are fractions (0 to 1),
while fields named `budget_pct` are percentage points. Check each field's
comment in `types.ts`; avoid converting the same value twice.

The API request encodes the raw path in a query parameter:

```text
/api/v1/fs/stat?path=%2Fdata%2Fnotes%20%231.txt
```

The browser's page URL is different:

```text
/files/data/notes%20%231.txt
```

Use `encodeURIComponent` for query values and `fileHref` for file-page links.
Do not manually decode an already decoded SvelteKit route parameter.

## Live events

SSE means **Server-Sent Events**: the server sends text events down one HTTP
connection. The client currently listens for `node_state`, `block_health`,
`throughput`, `job_update`, and `alert`. Each event's data must be valid JSON.
For example:

```text
event: node_state
data: {"id":"w3","state":"dead"}

```

A blank line terminates the event. The shared `live` store reloads the report
when a delta arrives; the demo ticker supplies a full report. A reconnect also
refreshes the report. Malformed events and connection failures surface as errors.
The store shares concurrent report reads so a burst of events does not create
one request per event. Jobs update alongside report updates.

History endpoints must either return the selected historical data or report
that history is unsupported. Returning today's values under yesterday's label
is misleading. Treemap and skew are current namespace views, not replay data.

## Build and source modes

| `VITE_DATA_SOURCE` | Behavior |
| --- | --- |
| unset during `npm run dev` | `auto`: try the gateway, then clearly labelled demo data if unavailable |
| unset during `npm run build` | `gateway`: require a compatible gateway; no automatic demo fallback |
| `demo` | Use local simulated data without HTTP requests |
| `gateway` | Require the real API, including during development |
| `auto` | Explicit fallback mode; appropriate only when simulated output is intended |

Vite reads these settings at startup/build time. Restart or rebuild after
changing them. They are public browser settings, not a place for secrets.
Authentication failures are never replaced by demo data, even in auto mode.
Reload after starting a gateway if the current page already selected demo mode.

## Integration acceptance checklist

- [x] Gateway regression tests cover endpoint shapes used by the UI.
- [x] Core records are adapted explicitly; missing metrics are represented honestly.
- [x] Empty directory, missing file, empty fragment list and zero-capacity node work.
- [x] Filenames containing spaces, Unicode, `#`, `?` and `%` round-trip correctly.
- [x] A slow old request cannot replace a newer selection or page.
- [x] Errors, malformed reports and reconnects are visible; no unhandled rejection.
- [x] Gateway routes do not fall through to the SPA HTML for unknown `/api` URLs.
- [x] Static assets and nested browser routes work after a hard reload.
- [x] `npm run check`, `npm test` and `npm run build` pass.

## Dashboard actions

- `GET /fs?path=...&name=...&limit=201&offset=0` filters by a case-insensitive
  name substring before pagination. The UI requests one extra row to determine
  whether another page exists.
- `POST /fs/rename?path=...&to=...` moves or renames a file or directory.
- `DELETE /fs?path=...&recursive=true` removes a non-empty directory.
- `POST /fs/attributes?path=...&mode=416&owner=...&group=...` sets metadata;
  mode is a decimal representation of the octal bits (416 is 0640).
- `POST /fs/replication?path=...&replication=3` changes whole replica copies.
- `POST /admin/repair` reports the number of restored replicas.
- `POST /jobs?kind=wordcount|sort&input=...&output=...&overwrite=false` accepts
  a local UTF-8 job and returns HTTP 202 with its running job record. Existing
  output files are rejected unless `overwrite=true`. Up to two jobs run at once.
- `GET /jobs` returns the latest 100 dashboard submissions for this service
  session, including state, input/output, elapsed time and any error. CLI jobs
  are not tracked here. The history resets when the service restarts.
