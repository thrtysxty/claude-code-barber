//! Mine — trace pattern detection via SQL aggregation
//!
//! Phase 1: pure frequency analysis, no LLM involved.
//! Runs SQL aggregation queries against traces.db to find:
//!   - tool_sequence: same tool+file across ≥N sessions
//!   - file_cluster: files edited together in same session ≥N times
//!   - error_fix: error→edit recovery sequence repeated ≥N times
//!   - persona_domain: persona→domain associations

use crate::features::memory::db::{self, MinedPattern, PatternType};
use anyhow::{bail, Result};
use rusqlite::Connection;
use std::path::PathBuf;

// ─────────────────────────────────────────────────────────────────────────────
// Types for raw query results
// ─────────────────────────────────────────────────────────────────────────────

/// A tool+file frequency result from mining.
#[derive(Debug)]
pub struct ToolSequence {
    pub tool: String,
    pub file_path: String,
    pub frequency: i64,
}

/// A file co-edit pair result from mining.
#[derive(Debug)]
pub struct FileCluster {
    pub file_a: String,
    pub file_b: String,
    pub frequency: i64,
}

/// An error→fix sequence result from mining.
#[derive(Debug)]
pub struct ErrorFix {
    pub error_type: String,
    pub fix_summary: String,
    pub frequency: i64,
}

/// A persona→domain association from mining.
#[derive(Debug)]
pub struct PersonaDomain {
    pub persona: String,
    pub domain: String,
    pub frequency: i64,
}

// ─────────────────────────────────────────────────────────────────────────────
// SQL pattern detection queries
// ─────────────────────────────────────────────────────────────────────────────

/// Detect tool+file frequency patterns.
///
/// AC2: same tool on same file across ≥min_freq sessions.
pub fn detect_tool_sequences(conn: &Connection, min_freq: i64) -> Result<Vec<ToolSequence>> {
    // In production: query trace_events table.
    // In tests with in-memory: we seed mock data and query the same schema.
    // The query groups by tool+file_path and counts distinct sessions.
    let mut stmt = conn.prepare(
        r#"
        SELECT tool, file_path, COUNT(DISTINCT session_id) as freq
        FROM trace_events
        WHERE file_path IS NOT NULL AND file_path != ''
          AND event_type = 'tool'
        GROUP BY tool, file_path
        HAVING freq >= ?
        ORDER BY freq DESC
        "#,
    )?;

    let rows = stmt.query_map([min_freq], |row| {
        Ok(ToolSequence {
            tool: row.get(0)?,
            file_path: row.get(1)?,
            frequency: row.get(2)?,
        })
    })?;

    let mut sequences = Vec::new();
    for row in rows {
        sequences.push(row?);
    }
    Ok(sequences)
}

/// Detect file co-edit clusters.
///
/// AC3: files edited together in the same session ≥min_freq times.
pub fn detect_file_clusters(conn: &Connection, min_freq: i64) -> Result<Vec<FileCluster>> {
    // Self-join trace_events on session_id, pairing files that appear together.
    // Use a.file_path < b.file_path to avoid duplicates and self-pairs.
    let mut stmt = conn.prepare(
        r#"
        SELECT a.file_path as file_a, b.file_path as file_b, COUNT(DISTINCT a.session_id) as freq
        FROM trace_events a, trace_events b
        WHERE a.session_id = b.session_id
          AND a.file_path < b.file_path
          AND a.event_type = 'edit'
          AND b.event_type = 'edit'
          AND a.file_path IS NOT NULL AND a.file_path != ''
          AND b.file_path IS NOT NULL AND b.file_path != ''
        GROUP BY a.file_path, b.file_path
        HAVING freq >= ?
        ORDER BY freq DESC
        "#,
    )?;

    let rows = stmt.query_map([min_freq], |row| {
        Ok(FileCluster {
            file_a: row.get(0)?,
            file_b: row.get(1)?,
            frequency: row.get(2)?,
        })
    })?;

    let mut clusters = Vec::new();
    for row in rows {
        clusters.push(row?);
    }
    Ok(clusters)
}

/// Detect error→fix sequences.
///
/// AC4: error event followed by successful edit in same session, repeated ≥2 times.
pub fn detect_error_fixes(conn: &Connection, min_freq: i64) -> Result<Vec<ErrorFix>> {
    // Window functions rank error and edit events within each session.
    // An error→fix pair is valid when error's rank < edit's rank (error came first).
    let mut stmt = conn.prepare(
        r#"
        SELECT e.error_type, f.edit_summary, COUNT(*) as freq
        FROM (
            SELECT session_id, error_type,
                   ROW_NUMBER() OVER(PARTITION BY session_id ORDER BY seq) as r
            FROM trace_events
            WHERE event_type = 'error' AND error_type IS NOT NULL
        ) e,
        (
            SELECT session_id, edit_summary,
                   ROW_NUMBER() OVER(PARTITION BY session_id ORDER BY seq) as r
            FROM trace_events
            WHERE event_type = 'edit' AND edit_summary IS NOT NULL
        ) f
        WHERE e.session_id = f.session_id AND e.r < f.r
        GROUP BY e.error_type, f.edit_summary
        HAVING freq >= ?
        ORDER BY freq DESC
        "#,
    )?;

    let rows = stmt.query_map([min_freq], |row| {
        Ok(ErrorFix {
            error_type: row.get(0)?,
            fix_summary: row.get(1)?,
            frequency: row.get(2)?,
        })
    })?;

    let mut fixes = Vec::new();
    for row in rows {
        fixes.push(row?);
    }
    Ok(fixes)
}

/// Detect persona→domain associations.
///
/// Persona used in sessions that touch the same domain repeatedly.
pub fn detect_persona_domains(conn: &Connection, min_freq: i64) -> Result<Vec<PersonaDomain>> {
    let mut stmt = conn.prepare(
        r#"
        SELECT persona, domain, COUNT(DISTINCT session_id) as freq
        FROM trace_events
        WHERE persona IS NOT NULL AND persona != ''
          AND domain IS NOT NULL AND domain != ''
        GROUP BY persona, domain
        HAVING freq >= ?
        ORDER BY freq DESC
        "#,
    )?;

    let rows = stmt.query_map([min_freq], |row| {
        Ok(PersonaDomain {
            persona: row.get(0)?,
            domain: row.get(1)?,
            frequency: row.get(2)?,
        })
    })?;

    let mut associations = Vec::new();
    for row in rows {
        associations.push(row?);
    }
    Ok(associations)
}

// ─────────────────────────────────────────────────────────────────────────────
// Run full mine
// ─────────────────────────────────────────────────────────────────────────────

pub struct MineStats {
    pub tool_sequences: usize,
    pub file_clusters: usize,
    pub error_fixes: usize,
    pub persona_domains: usize,
}

impl Default for MineStats {
    fn default() -> Self {
        Self {
            tool_sequences: 0,
            file_clusters: 0,
            error_fixes: 0,
            persona_domains: 0,
        }
    }
}

/// Run all pattern detection queries and persist to DB.
/// Returns MineStats counts and list of generated MinedPattern records.
pub fn run_mine(
    conn: &Connection,
    min_freq: i64,
    dry_run: bool,
) -> Result<(MineStats, Vec<MinedPattern>)> {
    use chrono::Utc;

    let now = Utc::now().to_rfc3339();
    let mut stats = MineStats::default();
    let mut all_patterns = Vec::new();

    // Tool sequences
    let sequences = detect_tool_sequences(conn, min_freq)?;
    for seq in &sequences {
        let desc = format!("Tool '{}' applied to '{}' across {} sessions", seq.tool, seq.file_path, seq.frequency);
        if !dry_run {
            let id = db::upsert_pattern(conn, PatternType::ToolSequence, &desc, seq.frequency, &now)?;
            if let Ok(Some(p)) = db::get_pattern(conn, id) {
                all_patterns.push(p);
            }
        } else {
            all_patterns.push(MinedPattern {
                id: None,
                pattern_type: PatternType::ToolSequence,
                description: desc,
                frequency: seq.frequency,
                first_seen: now.clone(),
                last_seen: now.clone(),
                skill_path: None,
                suppressed: false,
            });
        }
        stats.tool_sequences += 1;
    }

    // File clusters
    let clusters = detect_file_clusters(conn, min_freq)?;
    for cl in &clusters {
        let desc = format!("Files '{}' and '{}' edited together in {} sessions", cl.file_a, cl.file_b, cl.frequency);
        if !dry_run {
            let id = db::upsert_pattern(conn, PatternType::FileCluster, &desc, cl.frequency, &now)?;
            if let Ok(Some(p)) = db::get_pattern(conn, id) {
                all_patterns.push(p);
            }
        } else {
            all_patterns.push(MinedPattern {
                id: None,
                pattern_type: PatternType::FileCluster,
                description: desc,
                frequency: cl.frequency,
                first_seen: now.clone(),
                last_seen: now.clone(),
                skill_path: None,
                suppressed: false,
            });
        }
        stats.file_clusters += 1;
    }

    // Error fixes
    let fixes = detect_error_fixes(conn, min_freq)?;
    for fx in &fixes {
        let desc = format!("Error '{}' followed by fix '{}' in {} sessions", fx.error_type, fx.fix_summary, fx.frequency);
        if !dry_run {
            let id = db::upsert_pattern(conn, PatternType::ErrorFix, &desc, fx.frequency, &now)?;
            if let Ok(Some(p)) = db::get_pattern(conn, id) {
                all_patterns.push(p);
            }
        } else {
            all_patterns.push(MinedPattern {
                id: None,
                pattern_type: PatternType::ErrorFix,
                description: desc,
                frequency: fx.frequency,
                first_seen: now.clone(),
                last_seen: now.clone(),
                skill_path: None,
                suppressed: false,
            });
        }
        stats.error_fixes += 1;
    }

    // Persona domains
    let associations = detect_persona_domains(conn, min_freq)?;
    for assoc in &associations {
        let desc = format!("Persona '{}' associated with domain '{}' in {} sessions", assoc.persona, assoc.domain, assoc.frequency);
        if !dry_run {
            let id = db::upsert_pattern(conn, PatternType::PersonaDomain, &desc, assoc.frequency, &now)?;
            if let Ok(Some(p)) = db::get_pattern(conn, id) {
                all_patterns.push(p);
            }
        } else {
            all_patterns.push(MinedPattern {
                id: None,
                pattern_type: PatternType::PersonaDomain,
                description: desc,
                frequency: assoc.frequency,
                first_seen: now.clone(),
                last_seen: now.clone(),
                skill_path: None,
                suppressed: false,
            });
        }
        stats.persona_domains += 1;
    }

    Ok((stats, all_patterns))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn seed_trace_events(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE trace_events (
                id          INTEGER PRIMARY KEY,
                session_id  TEXT NOT NULL,
                seq         INTEGER NOT NULL,
                event_type  TEXT NOT NULL,  -- 'tool' | 'edit' | 'error'
                tool        TEXT,
                file_path   TEXT,
                error_type  TEXT,
                edit_summary TEXT,
                persona     TEXT,
                domain      TEXT,
                timestamp   TEXT NOT NULL
            );
            INSERT INTO trace_events (session_id, seq, event_type, tool, file_path, persona, domain, timestamp) VALUES
                ('s1', 1, 'tool', 'Read', '/a.rs', 'coder', 'rust', '2026-05-28T00:00:00Z'),
                ('s1', 2, 'tool', 'Edit', '/a.rs', 'coder', 'rust', '2026-05-28T00:01:00Z'),
                ('s2', 1, 'tool', 'Read', '/a.rs', 'coder', 'rust', '2026-05-28T01:00:00Z'),
                ('s3', 1, 'tool', 'Read', '/a.rs', 'coder', 'rust', '2026-05-28T02:00:00Z'),
                -- s1 and s2 co-edit a.rs + b.rs
                ('s1', 3, 'edit', NULL, '/a.rs', NULL, NULL, '2026-05-28T00:02:00Z'),
                ('s1', 4, 'edit', NULL, '/b.rs', NULL, NULL, '2026-05-28T00:03:00Z'),
                ('s2', 2, 'edit', NULL, '/a.rs', NULL, NULL, '2026-05-28T01:01:00Z'),
                ('s2', 3, 'edit', NULL, '/b.rs', NULL, NULL, '2026-05-28T01:02:00Z'),
                -- error→fix in s1 and s2
                ('s1', 5, 'error', NULL, NULL, 'clippy_warn', 'unused_var', NULL, '2026-05-28T00:04:00Z'),
                ('s1', 6, 'edit', NULL, NULL, 'add_underscore', NULL, NULL, '2026-05-28T00:05:00Z'),
                ('s2', 4, 'error', NULL, NULL, 'clippy_warn', 'unused_var', NULL, '2026-05-28T01:03:00Z'),
                ('s2', 5, 'edit', NULL, NULL, 'add_underscore', NULL, NULL, '2026-05-28T01:04:00Z'),
                -- persona→domain
                ('s1', 7, 'tool', 'grep', '/src', 'sentinel', 'security', '2026-05-28T00:06:00Z'),
                ('s2', 6, 'tool', 'grep', '/src', 'sentinel', 'security', '2026-05-28T01:05:00Z'),
                ('s3', 2, 'tool', 'grep', '/src', 'sentinel', 'security', '2026-05-28T02:01:00Z');
            "#,
        )
        .unwrap();
    }

    #[test]
    fn test_detect_tool_sequences() {
        let conn = Connection::open_in_memory().unwrap();
        seed_trace_events(&conn);

        let result = detect_tool_sequences(&conn, 3).unwrap();
        assert!(!result.is_empty());
        let seq = &result[0];
        assert_eq!(seq.tool, "Read");
        assert_eq!(seq.file_path, "/a.rs");
        assert_eq!(seq.frequency, 3); // s1, s2, s3 all used Read on /a.rs
    }

    #[test]
    fn test_detect_file_clusters() {
        let conn = Connection::open_in_memory().unwrap();
        seed_trace_events(&conn);

        let result = detect_file_clusters(&conn, 2).unwrap();
        assert!(!result.is_empty());
        // a.rs + b.rs co-edited in s1 and s2
        let cluster = result.iter().find(|c| c.file_a == "/a.rs" && c.file_b == "/b.rs").unwrap();
        assert_eq!(cluster.frequency, 2);
    }

    #[test]
    fn test_detect_error_fixes() {
        let conn = Connection::open_in_memory().unwrap();
        seed_trace_events(&conn);

        let result = detect_error_fixes(&conn, 2).unwrap();
        assert!(!result.is_empty());
        let fix = &result[0];
        assert_eq!(fix.error_type, "clippy_warn");
        assert_eq!(fix.fix_summary, "add_underscore");
        assert_eq!(fix.frequency, 2);
    }

    #[test]
    fn test_detect_persona_domains() {
        let conn = Connection::open_in_memory().unwrap();
        seed_trace_events(&conn);

        let result = detect_persona_domains(&conn, 3).unwrap();
        assert!(!result.is_empty());
        let assoc = &result[0];
        assert_eq!(assoc.persona, "sentinel");
        assert_eq!(assoc.domain, "security");
        assert_eq!(assoc.frequency, 3);
    }

    #[test]
    fn test_run_mine_dry_run() {
        let conn = Connection::open_in_memory().unwrap();
        seed_trace_events(&conn);

        let (stats, patterns) = run_mine(&conn, 2, true).unwrap();
        assert_eq!(stats.tool_sequences, 2); // Read+/a.rs (3), Edit+/a.rs (2)
        assert_eq!(stats.file_clusters, 1);  // a.rs+b.rs (2)
        assert_eq!(stats.error_fixes, 1);   // clippy_warn→add_underscore (2)
        assert_eq!(stats.persona_domains, 1); // sentinel→security (3)
        assert_eq!(patterns.len(), 5);

        // In dry-run mode, patterns have no DB ids
        for p in &patterns {
            assert!(p.id.is_none());
        }
    }
}