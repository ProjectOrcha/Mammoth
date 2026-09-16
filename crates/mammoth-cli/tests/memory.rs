use std::process::Command;
fn run(root: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .arg("--local-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
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
