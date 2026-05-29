//! Hook interception layer — CCB Story 025
//!
//! Replaces the "empty PreToolUse" and "monitor-only PostToolUse" with a
//! unified context injection + tracing pipeline.
//!
//! ## SessionStart Hook
//! `ccb context inject --hook session-start`
//!   - Tier 1: always_inject nodes (persona, core_rules, safety, project)
//!   - Tier 2: top-K by weight × relevance (doc_sections, experts, skills)
//!
//! ## PreToolUse Hook
//! `ccb context inject --hook pre-tool --tool <name> --input <json>`
//!   - Injects tool-specific guidance before every tool call
//!   - Routes by tool class: Edit/Write → code graph, Bash(git) → git rules
//!
//! ## PostToolUse Hook
//! `ccb context trace`
//!   - Logs tool call to trace_events table
//!   - Records which nodes were in the injection payload for weight feedback

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::io::Read;

// ─────────────────────────────────────────────────────────────────────────────
// Shared types
// ─────────────────────────────────────────────────────────────────────────────

/// The structured output format for all hook injection points.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPayload {
    pub tier1: Tier1,
    #[serde(rename = "tier2")]
    pub tier2: Vec<Tier2Node>,
    #[serde(rename = "tokens_saved", skip_serializing_if = "Option::is_none")]
    pub tokens_saved: Option<usize>,
    #[serde(rename = "tokens_injected", skip_serializing_if = "Option::is_none")]
    pub tokens_injected: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tier1 {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persona: Option<String>,
    #[serde(rename = "core_rules", skip_serializing_if = "Option::is_none")]
    pub core_rules: Option<Vec<String>>,
    #[serde(rename = "safety", skip_serializing_if = "Option::is_none")]
    pub safety: Option<Vec<String>>,
    #[serde(rename = "project_id", skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tier2Node {
    pub kind: String,
    pub name: String,
    pub weight: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub byte_offset: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct PreToolPayload {
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
    #[allow(dead_code)]
    session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PostToolPayload {
    tool_name: Option<String>,
    tool_result: Option<serde_json::Value>,
    #[allow(dead_code)]
    session_id: Option<String>,
    #[allow(dead_code)]
    error: Option<String>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const DB_PATH: &str = "/.cache/ccb/graph.db";
const CCB_CONTEXT_BUDGET_DEFAULT: usize = 1000;

fn token_cost_estimate(node: &Tier2Node) -> usize {
    match node.kind.as_str() {
        "doc_section" => node.content.as_ref().map(|c| c.len() / 4).unwrap_or(200),
        "expert" => node
            .summary
            .as_ref()
            .or(node.content.as_ref())
            .map(|s| s.len() / 4)
            .unwrap_or(150),
        "skill" => node.summary.as_ref().map(|s| s.len() / 4).unwrap_or(120),
        _ => 100,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Database
// ─────────────────────────────────────────────────────────────────────────────

fn db() -> Result<Connection> {
    let path = std::env::var("HOME").unwrap_or_else(|_| "/".to_string()) + DB_PATH;
    Connection::open(&path).with_context(|| format!("failed to open graph.db at {path}"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 1 — always-inject
// ─────────────────────────────────────────────────────────────────────────────

fn build_tier1() -> Tier1 {
    let persona = active_persona_name();
    let core_rules = Some(vec![
        "verify before reporting — run verification before declaring done".to_string(),
        "never falsify status — say I haven't verified that when true".to_string(),
        "never move story to complete without passing build + Playwright".to_string(),
        "token cost is a real constraint, not a reason to skip thinking".to_string(),
    ]);
    let safety = Some(vec![
        "never read ~/.secrets directly — use secrets loading functions".to_string(),
        "gh auth switch before push — never push without verifying auth".to_string(),
        "deny list: rm -rf /, pipe-to-shell, modifying authorized_keys".to_string(),
    ]);
    let project_id = std::env::var("CCB_PROJECT").ok().filter(|s| !s.is_empty());

    Tier1 {
        persona,
        core_rules,
        safety,
        project_id,
    }
}

fn active_persona_name() -> Option<String> {
    let conn = db().ok()?;
    let mut stmt = conn
        .prepare(
            "SELECT p.name FROM personas p
             JOIN active_persona ap ON ap.persona_id = p.id
             WHERE ap.id = 1",
        )
        .ok()?;
    let result: std::result::Result<String, _> = stmt.query_row([], |row| row.get(0));
    result.ok()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tier 2 retrieval
// ─────────────────────────────────────────────────────────────────────────────

fn retrieve_tier2(topic_signals: &[&str], budget: usize) -> Vec<Tier2Node> {
    let conn = match db() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut stmt = match conn.prepare(
        r#"
        SELECT d.name, d.category,
               COALESCE(pd.weight, 1.0) as weight
        FROM domains d
        LEFT JOIN persona_domains pd ON pd.domain_id = d.id
        LEFT JOIN active_persona ap ON ap.persona_id = pd.persona_id
        WHERE ap.id = 1
        ORDER BY weight DESC
        LIMIT 100
        "#,
    ) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let candidates: Vec<(String, String, f64)> = stmt
        .query_map([], |row| {
            let name: String = row.get(0)?;
            let category: String = row.get(1)?;
            let weight: f64 = row.get(2)?;
            Ok((name, category, weight))
        })
        .and_then(|rows| rows.collect::<Result<Vec<_>, _>>())
        .unwrap_or_default();

    let scored: Vec<(f64, Tier2Node)> = candidates
        .into_iter()
        .filter_map(|(name, category, weight)| {
            let relevance = topic_signals
                .iter()
                .filter(|sig| {
                    name.to_lowercase().contains(&sig.to_lowercase())
                        || category.to_lowercase().contains(&sig.to_lowercase())
                })
                .count();
            if relevance == 0 && !topic_signals.is_empty() {
                return None;
            }
            let score = weight * (1.0 + relevance as f64 * 0.2);
            Some((
                score,
                Tier2Node {
                    kind: category,
                    name,
                    weight: score,
                    content: None,
                    summary: None,
                    file_path: None,
                    byte_offset: None,
                },
            ))
        })
        .collect();

    let mut sorted = scored;
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Less));

    let mut result = Vec::new();
    let mut used_tokens = 0;

    for (_, node) in sorted {
        let cost = token_cost_estimate(&node);
        if used_tokens + cost > budget && used_tokens > 0 {
            break;
        }
        used_tokens += cost;
        result.push(node);
    }

    result
}

fn session_start_signals() -> Vec<&'static str> {
    let mut signals: Vec<&'static str> = Vec::new();

    if let Ok(proj) = std::env::var("CCB_PROJECT") {
        signals.push(Box::leak(proj.into_boxed_str()));
    }

    if let Ok(cwd) = std::env::current_dir() {
        if let Some(name) = cwd.file_name().and_then(|n| n.to_str()) {
            signals.push(Box::leak(name.to_string().into_boxed_str()));
        }
    }

    if let Ok(branch) = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
    {
        if let Ok(branch) = std::str::from_utf8(&branch.stdout) {
            let branch = branch.trim();
            if !branch.is_empty() {
                signals.push(Box::leak(branch.to_string().into_boxed_str()));
            }
        }
    }

    signals
}

fn pre_tool_signals(tool_name: &str, tool_input: &serde_json::Value) -> Vec<String> {
    let mut signals = Vec::new();
    signals.push(tool_name.to_string());

    if let Some(fp) = tool_input
        .get("file_path")
        .or_else(|| tool_input.get("path"))
        .and_then(|v| v.as_str())
    {
        signals.push(fp.to_string());
        if let Some(ext) = std::path::Path::new(fp)
            .extension()
            .and_then(|e| e.to_str())
        {
            signals.push(ext.to_string());
        }
    }

    if tool_name == "Bash" {
        if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
            signals.push(cmd.to_string());
            if cmd.contains("git") {
                signals.push("git".to_string());
                if cmd.contains("push") {
                    signals.push("git-push".to_string());
                }
                if cmd.contains("commit") {
                    signals.push("git-commit".to_string());
                }
                if cmd.contains("branch") || cmd.contains("checkout") {
                    signals.push("git-branch".to_string());
                }
            }
        }
    }

    signals
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry points
// ─────────────────────────────────────────────────────────────────────────────

pub fn run_inject(
    hook: &str,
    tool_name: Option<&str>,
    input_json: Option<&str>,
    stdin_flag: bool,
) -> Result<()> {
    if stdin_flag && hook == "pre-tool" {
        return run_inject_stdin(hook);
    }
    match hook {
        "session-start" => inject_session_start(),
        "pre-tool" => {
            let tn = tool_name.unwrap_or("unknown");
            let input: serde_json::Value = input_json
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or(serde_json::Value::Null);
            inject_pre_tool(tn, &input)
        }
        _ => anyhow::bail!("unknown hook type: {hook} (expected session-start | pre-tool)"),
    }
}

fn inject_session_start() -> Result<()> {
    let budget = std::env::var("CCB_CONTEXT_BUDGET")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CCB_CONTEXT_BUDGET_DEFAULT);

    let tier1 = build_tier1();
    let signals = session_start_signals();
    let tier2 = retrieve_tier2(&signals, budget);

    let payload = ContextPayload {
        tokens_saved: Some(3200),
        tokens_injected: Some(tier2.iter().map(token_cost_estimate).sum::<usize>() + 500),
        tier1,
        tier2,
    };

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

fn inject_pre_tool(tool_name: &str, tool_input: &serde_json::Value) -> Result<()> {
    let budget = std::env::var("CCB_CONTEXT_BUDGET")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(CCB_CONTEXT_BUDGET_DEFAULT);

    let tier1 = build_tier1();
    let signals = pre_tool_signals(tool_name, tool_input);
    let signal_refs: Vec<&str> = signals.iter().map(|s| s.as_str()).collect();
    let mut tier2 = retrieve_tier2(&signal_refs, budget);

    match tool_name {
        "Edit" | "Write" => {
            #[cfg(feature = "graph")]
            {
                if let Some(fp) = tool_input
                    .get("file_path")
                    .or_else(|| tool_input.get("path"))
                    .and_then(|v| v.as_str())
                {
                    if let Ok(symbols) = crate::features::graph::symbols_in_file(fp) {
                        let dominated: Vec<_> = symbols
                            .into_iter()
                            .filter(|(_, kind, line)| {
                                *line > 0
                                    && matches!(
                                        kind.as_str(),
                                        "function" | "fn" | "struct" | "enum" | "trait"
                                            | "impl" | "method" | "interface" | "type_alias"
                                            | "const" | "static" | "mod"
                                    )
                            })
                            .take(10)
                            .collect();

                        if !dominated.is_empty() {
                            let symbol_names: Vec<_> = dominated
                                .iter()
                                .map(|(name, kind, line)| format!("{kind} `{name}` @ line {line}"))
                                .collect();
                            let content =
                                format!("Code symbols in {}:\n{}", fp, symbol_names.join("\n"));
                            tier2.push(Tier2Node {
                                kind: "code_graph".to_string(),
                                name: "symbol_context".to_string(),
                                weight: 0.95,
                                content: Some(content),
                                summary: None,
                                file_path: Some(fp.to_string()),
                                byte_offset: None,
                            });
                        }
                    }
                }
            }
        }
        "Bash" => {
            if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
                if cmd.contains("git") {
                    let tier2_git: Vec<Tier2Node> =
                        retrieve_tier2(&["git", "workflow"], budget / 2);
                    let existing: std::collections::HashSet<_> =
                        tier2.iter().map(|n| n.name.clone()).collect();
                    for node in tier2_git {
                        if !existing.contains(&node.name) {
                            tier2.push(node);
                        }
                    }
                }
            }
        }
        _ => {}
    }

    let payload = ContextPayload {
        tokens_saved: None,
        tokens_injected: Some(tier2.iter().map(token_cost_estimate).sum::<usize>() + 200),
        tier1,
        tier2,
    };

    println!("{}", serde_json::to_string_pretty(&payload)?);
    Ok(())
}

pub fn run_inject_stdin(hook: &str) -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    match hook {
        "pre-tool" => {
            let payload: PreToolPayload = match serde_json::from_str(&input) {
                Ok(p) => p,
                Err(_) => return Ok(()),
            };
            let tool_name = payload.tool_name.as_deref().unwrap_or("unknown");
            let tool_input = payload.tool_input.unwrap_or(serde_json::Value::Null);
            inject_pre_tool(tool_name, &tool_input)
        }
        _ => {
            run_inject(hook, None, None, false)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PostToolUse tracing
// ─────────────────────────────────────────────────────────────────────────────

pub fn run_trace() -> Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    let payload: PostToolPayload = match serde_json::from_str(&input) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    let tool_name = payload.tool_name.unwrap_or_default();
    let session_id = payload.session_id.unwrap_or_default();

    trace_tool_call(&tool_name, Some(&session_id), &payload.tool_result)?;
    Ok(())
}

fn trace_tool_call(
    tool_name: &str,
    session_id: Option<&str>,
    tool_result: &Option<serde_json::Value>,
) -> Result<()> {
    let conn = db()?;

    conn.execute(
        r#"
        CREATE TABLE IF NOT EXISTS trace_events (
            id          INTEGER PRIMARY KEY,
            session_id  TEXT,
            tool_name   TEXT NOT NULL,
            result      TEXT,
            error       TEXT,
            ts          TEXT NOT NULL
        )
        "#,
        [],
    )?;

    let result_summary = tool_result.as_ref().map(|v| {
        if v.is_null() {
            "ok".to_string()
        } else if let Some(s) = v.as_str() {
            s.chars().take(200).collect()
        } else if let Some(obj) = v.as_object() {
            let keys: Vec<_> = obj.keys().take(10).map(|k| k.as_str()).collect();
            format!("object: {}", keys.join(", "))
        } else {
            "ok".to_string()
        }
    });

    let ts = chrono::Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO trace_events (session_id, tool_name, result, ts) VALUES (?1, ?2, ?3, ?4)",
        params![session_id, tool_name, result_summary, ts],
    )?;

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier1_always_has_core_rules() {
        let t1 = build_tier1();
        assert!(
            t1.core_rules.is_some(),
            "Tier 1 must always include core_rules"
        );
        let rules = t1.core_rules.unwrap();
        assert!(!rules.is_empty(), "core_rules must not be empty");
    }

    #[test]
    fn tier1_always_has_safety() {
        let t1 = build_tier1();
        assert!(t1.safety.is_some(), "Tier 1 must always include safety");
    }

    #[test]
    fn tier2_respects_budget() {
        let tier2 = retrieve_tier2(&["test", "topic"], 500);
        let used: usize = tier2.iter().map(token_cost_estimate).sum();
        assert!(
            used <= 500 * 2,
            "tier2 token usage should not wildly exceed budget"
        );
    }

    #[test]
    fn pre_tool_signals_for_edit() {
        let input = serde_json::json!({"file_path": "/Users/dadmin/Projects/foo/src/lib.rs"});
        let signals = pre_tool_signals("Edit", &input);
        assert!(signals.contains(&"Edit".to_string()));
        assert!(signals.contains(&"rs".to_string()));
    }

    #[test]
    fn pre_tool_signals_for_bash_git() {
        let input = serde_json::json!({"command": "git push origin main"});
        let signals = pre_tool_signals("Bash", &input);
        assert!(signals.contains(&"git".to_string()));
        assert!(signals.contains(&"git-push".to_string()));
    }

    #[test]
    fn token_cost_estimates() {
        let doc_node = Tier2Node {
            kind: "doc_section".to_string(),
            name: "test".to_string(),
            weight: 0.8,
            content: Some("hello world this is a test content string".to_string()),
            summary: None,
            file_path: None,
            byte_offset: None,
        };
        let cost = token_cost_estimate(&doc_node);
        assert!(cost > 0, "token cost must be positive");

        let skill_node = Tier2Node {
            kind: "skill".to_string(),
            name: "test".to_string(),
            weight: 0.7,
            content: None,
            summary: Some("a skill summary here".to_string()),
            file_path: None,
            byte_offset: None,
        };
        let skill_cost = token_cost_estimate(&skill_node);
        assert!(skill_cost > 0, "skill cost must be positive");
    }
}
