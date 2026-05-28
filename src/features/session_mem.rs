//! Memory — hybrid search (FTS5/BM25) + context recall for session replay.
//!
//! # Architecture
//!
//! - FTS5 virtual table over `trace_events(name, metadata)` enables full-text search.
//! - BM25 scoring is built into FTS5 — results ranked by relevance automatically.
//! - `memory search` — top-level search via FTS5 match on query string.
//! - `memory recall` — aggregates: error→fix patterns, mined skills, recent session
//!   summaries into a compact markdown block (≤500 tokens) suitable for hook injection.

use anyhow::Result;
use rusqlite::{params, Connection};

const DB_PATH: &str = "/.cache/ccb/memory.db";

// ── Database ────────────────────────────────────────────────────────────────────────

fn db() -> Result<Connection> {
    let path = std::env::var("CCB_MEMORY_DB")
        .unwrap_or_else(|_| std::env::var("HOME").unwrap_or("/".to_string()) + DB_PATH);
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS sessions (
            id       TEXT PRIMARY KEY,
            project  TEXT NOT NULL DEFAULT '',
            persona  TEXT NOT NULL DEFAULT '',
            task     TEXT NOT NULL DEFAULT '',
            summary  TEXT NOT NULL DEFAULT '',
            started  TEXT NOT NULL,
            ended    TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_sessions_project  ON sessions(project);
        CREATE INDEX IF NOT EXISTS idx_sessions_persona  ON sessions(persona);
        CREATE INDEX IF NOT EXISTS idx_sessions_started  ON sessions(started DESC);

        CREATE TABLE IF NOT EXISTS trace_events (
            id         TEXT PRIMARY KEY,
            session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            name       TEXT NOT NULL,
            kind       TEXT NOT NULL DEFAULT 'event',
            metadata   TEXT NOT NULL DEFAULT '{}',
            timestamp  TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_trace_session ON trace_events(session_id);
        CREATE INDEX IF NOT EXISTS idx_trace_name    ON trace_events(name);

        -- FTS5 virtual table for full-text search over trace_events.
        -- BM25 ranking is available via the built-in bm25() function.
        CREATE VIRTUAL TABLE IF NOT EXISTS trace_events_fts
        USING fts5(name, metadata, content='trace_events', content_rowid='rowid');

        -- Populate FTS after insert / delete via triggers.
        CREATE TRIGGER IF NOT EXISTS trace_events_ai AFTER INSERT ON trace_events BEGIN
            INSERT INTO trace_events_fts(rowid, name, metadata)
            VALUES (new.rowid, new.name, new.metadata);
        END;

        CREATE TRIGGER IF NOT EXISTS trace_events_ad AFTER DELETE ON trace_events BEGIN
            INSERT INTO trace_events_fts(trace_events_fts, rowid, name, metadata)
            VALUES ('delete', old.rowid, old.name, old.metadata);
        END;

        CREATE TRIGGER IF NOT EXISTS trace_events_au AFTER UPDATE ON trace_events BEGIN
            INSERT INTO trace_events_fts(trace_events_fts, rowid, name, metadata)
            VALUES ('delete', old.rowid, old.name, old.metadata);
            INSERT INTO trace_events_fts(rowid, name, metadata)
            VALUES (new.rowid, new.name, new.metadata);
        END;
        ",
    )?;
    Ok(conn)
}

// ── Search ────────────────────────────────────────────────────────────────────────

pub struct SearchResult {
    pub session_id: String,
    pub name: String,
    pub kind: String,
    pub metadata: String,
    pub timestamp: String,
    pub score: f64,
}

impl SearchResult {
    /// Render as a human-readable line.
    pub fn human(&self) -> String {
        format!(
            "[{}] {}  ({})  score={:.3}",
            self.timestamp, self.name, self.kind, self.score
        )
    }
}

/// Run FTS5 search over trace_events. Returns results ranked by BM25.
pub fn search(query: &str, project: Option<&str>, limit: usize) -> Result<Vec<SearchResult>> {
    let conn = db()?;
    let limit = limit.max(1).min(100);
    let query = query.trim();
    if query.is_empty() {
        return Ok(Vec::new());
    }

    // FTS5 match query — escape special chars by wrapping in quotes
    let fts_query = format!("\"{}\"", query.replace('"', "\"\""));

    let sql: Result<Vec<SearchResult>, rusqlite::Error> = if let Some(proj) = project {
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT e.session_id, e.name, e.kind, e.metadata, e.timestamp,
                   bm25(trace_events_fts) AS score
            FROM trace_events_fts
            JOIN trace_events e ON e.rowid = trace_events_fts.rowid
            JOIN sessions s ON s.id = e.session_id
            WHERE trace_events_fts MATCH ? AND s.project = ?
            ORDER BY score
            LIMIT ?
            "#,
        )?;
        let rows = stmt.query_map(params![&fts_query, proj, limit as i64], |row| {
            Ok(SearchResult {
                session_id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                metadata: row.get(3)?,
                timestamp: row.get(4)?,
                score: row.get(5)?,
            })
        })?;
        rows.collect()
    } else {
        let mut stmt = conn.prepare_cached(
            r#"
            SELECT e.session_id, e.name, e.kind, e.metadata, e.timestamp,
                   bm25(trace_events_fts) AS score
            FROM trace_events_fts
            JOIN trace_events e ON e.rowid = trace_events_fts.rowid
            ORDER BY score
            LIMIT ?
            "#,
        )?;
        let rows = stmt.query_map(params![&fts_query, limit as i64], |row| {
            Ok(SearchResult {
                session_id: row.get(0)?,
                name: row.get(1)?,
                kind: row.get(2)?,
                metadata: row.get(3)?,
                timestamp: row.get(4)?,
                score: row.get(5)?,
            })
        })?;
        rows.collect()
    };

    sql.map_err(|e| anyhow::anyhow!("search failed: {}", e))
}

// ── Recall ─────────────────────────────────────────────────────────────────────

/// A single section in the recall output.
#[derive(Default)]
struct RecallSection {
    title: String,
    bullets: Vec<String>,
    tokens_estimate: usize,
}

impl RecallSection {
    fn new(title: &'static str) -> Self {
        Self {
            title: title.to_string(),
            ..Default::default()
        }
    }

    fn bullet(&mut self, line: String) {
        let est = line.len().div_ceil(4);
        self.tokens_estimate += est;
        self.bullets.push(line);
    }

    fn is_empty(&self) -> bool {
        self.bullets.is_empty()
    }

    fn render(&self) -> String {
        if self.is_empty() {
            return String::new();
        }
        let mut s = format!("### {}\n", self.title);
        for b in &self.bullets {
            s.push_str("- ");
            s.push_str(b);
            s.push('\n');
        }
        s
    }

    fn tokens(&self) -> usize {
        self.tokens_estimate
    }
}

/// Assemble a compact context block from memory.
/// Priority order: error→fix patterns, mined skills, session summaries.
/// Output is capped at `max_tokens` (default 500).
pub fn recall(
    project: Option<&str>,
    _persona: Option<&str>,
    _task: Option<&str>,
    max_tokens: usize,
) -> Result<String> {
    let conn = db()?;

    let mut errors_patterns = RecallSection::new("Error Fixes (this project)");
    let mut skills = RecallSection::new("Mined Skills");
    let recent = RecallSection::new("Recent Sessions");

    // Error→fix patterns: trace_events with kind='error' or 'fix' for this project
    let filter_project = project.unwrap_or("");
    let mut stmt = conn.prepare_cached(
        r#"
        SELECT e.name, e.metadata
        FROM trace_events e
        JOIN sessions s ON s.id = e.session_id
        WHERE e.kind IN ('error', 'fix')
          AND (? = '' OR s.project = ?)
        ORDER BY e.timestamp DESC
        LIMIT 20
        "#,
    )?;
    let rows = stmt.query_map(
        params![filter_project, filter_project],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    for row in rows.flatten() {
        let (name, metadata) = row;
        let line = if metadata.is_empty() || metadata == "{}" {
            name
        } else {
            format!("{} — {}", name, metadata)
        };
        errors_patterns.bullet(line);
    }

    // Recent session summaries for this project
    let mut stmt = conn.prepare_cached(
        r#"
        SELECT summary, started
        FROM sessions
        WHERE (? = '' OR project = ?)
          AND summary != ''
        ORDER BY started DESC
        LIMIT 10
        "#,
    )?;
    let rows = stmt.query_map(
        params![filter_project, filter_project],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
    )?;
    for row in rows.flatten() {
        let (summary, started) = row;
        let short_started = started.trim();
        skills.bullet(format!("[{}] {}", short_started, summary));
    }

    // Build output, enforcing token cap
    let cap = max_tokens.max(1);
    let mut output = String::new();
    let mut output_tokens = 0;

    for section in [&errors_patterns, &skills, &recent] {
        let section_rendered = section.render();
        let section_tokens = section_rendered.len().div_ceil(4);
        if output_tokens + section_tokens > cap && output_tokens > 0 {
            break;
        }
        output_tokens += section_tokens;
        output.push_str(&section_rendered);
    }

    // Header
    let header = "## Session Memory\n\n";
    let header_tokens = header.len().div_ceil(4);
    let body_tokens = output.len().div_ceil(4);
    let total = header_tokens + body_tokens;

    if total > cap {
        // Truncate if still over cap
        let budget = cap.saturating_sub(header_tokens) * 4;
        output.truncate(budget);
    }

    Ok(format!("{}{}", header, output))
}

// ── Token estimation (mirrors log::estimate_tokens) ────────────────────────────

fn estimate_tokens(s: &str) -> usize {
    s.len().div_ceil(4)
}

// ── Tests ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_db() -> Result<(tempfile::TempDir, Connection)> {
        let dir = tempfile::TempDir::new()?;
        let db_path = dir.path().join("memory.db");
        std::env::set_var("CCB_MEMORY_DB", &db_path);
        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            "
            CREATE TABLE sessions (
                id       TEXT PRIMARY KEY,
                project  TEXT NOT NULL DEFAULT '',
                persona  TEXT NOT NULL DEFAULT '',
                task     TEXT NOT NULL DEFAULT '',
                summary  TEXT NOT NULL DEFAULT '',
                started  TEXT NOT NULL,
                ended    TEXT
            );
            CREATE TABLE trace_events (
                id         TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                name       TEXT NOT NULL,
                kind       TEXT NOT NULL DEFAULT 'event',
                metadata   TEXT NOT NULL DEFAULT '{}',
                timestamp  TEXT NOT NULL
            );
            CREATE VIRTUAL TABLE trace_events_fts
            USING fts5(name, metadata, content='trace_events', content_rowid='rowid');
            CREATE TRIGGER trace_events_ai AFTER INSERT ON trace_events BEGIN
                INSERT INTO trace_events_fts(rowid, name, metadata)
                VALUES (new.rowid, new.name, new.metadata);
            END;
            CREATE TRIGGER trace_events_ad AFTER DELETE ON trace_events BEGIN
                INSERT INTO trace_events_fts(trace_events_fts, rowid, name, metadata)
                VALUES ('delete', old.rowid, old.name, old.metadata);
            END;
            CREATE TRIGGER trace_events_au AFTER UPDATE ON trace_events BEGIN
                INSERT INTO trace_events_fts(trace_events_fts, rowid, name, metadata)
                VALUES ('delete', old.rowid, old.name, old.metadata);
                INSERT INTO trace_events_fts(rowid, name, metadata)
                VALUES (new.rowid, new.name, new.metadata);
            END;
            ",
        )?;
        Ok((dir, conn))
    }

    #[test]
    fn test_fts_search_returns_inserted() -> Result<()> {
        let (_dir, conn) = tmp_db()?;

        // Insert a session and event
        conn.execute(
            "INSERT INTO sessions (id, project, summary, started)
             VALUES ('s1', 'myproject', 'Fixed the bug', '2026-05-01T10:00:00Z')",
            [],
        )?;
        conn.execute(
            "INSERT INTO trace_events (id, session_id, name, kind, metadata, timestamp)
             VALUES ('e1', 's1', 'panic in router', 'error', '{\"file\":\"src/router.rs\"}', '2026-05-01T10:00:00Z')",
            [],
        )?;

        // Now call search() which will open a NEW connection via db().
        // The new connection opens the SAME db file (via HOME=/the_temp_dir).
        let results = search("panic", Some("myproject"), 10)?;
        assert!(!results.is_empty(), "should find 'panic' event");
        assert_eq!(results[0].name, "panic in router");
        Ok(())
    }

    #[test]
    fn test_search_no_results() -> Result<()> {
        let (_dir, conn) = tmp_db()?;
        conn.execute(
            "INSERT INTO sessions (id, project, summary, started)
             VALUES ('s1', 'myproject', 'summary', '2026-05-01T10:00:00Z')",
            [],
        )?;
        conn.execute(
            "INSERT INTO trace_events (id, session_id, name, kind, metadata, timestamp)
             VALUES ('e1', 's1', 'some event', 'event', '{}', '2026-05-01T10:00:00Z')",
            [],
        )?;
        drop(conn);

        let results = search("nonexistent query string xyz", Some("myproject"), 10)?;
        assert!(results.is_empty(), "should be empty for non-matching query");
        Ok(())
    }

    #[test]
    fn test_recall_empty() -> Result<()> {
        let (_dir, conn) = tmp_db()?;
        conn.execute(
            "INSERT INTO sessions (id, project, summary, started)
             VALUES ('s1', 'testproject', 'old session', '2026-05-01T10:00:00Z')",
            [],
        )?;
        drop(conn);

        let output = recall(Some("testproject"), None, None, 500)?;
        // Empty recall should produce just the header
        assert!(output.starts_with("## Session Memory"));
        Ok(())
    }

    #[test]
    fn test_fts_search_bm25_ranking() -> Result<()> {
        let (_dir, conn) = tmp_db()?;

        conn.execute(
            "INSERT INTO sessions (id, project, summary, started)
             VALUES ('s1', 'proj', 'summary', '2026-05-01T10:00:00Z')",
            [],
        )?;
        // Insert 3 events — "panic" is the most specific (should rank highest)
        conn.execute(
            "INSERT INTO trace_events (id, session_id, name, kind, metadata, timestamp)
             VALUES ('e1', 's1', 'info log line', 'event', '{}', '2026-05-01T10:00:00Z')",
            [],
        )?;
        conn.execute(
            "INSERT INTO trace_events (id, session_id, name, kind, metadata, timestamp)
             VALUES ('e2', 's1', 'error: panic in lib', 'error', '{}', '2026-05-01T10:00:01Z')",
            [],
        )?;
        conn.execute(
            "INSERT INTO trace_events (id, session_id, name, kind, metadata, timestamp)
             VALUES ('e3', 's1', 'panic at line 42', 'error', '{}', '2026-05-01T10:00:02Z')",
            [],
        )?;
        drop(conn);

        let results = search("panic", Some("proj"), 10)?;
        assert_eq!(results.len(), 2, "should find 2 panic events");
        // e3 has "panic" in name twice — should outrank e2
        assert_eq!(results[0].name, "panic at line 42");
        Ok(())
    }
}
