use anyhow::Result;

use rusqlite::{params, Connection};
use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use tree_sitter::Parser;

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

fn walk_tree(node: tree_sitter::Node, source: &[u8], symbols: &mut Vec<(String, String, i64)>, lang: &str) {
    let kind = node.kind();
    let line = (node.start_position().row + 1) as i64;

    match lang {
        "rust" => match kind {
            "function_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "fn".to_string(), line));
                    }
                }
            }
            "struct_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "struct".to_string(), line));
                    }
                }
            }
            "enum_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "enum".to_string(), line));
                    }
                }
            }
            "trait_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "trait".to_string(), line));
                    }
                }
            }
            "impl_item" => {
                if let Some(n) = node.child_by_field_name("type") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "impl".to_string(), line));
                    }
                }
            }
            "const_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "const".to_string(), line));
                    }
                }
            }
            "static_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "static".to_string(), line));
                    }
                }
            }
            "mod_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "mod".to_string(), line));
                    }
                }
            }
            "type_item" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "type".to_string(), line));
                    }
                }
            }
            _ => {}
        },
        "python" => match kind {
            "function_definition" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "fn".to_string(), line));
                    }
                }
            }
            "class_definition" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "class".to_string(), line));
                    }
                }
            }
            _ => {}
        },
        "typescript" | "javascript" => match kind {
            "function_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "fn".to_string(), line));
                    }
                }
            }
            "class_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "class".to_string(), line));
                    }
                }
            }
            "interface_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "interface".to_string(), line));
                    }
                }
            }
            "type_alias_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "type".to_string(), line));
                    }
                }
            }
            "enum_declaration" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "enum".to_string(), line));
                    }
                }
            }
            "method_definition" => {
                if let Some(n) = node.child_by_field_name("name") {
                    if let Ok(name) = n.utf8_text(source) {
                        symbols.push((name.to_string(), "method".to_string(), line));
                    }
                }
            }
            "lexical_declaration" | "variable_declaration" => {
                for i in 0..node.named_child_count() {
                    if let Some(decl) = node.named_child(i) {
                        if decl.kind() == "variable_declarator" {
                            let has_fn = decl.child_by_field_name("value")
                                .map(|v| matches!(v.kind(), "arrow_function" | "function"))
                                .unwrap_or(false);
                            if has_fn {
                                if let Some(n) = decl.child_by_field_name("name") {
                                    if let Ok(name) = n.utf8_text(source) {
                                        symbols.push((name.to_string(), "fn".to_string(), line));
                                    }
                                }
                            }
                        }
                    }
                }
                return; // children already visited
            }
            _ => {}
        },
        _ => {}
    }

    let cursor = &mut node.walk();
    for child in node.children(cursor) {
        walk_tree(child, source, symbols, lang);
    }
}

fn extract_symbols_ts(path: &Path, lang: &str) -> Result<Vec<(String, String, i64)>> {
    let source = std::fs::read_to_string(path)?;
    let source_bytes = source.as_bytes();

    let mut parser = Parser::new();
    let ts_lang = match lang {
        "rust" => tree_sitter_rust::language(),
        "python" => tree_sitter_python::language(),
        "typescript" => {
            if path.extension().map(|e| e == "tsx").unwrap_or(false) {
                tree_sitter_typescript::language_tsx()
            } else {
                tree_sitter_typescript::language_typescript()
            }
        }
        "javascript" => tree_sitter_javascript::language(),
        other => anyhow::bail!("Unsupported language: {}", other),
    };
    parser.set_language(&ts_lang)?;

    let tree = parser.parse(&source, None)
        .ok_or_else(|| anyhow::anyhow!("Failed to parse {}", path.display()))?;

    let mut symbols = Vec::new();
    walk_tree(tree.root_node(), source_bytes, &mut symbols, lang);
    Ok(symbols)
}

fn extract_symbols(path: &Path, lang: &str) -> Result<Vec<(String, String, i64)>> {
    extract_symbols_ts(path, lang)
}

pub fn index(path: &Path) -> Result<()> {
    let start = SystemTime::now();
    let db = db()?;

    const SKIP_DIRS: &[&str] = &[
        "node_modules", "dist-electron", "dist", ".git", "target",
        "__pycache__", ".venv", "build", "coverage", "e2e/test-results",
    ];

    // Refuse to index temp directories
    let path_str = path.display().to_string();
    if path_str.starts_with("/var/folders") || path_str.starts_with("/tmp") || path_str.contains("/tmp/") {
        anyhow::bail!("Refusing to index temp directory: {}", path_str);
    }

    let walker = walkdir::WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_entry(|e| {
            if e.file_type().is_dir() {
                let name = e.file_name().to_string_lossy();
                return !SKIP_DIRS.iter().any(|skip| name == *skip);
            }
            true
        })
        .filter_map(|e| e.ok());

    let mut indexed_count = 0;

    for entry in walker {
        let path = entry.path();

        if !path.is_file() {
            continue;
        }

        if let Some(lang) = detect_lang(path) {
            if let Ok(symbols) = extract_symbols(path, lang) {
                let path_str = path.display().to_string();
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_secs();

                // Delete stale symbols before re-inserting
                db.execute(
                    "DELETE FROM symbols WHERE file_id IN (SELECT id FROM files WHERE path = ?)",
                    params![&path_str],
                )?;

                db.execute(
                    "INSERT OR REPLACE INTO files (path, lang, indexed) VALUES (?, ?, ?)",
                    params![&path_str, lang, now],
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
    // Use display string directly — do NOT canonicalize, as WalkDir
    // stores non-canonical paths (e.g. /var -> /private/var on macOS).
    let canonical_str = file.display().to_string();
    let exists: i64 = conn.query_row(
        "SELECT COUNT(*) FROM files WHERE path = ?",
        params![canonical_str],
        |row| row.get(0),
    )?;
    if exists == 0 {
        println!("File not indexed: {}", file.display());
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT name, kind, line FROM symbols WHERE file_id = (SELECT id FROM files WHERE path = ?) ORDER BY line"
    )?;

    let rows = stmt.query(params![canonical_str.to_string()])?;
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

pub fn symbols_in_file(file_path: &str) -> Result<Vec<(String, String, i64)>> {
    let conn = db()?;
    let mut stmt = conn.prepare(
        "SELECT name, kind, line FROM symbols WHERE file_id = (SELECT id FROM files WHERE path = ?) ORDER BY line"
    )?;
    let rows = stmt.query(params![file_path])?;
    let results: Vec<(String, String, i64)> = rows
        .mapped(|row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(results)
}
