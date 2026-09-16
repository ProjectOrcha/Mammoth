use rmcp::{model::CallToolRequestParams, transport::TokioChildProcess, ServiceExt};
use serde_json::json;
use tokio::process::Command;
async fn connect(
    root: &std::path::Path,
    read_only: bool,
) -> rmcp::service::RunningService<rmcp::RoleClient, ()> {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mammoth-mcp"));
    command.arg("--local-root").arg(root).args(["--project", "app"]);
    if read_only {
        command.arg("--read-only");
    }
    ().serve(TokioChildProcess::new(command).unwrap()).await.unwrap()
}
#[tokio::test]
async fn actual_stdio_client_restart_and_read_only() {
    let dir = tempfile::tempdir().unwrap();
    let client = connect(dir.path(), false).await;
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 5);
    let put=CallToolRequestParams::new("memory_remember").with_arguments(json!({"key":"handoff","title":"Next step","content":"Run the integration tests","kind":"handoff","tags":["tests"]}).as_object().unwrap().clone());
    assert_ne!(client.call_tool(put).await.unwrap().is_error, Some(true));
    let recall = CallToolRequestParams::new("memory_recall")
        .with_arguments(json!({"query":"integration"}).as_object().unwrap().clone());
    let result = client.call_tool(recall).await.unwrap();
    assert_eq!(result.structured_content.unwrap()["memories"][0]["key"], "handoff");
    let invalid = CallToolRequestParams::new("memory_recall")
        .with_arguments(json!({"max_bytes":1}).as_object().unwrap().clone());
    assert_eq!(client.call_tool(invalid).await.unwrap().is_error, Some(true));
    let cross_project = CallToolRequestParams::new("memory_get")
        .with_arguments(json!({"key":"handoff","project":"other"}).as_object().unwrap().clone());
    assert!(client.call_tool(cross_project).await.map_or(true, |r| r.is_error == Some(true)));
    client.cancel().await.unwrap();
    let client = connect(dir.path(), true).await;
    let tools = client.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 3);
    assert!(tools.iter().all(|t| !matches!(t.name.as_ref(), "memory_remember" | "memory_forget")));
    let get = CallToolRequestParams::new("memory_get")
        .with_arguments(json!({"key":"handoff"}).as_object().unwrap().clone());
    assert_eq!(client.call_tool(get).await.unwrap().structured_content.unwrap()["revision"], 1);
    let forget = CallToolRequestParams::new("memory_forget").with_arguments(
        json!({"key":"handoff","expected_revision":1}).as_object().unwrap().clone(),
    );
    assert!(client.call_tool(forget).await.map_or(true, |r| r.is_error == Some(true)));
    client.cancel().await.unwrap();
}

#[tokio::test]
async fn legacy_stdio_acknowledgement_survives_forced_process_exit() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mammoth-mcp"))
        .arg("--local-root")
        .arg(dir.path())
        .args(["--project", "app"])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    let mut lines = BufReader::new(child.stdout.take().unwrap()).lines();
    input.write_all(b"{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":\"2025-06-18\",\"capabilities\":{},\"clientInfo\":{\"name\":\"test\",\"version\":\"1\"}}}\n").await.unwrap();
    let line = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let init: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(init["result"]["protocolVersion"], "2025-06-18");
    input
        .write_all(b"{\"jsonrpc\":\"2.0\",\"method\":\"notifications/initialized\"}\n")
        .await
        .unwrap();
    let message = json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"memory_remember","arguments":{"key":"durable","title":"Committed","content":"Survives a killed agent process","kind":"note"}}});
    input.write_all(format!("{message}\n").as_bytes()).await.unwrap();
    let line = tokio::time::timeout(std::time::Duration::from_secs(5), lines.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let result: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert_eq!(result["result"]["structuredContent"]["revision"], 1);
    child.kill().await.unwrap();
    child.wait().await.unwrap();
    assert_eq!(
        mammoth_memory::Store::open(dir.path()).unwrap().get("app", "durable").unwrap().content,
        "Survives a killed agent process"
    );
}
