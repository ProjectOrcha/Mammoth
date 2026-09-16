//! Local dashboard access to the same memory store as CLI and MCP.
use super::*;
use mammoth_memory::{Remember, Store};
pub async fn handle(
    dashboard: &Dashboard,
    op: &str,
    q: BTreeMap<String, String>,
    method: Method,
    body: Body,
) -> std::result::Result<Response, ApiError> {
    let root = dashboard
        .memory_root()
        .ok_or(Error::NotImplemented("memory requires a configured local service"))?;
    let project = q
        .get("project")
        .cloned()
        .ok_or_else(|| Error::InvalidInput("project is required".into()))?;
    let input: Option<Remember> = if method == Method::POST && op == "memory" {
        let bytes = axum::body::to_bytes(body, 128 * 1024)
            .await
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        Some(serde_json::from_slice(&bytes).map_err(|e| Error::InvalidInput(e.to_string()))?)
    } else {
        None
    };
    let op = op.to_owned();
    let result = tokio::task::spawn_blocking(move || -> mammoth_memory::Result<Value> {
        let mut store = Store::open(root)?;
        if let Some(input) = input {
            return Ok(serde_json::to_value(store.remember(&project, input)?)?);
        }
        let key = q.get("key").map(String::as_str).unwrap_or("");
        match (method.as_str(), op.as_str()) {
            ("GET", "memory") => Ok(serde_json::to_value(store.recall(
                &project,
                q.get("query").map(String::as_str).unwrap_or(""),
                20,
                32000,
            )?)?),
            ("GET", "memory/get") => Ok(serde_json::to_value(store.get(&project, key)?)?),
            ("GET", "memory/history") => Ok(json!({"memories":store.history(&project,key,10)?})),
            _ => Err(mammoth_memory::Error::NotFound("memory endpoint".into())),
        }
    })
    .await
    .map_err(|e| Error::Io(std::io::Error::other(e)))?;
    match result {
        Ok(value) => Ok(Json(value).into_response()),
        Err(error) => {
            let status = match &error {
                mammoth_memory::Error::Conflict => StatusCode::CONFLICT,
                mammoth_memory::Error::NotFound(_) => StatusCode::NOT_FOUND,
                mammoth_memory::Error::Invalid(_) => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            Ok((status, Json(json!({"message":error.to_string()}))).into_response())
        }
    }
}
