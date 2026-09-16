//! HTTP filesystem API, dashboard adapters, SSE and an embedded dashboard.
#![forbid(unsafe_code)]
mod browser;
mod dashboard;
mod jobs;
pub use dashboard::Dashboard;
pub mod s3;
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Path as AxumPath, Query, State},
    http::{header, Method, StatusCode, Uri},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{any, get},
    Extension, Json, Router,
};
use base64::Engine;
use futures_util::StreamExt;
use mammoth_core::{Backend, Error, FileStatus, Result};
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    convert::Infallible,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

#[derive(Clone)]
pub struct Gateway {
    pub backend: Arc<dyn Backend>,
    pub jobs: jobs::JobStore,
    pub dashboard: Dashboard,
}
include!(concat!(env!("OUT_DIR"), "/assets.rs"));

pub struct ApiError(pub Error);
impl From<Error> for ApiError {
    fn from(e: Error) -> Self {
        Self(e)
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match self.0 {
            Error::Remote { status, .. } => {
                StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY)
            }
            Error::NotFound(_) => StatusCode::NOT_FOUND,
            Error::AlreadyExists(_) | Error::LeaseHeld { .. } => StatusCode::CONFLICT,
            Error::InvalidInput(_) | Error::WrongKind { .. } | Error::Config(_) => {
                StatusCode::BAD_REQUEST
            }
            Error::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            Error::NotEnoughWorkers { .. } | Error::SafeMode { .. } => {
                StatusCode::SERVICE_UNAVAILABLE
            }
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status,Json(json!({"code":self.0.code(),"message":self.0.to_string(),"hints":self.0.hints(),"docs":self.0.docs_url()}))).into_response()
    }
}

pub fn router(backend: Arc<dyn Backend>) -> Router {
    let (stop, _) = tokio::sync::watch::channel(false);
    service_router(backend, jobs::JobStore::default(), Dashboard::default(), stop)
}
pub fn router_with_dashboard(backend: Arc<dyn Backend>, dashboard: Dashboard) -> Router {
    let (stop, _) = tokio::sync::watch::channel(false);
    service_router(backend, jobs::JobStore::default(), dashboard, stop)
}
fn service_router(
    backend: Arc<dyn Backend>,
    jobs: jobs::JobStore,
    dashboard: Dashboard,
    stop: tokio::sync::watch::Sender<bool>,
) -> Router {
    Router::new()
        .route("/healthz", get(|| async { Json(json!({"status":"ok"})) }))
        .route("/readyz", get(readiness))
        .route("/api/v1/events", get(events))
        .route("/api/v1/*op", any(api))
        .route("/api", any(api_not_found))
        .route("/api/*unknown", any(api_not_found))
        .fallback(static_file)
        .layer(DefaultBodyLimit::disable())
        .layer(Extension(stop))
        .layer(axum::middleware::from_fn(browser::guard))
        .with_state(Gateway { backend, jobs, dashboard })
}
/// A cheap namespace probe, distinct from a full replica health scan.
async fn readiness(State(state): State<Gateway>) -> Response {
    match state.backend.stat(Path::new("/")).await {
        Ok(root) if root.is_dir => Json(json!({"status":"ready"})).into_response(),
        _ => {
            (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status":"unavailable"}))).into_response()
        }
    }
}

async fn api_not_found() -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"code":"E0101","message":"unknown API endpoint"})))
        .into_response()
}

pub fn file_json(s: &FileStatus) -> Value {
    json!({"path":s.path,"name":s.path.file_name().unwrap_or_default().to_string_lossy(),"is_dir":s.is_dir,"len":s.len,"block_size":s.block_size,"replication":s.replication,"policy": if s.is_dir { "directory".into() } else if s.inlined { "inline".into() } else { format!("replication-{}",s.replication.unwrap_or(0)) },"blocks":s.blocks,"inlined":s.inlined,"mode":s.mode,"owner":s.owner,"group":s.group,"modified":s.modified.saturating_mul(1000),"checksum":s.checksum})
}
async fn report(be: &dyn Backend) -> Result<Value> {
    let core = be.cluster_report().await?;
    let nodes: Vec<_> = core.nodes.iter().map(|n|json!({"id":n.id,"rack":n.rack,"address":n.address,"state":n.state,"used":n.used,"capacity":n.capacity,"fragments":n.blocks,"volumes":n.volumes,"disk_p99_ms":null,"read_bps":null,"write_bps":null,"read_series":[]})).collect();
    Ok(
        json!({"name":core.name,"leader":core.leader,"safe_mode":core.safe_mode,"used":core.used,"capacity":core.capacity,"topology_epoch":1,"placement":"rendezvous","nodes":nodes,"health":core.health,"capabilities":{"local":true,"distributed_metrics":false,"history":false,"jobs":true},"memory_cache":be.cache_stats(),"read_path":null,"write_path":null,"repair":null,"start":null,"throughput":null,"alerts":[],"raft":[],"raft_index":null,"snapshot_age_s":null}),
    )
}
async fn blocks(be: &dyn Backend, path: &Path) -> Result<Value> {
    let s = be.stat(path).await?;
    let placements = be.block_layout(path).await?;
    let mut racks = BTreeMap::new();
    let mut nodes = BTreeSet::new();
    let mut warnings = vec![];
    let policy = if s.inlined {
        "inline".into()
    } else {
        format!("replication-{}", s.replication.unwrap_or(0))
    };
    let values: Vec<_> = placements.iter().map(|b| {
        let fragments: Vec<_> = b.replicas.iter().enumerate().map(|(i,r)| {
            nodes.insert(r.node.0.clone()); racks.insert(r.node.0.clone(),r.rack.clone());
            let bad = r.state == mammoth_core::ReplicaState::Corrupt;
            if bad { warnings.push(format!("Block {} has a damaged or missing replica on {}",b.id.0,r.node.0)); }
            json!({"kind":"replica","idx":i,"node":r.node,"rack":r.rack,"state":if bad {"corrupt"} else {"ok"},"preferred":r.state == mammoth_core::ReplicaState::Primary})
        }).collect();
        json!({"id":b.id,"index":b.index,"len":b.len,"policy":policy,"fragments":fragments})
    }).collect();
    Ok(
        json!({"path":s.path,"len":s.len,"block_size":s.block_size,"policy":policy,"inlined":s.inlined,"blocks":values,"nodes":nodes,"racks":racks,"warnings":warnings}),
    )
}
pub use mammoth_core::backend::walk;

async fn treemap(be: &dyn Backend, path: &Path, depth: usize) -> Result<Value> {
    let all = walk(be, path).await?;
    fn build(all: &[FileStatus], s: &FileStatus, depth: usize) -> Value {
        let value: u64 = all
            .iter()
            .filter(|f| !f.is_dir && (f.path == s.path || f.path.starts_with(&s.path)))
            .map(|f| f.len)
            .sum();
        let mut v = json!({"name":s.path.file_name().unwrap_or_default().to_string_lossy(),"path":s.path,"value":value,"age_days":(now_ms()/1000 - s.modified).max(0) as f64/86400.0,"reads":null});
        if s.is_dir && depth > 0 {
            v["children"] = all
                .iter()
                .filter(|f| f.path != s.path && f.path.parent() == Some(&s.path))
                .map(|f| build(all, f, depth - 1))
                .collect();
        }
        v
    }
    let root = all
        .iter()
        .find(|s| s.path == path)
        .or_else(|| all.first())
        .ok_or_else(|| Error::NotFound(path.into()))?;
    Ok(build(&all, root, depth.min(32)))
}
pub async fn skew(be: &dyn Backend, path: &Path) -> Result<Value> {
    let all = walk(be, path).await?;
    let files: Vec<_> = all.iter().filter(|s| !s.is_dir).collect();
    let mut sizes: Vec<_> = files.iter().map(|s| s.len).collect();
    sizes.sort_unstable();
    let n = sizes.len();
    Ok(
        json!({"path":path,"files":n,"total":sizes.iter().sum::<u64>(),"median":if n==0 {0} else {sizes[n/2]},"p99":if n==0 {0} else {sizes[((n*99).div_ceil(100)).saturating_sub(1)]},"max":sizes.last().copied().unwrap_or(0),"points":files.iter().map(|s|json!({"partition":s.path,"size":s.len,"reads":null,"writes":null})).collect::<Vec<_>>()}),
    )
}
fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
fn param<'a>(q: &'a BTreeMap<String, String>, key: &str, default: &'a str) -> &'a str {
    q.get(key).map(String::as_str).unwrap_or(default)
}
fn number(q: &BTreeMap<String, String>, key: &str, default: u64) -> Result<u64> {
    match q.get(key) {
        Some(v) => v.parse().map_err(|_| Error::InvalidInput(format!("invalid {key}"))),
        None => Ok(default),
    }
}

async fn api(
    State(state): State<Gateway>,
    AxumPath(op): AxumPath<String>,
    Query(q): Query<BTreeMap<String, String>>,
    method: Method,
    body: Body,
) -> std::result::Result<Response, ApiError> {
    let be = state.backend.as_ref();
    let path = PathBuf::from(param(&q, "path", "/"));
    if number(&q, "minutes_ago", 0)? != 0 {
        return Err(Error::NotImplemented("historical replay").into());
    }
    let value = match (method.as_str(), op.as_str()) {
        ("GET", "cluster/report") => {
            let mut value = report(be).await?;
            value["capabilities"]["benchmarks"] = json!(state.dashboard.available());
            value["capabilities"]["configuration"] = json!(state.dashboard.available());
            value
        }
        ("GET", "benchmarks") => state.dashboard.list().await?,
        ("POST", "benchmarks") => {
            let bytes = axum::body::to_bytes(body, 64 * 1024)
                .await
                .map_err(|e| Error::InvalidInput(e.to_string()))?;
            let options =
                serde_json::from_slice(&bytes).map_err(|e| Error::InvalidInput(e.to_string()))?;
            let value = state.dashboard.submit(options).await?;
            return Ok((StatusCode::ACCEPTED, Json(value)).into_response());
        }
        ("GET", "configuration") => state.dashboard.configuration(None)?,
        ("POST", "configuration/validate") => {
            let bytes = axum::body::to_bytes(body, 64 * 1024)
                .await
                .map_err(|e| Error::InvalidInput(e.to_string()))?;
            let settings =
                serde_json::from_slice(&bytes).map_err(|e| Error::InvalidInput(e.to_string()))?;
            state.dashboard.configuration(Some(settings))?
        }
        ("GET", "core/report") => serde_json::to_value(be.cluster_report().await?)
            .map_err(|e| Error::InvalidInput(e.to_string()))?,
        ("GET", "core/etag") => json!(be.etag(&path).await?),
        ("GET", "core/stat") => serde_json::to_value(be.stat(&path).await?)
            .map_err(|e| Error::InvalidInput(e.to_string()))?,
        ("GET", "core/list") => serde_json::to_value(be.list(&path).await?)
            .map_err(|e| Error::InvalidInput(e.to_string()))?,
        ("GET", "core/blocks") => serde_json::to_value(be.block_layout(&path).await?)
            .map_err(|e| Error::InvalidInput(e.to_string()))?,
        ("GET", "nodes") => report(be).await?["nodes"].clone(),
        ("GET", p) if p.starts_with("nodes/") => report(be).await?["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|n| n["id"].as_str() == Some(&p[6..]))
            .cloned()
            .ok_or_else(|| Error::NotFound(p.into()))?,
        ("GET", "fs") => be
            .list(&path)
            .await?
            .iter()
            .filter(|s| {
                s.path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_lowercase()
                    .contains(&param(&q, "name", "").to_lowercase())
            })
            .skip(number(&q, "offset", 0)? as usize)
            .take(number(&q, "limit", 200)?.min(10000) as usize)
            .map(file_json)
            .collect(),
        ("GET", "fs/search") => {
            walk(be, &path).await?.iter().filter(|s| !s.is_dir).take(1000).map(file_json).collect()
        }
        ("GET", "fs/stat") => file_json(&be.stat(&path).await?),
        ("GET", "fs/blocks") => blocks(be, &path).await?,
        ("GET", "fs/data") => {
            let start = number(&q, "start", 0)?;
            let end = number(&q, "end", u64::MAX)?;
            let snapshot = be.open_read(&path, start..end).await?;
            let info = base64::engine::general_purpose::STANDARD.encode(
                json!({"status":snapshot.status,"etag":snapshot.etag,"range":snapshot.range})
                    .to_string(),
            );
            return Ok((
                [
                    (header::CONTENT_TYPE, "application/octet-stream".into()),
                    (
                        header::CONTENT_LENGTH,
                        (snapshot.range.end - snapshot.range.start).to_string(),
                    ),
                    (header::ETAG, format!("\"{}\"", snapshot.etag)),
                    (header::HeaderName::from_static("x-mammoth-snapshot"), info),
                ],
                Body::from_stream(snapshot.data),
            )
                .into_response());
        }
        ("PUT", "fs/data") => {
            let data = Box::pin(
                body.into_data_stream().map(|r| r.map_err(|e| Error::Io(std::io::Error::other(e)))),
            );
            match param(&q, "create", "false") {
                "true" => be.create(&path, data).await?,
                "false" => be.write(&path, data).await?,
                _ => return Err(Error::InvalidInput("create must be true or false".into()).into()),
            }
            json!({"ok":true})
        }
        ("PUT", "fs/directory") => {
            be.mkdir(&path, param(&q, "parents", "false") == "true").await?;
            json!({"ok":true})
        }
        ("DELETE", "fs") => {
            be.remove(&path, param(&q, "recursive", "false") == "true").await?;
            json!({"ok":true})
        }
        ("POST", "fs/rename") => {
            be.rename(&path, Path::new(param(&q, "to", "/"))).await?;
            json!({"ok":true})
        }
        ("POST", "fs/attributes") => {
            let mode = q
                .get("mode")
                .map(|v| v.parse::<u32>().map_err(|_| Error::InvalidInput("invalid mode".into())))
                .transpose()?;
            be.set_attributes(&path, mode, q.get("owner").cloned(), q.get("group").cloned())
                .await?;
            json!({"ok":true})
        }
        ("POST", "fs/replication") => {
            let n = number(&q, "replication", 3)?;
            let n =
                u8::try_from(n).map_err(|_| Error::InvalidInput("invalid replication".into()))?;
            be.set_replication(&path, n).await?;
            json!({"ok":true})
        }
        ("POST", "admin/repair") => json!({"repaired":be.repair().await?}),
        ("POST", "admin/gc") => json!({"removed":be.gc().await?}),
        ("GET", "distribution/heat") => {
            let metric = param(&q, "metric", "usage");
            if !["usage", "fragments"].contains(&metric) {
                return Err(Error::NotImplemented("performance metrics").into());
            }
            be.cluster_report().await?.nodes.iter().map(|n|json!({"node":n.id,"rack":n.rack,"state":n.state,"usage":if n.capacity==0 {0.0} else {n.used as f64/n.capacity as f64*100.0},"fragments":n.blocks,"read_qps":null,"write_qps":null,"disk_p99_ms":null})).collect()
        }
        ("GET", "distribution/treemap") => {
            treemap(be, &path, number(&q, "depth", 3)? as usize).await?
        }
        ("GET", "distribution/skew") => skew(be, &path).await?,
        ("GET", "distribution/topology") => {
            let r = be.cluster_report().await?;
            let racks: BTreeSet<_> = r.nodes.iter().map(|n| &n.rack).collect();
            json!({"epoch":1,"nodes":r.nodes.iter().map(|n|json!({"id":n.id,"rack":n.rack,"capacity":n.capacity,"used":n.used,"state":n.state})).collect::<Vec<_>>(),"racks":racks,"links":[]})
        }
        ("GET", "distribution/flow") => {
            json!({"available":false,"window_s":0,"nodes":[],"links":[]})
        }
        ("GET", "jobs") => json!(state.jobs.list().await),
        ("POST", "jobs") => {
            let job = jobs::submit(
                &state,
                param(&q, "kind", ""),
                PathBuf::from(param(&q, "input", "")),
                PathBuf::from(param(&q, "output", "")),
                param(&q, "overwrite", "false") == "true",
            )
            .await?;
            return Ok((StatusCode::ACCEPTED, Json(job)).into_response());
        }
        _ => return Ok(api_not_found().await),
    };
    Ok(Json(value).into_response())
}
async fn events(
    Extension(stop): Extension<tokio::sync::watch::Sender<bool>>,
) -> Sse<impl futures_util::Stream<Item = std::result::Result<Event, Infallible>>> {
    let stream = futures_util::stream::unfold(
        tokio::time::interval(Duration::from_secs(2)),
        |mut timer| async move {
            timer.tick().await;
            Some((Ok(Event::default().event("block_health").data("{\"refresh\":true}")), timer))
        },
    );
    let stream = stream.take_until(wait_for_stop(stop.subscribe()));
    Sse::new(stream).keep_alive(KeepAlive::default())
}
async fn static_file(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let asset = Assets::get(path).or_else(|| {
        if !path.starts_with("files/") && path.rsplit('/').next().is_some_and(|s| s.contains('.')) {
            None
        } else {
            Assets::get("index.html")
        }
    });
    match asset {
        Some(file) => {
            let mime = if Assets::get(path).is_some() {
                mime_guess::from_path(path).first_or_octet_stream().to_string()
            } else {
                "text/html; charset=utf-8".into()
            };
            ([(header::CONTENT_TYPE, mime)], file.data.into_owned()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// Bind both listeners before returning; a port collision cannot leave half a service.
pub async fn serve(backend: Arc<dyn Backend>, ui: &str, s3: &str) -> Result<()> {
    let ui = tokio::net::TcpListener::bind(ui).await?;
    let s3_listener = tokio::net::TcpListener::bind(s3).await?;
    eprintln!(
        "Mammoth dashboard: http://{}\nS3 endpoint: http://{}\nPress Ctrl-C to stop.",
        ui.local_addr()?,
        s3_listener.local_addr()?
    );
    serve_with_shutdown(backend, ui, s3_listener, shutdown_signal()).await
}

/// Ctrl-C and service-manager termination both drain the service cleanly.
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        if let Ok(mut terminate) =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {},
                _ = terminate.recv() => {},
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}

async fn wait_for_stop(mut rx: tokio::sync::watch::Receiver<bool>) {
    if !*rx.borrow() && rx.changed().await.is_err() {
        // Standalone routers have no shutdown driver. Dropping a one-shot
        // router must not end the streaming response it just returned.
        std::future::pending::<()>().await;
    }
}

/// Run pre-bound listeners with one shutdown notification for HTTP and SSE.
/// Finish accepted requests and detached dashboard jobs before returning.
pub async fn serve_with_shutdown(
    backend: Arc<dyn Backend>,
    ui: tokio::net::TcpListener,
    s3_listener: tokio::net::TcpListener,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    serve_configured_with_shutdown(backend, ui, s3_listener, Dashboard::default(), shutdown).await
}

pub async fn serve_configured_with_shutdown(
    backend: Arc<dyn Backend>,
    ui: tokio::net::TcpListener,
    s3_listener: tokio::net::TcpListener,
    dashboard: Dashboard,
    shutdown: impl std::future::Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let (stop, _) = tokio::sync::watch::channel(false);
    let a = stop.subscribe();
    let b = stop.subscribe();
    let jobs = jobs::JobStore::default();
    let app = service_router(backend.clone(), jobs.clone(), dashboard.clone(), stop.clone());
    let signal = tokio::spawn(async move {
        shutdown.await;
        let _ = stop.send(true);
    });
    let result = tokio::try_join!(
        axum::serve(ui, app).with_graceful_shutdown(wait_for_stop(a)),
        axum::serve(s3_listener, s3::router(backend)).with_graceful_shutdown(wait_for_stop(b))
    );
    signal.abort();
    result?;
    jobs.wait_idle().await;
    dashboard.wait_idle().await;
    Ok(())
}
