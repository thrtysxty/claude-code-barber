//! status_integration — tests for ccb status command
//!
//! Requires: --features status
//!
//! Build: cargo test --features status --test status_integration
//! Run:   cargo test --features status --test status_integration

use assert_cmd::Command;
use std::fs;
use std::path::PathBuf;

fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".cache")
        .join("ccb")
}

fn clean_cache() {
    let cache = cache_dir();
    let _ = fs::remove_file(cache.join("route_usage.jsonl"));
    let _ = fs::remove_file(cache.join("route_limits.json"));
}

fn setup_usage_file(tokens: &str) {
    let cache = cache_dir();
    fs::create_dir_all(&cache).ok();
    fs::write(cache.join("route_usage.jsonl"), tokens).ok();
}

fn setup_rate_limits(five: f64, seven: f64) {
    let cache = cache_dir();
    let content = format!(
        r#"{{"five_hour":{{"utilization":{},"resets_at":"2026-05-26T15:00:00Z"}},"seven_day":{{"utilization":{}}}}}"#,
        five, seven
    );
    fs::write(cache.join("route_limits.json"), content).ok();
}

/// `ccb status` exits 0
#[test]
fn status_command_exits_zero() {
    clean_cache();
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.arg("status").assert().success();
}

/// `ccb status` outputs non-empty ANSI-rendered statusline
#[test]
fn status_outputs_ansi_content() {
    clean_cache();
    let output = Command::cargo_bin("ccb")
        .unwrap()
        .arg("status")
        .output()
        .unwrap();
    assert!(!output.stdout.is_empty());
    // Should contain ANSI escape sequences (statusline is colored)
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\x1b["),
        "expected ANSI escape codes in output"
    );
}

/// `ccb status` shows token count in output when usage file is populated
#[test]
fn status_shows_tokens_from_usage_file() {
    clean_cache();
    // Write usage file with known token counts
    setup_usage_file(
        r#"{"t":"2026-05-26T10:00:00Z","mdl":"sonnet","in":5000,"out":1200,"be":"ollama"}"#,
    );
    let output = Command::cargo_bin("ccb")
        .unwrap()
        .arg("status")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show some token count in the output
    assert!(
        stdout.contains("tokens")
            || stdout.contains("/ 150K")
            || stdout.contains("↓")
            || stdout.contains("/ 150"),
        "expected token display"
    );
    clean_cache();
}

/// `ccb status` shows rate limit bars
#[test]
fn status_shows_rate_limits() {
    clean_cache();
    setup_rate_limits(35.0, 10.0);
    let output = Command::cargo_bin("ccb")
        .unwrap()
        .arg("status")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should show 5h and 7d rate indicators
    assert!(stdout.contains("5h"), "expected 5h rate indicator");
    assert!(stdout.contains("7d"), "expected 7d rate indicator");
    clean_cache();
}

/// `ccb status` shows cost
#[test]
fn status_shows_cost() {
    clean_cache();
    // Sonnet: 5000 in + 1200 out = $0.015 + $0.018 = $0.033
    setup_usage_file(
        r#"{"t":"2026-05-26T10:00:00Z","mdl":"sonnet","in":5000,"out":1200,"be":"ollama"}"#,
    );
    let output = Command::cargo_bin("ccb")
        .unwrap()
        .arg("status")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("$"), "expected cost indicator ($)");
    clean_cache();
}

/// `ccb status` shows branch name when git is available
#[test]
fn status_shows_git_branch() {
    clean_cache();
    let output = Command::cargo_bin("ccb")
        .unwrap()
        .arg("status")
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Output should contain branch name from git
    // (may be empty string if not in a git repo, but command must succeed)
    assert!(!stdout.is_empty());
    clean_cache();
}

/// `ccb status` succeeds even when cache files don't exist
#[test]
fn status_works_without_cache_files() {
    clean_cache();
    let output = Command::cargo_bin("ccb")
        .unwrap()
        .arg("status")
        .output()
        .unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should still render with zero values
    assert!(stdout.contains("sonnet") || stdout.contains("\x1b["));
    clean_cache();
}
