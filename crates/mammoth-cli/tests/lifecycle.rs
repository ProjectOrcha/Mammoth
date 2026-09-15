use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpStream},
    path::Path,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env_remove("MAMMOTH_MASTERS")
        .env_remove("MAMMOTH_CONFIG")
        .arg("--local-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
struct Server(Child);
impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
fn start(root: &Path, command: &str) -> (Server, SocketAddr, SocketAddr) {
    let child = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env_remove("MAMMOTH_MASTERS")
        .env_remove("MAMMOTH_CONFIG")
        .arg("--local-root")
        .arg(root)
        .args([command, "--ui-listen", "127.0.0.1:0", "--s3-listen", "127.0.0.1:0"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut server = Server(child);
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        assert!(server.0.try_wait().unwrap().is_none(), "server exited before ready");
        if let Ok(bytes) = std::fs::read(root.join("service.json")) {
            let state: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            return (
                server,
                state["ui"].as_str().unwrap().parse().unwrap(),
                state["s3"].as_str().unwrap().parse().unwrap(),
            );
        }
        assert!(Instant::now() < deadline, "server never became ready");
        std::thread::sleep(Duration::from_millis(25));
    }
}
fn request(address: SocketAddr, path: &str) -> TcpStream {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2)).unwrap();
    stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    stream
}
fn finish(server: &mut Server) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(status) = server.0.try_wait().unwrap() {
            assert!(status.success());
            break;
        }
        assert!(Instant::now() < deadline, "server did not exit");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn stop_drains_live_events_releases_both_ports_and_preserves_files() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("store");
    let (mut server, ui, s3) = start(&root, "quickstart");
    let status = run(&root, &["status", "--json"]);
    let state: serde_json::Value = serde_json::from_slice(&status.stdout).unwrap();
    assert_eq!(state["state"], "running");
    let original = run(&root, &["cat", "/sample/words.txt"]);
    assert!(original.status.success());
    let mut health = String::new();
    request(ui, "/healthz").read_to_string(&mut health).unwrap();
    assert!(health.contains("200 OK"));
    let mut events = request(ui, "/api/v1/events");
    let mut chunk = [0; 4096];
    assert!(events.read(&mut chunk).unwrap() > 0);
    let duplicate = run(
        &root,
        &["serve", "--ui-listen", "127.0.0.1:0", "--s3-listen", "127.0.0.1:0", "--json"],
    );
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already running"));
    let other_root = dir.path().join("other");
    assert!(run(&other_root, &["stop", "--json"]).status.success());
    assert!(!other_root.exists());
    assert!(server.0.try_wait().unwrap().is_none());
    let out = run(&root, &["stop", "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["state"],
        "stopped"
    );
    events.read_to_end(&mut Vec::new()).unwrap();
    finish(&mut server);
    let mut banner = String::new();
    server.0.stderr.take().unwrap().read_to_string(&mut banner).unwrap();
    assert!(banner.contains(include_str!("../assets/banner.txt").trim()));
    assert!(TcpStream::connect(ui).is_err());
    assert!(TcpStream::connect(s3).is_err());
    assert!(!root.join("service.json").exists());
    assert_eq!(run(&root, &["cat", "/sample/words.txt"]).stdout, original.stdout);
    assert!(run(&root, &["shutdown", "--json"]).status.success());
    let (mut restarted, _, _) = start(&root, "serve");
    assert!(run(&root, &["shutdown", "--json"]).status.success());
    finish(&mut restarted);
    let mut banner = String::new();
    restarted.0.stderr.take().unwrap().read_to_string(&mut banner).unwrap();
    assert!(banner.contains(include_str!("../assets/banner.txt").trim()));
}

#[test]
fn stale_records_cannot_target_a_process_and_help_shows_the_logo() {
    let dir = tempfile::tempdir().unwrap();
    for args in [&[][..], &["--help"][..], &["logo"][..]] {
        let out = Command::new(env!("CARGO_BIN_EXE_mammoth")).args(args).output().unwrap();
        assert!(out.status.success());
        assert!(String::from_utf8_lossy(&out.stdout)
            .contains(include_str!("../assets/banner.txt").trim()));
    }
    std::fs::write(
        dir.path().join("service.json"),
        format!(
            "{{\"pid\":{},\"id\":\"1-1\",\"ui\":\"127.0.0.1:1\",\"s3\":\"127.0.0.1:2\"}}",
            std::process::id()
        ),
    )
    .unwrap();
    let out = run(dir.path(), &["stop", "--json"]);
    assert!(out.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&out.stdout).unwrap()["already_stopped"],
        true
    );
    assert!(!run(dir.path(), &["--masters", "http://127.0.0.1:8080", "stop", "--json"])
        .status
        .success());
    assert!(!run(dir.path(), &["stop", "--timeout", "0"]).status.success());
}

#[cfg(unix)]
#[test]
fn termination_signal_stops_streaming_connections() {
    let dir = tempfile::tempdir().unwrap();
    let (mut server, ui, _) = start(dir.path(), "serve");
    let mut events = request(ui, "/api/v1/events");
    assert!(events.read(&mut [0; 4096]).unwrap() > 0);
    assert!(Command::new("kill")
        .args(["-TERM", &server.0.id().to_string()])
        .status()
        .unwrap()
        .success());
    events.read_to_end(&mut Vec::new()).unwrap();
    finish(&mut server);
    assert!(!dir.path().join("service.json").exists());
}
