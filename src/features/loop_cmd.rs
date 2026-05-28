//! Loop — plan, build, and verify story implementation
//!
//! Commands:
//! - `ccb detect` — detect repo type from CWD
//! - `ccb plan <story-file>` — parse story, extract ACs, output phased JSON
//! - `ccb build [--plan <plan>] [--story <story>]` — implement phases with gates
//! - `ccb lesson <description>` — capture lessons to cache
//! - `ccb gates` — show gate sequence for detected repo type

use crate::cli;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

// ─────────────────────────────────────────────────────────────────────────────
// Repo Detection (AC1-5)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoType {
    Rust,
    TypeScript,
    Swift,
    Python,
    Makefile,
    Unknown,
}

impl std::fmt::Display for RepoType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RepoType::Rust => write!(f, "rust"),
            RepoType::TypeScript => write!(f, "typescript"),
            RepoType::Swift => write!(f, "swift"),
            RepoType::Python => write!(f, "python"),
            RepoType::Makefile => write!(f, "makefile"),
            RepoType::Unknown => write!(f, "unknown"),
        }
    }
}

/// Detect repo type by walking up from cwd for marker files.
/// Marker priority: Cargo.toml > package.json > Package.swift > pyproject.toml > Makefile
pub fn detect(cwd: Option<&Path>) -> Result<RepoType> {
    let start = match cwd {
        Some(p) => p.to_path_buf(),
        None => std::env::current_dir()?,
    };
    for entry in WalkDir::new(&start)
        .max_depth(20)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let name = entry.file_name();
        if name == "Cargo.toml" {
            return Ok(RepoType::Rust);
        }
        if name == "package.json" {
            return Ok(RepoType::TypeScript);
        }
        if name == "Package.swift" {
            return Ok(RepoType::Swift);
        }
        if name == "pyproject.toml" {
            return Ok(RepoType::Python);
        }
        if name == "Makefile" || name == "makefile" {
            return Ok(RepoType::Makefile);
        }
    }
    Ok(RepoType::Unknown)
}

/// Return the quality gates for a detected repo type.
pub fn gates_for(repo_type: RepoType) -> Vec<String> {
    match repo_type {
        RepoType::Rust => vec![
            "cargo fmt --check".to_string(),
            "cargo clippy".to_string(),
            "cargo test".to_string(),
            "cargo build".to_string(),
        ],
        RepoType::TypeScript => vec![
            "tsc --noEmit".to_string(),
            "npm run lint".to_string(),
            "npm test".to_string(),
            "npm run build".to_string(),
        ],
        RepoType::Swift => vec!["swift build".to_string(), "swift test".to_string()],
        RepoType::Python => vec!["pyright".to_string(), "pytest".to_string()],
        RepoType::Makefile => vec!["make".to_string()],
        RepoType::Unknown => vec![],
    }
}

/// Print human-readable list of gates for a repo type.
pub fn gates_help(repo_type: RepoType) {
    let gates = gates_for(repo_type);
    if gates.is_empty() {
        println!("No gates defined for unknown repo type.");
        return;
    }
    println!("Gates for {}:", repo_type);
    for (i, gate) in gates.iter().enumerate() {
        println!("  {}: {}", i + 1, gate);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Story Parsing (AC6-11)
// ─────────────────────────────────────────────────────────────────────────────

/// Acceptance criterion parsed from story markdown.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryAC {
    pub id: String,
    pub statement: String,
    pub status: String,
}

/// Phase in a story implementation plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPhase {
    pub name: String,
    pub description: String,
    pub acs: Vec<String>,
}

/// Story plan output by `ccb plan`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoryPlan {
    pub story_id: String,
    pub title: String,
    pub repo_type: String,
    pub phases: Vec<PlanPhase>,
    pub gates: Vec<String>,
}

/// Parse a story markdown file and extract ACs grouped by scope.
/// ACs are identified by lines matching checkbox patterns: `- [ ] **AC<N>:**`, `- [ ] AC<N>:`, `**AC<N>:`, etc.
fn parse_story_acs(content: &str) -> Vec<StoryAC> {
    let mut acs = vec![];
    for line in content.lines() {
        let line = line.trim();
        // Match patterns:
        // - [ ] **AC1:** something
        // - [ ] AC1: something
        // **AC1:** something
        // - AC1: something
        let rest = if let Some(r) = line.strip_prefix("- [ ] **AC") {
            r.trim_start_matches("**AC").trim_start_matches("AC")
        } else if let Some(r) = line.strip_prefix("- [ ] AC") {
            r.trim_start_matches("AC")
        } else if let Some(r) = line.strip_prefix("**AC") {
            r.trim_start_matches("**AC")
        } else if let Some(r) = line.strip_prefix("- AC") {
            r.trim_start_matches("- AC")
        } else {
            continue;
        };

        if let Some((id_part, rest2)) = rest.split_once(':').or_else(|| rest.split_once('.')) {
            let id_clean = id_part.trim().to_string();
            let statement = rest2.trim().trim_start_matches(":**").trim().to_string();
            if !id_clean.is_empty() && !statement.is_empty() {
                acs.push(StoryAC {
                    id: id_clean,
                    statement,
                    status: "pending".to_string(),
                });
            }
        }
    }
    acs
}

/// Group ACs by inferred scope (similar to feature modules).
fn group_acs_by_scope(acs: &[StoryAC]) -> HashMap<String, Vec<StoryAC>> {
    let mut groups: HashMap<String, Vec<StoryAC>> = HashMap::new();
    for ac in acs {
        // Simple scope inference: first word before common separators
        let scope = if ac.statement.contains("feature flag") || ac.statement.contains("Cargo.toml")
        {
            "infrastructure"
        } else if ac.statement.contains("test") {
            "testing"
        } else if ac.statement.contains("detect") || ac.statement.contains("repo type") {
            "detection"
        } else if ac.statement.contains("build") || ac.statement.contains("implement") {
            "implementation"
        } else {
            "core"
        };
        groups
            .entry(scope.to_string())
            .or_default()
            .push(ac.clone());
    }
    groups
}

/// Read a story file, run STEP ZERO (git log check), output plan JSON.
pub fn plan_story(story_path: &Path, save: bool) -> Result<StoryPlan> {
    let content = std::fs::read_to_string(story_path)?;
    let acs = parse_story_acs(&content);
    if acs.is_empty() {
        anyhow::bail!("no ACs found in story file");
    }

    // STEP ZERO: check git log for recent changes
    let git_check = check_git_log();
    tracing::info!("STEP ZERO git log check: {}", git_check);

    let title = extract_title(&content).unwrap_or_else(|| "untitled".to_string());
    let repo_type = detect(None)?;
    let groups = group_acs_by_scope(&acs);

    // Build phases from groups
    let mut phases: Vec<PlanPhase> = groups
        .into_iter()
        .map(|(scope, ac_list)| {
            let ac_ids: Vec<String> = ac_list.iter().map(|a| a.id.clone()).collect();
            PlanPhase {
                name: scope.clone(),
                description: format!("{} implementation", scope),
                acs: ac_ids,
            }
        })
        .collect();
    phases.sort_by(|a, b| a.name.cmp(&b.name));

    let story_id = slug_from_path(story_path);
    let gates = gates_for(repo_type).to_vec();

    let plan = StoryPlan {
        story_id,
        title,
        repo_type: repo_type.to_string(),
        phases,
        gates,
    };

    if save {
        let save_path = story_path.with_extension("plan.json");
        std::fs::write(&save_path, serde_json::to_string_pretty(&plan)?)?;
        tracing::info!("Plan saved to {}", save_path.display());
    }

    Ok(plan)
}

/// STEP ZERO: check recent git log for context.
fn check_git_log() -> String {
    let output = std::process::Command::new("git")
        .args(["log", "--oneline", "-10"])
        .output();
    match output {
        Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).trim().to_string(),
        _ => "no git log available".to_string(),
    }
}

fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("# ") {
            return Some(line.trim_start_matches("# ").to_string());
        }
    }
    None
}

fn slug_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Build Loop with Gates (AC12-22)
// ─────────────────────────────────────────────────────────────────────────────

const FAILURES_DIR: &str = "/.cache/ccb/failures";
const MAX_RETRIES: u32 = 3;

/// GateRun records one attempt at a gate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GateRun {
    pub gate: String,
    pub status: String, // pass | fail | retry
    pub attempt: u32,
    pub output: String,
}

/// Failure record persisted to ~/.cache/ccb/failures/<story_id>/<phase>.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureRecord {
    pub story_id: String,
    pub phase: String,
    pub gate: String,
    pub attempt: u32,
    pub output: String,
    pub timestamp: String,
}

fn failures_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(FAILURES_DIR.trim_start_matches('/'))
}

fn load_failures(story_id: &str) -> Result<Vec<FailureRecord>> {
    let path = failures_dir().join(story_id).with_extension("json");
    if !path.exists() {
        return Ok(vec![]);
    }
    let content = std::fs::read_to_string(&path)?;
    let records: Vec<FailureRecord> = serde_json::from_str(&content)?;
    Ok(records)
}

fn save_failure(record: &FailureRecord) -> Result<()> {
    let dir = failures_dir().join(&record.story_id);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", record.phase));
    let mut existing = load_failures(&record.story_id)?;
    existing.push(record.clone());
    std::fs::write(&path, serde_json::to_string_pretty(&existing)?)?;
    Ok(())
}

/// Run a single gate command, return Ok on success or Err with output on failure.
fn run_gate(gate: &str) -> Result<(), String> {
    let mut parts = gate.splitn(2, ' ');
    let cmd = parts.next().unwrap_or(gate);
    let args: Vec<&str> = parts
        .next()
        .map(|a| a.split_whitespace().collect())
        .unwrap_or_default();

    let output = std::process::Command::new(cmd).args(&args).output();
    match output {
        Ok(o) if o.status.success() => Ok(()),
        Ok(o) => Err(String::from_utf8_lossy(&o.stderr).to_string()),
        Err(e) => Err(e.to_string()),
    }
}

/// Implementation result for one phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseResult {
    pub phase: String,
    pub gates: Vec<GateRun>,
    pub success: bool,
}

/// Build result for an entire story.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BuildResult {
    pub story_id: String,
    pub branch: String,
    pub phases: Vec<PhaseResult>,
    pub overall_success: bool,
}

/// Create a branch for the story implementation.
fn create_branch(story_id: &str) -> Result<String> {
    let branch_name = format!("feat/{}", story_id);
    let status = std::process::Command::new("git")
        .args(["checkout", "-b", &branch_name])
        .status()?;
    if !status.success() {
        anyhow::bail!("failed to create branch {}", branch_name);
    }
    Ok(branch_name)
}

/// Commit changes on success.
fn commit_changes(story_id: &str) -> Result<()> {
    std::process::Command::new("git")
        .args(["add", "."])
        .status()?;
    let msg = format!("feat({}): implement {}", story_id, story_id);
    let status = std::process::Command::new("git")
        .args(["commit", "-m", &msg])
        .status()?;
    if !status.success() {
        anyhow::bail!("git commit failed");
    }
    Ok(())
}

/// Push branch and open PR.
fn push_and_pr(branch: &str) -> Result<()> {
    std::process::Command::new("git")
        .args(["push", "-u", "origin", branch])
        .status()?;
    // Open PR via gh
    std::process::Command::new("gh")
        .args(["pr", "create", "--fill"])
        .status()?;
    Ok(())
}

/// Main build loop: create branch → run gates for each phase → persist failures → commit on success.
pub fn build_story(plan: Option<&StoryPlan>, story_path: Option<&Path>) -> Result<BuildResult> {
    let story_id = plan
        .map(|p| p.story_id.clone())
        .or_else(|| story_path.map(slug_from_path))
        .unwrap_or_else(|| "unknown".to_string());

    let repo_type = detect(None)?;
    let gates = plan
        .map(|p| p.gates.clone())
        .unwrap_or_else(|| gates_for(repo_type).to_vec());

    // Create branch
    let branch = create_branch(&story_id)?;

    // For now, run all gates as one phase (simplified)
    let mut phase_result = PhaseResult {
        phase: "implementation".to_string(),
        gates: vec![],
        success: false,
    };

    let mut retry_count = 0;
    for gate in &gates {
        let mut run = GateRun {
            gate: gate.clone(),
            status: "pass".to_string(),
            attempt: 1,
            output: String::new(),
        };

        match run_gate(gate) {
            Ok(()) => {}
            Err(output) => {
                run.status = "retry".to_string();
                run.output = output.clone();
                retry_count += 1;

                // Retry up to MAX_RETRIES
                while retry_count < MAX_RETRIES {
                    run.attempt += 1;
                    match run_gate(gate) {
                        Ok(()) => {
                            run.status = "pass".to_string();
                            run.output = String::new();
                            break;
                        }
                        Err(output) => {
                            run.output = output;
                        }
                    }
                    retry_count += 1;
                }

                if run.status == "retry" {
                    // Persist failure
                    let record = FailureRecord {
                        story_id: story_id.clone(),
                        phase: phase_result.phase.clone(),
                        gate: gate.clone(),
                        attempt: run.attempt,
                        output: run.output.clone(),
                        timestamp: chrono::Utc::now().to_rfc3339(),
                    };
                    let _ = save_failure(&record);
                }
            }
        }
        phase_result.gates.push(run);
    }

    phase_result.success = phase_result.gates.iter().all(|g| g.status == "pass");
    let overall_success = phase_result.success;

    if overall_success {
        let _ = commit_changes(&story_id);
        let _ = push_and_pr(&branch);
    }

    Ok(BuildResult {
        story_id,
        branch,
        phases: vec![phase_result],
        overall_success,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Lessons (AC23-26)
// ─────────────────────────────────────────────────────────────────────────────

const LESSONS_DIR: &str = "/.cache/ccb/lessons";

/// Lesson entry stored in cache.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lesson {
    pub slug: String,
    pub description: String,
    pub repo: String,
    pub timestamp: String,
}

fn lessons_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(LESSONS_DIR.trim_start_matches('/'))
}

/// Capture a lesson from a failure description.
pub fn capture_lesson(repo: &str, description: &str) -> Result<Lesson> {
    let slug = slug_from_description(description);
    let lesson = Lesson {
        slug: slug.clone(),
        description: description.to_string(),
        repo: repo.to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };

    let dir = lessons_dir().join(repo);
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.md", slug));
    let content = format!(
        "# Lesson: {}\n\n**Repo:** {}\n**Captured:** {}\n\n{}\n",
        slug, repo, lesson.timestamp, lesson.description
    );
    std::fs::write(&path, content)?;
    Ok(lesson)
}

fn slug_from_description(desc: &str) -> String {
    desc.split_whitespace()
        .take(5)
        .collect::<Vec<_>>()
        .join("-")
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .collect()
}

/// List lessons for the current repo (or --all for all repos).
pub fn list_lessons(repo_filter: Option<&str>) -> Result<Vec<Lesson>> {
    let dir = lessons_dir();
    let mut lessons = vec![];

    if !dir.exists() {
        return Ok(vec![]);
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let repo = entry.file_name().to_string_lossy().to_string();
        if let Some(filter) = repo_filter {
            if repo != *filter {
                continue;
            }
        }
        if entry.path().is_dir() {
            for lesson_entry in std::fs::read_dir(entry.path())? {
                let lesson_entry = lesson_entry?;
                if lesson_entry
                    .path()
                    .extension()
                    .map(|e| e == "md")
                    .unwrap_or(false)
                {
                    let content = std::fs::read_to_string(lesson_entry.path())?;
                    // Extract description from first line after title
                    let desc = content.lines().nth(3).unwrap_or("").to_string();
                    let slug = lesson_entry
                        .file_name()
                        .to_str()
                        .unwrap_or("unknown")
                        .trim_end_matches(".md")
                        .to_string();
                    lessons.push(Lesson {
                        slug,
                        description: desc,
                        repo: repo.clone(),
                        timestamp: String::new(),
                    });
                }
            }
        }
    }
    Ok(lessons)
}

// ─────────────────────────────────────────────────────────────────────────────
// Command handlers
// ─────────────────────────────────────────────────────────────────────────────

// DetectArgs, PlanArgs, BuildArgs, LessonArgs, GatesArgs are defined in cli.rs
// DetectFormat is also in cli.rs (used by DetectArgs)

pub fn cmd_detect(args: cli::DetectArgs) -> Result<()> {
    let repo_type = detect(None)?;
    match args.format {
        cli::DetectFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&repo_type)?);
        }
        cli::DetectFormat::Human => {
            println!("Detected repo type: {}", repo_type);
            gates_help(repo_type);
        }
    }
    Ok(())
}

pub fn cmd_plan(args: cli::PlanArgs) -> Result<()> {
    let plan = plan_story(&args.story_file, args.save)?;
    println!("{}", serde_json::to_string_pretty(&plan)?);
    Ok(())
}

pub fn cmd_build(args: cli::BuildArgs) -> Result<()> {
    let plan = args.plan.as_ref().map(|p| {
        let content = std::fs::read_to_string(p).unwrap();
        serde_json::from_str(&content).unwrap()
    });

    let result = build_story(plan.as_ref(), args.story.as_deref())?;
    println!("{}", serde_json::to_string_pretty(&result)?);
    if result.overall_success {
        println!("Build succeeded. Branch: {}", result.branch);
    } else {
        println!("Build failed. Check ~/.cache/ccb/failures/");
        std::process::exit(1);
    }
    Ok(())
}

pub fn cmd_lesson(args: cli::LessonArgs) -> Result<()> {
    let repo = detect(None)?.to_string();
    let lesson = capture_lesson(&repo, &args.description)?;
    println!("Lesson captured: {}/{}", lesson.repo, lesson.slug);
    Ok(())
}

pub fn cmd_gates(args: cli::GatesArgs) -> Result<()> {
    let repo_type = detect(None)?;
    let gate_list = gates_for(repo_type);

    if args.run {
        for gate in gate_list {
            print!("Running {} ... ", gate);
            match run_gate(&gate) {
                Ok(()) => println!("PASS"),
                Err(e) => {
                    println!("FAIL: {}", e);
                    return Ok(());
                }
            }
        }
        println!("All gates passed.");
    } else {
        gates_help(repo_type);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_story_acs() {
        let content = r#"# Story 028: Test Story

- AC1: detect repo type
- AC2: parse story file
**AC3: extract ACs
- AC4: should work
"#;
        let acs = parse_story_acs(content);
        assert_eq!(acs.len(), 4);
        assert_eq!(acs[0].id, "1");
        assert_eq!(acs[0].statement, "detect repo type");
    }

    #[test]
    fn test_gates_for_rust() {
        let gates = gates_for(RepoType::Rust);
        assert_eq!(gates.len(), 4);
        assert_eq!(gates[0], "cargo fmt --check");
    }

    #[test]
    fn test_slug_from_description() {
        let slug = slug_from_description("This is a test description for learning");
        assert_eq!(slug, "this-is-a-test-description");
    }

    #[test]
    fn test_repo_type_display() {
        assert_eq!(RepoType::Rust.to_string(), "rust");
        assert_eq!(RepoType::Unknown.to_string(), "unknown");
    }
}
