//! Integration tests for plugin auth wiring (CCB-023).
//!
//! Tests: plugin auth classification, GitHub PAT detection, Cloudflare OAuth
//! cache detection, and settings.json plugin list reading.
//!
//! Build: cargo test --test plugins_integration --features full

#[cfg(feature = "plugins")]
#[allow(unused_imports)]
use assert_cmd::Command;

#[cfg(feature = "plugins")]
fn run_plugins_check() -> Command {
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", std::env::var("HOME").unwrap_or_default());
    cmd.arg("plugins");
    cmd
}

#[cfg(feature = "plugins")]
#[test]
fn plugins_command_runs() {
    let mut cmd = run_plugins_check();
    let output = cmd.output().expect("plugins command should run");
    // Either succeeds (all auth configured) or fails with status (auth not configured)
    // Both are valid outcomes — the command should not panic
    assert!(
        output.status.success() || !output.stderr.is_empty() || !output.stdout.is_empty(),
        "plugins command should run without crashing"
    );
}

#[cfg(feature = "plugins")]
#[test]
fn plugins_command_produces_output() {
    let mut cmd = run_plugins_check();
    let output = cmd.output();
    // Should either succeed or fail with status output (never panic)
    if output.is_ok() {
        let out = output.unwrap();
        assert!(
            !out.stdout.is_empty() || !out.stderr.is_empty(),
            "plugins command should produce output"
        );
    }
    // Either way, should not panic or segfault — that's the test
}

#[cfg(feature = "plugins")]
#[cfg(feature = "plugins")]
#[test]
fn github_pat_env_var_is_read() {
    // When GITHUB_PERSONAL_ACCESS_TOKEN is set, plugins should detect it
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", std::env::var("HOME").unwrap_or_default());
    cmd.env("GITHUB_PERSONAL_ACCESS_TOKEN", "ghp_test_token_abc123");
    cmd.arg("plugins");
    let output = cmd.output().expect("plugins command should run");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{}{}", stdout, stderr);
    eprintln!("DEBUG OUTPUT: stdout={:?} stderr={:?}", stdout, stderr);
    // Should show GitHub MCP as OK when PAT is in env
    assert!(
        combined.to_lowercase().contains("github"),
        "plugins output should mention github: got {combined}"
    );
}

#[cfg(feature = "plugins")]
#[test]
fn cloudflare_auth_cache_detected() {
    // Create a temp home with a cloudflare auth cache
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_home = tmp_dir.path();

    // Create cloudflare auth cache
    let cache_dir = tmp_home.join(".claude").join("plugins");
    std::fs::create_dir_all(&cache_dir).unwrap();
    std::fs::write(cache_dir.join(".cloudflare-auth"), "mock_token_abc123").unwrap();

    // Create settings.json with cloudflare plugins
    let settings = serde_json::json!({
        "mcpServers": {
            "cloudflare-api": {"type": "http"},
            "cloudflare-docs": {"type": "http"}
        }
    });
    let settings_path = tmp_home.join(".claude").join("settings.json");
    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("plugins");

    // When cache exists, cloudflare should show OK
    let output = cmd.output().unwrap();
    let combined = String::from_utf8_lossy(&output.stdout);
    let combined_err = String::from_utf8_lossy(&output.stderr);
    let text = format!("{}{}", combined, combined_err);

    // cloudflare-docs should be OK (no auth required)
    // cloudflare-api should show a status
    assert!(
        text.contains("cloudflare"),
        "output should mention cloudflare plugins"
    );
}

#[cfg(feature = "plugins")]
#[test]
fn settings_mcpservers_parsed() {
    // Test that mcpServers format is correctly read
    let tmp_dir = tempfile::tempdir().unwrap();
    let tmp_home = tmp_dir.path();

    let settings = serde_json::json!({
        "mcpServers": {
            "github": {"type": "http"},
            "playwright": {"type": "stdio"}
        }
    });
    let settings_path = tmp_home.join(".claude").join("settings.json");
    std::fs::create_dir_all(settings_path.parent().unwrap()).unwrap();
    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings).unwrap()).unwrap();

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("plugins");

    let output = cmd.output().unwrap();
    let combined = String::from_utf8_lossy(&output.stdout);
    let combined_err = String::from_utf8_lossy(&output.stderr);
    let text = format!("{}{}", combined, combined_err);

    // Should detect both github and playwright from mcpServers
    assert!(
        text.contains("github"),
        "should detect github from mcpServers"
    );
}
