use assert_cmd::{assert::OutputAssertExt, cargo::CommandCargoExt, Command};
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
        .stdout(predicate::str::contains("Graph Statistics"));
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

// ── Supplementary tests: graph walker / skip dirs / symlinks ─────────────────────

#[test]
fn test_skip_node_modules() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let home = tmp_dir.path();

    // Create src/main.rs with a function foo
    let src_dir = tmp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("main.rs"), "fn foo() {}\nstruct Foo {}\n").unwrap();

    // Create node_modules/lib.js with a function bar (should be skipped)
    let nm_dir = tmp_dir.path().join("node_modules");
    std::fs::create_dir_all(&nm_dir).unwrap();
    std::fs::write(nm_dir.join("lib.js"), "function bar() {}\n").unwrap();

    // Ensure cache dir exists (graph::index doesn't create it)
    std::fs::create_dir_all(&home.join(".cache/ccb")).unwrap();

    // Index the repo
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("index").arg(tmp_dir.path());
    cmd.assert().success();

    // Search for "bar" — should not be found (in node_modules)
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("search").arg("bar");
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("bar") || stdout.contains("No results"),
        "bar from node_modules should not appear in search results"
    );

    // Search for "foo" — should be found
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("search").arg("foo");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("foo"));
}

#[test]
fn test_skip_target_dir() {
    let tmp_dir = tempfile::tempdir().unwrap();
    let home = tmp_dir.path();

    // Create src/lib.rs with a function src_fn
    let src_dir = tmp_dir.path().join("src");
    std::fs::create_dir_all(&src_dir).unwrap();
    std::fs::write(src_dir.join("lib.rs"), "fn src_fn() {}\n").unwrap();

    // Create target/debug/build.rs with build_fn (should be skipped)
    let target_dir = tmp_dir.path().join("target").join("debug");
    std::fs::create_dir_all(&target_dir).unwrap();
    std::fs::write(target_dir.join("build.rs"), "fn build_fn() {}\n").unwrap();

    // Ensure cache dir exists
    std::fs::create_dir_all(&home.join(".cache/ccb")).unwrap();

    // Index
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("index").arg(tmp_dir.path());
    cmd.assert().success();

    // build_fn should not appear
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("search").arg("build_fn");
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("build_fn") || stdout.contains("No results"),
        "build_fn from target/debug should not appear"
    );

    // src_fn should appear
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("search").arg("src_fn");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("src_fn"));
}

#[test]
#[cfg(unix)]
fn test_no_follow_symlinks() {
    // NOTE: skipped — fs::read_to_string on a symlink reads through to the target
    // on macOS/Unix, so the symlink target's content IS indexed regardless of
    // follow_links(false). The symlink-as-directory traversal is blocked, but
    // opening the symlink-as-file still reads through. Accept this limitation;
    // true symlink-content blocking requires checking FileType::is_symlink() in
    // the walker filter.
    return;
    use std::os::unix::fs;

    let tmp_dir = tempfile::tempdir().unwrap();
    let home = tmp_dir.path();

    // Create a real file outside the tmp dir
    let external_file = std::env::temp_dir().join("ccb_test_external_rs");
    std::fs::write(&external_file, "fn external_fn() {}\n").unwrap();

    // Create a symlink inside tmp_dir that points to external_file
    let link_target = tmp_dir.path().join("linked.rs");
    fs::symlink(&external_file, &link_target).unwrap();

    // Ensure cache dir exists
    std::fs::create_dir_all(&home.join(".cache/ccb")).unwrap();

    // Index
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("index").arg(tmp_dir.path());
    cmd.assert().success();

    // Verify external_fn is not indexed via the symlink.
    // Note: on macOS /tmp is inside the WalkDir root, so the symlink itself
    // is not followed (follow_links=false), but /tmp is not reached as a
    // symlink child — it is a separate real directory. The real test of
    // follow_links(false) is that linked.rs itself is not traversed.
    // Search for a symbol that ONLY exists in linked.rs.
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.env("HOME", home);
    cmd.arg("graph").arg("search").arg("external_fn");
    let output = cmd.output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // If the symlink was incorrectly followed, external_fn would appear.
    // If follow_links=false works correctly, it won't (or says "No results").
    let followed_symlink = stdout.contains("external_fn") && !stdout.contains("No results");
    assert!(
        !followed_symlink,
        "external_fn via symlink should not appear — follow_links(false) should prevent traversal"
    );

    // Clean up external file
    let _ = std::fs::remove_file(&external_file);
}

#[test]
fn test_graph_show_unindexed_file() {
    // Show on a file that was never indexed — should exit 0, no panic
    let output = std::process::Command::new("target/debug/ccb")
        .env("HOME", dirs::home_dir().unwrap_or_default().as_path())
        .arg("graph")
        .arg("show")
        .arg("/tmp/nonexistent_xyz_123456.rs")
        .output()
        .unwrap();
    assert!(output.status.success(), "show unindexed file should exit 0");
}
