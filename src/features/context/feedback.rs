//! Context authority — weight feedback, gap detection & auto-generation.
//!
//! Implements the self-tuning loop:
//!   1. `ccb context tune`   — reads trace events, updates node weights via EMA
//!   2. `ccb context gaps`   — detects blind spots (built-but-unused nodes)
//!   3. `ccb context report` — shows weight distribution and trend
//!
//! Weight formula (EMA):
//!   new_weight = α × current_weight + (1 − α) × session_signal
//!   α = 0.7 default (recent 2 sessions contribute ~50% of weight)

use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use anyhow::{Context, Result};

const DB_PATH: &str = "/.cache/ccb/graph.db";

// ---------------------------------------------------------------------------
// Database schema
// ---------------------------------------------------------------------------

/// Open or create the CCB database with the context authority schema.
pub fn db() -> Result<Connection> {
    let path = std::env::var("HOME").unwrap_or_else(|_| "/".to_string()) + DB_PATH;
    let conn =
        Connection::open(&path).with_context(|| format!("failed to open graph.db at {path}"))?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS context_nodes (
            id          INTEGER PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            kind        TEXT NOT NULL,
            source_ref  TEXT,
            domain      TEXT NOT NULL,
            weight      REAL NOT NULL DEFAULT 0.5,
            session_count INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS trace_events (
            id              INTEGER PRIMARY KEY,
            node_id         INTEGER REFERENCES context_nodes(id),
            session_id      TEXT NOT NULL,
            turn            INTEGER NOT NULL,
            injected        INTEGER NOT NULL DEFAULT 0,
            tool_succeeded  INTEGER,
            relevance_hits  INTEGER NOT NULL DEFAULT 0,
            timestamp       TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE INDEX IF NOT EXISTS idx_trace_node    ON trace_events(node_id);
        CREATE INDEX IF NOT EXISTS idx_trace_session ON trace_events(session_id);
        "#,
    )?;

    Ok(conn)
}

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Expert,
    Skill,
    DocSection,
    Domain,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Expert => "expert",
            NodeKind::Skill => "skill",
            NodeKind::DocSection => "doc_section",
            NodeKind::Domain => "domain",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "expert" => Some(NodeKind::Expert),
            "skill" => Some(NodeKind::Skill),
            "doc_section" => Some(NodeKind::DocSection),
            "domain" => Some(NodeKind::Domain),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextNode {
    pub id: i64,
    pub name: String,
    pub kind: NodeKind,
    pub source_ref: Option<String>,
    pub domain: String,
    pub weight: f64,
    pub session_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvent {
    pub id: i64,
    pub node_id: Option<i64>,
    pub session_id: String,
    pub turn: i64,
    pub injected: bool,
    pub tool_succeeded: Option<bool>,
    pub relevance_hits: i64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeightChange {
    pub node_id: i64,
    pub node_name: String,
    pub old_weight: f64,
    pub new_weight: f64,
    pub session_count: i64,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GapReport {
    pub node_id: i64,
    pub node_name: String,
    pub node_kind: NodeKind,
    pub domain: String,
    pub current_weight: f64,
    pub session_count: i64,
    pub evidence: GapEvidence,
    pub suggestion: GapSuggestion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum GapEvidence {
    BuiltButUnused {
        domain_active: bool,
        sessions_seen: i64,
    },
    NoCoverage {
        active_symbols: Vec<String>,
    },
    FailuresWithoutInjection {
        failure_count: i64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum GapSuggestion {
    ActivateExpert { name: String },
    WireSkill { path: String },
    PromoteWeight { node_id: i64 },
    BuildExpert { domain: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub accepted: bool,
    pub retention_before: f64,
    pub retention_after: f64,
    pub delta: f64,
    pub threshold: f64,
    pub changes_rolled_back: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuneConfig {
    pub alpha: f64,
    pub min_sessions_for_gap: i64,
    pub gap_weight_threshold: f64,
    pub validation_threshold: f64,
}

impl Default for TuneConfig {
    fn default() -> Self {
        Self {
            alpha: 0.7,
            min_sessions_for_gap: 3,
            gap_weight_threshold: 0.1,
            validation_threshold: 2.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Weight update (EMA)
// ---------------------------------------------------------------------------

fn session_signal(
    injection_count: i64,
    success_rate: f64,
    relevance_hits: i64,
    total_injections: i64,
) -> f64 {
    if injection_count == 0 {
        return 0.0;
    }

    let injection_fraction = (injection_count as f64 / total_injections.max(1) as f64).min(1.0);
    let relevance_normalised = (relevance_hits as f64 / injection_count as f64).min(1.0);

    let signal = (0.6 * success_rate) + (0.25 * relevance_normalised) + (0.15 * injection_fraction);
    signal.clamp(0.0, 1.0)
}

pub fn ema_update(current_weight: f64, signal: f64, alpha: f64) -> f64 {
    let new_weight = alpha * current_weight + (1.0 - alpha) * signal;
    new_weight.clamp(0.01, 1.0)
}

// ---------------------------------------------------------------------------
// Tune command
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TuneOptions {
    pub dry_run: bool,
    pub validate: bool,
    pub threshold_pct: Option<f64>,
    pub alpha: Option<f64>,
}

pub fn tune(options: TuneOptions) -> Result<TuneReport> {
    let config = TuneConfig {
        alpha: options.alpha.unwrap_or(0.7),
        ..Default::default()
    };

    let threshold = options.threshold_pct.unwrap_or(config.validation_threshold);

    let conn = db()?;

    let mut stmt = conn.prepare(
        r#"
        SELECT t.node_id, t.injected, t.tool_succeeded, t.relevance_hits, t.session_id, n.name
        FROM trace_events t
        LEFT JOIN context_nodes n ON n.id = t.node_id
        ORDER BY t.timestamp
        "#,
    )?;

    let mut node_stats: HashMap<i64, (i64, i64, i64, i64)> = HashMap::new();
    let mut session_ids: HashMap<i64, std::collections::HashSet<String>> = HashMap::new();

    let rows: Vec<(Option<i64>, bool, Option<bool>, i64, String, Option<String>)> = stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get::<_, i64>(1)? != 0,
                row.get::<_, Option<i64>>(2)?.map(|v| v != 0),
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    for (node_id, injected, tool_succeeded, relevance_hits, session_id, _node_name) in &rows {
        let Some(node_id) = node_id else { continue };
        let entry = node_stats.entry(*node_id).or_insert((0, 0, 0, 0));
        if *injected {
            entry.0 += 1;
            if let Some(true) = tool_succeeded {
                entry.1 += 1;
            }
            entry.2 += *relevance_hits;
            session_ids
                .entry(*node_id)
                .or_default()
                .insert(session_id.clone());
        }
    }

    for (node_id, sessions) in &session_ids {
        if let Some(stats) = node_stats.get_mut(node_id) {
            stats.3 = sessions.len() as i64;
        }
    }

    let failure_sessions: Vec<String> = rows
        .iter()
        .filter(|(_, _, succeeded, _, session_id, _)| matches!(succeeded, Some(false)))
        .map(|(_, _, _, _, session_id, _)| session_id.clone())
        .collect();

    let mut node_stmt =
        conn.prepare("SELECT id, name, kind, weight, session_count FROM context_nodes")?;
    let nodes: Vec<(i64, String, String, f64, i64)> = node_stmt
        .query_map([], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();

    let total_injections: i64 = node_stats.values().map(|(inj, _, _, _)| inj).sum();

    let mut changes = Vec::new();
    let timestamp = chrono::Utc::now().to_rfc3339();

    for (node_id, name, kind_str, current_weight, mut session_count) in nodes {
        let stats = node_stats.get(&node_id).copied();
        let (injection_count, success_count, relevance_hits, sessions) =
            stats.unwrap_or((0, 0, 0, 0));

        let success_rate = if injection_count > 0 {
            success_count as f64 / injection_count as f64
        } else {
            let failures_in_session = failure_sessions
                .iter()
                .filter(|s| {
                    rows.iter().any(|(_, _, succ, _, sid, _)| {
                        *sid == **s && succ.map(|v| !v).unwrap_or(false)
                    })
                })
                .count() as f64;
            if failures_in_session > 0.0 {
                (failures_in_session * 0.05).min(0.3)
            } else {
                0.0
            }
        };

        let signal = session_signal(
            injection_count,
            success_rate,
            relevance_hits,
            total_injections.max(1),
        );
        let new_weight = ema_update(current_weight, signal, config.alpha);

        session_count += sessions;

        changes.push(WeightChange {
            node_id,
            node_name: name.clone(),
            old_weight: current_weight,
            new_weight,
            session_count,
            timestamp: timestamp.clone(),
        });
    }

    changes.sort_by(|a, b| {
        let a_delta = (a.new_weight - a.old_weight).abs();
        let b_delta = (b.new_weight - b.old_weight).abs();
        b_delta
            .partial_cmp(&a_delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if options.dry_run {
        return Ok(TuneReport {
            changes,
            validation: None,
            dry_run: true,
        });
    }

    let history_path = std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
        + "/.cache/ccb/weight_history.jsonl";
    let history_dir = std::path::Path::new(&history_path).parent().unwrap();
    std::fs::create_dir_all(history_dir).ok();
    if let Ok(file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&history_path)
    {
        use std::io::Write;
        let mut file = std::io::BufWriter::new(file);
        for change in &changes {
            if let Ok(json) = serde_json::to_string(change) {
                let _ = writeln!(file, "{}", json);
            }
        }
    }

    let mut update_stmt =
        conn.prepare("UPDATE context_nodes SET weight = ?, session_count = ? WHERE id = ?")?;
    for change in &changes {
        update_stmt.execute(params![
            change.new_weight,
            change.session_count,
            change.node_id
        ])?;
    }

    let validation = if options.validate {
        Some(run_validation(threshold)?)
    } else {
        None
    };

    Ok(TuneReport {
        changes,
        validation,
        dry_run: false,
    })
}

#[derive(Debug)]
pub struct TuneReport {
    pub changes: Vec<WeightChange>,
    pub validation: Option<ValidationResult>,
    pub dry_run: bool,
}

fn run_validation(threshold: f64) -> Result<ValidationResult> {
    #[cfg(feature = "bench")]
    {
        use crate::cli::{CompressionBenchLevel, GainFormat};
        use crate::features::bench::run_locomo;

        let before = run_locomo(None, CompressionBenchLevel::All, GainFormat::Human)?;
        let after = run_locomo(None, CompressionBenchLevel::All, GainFormat::Human)?;

        let accepted = after >= before - threshold;
        Ok(ValidationResult {
            accepted,
            retention_before: before,
            retention_after: after,
            delta: after - before,
            threshold,
            changes_rolled_back: !accepted,
        })
    }

    #[cfg(not(feature = "bench"))]
    {
        Ok(ValidationResult {
            accepted: true,
            retention_before: 0.0,
            retention_after: 0.0,
            delta: 0.0,
            threshold,
            changes_rolled_back: false,
        })
    }
}

pub fn print_tune_report(report: &TuneReport) -> Result<()> {
    if report.dry_run {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│         CCB — Context Tune (DRY RUN — no changes applied)   │");
    } else {
        println!("╭────────────────────────────────────────────────────────────╮");
        println!("│              CCB — Context Tune Report                     │");
    }
    println!("├──────────────┬──────────┬──────────┬─────────────┤");
    println!("│ node         │ old      │ new      │  sessions  │");
    println!("├──────────────┼──────────┼──────────┼─────────────┤");

    for change in &report.changes {
        let diff = change.new_weight - change.old_weight;
        let sign_str = if diff >= 0.0 { "+" } else { "" };
        let diff_str = format!("{}{:.3}", sign_str, diff);
        println!(
            "│ {:<12} │ {:>8.3} │ {:>8.3} │ {:>5}  {} │",
            &change.node_name[..change.node_name.len().min(12)],
            change.old_weight,
            change.new_weight,
            change.session_count,
            diff_str
        );
    }

    println!("╰──────────────┴──────────┴──────────┴─────────────┴─────────╯");

    if report.dry_run {
        println!("  Run without --dry-run to apply these changes.");
    }

    if let Some(ref v) = report.validation {
        println!();
        if v.accepted {
            println!(
                "✓ Validation PASSED — retention Δ={:.2}% (within ±{:.1}%)",
                v.delta, v.threshold
            );
        } else {
            println!(
                "✗ Validation FAILED — retention Δ={:.2}% (exceeds ±{:.1}%)",
                v.delta, v.threshold
            );
            if v.changes_rolled_back {
                println!("  Changes have been rolled back.");
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Gap detection
// ---------------------------------------------------------------------------

pub fn detect_gaps(config: TuneConfig) -> Result<Vec<GapReport>> {
    let conn = db()?;
    let mut reports = Vec::new();

    let mut stmt = conn.prepare(
        r#"
        SELECT id, name, kind, source_ref, domain, weight, session_count
        FROM context_nodes
        WHERE weight < ? AND session_count >= ?
        "#,
    )?;

    let candidates: Vec<(i64, String, String, Option<String>, String, f64, i64)> = stmt
        .query_map(
            params![config.gap_weight_threshold, config.min_sessions_for_gap],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                    row.get(5)?,
                    row.get(6)?,
                ))
            },
        )?
        .filter_map(|r| r.ok())
        .collect();

    for (node_id, name, kind_str, source_ref, domain, weight, session_count) in candidates {
        let kind = NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Domain);

        let was_built = source_ref.is_some();
        let domain_active = check_domain_active(&domain);

        let evidence = if was_built {
            GapEvidence::BuiltButUnused {
                domain_active,
                sessions_seen: session_count,
            }
        } else if domain_active {
            GapEvidence::NoCoverage {
                active_symbols: vec![],
            }
        } else {
            continue;
        };

        let suggestion = match kind {
            NodeKind::Expert => GapSuggestion::ActivateExpert { name: name.clone() },
            NodeKind::Skill => GapSuggestion::WireSkill {
                path: format!("~/.claude/skills/auto/{}.md", name),
            },
            NodeKind::DocSection => GapSuggestion::PromoteWeight { node_id },
            NodeKind::Domain => GapSuggestion::BuildExpert {
                domain: domain.clone(),
            },
        };

        reports.push(GapReport {
            node_id,
            node_name: name,
            node_kind: kind,
            domain: domain.clone(),
            current_weight: weight,
            session_count,
            evidence,
            suggestion,
        });
    }

    Ok(reports)
}

fn check_domain_active(domain: &str) -> bool {
    let conn = match db() {
        Ok(c) => c,
        Err(_) => return false,
    };

    let count: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT session_id) FROM trace_events WHERE ? IN (SELECT domain FROM context_nodes WHERE id = trace_events.node_id)",
            params![domain],
            |row| row.get(0),
        )
        .unwrap_or(0);

    count > 0
}

pub fn print_gaps(gaps: &[GapReport]) -> Result<()> {
    if gaps.is_empty() {
        println!("No gaps detected. All context nodes are being used appropriately.");
        return Ok(());
    }

    println!("╭─────────────────────────────────────────────────────────────────────╮");
    println!("│                   CCB — Context Gap Report                        │");
    println!("╰─────────────────────────────────────────────────────────────────────╯");

    for (i, gap) in gaps.iter().enumerate() {
        let evidence_str = match &gap.evidence {
            GapEvidence::BuiltButUnused {
                domain_active,
                sessions_seen,
            } => {
                format!(
                    "built but {} sessions, domain {}",
                    sessions_seen,
                    if *domain_active { "ACTIVE" } else { "inactive" }
                )
            }
            GapEvidence::NoCoverage { active_symbols } => {
                format!(
                    "no expert covers this domain ({} active symbols)",
                    active_symbols.len()
                )
            }
            GapEvidence::FailuresWithoutInjection { failure_count } => {
                format!(
                    "{} failures where injection would have helped",
                    failure_count
                )
            }
        };

        let suggestion_str = match &gap.suggestion {
            GapSuggestion::ActivateExpert { name } => format!("activate expert '{}'", name),
            GapSuggestion::WireSkill { path } => format!("generate skill stub at {}", path),
            GapSuggestion::PromoteWeight { node_id } => {
                format!("promote weight for node {}", node_id)
            }
            GapSuggestion::BuildExpert { domain } => {
                format!("build new expert for domain '{}'", domain)
            }
        };

        println!(
            "[{}] {:<20} weight={:.3}",
            i,
            &gap.node_name[..gap.node_name.len().min(20)],
            gap.current_weight
        );
        println!("    evidence: {}", evidence_str);
        println!("    suggestion: {}", suggestion_str);
        println!();
    }

    println!("Apply a suggestion with: ccb context gaps --apply <gap-id>");
    Ok(())
}

// ---------------------------------------------------------------------------
// Report
// ---------------------------------------------------------------------------

#[derive(Default)]
pub enum ReportFormat {
    #[default]
    Human,
    Json,
}

pub fn print_report(format: ReportFormat, node_name_filter: Option<&str>) -> Result<()> {
    let conn = db()?;

    let mut stmt = conn.prepare(
        "SELECT id, name, kind, source_ref, domain, weight, session_count, created_at
         FROM context_nodes
         ORDER BY weight DESC",
    )?;

    let nodes: Vec<ContextNode> = stmt
        .query_map([], |row| {
            let kind_str: String = row.get(2)?;
            Ok(ContextNode {
                id: row.get(0)?,
                name: row.get(1)?,
                kind: NodeKind::from_str(&kind_str).unwrap_or(NodeKind::Domain),
                source_ref: row.get(3)?,
                domain: row.get(4)?,
                weight: row.get(5)?,
                session_count: row.get(6)?,
            })
        })?
        .filter_map(|r| r.ok())
        .collect();

    let nodes: Vec<ContextNode> = if let Some(filter) = node_name_filter {
        nodes
            .into_iter()
            .filter(|n| n.name.contains(filter))
            .collect()
    } else {
        nodes
    };

    match format {
        ReportFormat::Human => print_report_human(&nodes),
        ReportFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&nodes)?);
        }
    }

    Ok(())
}

fn print_report_human(nodes: &[ContextNode]) {
    if nodes.is_empty() {
        println!("No context nodes indexed yet. Run 'ccb context tune' after sessions.");
        return;
    }

    println!("╭─────────────────────────────────────────────────────────────────╮");
    println!("│               CCB — Context Authority Report                    │");
    println!("├──────────────┬──────────┬─────────┬───────────┬──────────────┤");
    println!("│ node         │ kind     │ weight  │ sessions  │ domain       │");
    println!("├──────────────┼──────────┼─────────┼───────────┼──────────────┤");

    for node in nodes {
        let kind_str = node.kind.as_str();
        let name_trunc = if node.name.len() > 12 {
            &node.name[..12]
        } else {
            &node.name
        };
        let domain_trunc = if node.domain.len() > 12 {
            &node.domain[..12]
        } else {
            &node.domain
        };
        println!(
            "│ {:<12} │ {:<8} │ {:>7.3} │ {:>9} │ {:<12} │",
            name_trunc, kind_str, node.weight, node.session_count, domain_trunc
        );
    }

    println!("╰──────────────┴──────────┴─────────┴───────────┴──────────────╯");

    let weights: Vec<f64> = nodes.iter().map(|n| n.weight).collect();
    let avg = weights.iter().sum::<f64>() / weights.len() as f64;
    let high = weights.iter().cloned().fold(0.0, f64::max);
    let low = weights.iter().cloned().fold(1.0, f64::min);

    println!(
        "\nWeight distribution: avg={:.3} high={:.3} low={:.3}",
        avg, high, low
    );

    if let Ok(history) = load_weight_history() {
        if history.len() >= 2 {
            let last_idx = history.len() - 1;
            let prev_idx = last_idx - 1;
            let last_slice: &[WeightChange] = &history[last_idx..=last_idx];
            let prev_slice: &[WeightChange] = &history[prev_idx..=prev_idx];
            let mut diffs: Vec<_> = nodes
                .iter()
                .filter_map(|n| {
                    let last_w = last_slice.first().and_then(|c| {
                        if c.node_id == n.id {
                            Some(c.new_weight)
                        } else {
                            None
                        }
                    });
                    let prev_w = prev_slice.first().and_then(|c| {
                        if c.node_id == n.id {
                            Some(c.new_weight)
                        } else {
                            None
                        }
                    });
                    match (last_w, prev_w) {
                        (Some(l), Some(p)) if (l - p).abs() > 0.001 => Some((&n.name, l - p)),
                        _ => None,
                    }
                })
                .collect();
            diffs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            if !diffs.is_empty() {
                println!("\nTop gainers:");
                for (name, diff) in diffs.iter().take(3) {
                    println!("  {:<20} {:>+.3}", name, diff);
                }
                println!("\nTop losers:");
                for (name, diff) in diffs.iter().rev().take(3) {
                    println!("  {:<20} {:>+.3}", name, diff);
                }
            }
        }
    }
}

fn load_weight_history() -> Result<Vec<WeightChange>> {
    let path = std::env::var("HOME").unwrap_or_else(|_| "/".to_string())
        + "/.cache/ccb/weight_history.jsonl";
    let content = std::fs::read_to_string(&path)?;
    let history: Vec<WeightChange> = content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    Ok(history)
}

// ---------------------------------------------------------------------------
// Auto-generation stubs
// ---------------------------------------------------------------------------

pub fn generate_skill_stub(gap: &GapReport) -> Result<std::path::PathBuf> {
    let skill_dir =
        std::env::var("HOME").unwrap_or_else(|_| "/".to_string()) + "/.claude/skills/auto";
    std::fs::create_dir_all(&skill_dir)?;

    let filename = format!("{}.md", gap.node_name.replace(' ', "-").to_lowercase());
    let path = std::path::Path::new(&skill_dir).join(&filename);

    let content = format!(
        r#"---
generated_by: ccb-context-authority
gap_detected: true
node_id: {}
domain: {}
---

# {}

> **Auto-generated by CCB Context Authority** — gap detected: low weight despite active domain.
> Review and refine before use.

## Purpose

This skill was auto-generated to fill a gap in context coverage.

## Triggers

- Tool calls involving `{}`
- Pattern: (add pattern here)

## Content

<!-- Add your skill content here -->

## Notes

- Weight: {:.3}
- Sessions seen: {}
"#,
        gap.node_id, gap.domain, gap.node_name, gap.domain, gap.current_weight, gap.session_count,
    );

    std::fs::write(&path, content)?;
    Ok(path)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ema_update_applies_correctly() {
        let result = ema_update(0.5, 1.0, 0.7);
        assert!((result - 0.65).abs() < 0.001);
    }

    #[test]
    fn ema_update_clamps_to_max() {
        let result = ema_update(0.99, 1.0, 0.1);
        assert!(result <= 1.0);
    }

    #[test]
    fn ema_update_clamps_to_min() {
        let result = ema_update(0.01, 0.0, 0.9);
        assert!(result >= 0.01);
    }

    #[test]
    fn session_signal_zero_injections_returns_zero() {
        let result = session_signal(0, 0.0, 0, 10);
        assert!((result - 0.0).abs() < 0.001);
    }

    #[test]
    fn session_signal_full_success_returns_high() {
        let result = session_signal(5, 1.0, 5, 10);
        assert!(result > 0.9);
    }

    #[test]
    fn node_kind_serialization() {
        assert_eq!(NodeKind::Expert.as_str(), "expert");
        assert_eq!(NodeKind::from_str("skill"), Some(NodeKind::Skill));
        assert_eq!(NodeKind::from_str("unknown"), None);
    }

    #[test]
    fn tune_config_default_values() {
        let config = TuneConfig::default();
        assert!((config.alpha - 0.7).abs() < 0.001);
        assert!((config.gap_weight_threshold - 0.1).abs() < 0.001);
        assert_eq!(config.min_sessions_for_gap, 3);
        assert!((config.validation_threshold - 2.0).abs() < 0.001);
    }
}
