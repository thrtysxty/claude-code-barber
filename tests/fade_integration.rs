use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;

fn setup_skills_dir() {
    let skills_dir = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("skills");
    let _ = fs::create_dir_all(&skills_dir);
    let index_path = skills_dir.join("INDEX.md");
    if !index_path.exists() {
        let _ = fs::write(
            &index_path,
            "# Skills\n| Name | Description |\n|------|-------------|\n",
        );
    }
}

#[test]
fn test_fade_list_runs() {
    setup_skills_dir();
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.arg("fade");
    cmd.assert().success();
}

#[test]
fn test_fade_unknown_skill() {
    setup_skills_dir();
    let mut cmd = Command::cargo_bin("ccb").unwrap();
    cmd.arg("fade").arg("this_skill_does_not_exist_xyz");
    // Unknown skill exits 1 with a clear "not found" message
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not found"));
}
