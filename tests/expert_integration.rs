use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Writes the fixture dataset and returns the path.
fn fixture_dataset(tmp_home: &std::path::Path) -> std::path::PathBuf {
    let graph_dir = tmp_home.join(".cache/ccb");
    fs::create_dir_all(&graph_dir).unwrap();
    let path = graph_dir.join("test_dataset.json");
    let data = r#"{
  "persona": "test_sentinel",
  "description": "Test security persona",
  "domains": [
    {
      "name": "test_injection",
      "category": "security",
      "patterns": [
        {"id": "TEST-01", "name": "Test Pattern", "mitigations": ["validate", "escape"]}
      ]
    }
  ]
}"#;
    fs::write(&path, data).unwrap();
    path
}

#[test]
fn test_expert_build() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    let dataset = fixture_dataset(tmp_home);

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert")
        .arg("build")
        .arg("test_sentinel")
        .arg("--dataset")
        .arg(&dataset);
    cmd.assert().success();
}

#[test]
fn test_expert_list() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    let dataset = fixture_dataset(tmp_home);

    // Build first
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert")
            .arg("build")
            .arg("test_sentinel")
            .arg("--dataset")
            .arg(&dataset);
        cmd.assert().success();
    }

    // Then list
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert").arg("list");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("test_sentinel"));
}

#[test]
fn test_expert_activate() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    let dataset = fixture_dataset(tmp_home);

    // Build
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert")
            .arg("build")
            .arg("test_sentinel")
            .arg("--dataset")
            .arg(&dataset);
        cmd.assert().success();
    }

    // Activate
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert").arg("activate").arg("test_sentinel");
    cmd.assert().success();
}

#[test]
fn test_expert_activate_unknown() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    // Create the .cache/ccb directory so the DB can be created
    let graph_dir = tmp_home.join(".cache/ccb");
    fs::create_dir_all(&graph_dir).unwrap();

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert").arg("activate").arg("nonexistent");
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}

#[test]
fn test_expert_query_empty() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    let graph_dir = tmp_home.join(".cache/ccb");
    fs::create_dir_all(&graph_dir).unwrap();

    // No active persona — should return {} and exit 0
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert").arg("query");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("{}"));
}

#[test]
fn test_expert_query_active_json() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    let dataset = fixture_dataset(tmp_home);

    // Build and activate
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert")
            .arg("build")
            .arg("test_sentinel")
            .arg("--dataset")
            .arg(&dataset);
        cmd.assert().success();

        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert").arg("activate").arg("test_sentinel");
        cmd.assert().success();
    }

    // Query
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert").arg("query").arg("--format").arg("json");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(json["persona"], "test_sentinel");
}

#[test]
fn test_expert_deactivate() {
    let tmp_dir = TempDir::new().unwrap();
    let tmp_home = tmp_dir.path();
    let dataset = fixture_dataset(tmp_home);

    // Build, activate, deactivate
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert")
            .arg("build")
            .arg("test_sentinel")
            .arg("--dataset")
            .arg(&dataset);
        cmd.assert().success();

        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert").arg("activate").arg("test_sentinel");
        cmd.assert().success();

        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_home);
        cmd.arg("expert").arg("deactivate");
        cmd.assert().success();
    }

    // Query — should return {}
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", tmp_home);
    cmd.arg("expert").arg("query");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("{}"));
}

// ── Supplementary tests: expert::ingest(YAML) ──────────────────────────────────
// expert feature gates access to the ingest function via `ccb expert ingest`.

#[cfg(feature = "expert")]
mod ingest_tests {
    use super::*;

    #[test]
    fn test_expert_ingest_valid_yaml() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let yaml_path = tmp_dir.path().join("test_expert.yaml");
        let yaml_content = r#"
personas:
  - name: test-expert
    description: Test expert persona
    domains:
      - name: test_domain
        category: test
        patterns:
          - id: TEST-01
            name: Test Pattern
            mitigations:
              - validate input
              - escape output
"#;
        std::fs::write(&yaml_path, yaml_content).unwrap();

        // Ensure .cache/ccb directory exists (expert db() doesn't create it)
        let cache_dir = tmp_dir.path().join(".cache/ccb");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_dir.path());
        cmd.arg("expert").arg("ingest").arg("--dataset").arg(&yaml_path);
        cmd.assert().success();

        let db_path = tmp_dir.path().join(".cache/ccb/graph.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM personas WHERE name = 'test-expert'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "Should have exactly one 'test-expert' persona in DB");
    }

    #[test]
    fn test_expert_ingest_idempotent() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let yaml_path = tmp_dir.path().join("idempotent_expert.yaml");
        let yaml_content = r#"
personas:
  - name: idempotent-expert
    description: First description
    domains:
      - name: idem_domain
        category: test
        patterns:
          - id: IDEM-01
            name: Idempotent Pattern
            mitigations:
              - mitigate
"#;
        std::fs::write(&yaml_path, yaml_content).unwrap();

        let cache_dir = tmp_dir.path().join(".cache/ccb");
        std::fs::create_dir_all(&cache_dir).unwrap();

        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_dir.path());
        cmd.arg("expert").arg("ingest").arg("--dataset").arg(&yaml_path);
        cmd.assert().success();

        let mut cmd2 = Command::cargo_bin("ccb").unwrap();
        cmd2.env("HOME", tmp_dir.path());
        cmd2.arg("expert").arg("ingest").arg("--dataset").arg(&yaml_path);
        cmd2.assert().success();

        let db_path = tmp_dir.path().join(".cache/ccb/graph.db");
        let conn = rusqlite::Connection::open(&db_path).unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM personas WHERE name = 'idempotent-expert'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1, "Should still have exactly one 'idempotent-expert' after two ingests");
    }

    #[test]
    fn test_expert_ingest_rejects_bad_yaml() {
        let tmp_dir = tempfile::tempdir().unwrap();
        let bad_path = tmp_dir.path().join("bad.yaml");
        std::fs::write(&bad_path, "{bad yaml :::").unwrap();

        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", tmp_dir.path());
        cmd.arg("expert").arg("ingest").arg("--dataset").arg(&bad_path);
        cmd.assert().failure();
    }
}
