use anyhow::Result;

use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

const DB_PATH: &str = "/.cache/ccb/graph.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

fn db() -> Result<Connection> {
    let path = std::env::var("HOME").unwrap_or("/".to_string()) + DB_PATH;
    let conn = Connection::open(&path)?;
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS files (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL UNIQUE,
            lang TEXT NOT NULL,
            indexed INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS symbols (
            id INTEGER PRIMARY KEY,
            file_id INTEGER NOT NULL REFERENCES files(id) ON DELETE CASCADE,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            line INTEGER NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_symbols_name ON symbols(name);
        CREATE INDEX IF NOT EXISTS idx_symbols_file ON symbols(file_id);
        ",
    )?;
    Ok(conn)
}

fn detect_lang(path: &Path) -> Option<&'static str> {
    match path.extension()?.to_str()? {
        "rs" => Some("rust"),
        "py" => Some("python"),
        "ts" | "tsx" => Some("typescript"),
        "js" | "jsx" => Some("javascript"),
        _ => None,
    }
}

fn extract_rust_symbols(path: &Path) -> Result<Vec<(String, String, i64)>> {
    let source = std::fs::read_to_string(path)?;
    let mut symbols = Vec::new();

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(name) = trimmed.split_whitespace().nth(1) {
            if name.starts_with('a') && name.starts_with(|c: char| c.is_alphabetic()) {
                if let Some(name) = name.strip_prefix("async fn ") {
                    symbols.push((name.to_string(), "fn".to_string(), 0));
                }
            } else if name.starts_with(|c: char| c.is_alphabetic()) {
                let kind = if line.contains("fn ") { "fn" } else { "struct" };
                symbols.push((name.to_string(), kind.to_string(), 0));
            }
        }
    }
    Ok(symbols)
}

fn extract_python_symbols(path: &Path) -> Result<Vec<(String, String, i64)>> {
    let source = std::fs::read_to_string(path)?;
    let mut symbols = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(colon_pos) = trimmed.find(':') {
            let before_colon = trimmed[..colon_pos].trim();
            if before_colon.starts_with("def ") {
                let name = before_colon[4..].trim();
                if !name.is_empty() {
                    symbols.push((name.to_string(), "def".to_string(), idx as i64));
                }
            } else if before_colon.starts_with("class ") {
                let name = before_colon[6..].trim();
                if !name.is_empty() {
                    symbols.push((name.to_string(), "class".to_string(), idx as i64));
                }
            }
        }
    }
    Ok(symbols)
}

fn extract_tsjs_symbols(path: &Path) -> Result<Vec<(String, String, i64)>> {
    let source = std::fs::read_to_string(path)?;
    let mut symbols = Vec::new();

    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with("//") {
            continue;
        }

        if let Some(name) = trimmed.split_whitespace().nth(1) {
            if name.starts_with(|c: char| c.is_alphabetic()) {
                symbols.push((name.to_string(), "fn".to_string(), idx as i64));
            }
        }
    }
    Ok(symbols)
}

fn extract_symbols(path: &Path, lang: &str) -> Result<Vec<(String, String, i64)>> {
    match lang {
        "rust" => extract_rust_symbols(path),
        "python" => extract_python_symbols(path),
        "typescript" | "javascript" => extract_tsjs_symbols(path),
        _ => anyhow::bail!("Unsupported language: {}", lang),
    }
}

pub fn index(path: &Path) -> Result<()> {
    let start = SystemTime::now();
    let db = db()?;

    let walker = walkdir::WalkDir::new(path)
        .follow_links(true)
        .into_iter()
        .filter_map(|e| e.ok());

    let mut indexed_count = 0;

    for entry in walker {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Some(lang) = detect_lang(path) {
            if let Ok(symbols) = extract_symbols(path, lang) {
                db.execute(
                    "INSERT OR REPLACE INTO files (path, lang, indexed) VALUES (?, ?, ?)",
                    params![
                        path.display().to_string(),
                        lang,
                        SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .unwrap()
                            .as_secs()
                    ],
                )?;
                let file_id = db.last_insert_rowid();

                for (name, kind, line) in symbols {
                    db.execute(
                        "INSERT INTO symbols (file_id, name, kind, line) VALUES (?, ?, ?, ?)",
                        params![file_id, name, kind, line],
                    )
                    .ok();
                }

                indexed_count += 1;
            }
        }
    }

    let duration = start.elapsed();
    let total_symbols: i64 = db.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;
    let total_files: i64 = db.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;

    println!(
        "Indexed {} files, {} symbols in {:?}",
        indexed_count, total_symbols, duration
    );
    println!(
        "Total in database: {} files, {} symbols",
        total_files, total_symbols
    );
    Ok(())
}

pub fn search(pattern: &str, format: OutputFormat) -> Result<()> {
    let conn = db()?;
    let like_pattern = format!("%{}%", pattern);

    let mut stmt = conn.prepare(
        "SELECT name, kind, line, path FROM symbols s JOIN files f ON s.file_id = f.id WHERE s.name LIKE ? ORDER BY s.name LIMIT 50"
    )?;

    let rows = stmt.query(params![&like_pattern])?;
    let results: Vec<(String, String, i64, String)> = rows
        .mapped(|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
        .collect::<rusqlite::Result<Vec<_>>>()?;

    match format {
        OutputFormat::Human => {
            print_human_search(&results, pattern);
        }
        OutputFormat::Json => {
            print_json_search(&results, pattern);
        }
    }
    Ok(())
}

pub fn show(file: &Path, format: OutputFormat) -> Result<()> {
    let conn = db()?;
    let exists = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE path = ?",
        params![file.display().to_string()],
        |row| row.get(0),
    ) == Ok(0);

    if !exists {
        println!("File not indexed: {}", file.display());
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT name, kind, line FROM symbols WHERE file_id = (SELECT id FROM files WHERE path = ?) ORDER BY line"
    )?;

    let rows = stmt.query(params![file.display().to_string()])?;
    let symbols: Vec<(String, String, i64)> = rows
        .mapped(|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .collect::<rusqlite::Result<Vec<_>>>()?;

    match format {
        OutputFormat::Human => {
            print_human_show(&symbols, file.display().to_string());
        }
        OutputFormat::Json => {
            print_json_show(&symbols, file.display().to_string());
        }
    }
    Ok(())
}

fn print_human_search(results: &[(String, String, i64, String)], pattern: &str) {
    if results.is_empty() {
        println!("No results found for '{}'", pattern);
        return;
    }
    println!("Found {} results for '{}':", results.len(), pattern);
    for (name, kind, line, path) in results {
        println!("  [{}] {}: {} (line {}, {})", kind, name, path, line, name);
    }
}

fn print_human_show(symbols: &[(String, String, i64)], file: String) {
    println!("Symbols in {}:", file);
    for (name, kind, line) in symbols {
        println!("  [{}] {} (line {})", kind, name, line);
    }
}

fn print_json_search(results: &[(String, String, i64, String)], pattern: &str) {
    let output = serde_json::json!({
        "pattern": pattern,
        "results": results.iter().map(|(name, kind, line, path)| {
            serde_json::json!({
                "name": name,
                "kind": kind,
                "file": path,
                "line": line
            })
        }).collect::<Vec<_>>()
    });
    println!("{}", output);
}

fn print_json_show(symbols: &[(String, String, i64)], file: String) {
    let output = serde_json::json!({
        "file": file,
        "symbols": symbols.iter().map(|(name, kind, line)| {
            serde_json::json!({
                "name": name,
                "kind": kind,
                "line": line
            })
        }).collect::<Vec<_>>()
    });
    println!("{}", output);
}

pub fn stats(format: OutputFormat) -> Result<()> {
    let conn = db()?;

    let total_files: i64 = conn.query_row("SELECT COUNT(*) FROM files", [], |row| row.get(0))?;
    let total_symbols: i64 =
        conn.query_row("SELECT COUNT(*) FROM symbols", [], |row| row.get(0))?;

    let mut stmt = conn.prepare("SELECT lang, COUNT(*) FROM files GROUP BY lang")?;
    let rows = stmt.query([])?;
    let by_language: HashMap<String, i64> = rows
        .mapped(|row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .collect();

    let last_indexed: Option<i64> =
        conn.query_row("SELECT MAX(indexed) FROM files", [], |row| row.get(0))?;

    let db_path = std::env::var("HOME").unwrap_or("/".to_string()) + DB_PATH;

    match format {
        OutputFormat::Human => {
            println!("Graph Statistics:");
            println!("  Total files: {}", total_files);
            println!("  Total symbols: {}", total_symbols);
            println!("  Database: {}", db_path);
            println!("  Last indexed: {:?}", last_indexed);
            println!("\nBy language:");
            for (lang, count) in &by_language {
                println!("  {}: {}", lang, count);
            }
        }
        OutputFormat::Json => {
            let last_rfc3339 = last_indexed
                .and_then(|ts| {
                    (SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(ts as u64))
                        .duration_since(UNIX_EPOCH)
                        .ok()
                })
                .and_then(|d| chrono::DateTime::from_timestamp(d.as_secs() as i64, 0))
                .map(|dt| dt.to_rfc3339());

            let output = serde_json::json!({
                "total_files": total_files,
                "total_symbols": total_symbols,
                "by_language": by_language,
                "db_path": db_path,
                "last_indexed": last_rfc3339
            });
            println!("{}", output);
        }
    }
    Ok(())
}
