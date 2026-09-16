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

#[test]
fn put_checks_for_empty_sources_without_losing_streamed_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let store = dir.path().join("store");
    let source = dir.path().join("binary.pt");
    let payload: Vec<u8> = (0..3 * 1024 * 1024 + 17).map(|i| (i % 251) as u8).collect();
    std::fs::write(&source, &payload).unwrap();
    let out = run(&store, &["put", source.to_str().unwrap(), "/binary.pt", "--json"]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stat: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stat["len"], payload.len());
    let downloaded = dir.path().join("download.pt");
    assert!(run(&store, &["get", "/binary.pt", downloaded.to_str().unwrap()]).status.success());
    assert_eq!(std::fs::read(downloaded).unwrap(), payload);

    std::fs::write(&source, []).unwrap();
    for target in ["/binary.pt", "/new.pt"] {
        let out = run(&store, &["put", source.to_str().unwrap(), target, "--json"]);
        assert!(!out.status.success());
        assert!(out.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&out.stderr).unwrap();
        assert!(error["message"].as_str().unwrap().contains("empty (0 bytes)"));
        assert!(error["message"].as_str().unwrap().contains("--allow-empty"));
    }
    assert_eq!(run(&store, &["cat", "/binary.pt"]).stdout, payload);
    assert!(!run(&store, &["stat", "/new.pt"]).status.success());

    let out =
        run(&store, &["put", source.to_str().unwrap(), "/binary.pt", "--allow-empty", "--json"]);
    assert!(out.status.success());
    let stat: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(stat["len"], 0);
    assert!(run(&store, &["cat", "/binary.pt"]).stdout.is_empty());
}

#[test]
fn put_checks_empty_stdin_and_preserves_nonempty_stdin() {
    use std::{io::Write, process::Stdio};
    let dir = tempfile::tempdir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .arg("--local-root")
        .arg(dir.path())
        .args(["put", "-", "/stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"streamed bytes\0\xff").unwrap();
    assert!(child.wait_with_output().unwrap().status.success());
    assert_eq!(run(dir.path(), &["cat", "/stdin"]).stdout, b"streamed bytes\0\xff");
    // Command::output closes stdin, so this is an immediate EOF.
    assert!(!run(dir.path(), &["put", "-", "/stdin"]).status.success());
    assert_eq!(run(dir.path(), &["cat", "/stdin"]).stdout, b"streamed bytes\0\xff");
    assert!(run(dir.path(), &["put", "-", "/stdin", "--allow-empty"]).status.success());
    assert!(run(dir.path(), &["cat", "/stdin"]).stdout.is_empty());
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

#[test]
fn local_service_rejects_unimplemented_security_before_creating_a_store() {
    let dir = tempfile::tempdir().unwrap();
    for setting in ["auth = \"token\"", "tls = \"required\""] {
        let config = dir.path().join("bad.toml");
        std::fs::write(&config, format!("[security]\n{setting}\n")).unwrap();
        let root = dir.path().join("untouched");
        let out = run(&root, &["--config", config.to_str().unwrap(), "serve", "--json"]);
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("not implemented"));
        assert!(!root.exists());
    }
}

#[test]
fn cli_jobs_preserve_existing_outputs_without_overwrite() {
    let dir = tempfile::tempdir().unwrap();
    let source = dir.path().join("input");
    let root = dir.path().join("store");
    std::fs::write(&source, b"b\na\n").unwrap();
    assert!(run(&root, &["put", source.to_str().unwrap(), "/in"]).status.success());
    assert!(run(&root, &["job", "sort", "/in", "/out"]).status.success());
    assert!(!run(&root, &["job", "wordcount", "/in", "/out"]).status.success());
    assert_eq!(run(&root, &["cat", "/out"]).stdout, b"a\nb\n");
    assert!(run(&root, &["job", "wordcount", "/in", "/out", "--overwrite"]).status.success());
    assert_eq!(run(&root, &["cat", "/out"]).stdout, b"a\t1\nb\t1\n");
}
