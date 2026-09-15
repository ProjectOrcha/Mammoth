use std::{path::Path, process::Command};
fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .arg("--local-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn help_version_and_structured_errors() {
    let dir = tempfile::tempdir().unwrap();
    for args in [vec!["--help"], vec!["--version"], vec!["version"]] {
        let out = run(dir.path(), &args);
        assert!(out.status.success());
        assert!(String::from_utf8(out.stdout).unwrap().contains("mammoth"));
    }
    let out = run(dir.path(), &["stat", "/missing", "--json"]);
    assert!(!out.status.success());
    assert!(out.stdout.is_empty());
    let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
    assert_eq!(error["code"], "E0101");
}
#[test]
fn filesystem_cli_survives_process_restarts_and_preserves_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"mammoth rust\nmammoth\n").unwrap();
    let store = dir.path().join("store");
    for args in [
        vec!["put", source.to_str().unwrap(), "/data/notes #1.txt"],
        vec!["stat", "/data/notes #1.txt"],
        vec!["ls", "/data"],
        vec!["cp", "/data/notes #1.txt", "/data/copy"],
        vec!["mv", "/data/copy", "/data/moved"],
        vec!["setrep", "2", "/data/moved"],
        vec!["job", "wordcount", "/data/moved", "/data/counts"],
    ] {
        let out = run(&store, &args);
        assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
    assert_eq!(run(&store, &["cat", "/data/notes #1.txt"]).stdout, b"mammoth rust\nmammoth\n");
    assert_eq!(run(&store, &["head", "/data/moved", "-n", "1"]).stdout, b"mammoth rust\n");
    assert_eq!(run(&store, &["tail", "/data/moved", "-n", "1"]).stdout, b"mammoth\n");
    assert_eq!(run(&store, &["cat", "/data/counts"]).stdout, b"mammoth\t2\nrust\t1\n");
    let dst = dir.path().join("download");
    let out = run(&store, &["get", "/data/moved", dst.to_str().unwrap()]);
    assert!(out.status.success());
    assert_eq!(std::fs::read(dst).unwrap(), std::fs::read(source).unwrap());
    for format in ["json", "yaml", "csv", "table"] {
        let out = run(&store, &["ls", "/data", "--output", format]);
        assert!(out.status.success());
        assert!(!out.stdout.contains(&27));
    }
    assert!(run(&store, &["rm", "/data", "--recursive"]).status.success());
    assert!(run(&store, &["doctor"]).status.success());
}

#[test]
fn tree_transfer_roundtrip_and_invalid_upload_options() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    let store = dir.path().join("store");
    std::fs::create_dir_all(source.join("sub")).unwrap();
    std::fs::write(source.join("sub/a #.txt"), b"round trip").unwrap();
    let out = run(&store, &["migrate", "import", source.to_str().unwrap(), "/imported"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let exported = dir.path().join("exported");
    let out = run(&store, &["migrate", "export", "/imported", exported.to_str().unwrap()]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    assert_eq!(std::fs::read(exported.join("sub/a #.txt")).unwrap(), b"round trip");
    let out = run(
        &store,
        &["put", source.join("sub/a #.txt").to_str().unwrap(), "/invalid", "--replication", "0"],
    );
    assert!(!out.status.success());
    assert!(!run(&store, &["stat", "/invalid"]).status.success());
}

#[cfg(unix)]
#[test]
fn migration_rejects_source_symlinks() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("source");
    std::fs::write(&source, b"private bytes").unwrap();
    let link = dir.path().join("link");
    std::os::unix::fs::symlink(&source, &link).unwrap();
    let store = dir.path().join("store");
    let out = run(&store, &["migrate", "import", link.to_str().unwrap(), "/linked"]);
    assert!(!out.status.success());
    assert!(!run(&store, &["stat", "/linked"]).status.success());
}
