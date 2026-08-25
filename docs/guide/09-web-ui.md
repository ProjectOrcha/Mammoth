# Chapter 9 — The web UI and the gateway

**What you'll build:** a browser dashboard, served by the `mammoth` binary itself.

**Time:** about 4 hours.

---

The CLI is for you. The web UI is for everyone else — and it is what people
screenshot. This chapter builds the smallest version that is genuinely useful:
a live cluster overview and a file browser with a block map.

> **This chapter is a different skill set** from chapters 5–8: TypeScript,
> Svelte, HTTP. If your team split the work as chapter 3 suggested, this is
> person C's track and it can run in parallel with chapters 7–8.

## The shape of it

```
  browser ──HTTP──▶ mammoth-gateway ──Backend trait──▶ LocalBackend
                    (axum, port 8080)                  (chapters 5–6)
                          │
                          └── serves the built Svelte app from inside the binary
```

Two things worth noticing before you write code:

**The gateway talks to a `Backend`, not to `LocalBackend`.** Same trait, same
guarantee: this dashboard will work against a real cluster unchanged.

**The UI is compiled *into* the binary** with `rust-embed`. No separate web
server, no "where do I put the static files" step. `mammoth ui` just works, and
there is exactly one artifact to ship.

## Step 1 · The API

The endpoints the UI needs are already specified in
[`web/src/content/docs/api/index.md`](../../web/src/content/docs/api/index.md).
Start with three:

```
GET /api/v1/cluster/report          the overview page
GET /api/v1/fs?path=/data           the file browser
GET /api/v1/fs/blocks?path=/data/x  the block matrix
```

Add the dependencies to `crates/mammoth-gateway/Cargo.toml`:

```toml
[dependencies]
mammoth-core  = { workspace = true }
mammoth-local = { workspace = true }
axum          = { workspace = true }
tokio         = { workspace = true }
serde         = { workspace = true }
serde_json    = { workspace = true }
tower-http    = { version = "0.6", features = ["cors", "trace"] }
rust-embed    = { version = "8", features = ["axum"] }
mime_guess    = "2"
```

Then `crates/mammoth-gateway/src/lib.rs`:

```rust
//! Web server, REST API and the embedded UI.

#![forbid(unsafe_code)]

pub mod api;
pub mod ui;

use std::sync::Arc;

use axum::Router;
use mammoth_core::Backend;

/// Shared state every handler gets.
#[derive(Clone)]
pub struct AppState {
    pub backend: Arc<dyn Backend>,
}

/// Build the whole application: API under /api/v1, UI everywhere else.
pub fn app(backend: Arc<dyn Backend>) -> Router {
    Router::new()
        .nest("/api/v1", api::routes())
        .fallback(ui::handler)
        .with_state(AppState { backend })
}
```

**`Arc<dyn Backend>`** is the trait object again, this time shared across
threads. `Arc` is a reference count, so cloning `AppState` for each request is
cheap — it bumps a counter, it does not copy the backend.

Now `crates/mammoth-gateway/src/api.rs`:

```rust
//! The REST API. The CLI and the UI consume the same endpoints.

use std::path::PathBuf;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cluster/report", get(cluster_report))
        .route("/fs", get(list))
        .route("/fs/blocks", get(blocks))
}

#[derive(Deserialize)]
pub struct PathQuery {
    path: PathBuf,
}

async fn cluster_report(State(s): State<AppState>) -> Result<Response, ApiError> {
    let report = s.backend.cluster_report().await?;
    Ok(Json(report).into_response())
}

async fn list(
    State(s): State<AppState>,
    Query(q): Query<PathQuery>,
) -> Result<Response, ApiError> {
    let entries = s.backend.list(&q.path).await?;
    Ok(Json(entries).into_response())
}

async fn blocks(
    State(s): State<AppState>,
    Query(q): Query<PathQuery>,
) -> Result<Response, ApiError> {
    let layout = s.backend.block_layout(&q.path).await?;
    Ok(Json(layout).into_response())
}

/// Wraps a Mammoth error so it becomes a sensible HTTP response *and* keeps
/// the error code the CLI shows. Same errors, same codes, two surfaces.
pub struct ApiError(mammoth_core::Error);

impl From<mammoth_core::Error> for ApiError {
    fn from(e: mammoth_core::Error) -> Self {
        ApiError(e)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        use mammoth_core::Error::*;
        let status = match self.0 {
            NotFound(_) => StatusCode::NOT_FOUND,
            WrongKind { .. } => StatusCode::BAD_REQUEST,
            NotEnoughWorkers { .. } | SafeMode { .. } => StatusCode::SERVICE_UNAVAILABLE,
            LeaseHeld { .. } => StatusCode::CONFLICT,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let body = serde_json::json!({
            "code": self.0.code(),
            "message": self.0.to_string(),
            "hints": self.0.hints(),
            "docs": self.0.docs_url(),
        });
        (status, Json(body)).into_response()
    }
}
```

**That `ApiError` type is the chapter's best idea.** One error enum, defined in
`mammoth-core`, now produces both the teaching CLI output *and* a structured
JSON error with the right HTTP status. Add a variant once, and both surfaces
get it.

## Step 2 · Embed the UI

`crates/mammoth-gateway/src/ui.rs`:

```rust
//! Serves the built Svelte app from inside the binary.

use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../../ui/build/"]
struct Assets;

/// Serve an embedded asset, falling back to index.html so client-side
/// routing works on a deep link like /files/data/sales.csv.
pub async fn handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match Assets::get(path).or_else(|| Assets::get("index.html")) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            ([(header::CONTENT_TYPE, mime.as_ref())], content.data).into_response()
        }
        None => (StatusCode::NOT_FOUND, "ui not built — run `cargo xtask build-ui`").into_response(),
    }
}
```

> **`#[folder = "../../ui/build/"]` must exist at compile time.** If you have
> not run the UI build yet, `rust-embed` will complain. Create the directory
> with a placeholder to get moving:
>
> ```bash
> mkdir -p ui/build && echo "<h1>not built yet</h1>" > ui/build/index.html
> ```

## Step 3 · The `serve` command

Add to `crates/mammoth-cli/src/commands/mod.rs`:

```rust
pub mod serve;
```

and create `crates/mammoth-cli/src/commands/serve.rs`:

```rust
//! `mammoth serve --role gateway`

use std::sync::Arc;

use mammoth_core::Result;

pub async fn gateway(addr: &str) -> Result<()> {
    let backend = Arc::new(super::backend()?);
    let app = mammoth_gateway::app(backend);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("  Web UI  →  http://{addr}");
    println!("  API     →  http://{addr}/api/v1/cluster/report");
    println!();
    println!("  press ctrl-c to stop");

    axum::serve(listener, app).await?;
    Ok(())
}
```

Wire it in `main.rs`:

```rust
        cli::Command::Serve { role } => match role.as_str() {
            "gateway" | "all" => commands::serve::gateway("127.0.0.1:8080").await,
            other => Err(mammoth_core::Error::Config(format!(
                "unknown role: {other} (try master, worker, gateway or all)"
            ))),
        },
```

and add `mammoth-gateway = { workspace = true }` to the CLI's `Cargo.toml`.

## Check the API works

```bash
cargo run -p mammoth-cli -- serve --role gateway
```

In another terminal:

```bash
curl -s localhost:8080/api/v1/cluster/report | python3 -m json.tool | head -20
```

```json
{
    "name": "local",
    "leader": "local",
    "safe_mode": false,
    "used": 3670016,
    "capacity": 1030792151040,
    "nodes": [
        {
            "id": "w1",
            "address": "127.0.0.1:7001",
            "rack": "/dc1/rack-a",
            "state": "healthy",
```

```bash
curl -s "localhost:8080/api/v1/fs/blocks?path=/data/sales.csv" | python3 -m json.tool | head
```

And the error path, which should give you a 404 *and* a usable body:

```bash
curl -s -w '\n%{http_code}\n' "localhost:8080/api/v1/fs?path=/nope"
```

```json
{"code":"E0101","docs":"https://projectorcha.github.io/Mammoth/errors/E0101","hints":[],"message":"no such path: /nope"}
404
```

## Step 4 · The front end

```bash
cd ui && npm install
```

Replace `ui/src/routes/+page.svelte` with a real overview:

```svelte
<script lang="ts">
  import { onMount } from 'svelte';
  import { api, type ClusterReport } from '$lib/api';

  let report = $state<ClusterReport | null>(null);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      report = await api.clusterReport();
    } catch (e) {
      error = String(e);
    }
  });

  const pct = (used: number, cap: number) => (cap === 0 ? 0 : (used / cap) * 100);

  function human(bytes: number): string {
    const units = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];
    let v = bytes, u = 0;
    while (v >= 1024 && u < units.length - 1) { v /= 1024; u++; }
    return `${v.toFixed(1)} ${units[u]}`;
  }
</script>

<h1>Mammoth {report?.name ?? ''}</h1>

{#if error}
  <p class="error">{error}</p>
{:else if !report}
  <p>loading…</p>
{:else}
  <section>
    <h2>Capacity</h2>
    <div class="meter">
      <div class="fill" style="width: {pct(report.used, report.capacity)}%"></div>
    </div>
    <p>{human(report.used)} / {human(report.capacity)}</p>
  </section>

  <section>
    <h2>Nodes</h2>
    <table>
      <thead>
        <tr><th>node</th><th>rack</th><th>state</th><th>used</th><th>blocks</th></tr>
      </thead>
      <tbody>
        {#each report.nodes as n (n.id)}
          <tr>
            <td>{n.id}</td>
            <td>{n.rack}</td>
            <td class={n.state}>{n.state}</td>
            <td>{human(n.used)}</td>
            <td>{n.blocks}</td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}

<style>
  .meter { background: #eee; height: 1.5rem; border-radius: 3px; overflow: hidden; }
  .fill { background: #2e7d32; height: 100%; }
  table { border-collapse: collapse; }
  th, td { text-align: left; padding: 0.3rem 1rem 0.3rem 0; }
  .dead { color: #b71c1c; }
  .warn { color: #f9a825; }
  .error { color: #b71c1c; }
</style>
```

`$state` is Svelte 5's rune syntax — it makes a variable reactive, so the page
re-renders when it changes. `ui/src/lib/api.ts` already has the typed client and
the `ClusterReport` interface; you wrote none of that, it shipped with the
scaffold.

```bash
npm run dev
```

Open <http://localhost:5173>. Vite proxies `/api` to port 8080 (that is the
`proxy` block in `vite.config.ts`), so the dev server talks to your real Rust
gateway with hot reload on the front end.

## Step 5 · Build it into the binary

```bash
cd ui && npm run build
```

That writes `ui/build/`, which is exactly the folder `rust-embed` points at.
Rebuild the binary and the UI is inside it:

```bash
cargo build -p mammoth-cli
./target/debug/mammoth serve --role gateway
```

Open <http://localhost:8080> — same page, no Node running.

Wire it into `xtask` so nobody has to remember the two steps. In
`xtask/src/main.rs`, replace the `build-ui` arm:

```rust
        Some("build-ui") => {
            let status = std::process::Command::new("npm")
                .args(["ci"])
                .current_dir("ui")
                .status()
                .expect("npm not found — install Node 20+");
            assert!(status.success(), "npm ci failed");

            let status = std::process::Command::new("npm")
                .args(["run", "build"])
                .current_dir("ui")
                .status()
                .expect("npm run build failed to start");
            assert!(status.success(), "npm run build failed");

            ExitCode::SUCCESS
        }
```

```bash
cargo xtask build-ui
```

## Commit it

```bash
cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace
```

```bash
cd ui && npm run check && cd ..
```

```bash
git add -A && git commit -m "feat(gateway): add REST API, embedded UI and serve --role gateway"
```

Note `ui/build/` and `ui/.svelte-kit/` are already in `.gitignore` — build
output does not belong in Git. CI rebuilds it.

## Exercises

1. **The file browser.** `/files` calling `api.list(path)`, with directories as
   links. This is the page people will actually use.
2. **The block matrix.** `/files/[...path]` calling `api.blocks(path)`. The same
   grid you built in chapter 8, in HTML. Use a CSS grid, one cell per replica.
3. **Live updates.** The scaffold's `subscribe()` in `api.ts` expects an SSE
   endpoint at `/api/v1/events`. Implement it with `axum::response::sse` and
   push a `cluster_report` every two seconds. Now the dashboard updates itself.
4. **`mammoth ui`.** Start the gateway and open the browser in one command.
   `webbrowser` is the crate.

## If it went wrong

**`rust-embed` fails with "folder does not exist"** — create the placeholder
`ui/build/index.html` as shown above, or run `npm run build` first.

**The page loads but every API call 404s** — check the gateway is on 8080 and
that you are hitting the Vite dev server on 5173, not the other way round. In
production (`serve --role gateway`) everything is on 8080.

**CORS errors in the browser console** — you are calling 8080 directly from
5173 instead of using the proxy. Use relative URLs (`/api/v1/...`), which is
what `api.ts` does.

**Changes to the Svelte code do not show up** — you are looking at the embedded
build on 8080, not the dev server on 5173. `rust-embed` bakes the assets in at
compile time; rebuild the binary to refresh them.

**`error[E0277]: the trait bound Arc<LocalBackend>: Backend is not satisfied`**
— you need `Arc<dyn Backend>`, not `Arc<LocalBackend>`. Write
`let backend: Arc<dyn Backend> = Arc::new(super::backend()?);`.

**`npm run check` complains about `$state`** — you are on Svelte 4. Runes need
Svelte 5. Check `ui/package.json` says `"svelte": "^5.0.0"`.

---

**Next:** [Chapter 10 — Publishing the docs to GitHub Pages](10-github-pages.md)
