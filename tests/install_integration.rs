//! Integration tests for the `ccb install` command.
//! install.rs uses dirs::home_dir() — each test redirects HOME to a temp dir.

use assert_cmd::Command;
use std::fs;

/// Helper: run `ccb install --auto` with HOME set to tmp_dir.
fn run_install_auto(tmp_home: &std::path::Path, dry_run: bool) -> assert_cmd::Command {
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("install");
    if dry_run {
        cmd.arg("--dry-run");
    }
    cmd.arg("--auto");
    cmd
}

#[test]
fn dry_run_no_writes() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_home = tmp_dir.path();

    let mut cmd = run_install_auto(tmp_home, true);
    cmd.assert().success();

    // No .claude directory created
    let claude_dir = tmp_home.join(".claude");
    assert!(
        !claude_dir.exists(),
        "dry_run should not create .claude directory"
    );
}

#[test]
fn idempotent_posttooluse_once() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_home = tmp_dir.path();

    // Run twice with auto=true (skip confirmation)
    run_install_auto(tmp_home, false).assert().success();
    run_install_auto(tmp_home, false).assert().success();

    // Parse settings.json — context_monitor should appear exactly once in PostToolUse
    let settings_path = tmp_home.join(".claude").join("settings.json");
    let content = fs::read_to_string(&settings_path).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let posttooluse = settings["hooks"]["PostToolUse"].as_array().unwrap();
    let context_count = posttooluse
        .iter()
        .filter(|entry| entry.to_string().contains("context_monitor"))
        .count();

    assert_eq!(
        context_count, 1,
        "context_monitor should appear exactly once in PostToolUse"
    );
}

#[test]
fn creates_hooks_dir_and_executable() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_home = tmp_dir.path();

    let mut cmd = run_install_auto(tmp_home, false);
    cmd.assert().success();

    let hook_path = tmp_home
        .join(".claude")
        .join("hooks")
        .join("context_monitor.sh");
    assert!(hook_path.exists(), "context_monitor.sh should be created");

    // Executable check (unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = fs::metadata(&hook_path).unwrap().permissions();
        assert_ne!(
            perms.mode() & 0o111,
            0,
            "context_monitor.sh should be executable"
        );
    }
}

#[test]
fn patches_empty_settings() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_home = tmp_dir.path();

    // No settings.json exists initially
    let settings_path = tmp_home.join(".claude").join("settings.json");
    assert!(!settings_path.exists());

    let mut cmd = run_install_auto(tmp_home, false);
    cmd.assert().success();

    assert!(settings_path.exists());

    let content = fs::read_to_string(&settings_path).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&content).unwrap();

    let posttooluse = settings["hooks"]["PostToolUse"].as_array().unwrap();
    let pretooluse = settings["hooks"]["PreToolUse"].as_array().unwrap();

    assert!(
        !posttooluse.is_empty(),
        "PostToolUse should be non-empty after install"
    );
    assert!(
        !pretooluse.is_empty(),
        "PreToolUse should be non-empty after install"
    );
}
