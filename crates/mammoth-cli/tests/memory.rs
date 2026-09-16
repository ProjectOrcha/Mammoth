use std::process::Command;
fn run(root: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env_remove("MAMMOTH_MASTERS")
        .env_remove("MAMMOTH_CONFIG")
        .arg("--local-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn memory_output_handles_a_closed_pipe_without_panicking() {
    let dir = tempfile::tempdir().unwrap();
    assert!(run(
        dir.path(),
        &[
            "memory",
            "--project",
            "app",
            "remember",
            "long",
            "--title",
            "Long memory",
            "--content",
            &"x".repeat(16 * 1024),
        ]
    )
    .status
    .success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env_remove("MAMMOTH_MASTERS")
        .env_remove("MAMMOTH_CONFIG")
        .arg("--local-root")
        .arg(dir.path())
        .args(["memory", "--project", "app", "get", "long"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    assert!(result.stderr.is_empty());
}
#[test]
fn cli_memory_survives_process_exit_and_reports_conflicts() {
    let dir = tempfile::tempdir().unwrap();
    let put = [
        "memory",
        "--project",
        "app",
        "remember",
        "handoff",
        "--title",
        "Next task",
        "--content",
        "Verify pagination",
        "--kind",
        "handoff",
    ];
    let result = run(dir.path(), &put);
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let value: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(value["revision"], 1);
    assert!(!run(dir.path(), &put).status.success());
    let result = run(dir.path(), &["memory", "--project", "app", "recall", "pagination"]);
    assert!(result.status.success());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["memories"][0]["key"],
        "handoff"
    );
    let result = run(dir.path(), &["memory", "--project", "other", "recall"]);
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap()["memories"],
        serde_json::json!([])
    );
    assert!(run(
        dir.path(),
        &["memory", "--project", "app", "forget", "handoff", "--expected-revision", "1"]
    )
    .status
    .success());
    assert!(!run(dir.path(), &["memory", "--project", "app", "get", "handoff"]).status.success());
}
