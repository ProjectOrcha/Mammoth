use std::process::Command;

#[test]
fn cli_exports_verified_report_without_opening_live_namespace() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().join("store");
    let report = directory.path().join("report.json");
    let result = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .arg("--local-root")
        .arg(&root)
        .args([
            "bench",
            "suite",
            "--size",
            "65KiB",
            "--block-size",
            "32KiB",
            "--files",
            "2",
            "--ops",
            "3",
            "--concurrency",
            "2",
            "--iterations",
            "1",
            "--warmups",
            "0",
            "--replication",
            "1,3",
            "--compute-memory",
            "64KiB",
            "--read-cache",
            "1MiB",
            "--report",
        ])
        .arg(&report)
        .arg("--json")
        .output()
        .unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    let stdout: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
    let disk: serde_json::Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
    assert_eq!(stdout, disk);
    assert_eq!(stdout["verified"], true);
    assert_eq!(stdout["samples"].as_array().unwrap().len(), 18);
    assert_eq!(stdout["options"]["compute_memory_budget"], 65536);
    assert_eq!(stdout["options"]["read_cache_size"], 1048576);
    assert!(
        stdout["samples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|sample| sample["phase"] == "sort"
                && sample["compute"]["jobs"][0]["mode"] == "spill")
    );
    assert!(stdout["samples"]
        .as_array()
        .unwrap()
        .iter()
        .any(|sample| sample["phase"] == "read_cached"
            && sample["cache"]["hits"].as_u64().unwrap() > 0));
    assert!(!root.join("ns").exists());
    let invalid = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .arg("--local-root")
        .arg(&root)
        .args(["bench", "--concurrency", "0", "--json"])
        .output()
        .unwrap();
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert_eq!(std::fs::read_dir(root.join("benchmarks/reports")).unwrap().count(), 1);
}
