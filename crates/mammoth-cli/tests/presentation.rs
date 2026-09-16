use std::{path::Path, process::Command};

fn run(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env_remove("MAMMOTH_MASTERS")
        .env_remove("MAMMOTH_CONFIG")
        .env_remove("NO_COLOR")
        .env("TERM", "xterm-256color")
        .env("COLUMNS", "80")
        .arg("--local-root")
        .arg(root)
        .args(args)
        .output()
        .unwrap()
}
fn text(root: &Path, args: &[&str]) -> String {
    let out = run(root, args);
    assert!(out.status.success(), "{args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}
fn strip_color(text: &str) -> String {
    let mut chars = text.chars();
    let mut out = String::new();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            assert_eq!(chars.next(), Some('['));
            for ch in chars.by_ref() {
                if ch == 'm' {
                    break;
                }
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[test]
fn charts_render_live_storage_with_bounded_lines_and_empty_states() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("store");
    let source = dir.path().join("source");
    std::fs::write(&source, vec![b'x'; 65536]).unwrap();
    let name = format!("/data/{}.bin", "long-name-".repeat(12));
    text(&root, &["put", source.to_str().unwrap(), &name, "--block-size", "4KiB"]);
    std::fs::write(&source, b"").unwrap();
    text(&root, &["put", source.to_str().unwrap(), "/data/empty", "--allow-empty"]);
    text(&root, &["mkdir", "/empty"]);
    for (command, expected) in [
        (vec!["viz", "cluster"], "WORKER CAPACITY"),
        (vec!["viz", "heatmap"], "WORKER CAPACITY"),
        (vec!["viz", "topology"], "RACK TOPOLOGY"),
        (vec!["viz", "skew", "/"], "SIZE SKEW"),
        (vec!["viz", "skew", "/", "--by-partition"], "partitions"),
        (vec!["viz", "treemap", "/"], "NAMESPACE SIZE TREE"),
        (vec!["viz", "health"], "REPLICA HEALTH"),
        (vec!["viz", "blocks", &name], "BLOCK PLACEMENT"),
        (vec!["top", "--once"], "REPLICA HEALTH"),
        (vec!["ls", "/data"], "storage"),
    ] {
        let mut args = vec!["--output", "table", "--color", "never"];
        args.extend(command);
        let view = text(&root, &args);
        assert!(view.contains(expected), "{args:?}: {view}");
        assert!(!view.contains('\u{1b}'));
        for line in view.lines() {
            assert!(line.chars().count() <= 80, "{} columns: {line}", line.chars().count());
        }
        assert!(!view.contains("{\"partition\""));
    }
    let skew = text(&root, &["viz", "skew", "/empty", "--output", "table"]);
    assert!(skew.contains("No files to plot"));
    assert!(!skew.contains("NaN"));
    let tree = text(&root, &["viz", "treemap", "/", "--depth", "1", "--output", "table"]);
    assert!(tree.contains("64.0 KiB"));
    assert!(!tree.contains("long-name"));
    assert!(text(&root, &["viz", "blocks", "/data/empty", "--output", "table"])
        .contains("Empty or inline"));
}

#[test]
fn color_controls_preserve_machine_output_and_raw_files() {
    let dir = tempfile::tempdir().unwrap();
    text(dir.path(), &["mkdir", "/data"]);
    for command in [
        vec!["ls"],
        vec!["viz", "cluster"],
        vec!["viz", "treemap"],
        vec!["viz", "health"],
        vec!["commands"],
    ] {
        let mut args = vec!["--color", "always", "--output", "table"];
        args.extend(command.clone());
        let colored = text(dir.path(), &args);
        assert!(colored.contains('\u{1b}'), "{args:?}");
        args[1] = "never";
        assert_eq!(
            strip_color(&colored),
            text(dir.path(), &args),
            "Color changed layout: {args:?}"
        );
        let mut args = vec!["--color", "always", "--json"];
        args.extend(command);
        let output = text(dir.path(), &args);
        assert!(!output.contains('\u{1b}'));
        serde_json::from_str::<serde_json::Value>(&output).unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env("NO_COLOR", "1")
        .arg("--local-root")
        .arg(dir.path())
        .args(["ls", "--output", "table"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!output.stdout.contains(&27));
    let forced = Command::new(env!("CARGO_BIN_EXE_mammoth"))
        .env("NO_COLOR", "1")
        .arg("--local-root")
        .arg(dir.path())
        .args(["ls", "--output", "table", "--color", "always"])
        .output()
        .unwrap();
    assert!(forced.status.success());
    assert!(
        String::from_utf8_lossy(&forced.stdout).contains("38;5;"),
        "Explicit color must override NO_COLOR"
    );
    let error = run(dir.path(), &["stat", "/missing", "--output", "table", "--color", "always"]);
    assert!(!error.status.success());
    assert!(error.stderr.contains(&27));
    let source = dir.path().join("raw");
    let bytes = b"unaltered\n\x1b[0m";
    std::fs::write(&source, bytes).unwrap();
    text(dir.path(), &["put", source.to_str().unwrap(), "/data/raw"]);
    assert_eq!(run(dir.path(), &["cat", "/data/raw", "--color", "always"]).stdout, bytes);
    let top = text(dir.path(), &["top", "--json"]);
    assert!(serde_json::from_str::<serde_json::Value>(&top).unwrap()["nodes"].is_array());
}

#[test]
fn root_help_exposes_the_website_commands_and_color_options() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("uninitialized");
    let help = text(&root, &["--help"]);
    for name in [
        "viz blocks",
        "viz cluster",
        "viz topology",
        "viz skew",
        "viz treemap",
        "viz health",
        "viz flow",
        "node inspect",
        "node repair",
        "cluster status",
        "admin repair",
        "admin gc",
        "config template",
        "job wordcount",
        "job sort",
        "migrate import",
        "migrate export",
        "shutdown",
        "--color",
    ] {
        assert!(help.contains(name), "missing {name}");
    }
    let catalog: Vec<serde_json::Value> =
        serde_json::from_str(&text(&root, &["commands", "--json"])).unwrap();
    for entry in catalog {
        assert!(!entry["description"].as_str().unwrap().is_empty(), "{entry}");
    }
    assert!(text(&root, &["--color=always", "--help"]).contains('\u{1b}'));
    assert!(!text(&root, &["--color=never", "--help"]).contains('\u{1b}'));
    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let completions = text(&root, &["completions", shell]);
        assert!(completions.contains("memory"), "{shell}");
        assert!(completions.contains("mcp"), "{shell}");
    }
    assert!(!root.exists(), "Help and completions must not initialize storage");
}

#[test]
fn ls_uses_the_exported_store_and_allows_an_explicit_override() {
    let dir = tempfile::tempdir().unwrap();
    let exported = dir.path().join("exported-store");
    let explicit = dir.path().join("explicit-store");
    text(&exported, &["mkdir", "/from-export"]);
    text(&explicit, &["mkdir", "/from-flag"]);
    for override_root in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mammoth"));
        command
            .env_remove("MAMMOTH_MASTERS")
            .env_remove("MAMMOTH_CONFIG")
            .env("MAMMOTH_LOCAL_ROOT", &exported)
            .current_dir(dir.path());
        if override_root {
            command.arg("--local-root").arg(&explicit);
        }
        let out = command.args(["ls", "/", "--json"]).output().unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        let entries: Vec<serde_json::Value> = serde_json::from_slice(&out.stdout).unwrap();
        assert_eq!(entries.len(), 1, "ls must list stored paths, not the working directory");
        assert_eq!(entries[0]["path"], if override_root { "/from-flag" } else { "/from-export" });
        assert_eq!(entries[0]["is_dir"], true);
    }
}
