use std::process::Command;

#[test]
fn unimplemented_commands_explain_the_scaffold_without_panicking() {
    let output = Command::new(env!("CARGO_BIN_EXE_mammoth")).arg("quickstart").output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    let error = String::from_utf8(output.stderr).unwrap();
    assert!(error.contains("error[E0002]"));
    assert!(error.contains("mammoth --help"));
    assert!(!error.contains("panicked"));
}

#[test]
fn help_and_version_still_work() {
    for flag in ["--help", "--version"] {
        let output = Command::new(env!("CARGO_BIN_EXE_mammoth")).arg(flag).output().unwrap();
        assert!(output.status.success());
        assert!(String::from_utf8(output.stdout).unwrap().contains("mammoth"));
    }
}
