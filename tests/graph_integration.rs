use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

/// Creates a temp directory with a minimal Rust main.rs and Python lib.py.
fn tmp_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let main_rs = dir.path().join("main.rs");
    let lib_py = dir.path().join("lib.py");
    fs::write(&main_rs, "fn main() {}\nstruct Foo {}\n").unwrap();
    fs::write(&lib_py, "def bar():\n    pass\n\nclass Baz:\n    pass\n").unwrap();
    dir
}

#[test]
fn test_graph_index_creates_db() {
    let repo = tmp_repo();
    let home = dirs::home_dir().unwrap_or_default();

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph").arg("index").arg(repo.path());
    cmd.assert().success();

    let db_path = home.join(".cache/ccb/graph.db");
    assert!(
        db_path.exists(),
        "graph.db should be created at {:?}",
        db_path
    );
}

#[test]
fn test_graph_search_finds_symbol() {
    let repo = tmp_repo();
    let home = dirs::home_dir().unwrap_or_default();

    // Index first
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", home.as_path());
        cmd.arg("graph").arg("index").arg(repo.path());
        cmd.assert().success();
    }

    // Then search
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph").arg("search").arg("main");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("main"));
}

#[test]
fn test_graph_search_json_valid() {
    let repo = tmp_repo();
    let home = dirs::home_dir().unwrap_or_default();

    // Index
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", home.as_path());
        cmd.arg("graph").arg("index").arg(repo.path());
        cmd.assert().success();
    }

    // Search JSON
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph")
        .arg("search")
        .arg("foo")
        .arg("--format")
        .arg("json");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("results").is_some(),
        "JSON output should have 'results' key"
    );
}

#[test]
fn test_graph_show_file() {
    let repo = tmp_repo();
    let home = dirs::home_dir().unwrap_or_default();
    let main_rs = repo.path().join("main.rs");

    // Index
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", home.as_path());
        cmd.arg("graph").arg("index").arg(repo.path());
        cmd.assert().success();
    }

    // Show
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph").arg("show").arg(&main_rs);
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_graph_show_json_valid() {
    let repo = tmp_repo();
    let home = dirs::home_dir().unwrap_or_default();
    let main_rs = repo.path().join("main.rs");

    // Index
    {
        let mut cmd = Command::cargo_bin("ccb").unwrap();
        cmd.env("HOME", home.as_path());
        cmd.arg("graph").arg("index").arg(repo.path());
        cmd.assert().success();
    }

    // Show JSON
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph")
        .arg("show")
        .arg(&main_rs)
        .arg("--format")
        .arg("json");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("symbols").is_some(),
        "JSON output should have 'symbols' key"
    );
}

#[test]
fn test_graph_stats() {
    let home = dirs::home_dir().unwrap_or_default();

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph").arg("stats");
    cmd.assert()
        .success()
        .stdout(predicate::str::is_empty().not());
}

#[test]
fn test_graph_stats_json_valid() {
    let home = dirs::home_dir().unwrap_or_default();

    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home.as_path());
    cmd.arg("graph").arg("stats").arg("--format").arg("json");
    cmd.assert().success();

    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert!(
        json.get("total_files").is_some(),
        "JSON output should have 'total_files' key"
    );
}
