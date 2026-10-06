mod common;

use assert_cmd::Command;

fn bin() -> Command {
    Command::cargo_bin("xlsx-dump").unwrap()
}

#[test]
fn help_runs() {
    let out = bin().arg("--help").output().unwrap();
    assert!(out.status.success());
}

#[test]
fn help_documents_exit_codes() {
    let out = bin().arg("--help").output().unwrap();
    let text = String::from_utf8(out.stdout).unwrap();
    assert!(text.contains("Exit codes:"));
}
