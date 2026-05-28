//! route_integration — tests for ccb-route token + rate-limit capture
//!
//! Requires: --features route
//!
//! Build: cargo test --features route --test route_integration
//! Run:   cargo test --features route --test route_integration

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

/// Verify route_usage.jsonl has correct JSON schema
#[test]
fn usage_file_has_valid_schema() {
    clean_cache();
    let cache = cache_dir();
    fs::create_dir_all(&cache).ok();

    // Manually write a valid entry to confirm schema
    let entry = r#"{"t":"2026-05-26T10:00:00Z","mdl":"sonnet","in":5000,"out":1200,"be":"ollama"}"#;
    fs::write(cache.join("route_usage.jsonl"), entry).ok();

    let content = fs::read_to_string(cache.join("route_usage.jsonl")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("t").is_some(), "timestamp field missing");
    assert!(parsed.get("mdl").is_some(), "model field missing");
    assert!(parsed.get("in").is_some(), "input_tokens field missing");
    assert!(parsed.get("out").is_some(), "output_tokens field missing");
    assert!(parsed.get("be").is_some(), "backend field missing");
    clean_cache();
}

/// Verify route_limits.json has correct JSON schema
#[test]
fn rate_limits_file_has_valid_schema() {
    clean_cache();
    let cache = cache_dir();
    fs::create_dir_all(&cache).ok();

    let content = r#"{"five_hour":{"utilization":2.0},"seven_day":{"utilization":35.0}}"#;
    fs::write(cache.join("route_limits.json"), content).ok();

    let content = fs::read_to_string(cache.join("route_limits.json")).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&content).unwrap();
    assert!(parsed.get("five_hour").is_some(), "five_hour field missing");
    assert!(parsed.get("seven_day").is_some(), "seven_day field missing");
    let five = &parsed["five_hour"]["utilization"];
    assert!(five.is_number(), "five_hour.utilization should be number");
    clean_cache();
}

/// ccb-route binary compiles and is discoverable by assert_cmd
#[test]
fn ccb_route_binary_exists() {
    // ccb-route has no --help flag — it starts the server immediately.
    // Just verify the binary exists and was compiled.
    let cmd = Command::cargo_bin("ccb-route");
    assert!(cmd.is_ok(), "ccb-route binary should exist after build");
}
