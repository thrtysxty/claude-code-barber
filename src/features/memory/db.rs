//! DB — mined_patterns table schema and CRUD
//!
//! Stores discovered patterns from trace mining.
//! Uses traces.db (configured by CCB-018) as the backing store.

use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const TRACES_DB: &str = "/.cache/ccb/traces.db";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PatternType {
    ToolSequence,
    FileCluster,
    ErrorFix,
    PersonaDomain,
}

impl std::fmt::Display for PatternType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PatternType::ToolSequence => "tool_sequence",
            PatternType::FileCluster => "file_cluster",
            PatternType::ErrorFix => "error_fix",
            PatternType::PersonaDomain => "persona_domain",
        };
        write!(f, "{}", s)
    }
}

impl PatternType {
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "tool_sequence" => Some(PatternType::ToolSequence),
            "file_cluster" => Some(PatternType::FileCluster),
            "error_fix" => Some(PatternType::ErrorFix),
            "persona_domain" => Some(PatternType::PersonaDomain),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MinedPattern {
    pub id: Option<i64>,
    pub pattern_type: PatternType,
    pub description: String,
    pub frequency: i64,
    pub first_seen: String,
    pub last_seen: String,
    pub skill_path: Option<String>,
    pub suppressed: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// DB init
// ─────────────────────────────────────────────────────────────────────────────

fn db_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".cache")
        .join("ccb")
        .join("traces.db")
}

pub fn init() -> Result<Connection> {
    let conn = Connection::open(db_path())?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS mined_patterns (
            id            INTEGER PRIMARY KEY,
            pattern_type  TEXT NOT NULL,
            description   TEXT NOT NULL,
            frequency     INTEGER NOT NULL DEFAULT 1,
            first_seen    TEXT NOT NULL,
            last_seen     TEXT NOT NULL,
            skill_path    TEXT,
            suppressed    INTEGER NOT NULL DEFAULT 0
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_mp_type_desc
            ON mined_patterns(pattern_type, description);
        "#,
    )?;
    Ok(conn)
}

// ─────────────────────────────────────────────────────────────────────────────
// CRUD
// ─────────────────────────────────────────────────────────────────────────────

/// List patterns, optionally filtered by type.
pub fn list_patterns(conn: &Connection, type_filter: Option<PatternType>) -> Result<Vec<MinedPattern>> {
    let mut patterns = Vec::new();
    let query = match type_filter {
        Some(t) => "SELECT * FROM mined_patterns WHERE pattern_type = ?1 AND suppressed = 0 ORDER BY frequency DESC",
        None => "SELECT * FROM mined_patterns WHERE suppressed = 0 ORDER BY frequency DESC",
    };
    let mut stmt = conn.prepare(query)?;
    let rows = if let Some(t) = type_filter {
        stmt.query(params![t.to_string()])?
    } else {
        stmt.query([])?
    };
    let mut rows = rows;
    while let Some(row) = rows.next()? {
        patterns.push(row_to_pattern(row)?);
    }
    Ok(patterns)
}

/// Insert or update a pattern. Returns the pattern id.
pub fn upsert_pattern(
    conn: &Connection,
    pattern_type: PatternType,
    description: &str,
    frequency: i64,
    now: &str,
) -> Result<i64> {
    conn.execute(
        r#"
        INSERT INTO mined_patterns (pattern_type, description, frequency, first_seen, last_seen, suppressed)
        VALUES (?1, ?2, ?3, ?4, ?4, 0)
        ON CONFLICT(pattern_type, description) DO UPDATE SET
            frequency = frequency + ?3,
            last_seen = ?4
        "#,
        params![pattern_type.to_string(), description, frequency, now],
    )?;
    let mut stmt = conn.prepare("SELECT id FROM mined_patterns WHERE pattern_type = ?1 AND description = ?2")?;
    let id: i64 = stmt.query_row(params![pattern_type.to_string(), description], |r| r.get(0))?;
    Ok(id)
}

/// Update the skill_path for a pattern.
pub fn set_skill_path(conn: &Connection, id: i64, skill_path: &str) -> Result<()> {
    conn.execute(
        "UPDATE mined_patterns SET skill_path = ?1 WHERE id = ?2",
        params![skill_path, id],
    )?;
    Ok(())
}

/// Mark a pattern as suppressed.
pub fn suppress_pattern(conn: &Connection, id: i64) -> Result<()> {
    conn.execute(
        "UPDATE mined_patterns SET suppressed = 1 WHERE id = ?1",
        params![id],
    )?;
    Ok(())
}

/// Get a single pattern by id.
pub fn get_pattern(conn: &Connection, id: i64) -> Result<Option<MinedPattern>> {
    let mut stmt = conn.prepare("SELECT * FROM mined_patterns WHERE id = ?1")?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_pattern(row)?))
    } else {
        Ok(None)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn row_to_pattern(row: &rusqlite::Row) -> Result<MinedPattern> {
    let pt_str: String = row.get("pattern_type")?;
    let pattern_type = PatternType::from_str(&pt_str).unwrap_or(PatternType::ToolSequence);
    Ok(MinedPattern {
        id: row.get("id").ok(),
        pattern_type,
        description: row.get("description")?,
        frequency: row.get("frequency")?,
        first_seen: row.get("first_seen")?,
        last_seen: row.get("last_seen")?,
        skill_path: row.get("skill_path").ok(),
        suppressed: row.get::<_, i32>("suppressed")? != 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_type_display() {
        assert_eq!(PatternType::ToolSequence.to_string(), "tool_sequence");
        assert_eq!(PatternType::FileCluster.to_string(), "file_cluster");
        assert_eq!(PatternType::ErrorFix.to_string(), "error_fix");
        assert_eq!(PatternType::PersonaDomain.to_string(), "persona_domain");
    }

    #[test]
    fn pattern_type_from_str() {
        assert_eq!(PatternType::from_str("tool_sequence"), Some(PatternType::ToolSequence));
        assert_eq!(PatternType::from_str("file_cluster"), Some(PatternType::FileCluster));
        assert_eq!(PatternType::from_str("error_fix"), Some(PatternType::ErrorFix));
        assert_eq!(PatternType::from_str("persona_domain"), Some(PatternType::PersonaDomain));
        assert_eq!(PatternType::from_str("bogus"), None);
    }

    #[test]
    fn round_trip_in_memory() {
        let conn = Connection::open_in_memory().unwrap();
        init().unwrap(); // uses traces.db path — table creation is isolated

        // We can still test the schema by using the in-memory for this test
        // since init() creates the table on the connection we pass it
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS mined_patterns (
                id            INTEGER PRIMARY KEY,
                pattern_type  TEXT NOT NULL,
                description   TEXT NOT NULL,
                frequency     INTEGER NOT NULL DEFAULT 1,
                first_seen    TEXT NOT NULL,
                last_seen     TEXT NOT NULL,
                skill_path    TEXT,
                suppressed    INTEGER NOT NULL DEFAULT 0
            );
            CREATE UNIQUE INDEX idx_mp_type_desc ON mined_patterns(pattern_type, description);
            "#,
        )
        .unwrap();

        // Test upsert
        let id = upsert_pattern(&conn, PatternType::ToolSequence, "Read then Write", 5, "2026-05-28T00:00:00Z").unwrap();
        assert!(id > 0);

        // Test list
        let patterns = list_patterns(&conn, None).unwrap();
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].frequency, 5);

        // Test update frequency (second upsert)
        upsert_pattern(&conn, PatternType::ToolSequence, "Read then Write", 3, "2026-05-28T12:00:00Z").unwrap();
        let patterns = list_patterns(&conn, None).unwrap();
        assert_eq!(patterns[0].frequency, 8); // 5 + 3

        // Test suppress
        suppress_pattern(&conn, id).unwrap();
        let patterns = list_patterns(&conn, None).unwrap();
        assert_eq!(patterns.len(), 0); // suppressed patterns filtered out

        // Test get_pattern
        let p = get_pattern(&conn, id).unwrap().unwrap();
        assert!(p.suppressed);

        // Cleanup
        drop(conn);
    }
}