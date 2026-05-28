//! Patterns — list and suppress mined patterns
//!
//! `ccb memory patterns [--type T]` — list all mined patterns
//! `ccb memory suppress <id>` — mark a pattern as suppressed

use crate::features::memory::db::{self, PatternType};
use anyhow::Result;
use rusqlite::Connection;

/// List patterns, optionally filtered by type.
/// Prints human-readable table to stdout.
pub fn list_patterns(conn: &Connection, type_filter: Option<PatternType>) -> Result<()> {
    let patterns = db::list_patterns(conn, type_filter)?;

    if patterns.is_empty() {
        println!("No patterns found.");
        return Ok(());
    }

    println!("╭─────────────────────────────────────────────────────────────────────────────╮");
    println!("│                        CCB — Mined Patterns                                  │");
    println!("├──────┬──────────────────┬──────────┬──────────────┬─────────────────────────────┤");
    println!("│ id   │ type            │ freq     │ last seen   │ description                  │");
    println!("├──────┼──────────────────┼──────────┼──────────────┼─────────────────────────────┤");

    for p in &patterns {
        let id = p.id.map(|i| i.to_string()).unwrap_or_else(|| "-".to_string());
        let type_str = p.pattern_type.to_string();
        let freq = p.frequency.to_string();
        let last = &p.last_seen[..10]; // YYYY-MM-DD
        let desc = if p.description.len() > 27 {
            format!("{}...", &p.description[..27])
        } else {
            p.description.clone()
        };
        println!(
            "│ {:>4} │ {:<16} │ {:>8} │ {:<12} │ {:<27} │",
            id, type_str, freq, last, desc
        );
    }

    println!("╰──────┴──────────────────┴──────────┴──────────────┴─────────────────────────────╯");
    println!("  {} pattern(s)", patterns.len());

    Ok(())
}

/// Suppress a pattern by id. Suppressed patterns won't be regenerated.
pub fn suppress(conn: &Connection, id: i64) -> Result<()> {
    db::suppress_pattern(conn, id)?;
    println!("Suppressed pattern {}", id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_empty() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE mined_patterns (
                id            INTEGER PRIMARY KEY,
                pattern_type  TEXT NOT NULL,
                description   TEXT NOT NULL,
                frequency     INTEGER NOT NULL DEFAULT 1,
                first_seen    TEXT NOT NULL,
                last_seen     TEXT NOT NULL,
                skill_path    TEXT,
                suppressed    INTEGER NOT NULL DEFAULT 0
            );
            "#,
        )
        .unwrap();

        list_patterns(&conn, None).unwrap();
        // No panic = empty state handled
    }
}