use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use futures_util::StreamExt;
use http_body_util::BodyExt;
use mammoth_core::Backend;
use mammoth_local::{body, LocalBackend};
use std::{path::Path, sync::Arc};
use tower::ServiceExt;

async fn request(
    router: Router,
    method: &str,
    uri: &str,
    data: &[u8],
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = router
        .oneshot(
            Request::builder().method(method).uri(uri).body(Body::from(data.to_vec())).unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = response.into_body().collect().await.unwrap().to_bytes().to_vec();
    (status, headers, bytes)
}
#[tokio::test]
async fn dashboard_contract_real_data_errors_static_and_pagination() {
    let dir = tempfile::tempdir().unwrap();
    let be = Arc::new(
        LocalBackend::open(dir.path()).unwrap().with_block_size(100).with_inline_threshold(10),
    );
    let app = mammoth_gateway::router(be.clone());
    let (status, _, _) = request(
        app.clone(),
        "PUT",
        "/api/v1/fs/data?path=%2Fdata%2Fnotes%20%23%3F%25%20%E9%9B%AA.txt",
        &[9; 350],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, bytes) = request(
        app.clone(),
        "GET",
        "/api/v1/fs/stat?path=%2Fdata%2Fnotes%20%23%3F%25%20%E9%9B%AA.txt",
        b"",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let stat: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(stat["name"], "notes #?% 雪.txt");
    assert_eq!(stat["blocks"], 4);
    assert!(stat["modified"].as_i64().unwrap() > 1_000_000_000_000);
    let (_, _, bytes) = request(app.clone(), "GET", "/api/v1/cluster/report", b"").await;
    let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["nodes"].as_array().unwrap().len(), 6);
    assert_eq!(report["used"], 1050);
    assert!(report["read_path"].is_null());
    assert_eq!(report["capabilities"]["local"], true);
    for (uri, expected) in [
        ("/api/v1/fs/stat?path=/missing", 404),
        ("/api/v1/unknown", 404),
        ("/api/unknown", 404),
        ("/api/v1/cluster/report?minutes_ago=1", 501),
        ("/api/v1/fs?path=/data&limit=bad", 400),
    ] {
        let (status, headers, data) = request(app.clone(), "GET", uri, b"").await;
        assert_eq!(status.as_u16(), expected, "{uri}: {}", String::from_utf8_lossy(&data));
        assert_eq!(headers["content-type"], "application/json");
    }
    let (status, headers, _) = request(app.clone(), "GET", "/files/data/notes.txt", b"").await;
    assert_eq!(status, 200);
    assert!(headers["content-type"].to_str().unwrap().contains("text/html"));
    be.write(Path::new("/data/z"), body(vec![1])).await.unwrap();
    let (_, _, data) =
        request(app.clone(), "GET", "/api/v1/fs?path=/data&offset=1&limit=1", b"").await;
    let page: serde_json::Value = serde_json::from_slice(&data).unwrap();
    assert_eq!(page.as_array().unwrap().len(), 1);
    assert_eq!(page[0]["name"], "z");
}

#[tokio::test]
async fn s3_buckets_objects_ranges_head_listing_copy_and_errors() {
    let dir = tempfile::tempdir().unwrap();
    let be = Arc::new(LocalBackend::open(dir.path()).unwrap());
    let app = mammoth_gateway::s3::router(be);
    assert_eq!(request(app.clone(), "PUT", "/warehouse", b"").await.0, 200);
    assert_eq!(
        request(app.clone(), "PUT", "/warehouse/data/a%20%23.txt", b"0123456789").await.0,
        200
    );
    assert_eq!(request(app.clone(), "PUT", "/warehouse/data/b", b"b").await.0, 200);
    let (_, headers, bytes) =
        request(app.clone(), "HEAD", "/warehouse/data/a%20%23.txt", b"").await;
    assert_eq!(headers["content-length"], "10");
    assert!(bytes.is_empty());
    for (range, expected, status) in [
        ("bytes=2-5", b"2345".as_slice(), 206),
        ("bytes=-3", b"789".as_slice(), 206),
        ("bytes=50-", b"".as_slice(), 416),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/warehouse/data/a%20%23.txt")
                    .header("range", range)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert!(response.headers().contains_key("content-range"));
        assert_eq!(response.into_body().collect().await.unwrap().to_bytes(), expected);
    }
    let (_, _, bytes) =
        request(app.clone(), "GET", "/warehouse?list-type=2&prefix=data%2F&max-keys=1", b"").await;
    let xml = String::from_utf8(bytes).unwrap();
    assert!(xml.contains("<IsTruncated>true</IsTruncated>"));
    assert!(xml.contains("<Key>data/a #.txt</Key>"));
    let (_, _, bytes) =
        request(app.clone(), "GET", "/warehouse?list-type=2&delimiter=%2F", b"").await;
    assert!(String::from_utf8(bytes).unwrap().contains("<CommonPrefixes><Prefix>data/</Prefix>"));
    assert_eq!(request(app.clone(), "POST", "/warehouse/a?uploads", b"").await.0, 501);
    assert_eq!(request(app.clone(), "DELETE", "/warehouse", b"").await.0, 409);
    assert_eq!(request(app.clone(), "GET", "/warehouse/missing", b"").await.0, 404);
    assert_eq!(request(app.clone(), "DELETE", "/warehouse/missing", b"").await.0, 204);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/warehouse/copy")
                .header("x-amz-copy-source", "/warehouse/data/a%20%23.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(request(app.clone(), "GET", "/warehouse/copy", b"").await.2, b"0123456789");
}

#[tokio::test]
async fn remote_backend_roundtrip_over_tcp_and_live_events() {
    let dir = tempfile::tempdir().unwrap();
    let be = Arc::new(LocalBackend::open(dir.path()).unwrap());
    let app = mammoth_gateway::router(be.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let remote = mammoth_client::ClusterBackend::connect(&format!("http://{address}")).unwrap();
    assert_eq!(remote.stat(Path::new("/absent")).await.unwrap_err().code(), "E0101");
    assert_eq!(remote.write(Path::new("/"), body("invalid")).await.unwrap_err().code(), "E0102");
    let payload = vec![67; 3 * 1024 * 1024];
    remote.write(Path::new("/remote #.bin"), body(payload.clone())).await.unwrap();
    let mut stream = remote.read(Path::new("/remote #.bin"), 1024..2048).await.unwrap();
    let mut got = vec![];
    while let Some(c) = stream.next().await {
        got.extend_from_slice(&c.unwrap());
    }
    assert_eq!(got, payload[1024..2048]);
    assert_eq!(remote.stat(Path::new("/remote #.bin")).await.unwrap().len, payload.len() as u64);
    remote.rename(Path::new("/remote #.bin"), Path::new("/renamed")).await.unwrap();
    remote.remove(Path::new("/renamed"), false).await.unwrap();
    assert!(be.list(Path::new("/")).await.unwrap().is_empty());
    server.abort();
}

#[tokio::test]
async fn bad_s3_digest_preserves_the_previous_object_and_sse_emits_json() {
    let dir = tempfile::tempdir().unwrap();
    let be = Arc::new(LocalBackend::open(dir.path()).unwrap());
    let app = mammoth_gateway::s3::router(be.clone());
    request(app.clone(), "PUT", "/bucket", b"").await;
    request(app.clone(), "PUT", "/bucket/key", b"original").await;
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/bucket/key")
                .header("content-md5", "invalid")
                .body(Body::from("replacement"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 400);
    assert_eq!(request(app, "GET", "/bucket/key", b"").await.2, b"original");
    let response = mammoth_gateway::router(be)
        .oneshot(Request::builder().uri("/api/v1/events").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["content-type"], "text/event-stream");
    let mut body = response.into_body();
    let frame = tokio::time::timeout(std::time::Duration::from_secs(2), body.frame())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let text = String::from_utf8(frame.into_data().unwrap().to_vec()).unwrap();
    assert!(text.contains("block_health"));
    assert!(text.contains("{\"refresh\":true}"));
}

#[tokio::test]
async fn dashboard_jobs_complete_report_failures_and_protect_existing_outputs() {
    let dir = tempfile::tempdir().unwrap();
    let be = Arc::new(LocalBackend::open(dir.path()).unwrap());
    be.write(Path::new("/words"), body("z a z\n")).await.unwrap();
    let app = mammoth_gateway::router(be.clone());
    let (status, _, bytes) = request(
        app.clone(),
        "POST",
        "/api/v1/jobs?kind=wordcount&input=/words&output=/counts",
        b"",
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let accepted: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let mut finished = None;
    for _ in 0..100 {
        let (_, _, bytes) = request(app.clone(), "GET", "/api/v1/jobs", b"").await;
        let jobs: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        if jobs[0]["state"] != "running" {
            finished = Some(jobs[0].clone());
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let finished = finished.expect("job must finish");
    assert_eq!(finished["id"], accepted["id"]);
    assert_eq!(finished["state"], "succeeded");
    assert_eq!(finished["stages"][0]["done"], 1);
    assert_eq!(finished["tasks"][0]["state"], "done");
    assert!(finished["tasks"][0]["dur_s"].as_f64().unwrap() >= 0.0);
    assert_eq!(
        request(app.clone(), "GET", "/api/v1/fs/data?path=/counts", b"").await.2,
        b"a\t1\nz\t2\n"
    );
    assert_eq!(
        request(app.clone(), "POST", "/api/v1/jobs?kind=sort&input=/words&output=/counts", b"")
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        request(app.clone(), "POST", "/api/v1/jobs?kind=sort&input=/words&output=/words", b"")
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    be.write(Path::new("/binary"), body(vec![255])).await.unwrap();
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/jobs?kind=sort&input=/binary&output=/counts&overwrite=true",
            b""
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    for _ in 0..100 {
        let (_, _, bytes) = request(app.clone(), "GET", "/api/v1/jobs", b"").await;
        let jobs: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        if jobs[0]["state"] != "running" {
            assert_eq!(jobs[0]["state"], "failed");
            assert_eq!(jobs[0]["tasks"][0]["state"], "failed");
            assert!(jobs[0]["error"].as_str().unwrap().contains("UTF-8"));
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(
        request(app.clone(), "GET", "/api/v1/fs/data?path=/counts", b"").await.2,
        b"a\t1\nz\t2\n"
    );
}

#[tokio::test]
async fn file_filters_apply_before_pagination_and_management_changes_real_data() {
    let dir = tempfile::tempdir().unwrap();
    let be = Arc::new(LocalBackend::open(dir.path()).unwrap());
    let app = mammoth_gateway::router(be.clone());
    for name in ["a", "match-b", "match-c", "z"] {
        be.write(Path::new(&format!("/files/{name}")), body(name)).await.unwrap();
    }
    let (_, _, bytes) =
        request(app.clone(), "GET", "/api/v1/fs?path=/files&name=MATCH&offset=1&limit=1", b"")
            .await;
    let rows: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["name"], "match-c");
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/fs/rename?path=/files/match-c&to=/files/renamed",
            b""
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            app.clone(),
            "POST",
            "/api/v1/fs/attributes?path=/files/renamed&mode=416&owner=alice&group=team",
            b""
        )
        .await
        .0,
        StatusCode::OK
    );
    let status = be.stat(Path::new("/files/renamed")).await.unwrap();
    assert_eq!(status.owner, "alice");
    assert_eq!(status.mode, 0o640);
    assert_eq!(
        request(app.clone(), "DELETE", "/api/v1/fs?path=/files", b"").await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(app.clone(), "DELETE", "/api/v1/fs?path=/files&recursive=true", b"").await.0,
        StatusCode::OK
    );
    assert!(be.stat(Path::new("/files")).await.is_err());
}
