//! Skills — generate skill files from mined patterns
//!
//! Generated skill files are written to ~/.claude/skills/auto/
//! with descriptive filenames derived from the pattern.
//!
//! After generating skill files, runs `style index-build` to update INDEX.md.

use crate::features::memory::db::MinedPattern;
use anyhow::{bail, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

/// The auto-generated skills directory.
fn auto_skills_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude")
        .join("skills")
        .join("auto")
}

/// Sanitize a string into a safe filename component.
fn to_filename_component(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
        .collect::<String>()
        .chars()
        .take(40)
        .collect()
}

/// Derive a safe filename from a pattern.
fn pattern_filename(pattern: &MinedPattern, index: usize) -> String {
    let base = match pattern.pattern_type {
        db::PatternType::ToolSequence => "tool_seq",
        db::PatternType::FileCluster => "file_cluster",
        db::PatternType::ErrorFix => "error_fix",
        db::PatternType::PersonaDomain => "persona_domain",
    };
    // Hash the description to keep filename unique and short
    let desc_hash = pattern.description
        .chars()
        .filter(|c| c.is_alphanumeric())
        .take(12)
        .collect::<String>()
        .to_lowercase();
    format!("{}_{}_{}", base, desc_hash, index)
}

/// Generate skill file content from a pattern.
fn generate_content(pattern: &MinedPattern) -> String {
    use crate::features::memory::db::PatternType;
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();

    let (pattern_detail, when_to_apply) = match pattern.pattern_type {
        PatternType::ToolSequence => {
            // Extract tool and file from description
            let detail = format!(
                r#"## Tool Usage Pattern

This pattern was detected from {} sessions.

Apply when using the described tool on the matching file type."#,
                pattern.frequency
            );
            let when = "When you encounter the same tool+file combination in a new session.";
            (detail, when.to_string())
        }
        PatternType::FileCluster => {
            let detail = format!(
                r#"## Co-edited Files

These files are frequently edited together ({} sessions).

Expect coordinated changes to both files when working in this area."#,
                pattern.frequency
            );
            let when = "When you edit one of the paired files, check if the other needs updating too.".to_string();
            (detail, when)
        }
        PatternType::ErrorFix => {
            let detail = format!(
                r#"## Error Recovery Pattern

This error→fix sequence has repeated {} times.

When you encounter the error, apply the corresponding fix."#,
                pattern.frequency
            );
            let when = "When you see the specific error type, apply the proven fix.".to_string();
            (detail, when)
        }
        PatternType::PersonaDomain => {
            let detail = format!(
                r#"## Persona→Domain Association

This persona has been used {} times for this domain.

Use this persona when working on tasks in this domain."#,
                pattern.frequency
            );
            let when = "When working in the associated domain, activate the matched persona.".to_string();
            (detail, when)
        }
    };

    let front_matter = format!(
        r#"---
name: auto_{}
description: Auto-generated pattern: {}
source: ccb-memory-mine
frequency: {}
first_seen: {}
last_seen: {}
metadata:
  pattern_type: {}
  auto_generated: true
---

"#,
        to_filename_component(&pattern.description),
        pattern.description.replace('"', "'"),
        pattern.frequency,
        pattern.first_seen,
        pattern.last_seen,
        pattern.pattern_type,
    );

    format!(
        "{}{}\n## Description\n\n{}\n\n## When to apply\n\n{}\n",
        front_matter,
        pattern.description,
        pattern_detail,
        when_to_apply
    )
}

/// Write a single skill file. Returns the path it was written to.
pub fn generate_skill_file(pattern: &MinedPattern) -> Result<PathBuf> {
    let dir = auto_skills_dir();
    std::fs::create_dir_all(&dir)?;

    let filename = format!("{}.md", to_filename_component(&pattern.description));
    let path = dir.join(&filename);

    // If file already exists with same content, skip (idempotent)
    let content = generate_content(pattern);
    if path.exists() {
        let existing = std::fs::read_to_string(&path)?;
        if existing == content {
            return Ok(path);
        }
    }

    std::fs::write(&path, &content)?;
    Ok(path)
}

/// Generate skill files for all patterns. Returns paths written.
pub fn generate_all_skills(patterns: &[MinedPattern]) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for (i, pattern) in patterns.iter().enumerate() {
        // Only generate for patterns that don't already have a skill_path
        // (don't regenerate suppressed or previously generated)
        if pattern.skill_path.is_none() {
            let path = generate_skill_file_with_index(pattern, i)?;
            paths.push(path);
        }
    }
    Ok(paths)
}

/// Generate with index appended to ensure unique filename even for similar descriptions.
fn generate_skill_file_with_index(pattern: &MinedPattern, index: usize) -> Result<PathBuf> {
    let dir = auto_skills_dir();
    std::fs::create_dir_all(&dir)?;

    let filename = format!("{}.md", pattern_filename(pattern, index));
    let path = dir.join(&filename);
    let content = generate_content(pattern);
    std::fs::write(&path, &content)?;
    Ok(path)
}

/// Run `style index-build` to regenerate INDEX.md.
pub fn rebuild_index() -> Result<()> {
    let skills_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".claude")
        .join("skills");

    let output = Command::new("ccb")
        .args(["style", "index-build"])
        .current_dir(skills_dir)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("style index-build failed: {}", stderr);
    }
    Ok(())
}

/// Update the skill_path on a pattern in the DB.
pub fn link_skill_path(
    pattern: &MinedPattern,
    skill_path: &Path,
    conn: &rusqlite::Connection,
) -> Result<()> {
    if let Some(id) = pattern.id {
        crate::features::memory::db::set_skill_path(conn, id, skill_path.to_str().unwrap_or(""))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::memory::db::{MinedPattern, PatternType};

    #[test]
    fn to_filename_component_works() {
        assert_eq!(to_filename_component("Hello World!"), "HelloWorld");
        assert_eq!(to_filename_component("foo/bar.rs"), "foobar");
        assert_eq!(to_filename_component("a[b]c"), "abc");
    }

    #[test]
    fn pattern_filename_format() {
        let pattern = MinedPattern {
            id: Some(1),
            pattern_type: PatternType::ToolSequence,
            description: "Tool 'Read' applied to '/a.rs' across 3 sessions".to_string(),
            frequency: 3,
            first_seen: "2026-05-28T00:00:00Z".to_string(),
            last_seen: "2026-05-28T12:00:00Z".to_string(),
            skill_path: None,
            suppressed: false,
        };
        let name = pattern_filename(&pattern, 0);
        assert!(name.starts_with("tool_seq_"));
        assert!(name.ends_with("_0.md"));
    }

    #[test]
    fn generate_content_for_tool_sequence() {
        let pattern = MinedPattern {
            id: Some(1),
            pattern_type: PatternType::ToolSequence,
            description: "Tool 'Read' applied to '/a.rs' across 3 sessions".to_string(),
            frequency: 3,
            first_seen: "2026-05-28T00:00:00Z".to_string(),
            last_seen: "2026-05-28T12:00:00Z".to_string(),
            skill_path: None,
            suppressed: false,
        };
        let content = generate_content(&pattern);
        assert!(content.contains("name: auto_"));
        assert!(content.contains("description: Auto-generated pattern:"));
        assert!(content.contains("Tool Usage Pattern"));
        assert!(content.contains("pattern_type: tool_sequence"));
    }

    #[test]
    fn generate_skills_dir() {
        let dir = auto_skills_dir();
        assert!(dir.to_str().unwrap().ends_with(".claude/skills/auto"));
    }
}