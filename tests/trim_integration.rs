use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

#[test]
fn test_trim_compresses_output() {
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.arg("trim").arg("echo").arg("hello world");
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_trim_no_args_fails() {
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.arg("trim");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("usage: ccb trim"));
}

#[test]
fn test_trim_logs_to_jsonl() {
    // Use a temp HOME so this test's log file is isolated
    let temp_dir = tempfile::tempdir().unwrap();
    let temp_home = temp_dir.path();
    let log_dir = temp_home.join(".claude");
    fs::create_dir_all(&log_dir).unwrap();
    let log_path = log_dir.join("ccb_log.jsonl");

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", temp_home);
    cmd.arg("trim").arg("echo").arg("test from integration");
    cmd.assert().success();

    // Log file must exist
    assert!(log_path.exists(), "log file should exist at {:?}", log_path);

    let content = fs::read_to_string(&log_path).unwrap();
    let last_line = content.lines().last().unwrap_or("");
    assert!(
        !last_line.is_empty(),
        "log file should have at least one line"
    );

    // Must be valid JSON with feature:"trim"
    let json: serde_json::Value = serde_json::from_str(last_line).unwrap();
    assert_eq!(json["feature"], "trim");
}
