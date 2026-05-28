//! Expert knowledge graph — Layer 3
//!
//! Stores personas, domains, and mitigation patterns in the shared
//! `~/.cache/ccb/graph.db`. Schema is migrated on first run via
//! `CREATE TABLE IF NOT EXISTS`.

use crate::cli::ExportFormat;
use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use std::io::Write;

const DB_PATH: &str = "/.cache/ccb/graph.db";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

// ---------------------------------------------------------------------------
// Database
// ---------------------------------------------------------------------------

/// Open the shared CCB database, creating the Layer 3 schema if absent.
fn db() -> Result<Connection> {
    let path = std::env::var("HOME").unwrap_or_else(|_| "/".to_string()) + DB_PATH;
    let conn =
        Connection::open(&path).with_context(|| format!("failed to open graph.db at {path}"))?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS personas (
            id          INTEGER PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS domains (
            id          INTEGER PRIMARY KEY,
            name        TEXT NOT NULL UNIQUE,
            category    TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS persona_domains (
            persona_id  INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
            domain_id   INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
            weight      REAL NOT NULL DEFAULT 1.0,
            PRIMARY KEY (persona_id, domain_id)
        );

        CREATE TABLE IF NOT EXISTS patterns (
            id          INTEGER PRIMARY KEY,
            domain_id   INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
            pattern_id  TEXT NOT NULL,
            name        TEXT NOT NULL,
            mitigations TEXT NOT NULL,
            UNIQUE(domain_id, pattern_id)
        );

        CREATE TABLE IF NOT EXISTS active_persona (
            id          INTEGER PRIMARY KEY CHECK (id = 1),
            persona_id  INTEGER REFERENCES personas(id)
        );

        CREATE INDEX IF NOT EXISTS idx_persona_domains ON persona_domains(persona_id);
        CREATE INDEX IF NOT EXISTS idx_patterns_domain  ON patterns(domain_id);
        "#,
    )?;

    Ok(conn)
}

// ---------------------------------------------------------------------------
// Dataset format
// ---------------------------------------------------------------------------

use serde::Serialize;

/// A single instruction-tuning pair in Alpaca format.
#[derive(Debug, Serialize)]
struct AlpacaPair {
    instruction: String,
    input: String,
    output: String,
}

/// A single instruction-tuning pair in ShareGPT format.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ShareGPTPair {
    conversations: Vec<ShareGPTMessage>,
}

#[derive(Debug, Serialize)]
struct ShareGPTMessage {
    from: String,
    value: String,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Export the active persona's patterns as instruction-tuning pairs.
/// `format` selects between Alpaca (default) and ShareGPT.
pub fn export(
    persona_name: &str,
    format: ExportFormat,
    output_path: &std::path::Path,
) -> Result<()> {
    let conn = db()?;

    // Resolve persona
    let persona_id: i64 = conn
        .query_row(
            "SELECT id FROM personas WHERE name = ?",
            params![persona_name],
            |row| row.get(0),
        )
        .with_context(|| format!("persona '{persona_name}' not found"))?;

    // Fetch domains and patterns
    let mut stmt = conn.prepare(
        "SELECT d.name, d.category, p.pattern_id, p.name, p.mitigations
         FROM patterns p
         JOIN domains d ON d.id = p.domain_id
         JOIN persona_domains pd ON pd.domain_id = d.id
         WHERE pd.persona_id = ?
         ORDER BY d.name, p.pattern_id",
    )?;

    let rows: Vec<(String, String, String, String, String)> = stmt
        .query_map(params![persona_id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if rows.is_empty() {
        anyhow::bail!("persona '{persona_name}' has no patterns to export");
    }

    let mut count = 0;

    let file = std::fs::File::create(output_path)
        .with_context(|| format!("creating output file {}", output_path.display()))?;
    let mut writer = std::io::BufWriter::new(file);

    // Group rows by domain for synthesis pairs
    let by_domain: std::collections::HashMap<String, Vec<(String, String, String)>> =
        rows.iter().fold(
            std::collections::HashMap::new(),
            |mut acc, (d, _, pid, pn, m)| {
                acc.entry(d.clone())
                    .or_default()
                    .push((pid.clone(), pn.clone(), m.clone()));
                acc
            },
        );

    match format {
        ExportFormat::Alpaca => {
            for (domain_name, patterns) in &by_domain {
                // One pair per pattern
                for (_, pattern_name, mitigations_json) in patterns {
                    let mitigations: Vec<String> =
                        serde_json::from_str(mitigations_json).unwrap_or_default();
                    let output = mitigations.join(". ");
                    if output.is_empty() {
                        continue;
                    }
                    let pair = AlpacaPair {
                        instruction: format!("What mitigations apply to {}?", pattern_name),
                        input: String::new(),
                        output,
                    };
                    serde_json::to_writer(&mut writer, &pair)?;
                    writer.write_all(b"\n")?;
                    count += 1;
                }

                // Cross-domain synthesis: all mitigations for the domain as one response
                if patterns.len() > 1 {
                    let all_mits: Vec<String> = patterns
                        .iter()
                        .filter_map(|(_, _, m)| {
                            let v: Vec<String> = serde_json::from_str(m).ok()?;
                            Some(v)
                        })
                        .flatten()
                        .collect();
                    if !all_mits.is_empty() {
                        let pair = AlpacaPair {
                            instruction: format!(
                                "What are the key mitigations for {} security risks?",
                                domain_name
                            ),
                            input: String::new(),
                            output: all_mits.join(". "),
                        };
                        serde_json::to_writer(&mut writer, &pair)?;
                        writer.write_all(b"\n")?;
                        count += 1;
                    }
                }
            }
        }
        ExportFormat::Sharegpt => {
            for (domain_name, patterns) in &by_domain {
                for (_, pattern_name, mitigations_json) in patterns {
                    let mitigations: Vec<String> =
                        serde_json::from_str(mitigations_json).unwrap_or_default();
                    let output = mitigations.join(". ");
                    if output.is_empty() {
                        continue;
                    }
                    let pair = ShareGPTPair {
                        conversations: vec![
                            ShareGPTMessage {
                                from: String::from("human"),
                                value: format!("What mitigations apply to {}?", pattern_name),
                            },
                            ShareGPTMessage {
                                from: String::from("gpt"),
                                value: output,
                            },
                        ],
                    };
                    serde_json::to_writer(&mut writer, &pair)?;
                    writer.write_all(b"\n")?;
                    count += 1;
                }

                // Cross-domain synthesis
                if patterns.len() > 1 {
                    let all_mits: Vec<String> = patterns
                        .iter()
                        .filter_map(|(_, _, m)| {
                            let v: Vec<String> = serde_json::from_str(m).ok()?;
                            Some(v)
                        })
                        .flatten()
                        .collect();
                    if !all_mits.is_empty() {
                        let pair = ShareGPTPair {
                            conversations: vec![
                                ShareGPTMessage {
                                    from: String::from("human"),
                                    value: format!(
                                        "What are the key mitigations for {} security risks?",
                                        domain_name
                                    ),
                                },
                                ShareGPTMessage {
                                    from: String::from("gpt"),
                                    value: all_mits.join(". "),
                                },
                            ],
                        };
                        serde_json::to_writer(&mut writer, &pair)?;
                        writer.write_all(b"\n")?;
                        count += 1;
                    }
                }
            }
        }
    }

    drop(writer);
    println!(
        "Exported {} training pairs to {}",
        count,
        output_path.display()
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Dataset format
// ---------------------------------------------------------------------------

/// Expected JSON shape for `ccb expert build --dataset <path>`.
#[derive(serde::Deserialize)]
struct PersonaDataset {
    persona: String,
    description: String,
    domains: Vec<DomainSpec>,
}

#[derive(serde::Deserialize)]
struct DomainSpec {
    name: String,
    category: String,
    patterns: Vec<PatternSpec>,
}

#[derive(serde::Deserialize)]
struct PatternSpec {
    id: String,
    name: String,
    mitigations: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Build or update a persona and all its domains / patterns from a dataset file.
pub fn build(_name: &str, dataset_path: &std::path::Path) -> Result<()> {
    let data = std::fs::read_to_string(dataset_path)
        .with_context(|| format!("reading dataset {path}", path = dataset_path.display()))?;
    let ds: PersonaDataset = serde_json::from_str(&data).with_context(|| {
        "parsing dataset JSON — expected {persona, description, domains[]} shape"
    })?;

    let conn = db()?;

    // Upsert persona
    conn.execute(
        "INSERT INTO personas (name, description) VALUES (?, ?)
         ON CONFLICT(name) DO UPDATE SET description = excluded.description",
        params![&ds.persona, &ds.description],
    )?;
    let persona_id: i64 = conn.query_row(
        "SELECT id FROM personas WHERE name = ?",
        params![&ds.persona],
        |row| row.get(0),
    )?;

    for domain in &ds.domains {
        // Upsert domain
        conn.execute(
            "INSERT INTO domains (name, category) VALUES (?, ?)
             ON CONFLICT(name) DO UPDATE SET category = excluded.category",
            params![&domain.name, &domain.category],
        )?;
        let domain_id: i64 = conn.query_row(
            "SELECT id FROM domains WHERE name = ?",
            params![&domain.name],
            |row| row.get(0),
        )?;

        // Upsert persona → domain edge
        conn.execute(
            "INSERT INTO persona_domains (persona_id, domain_id, weight) VALUES (?, ?, 1.0)
             ON CONFLICT(persona_id, domain_id) DO NOTHING",
            params![persona_id, domain_id],
        )?;

        for pattern in &domain.patterns {
            let mitigations_json =
                serde_json::to_string(&pattern.mitigations).context("serialising mitigations")?;
            conn.execute(
                "INSERT INTO patterns (domain_id, pattern_id, name, mitigations) VALUES (?, ?, ?, ?)
                 ON CONFLICT(domain_id, pattern_id) DO UPDATE SET
                   name = excluded.name, mitigations = excluded.mitigations",
                params![domain_id, &pattern.id, &pattern.name, &mitigations_json],
            )?;
        }
    }

    println!(
        "Built persona '{}' with {} domain(s)",
        ds.persona,
        ds.domains.len()
    );
    Ok(())
}

/// Set the named persona as active. Returns an error if not found.
pub fn activate(name: &str) -> Result<()> {
    let conn = db()?;
    let persona_id: i64 = conn
        .query_row(
            "SELECT id FROM personas WHERE name = ?",
            params![name],
            |row| row.get(0),
        )
        .with_context(|| format!("persona '{name}' not found"))?;

    conn.execute("DELETE FROM active_persona", [])?;
    conn.execute(
        "INSERT INTO active_persona (id, persona_id) VALUES (1, ?)",
        params![persona_id],
    )?;
    println!("Activated persona '{name}'");
    Ok(())
}

/// Clear the active persona.
pub fn deactivate() -> Result<()> {
    let conn = db()?;
    conn.execute("DELETE FROM active_persona", [])?;
    println!("Deactivated current persona");
    Ok(())
}

/// Print all personas with domain count and active status.
pub fn list() -> Result<()> {
    let conn = db()?;

    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.description,
                (SELECT COUNT(*) FROM persona_domains pd WHERE pd.persona_id = p.id) AS domain_count,
                EXISTS(SELECT 1 FROM active_persona ap WHERE ap.persona_id = p.id) AS is_active
         FROM personas p
         ORDER BY p.name"
    )?;

    let rows = stmt.query_map([], |row| {
        Ok(PersonaRow {
            id: row.get(0)?,
            name: row.get(1)?,
            description: row.get(2)?,
            domain_count: row.get(3)?,
            is_active: row.get(4)?,
        })
    })?;

    let personas: Vec<PersonaRow> = rows.collect::<rusqlite::Result<Vec<_>>>()?;

    if personas.is_empty() {
        println!("No personas registered. Run 'ccb expert build' first.");
        return Ok(());
    }

    println!("{:<20} {:>6}  STATUS", "PERSONA", "DOMAINS");
    println!("{}", "-".repeat(48));
    for p in &personas {
        let status = if p.is_active { "ACTIVE" } else { "" };
        println!("{:<20} {:>6}  {}", p.name, p.domain_count, status);
    }

    Ok(())
}

#[derive(Debug)]
#[allow(dead_code)]
struct PersonaRow {
    id: i64,
    name: String,
    description: String,
    domain_count: i64,
    is_active: bool,
}

/// Query the active persona and return structured data.
///
/// - `Json` format: prints a single valid JSON object to stdout.
/// - `Human` format: prints a readable table to stdout.
/// - No active persona: prints `{}` and exits successfully (never errors).
pub fn query_active(format: OutputFormat) -> Result<()> {
    let conn = db()?;

    let active: Option<i64> = conn
        .query_row("SELECT persona_id FROM active_persona", [], |row| {
            row.get(0)
        })
        .ok();

    let Some(persona_id) = active else {
        println!("{{}}");
        return Ok(());
    };

    let persona_name: String = conn.query_row(
        "SELECT name FROM personas WHERE id = ?",
        params![persona_id],
        |row| row.get(0),
    )?;

    // Fetch domains
    let mut stmt = conn.prepare(
        "SELECT d.name FROM domains d
         JOIN persona_domains pd ON pd.domain_id = d.id
         WHERE pd.persona_id = ?
         ORDER BY d.name",
    )?;
    let domains: Vec<String> = stmt
        .query_map(params![persona_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Fetch patterns
    let mut stmt = conn.prepare(
        "SELECT p.pattern_id, p.name, p.mitigations
         FROM patterns p
         JOIN domains d ON d.id = p.domain_id
         JOIN persona_domains pd ON pd.domain_id = d.id
         WHERE pd.persona_id = ?
         ORDER BY p.pattern_id",
    )?;
    let patterns: Vec<PatternRow> = stmt
        .query_map(params![persona_id], |row| {
            let mitigations_json: String = row.get(2)?;
            let mitigations: Vec<String> =
                serde_json::from_str(&mitigations_json).unwrap_or_default();
            Ok(PatternRow {
                id: row.get(0)?,
                name: row.get(1)?,
                mitigations,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    match format {
        OutputFormat::Human => {
            println!("Persona: {persona_name}");
            println!("Domains ({})", domains.len());
            for d in &domains {
                println!("  - {d}");
            }
            println!("Patterns ({})", patterns.len());
            for p in &patterns {
                println!("  [{}] {}", p.id, p.name);
                for m in &p.mitigations {
                    println!("      - {m}");
                }
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "persona": persona_name,
                "active_domains": domains,
                "patterns": patterns.iter().map(|p| {
                    serde_json::json!({
                        "id": p.id,
                        "name": p.name,
                        "mitigations": p.mitigations
                    })
                }).collect::<Vec<_>>()
            });
            println!("{output}");
        }
    }

    Ok(())
}

pub fn query_active_json() -> Result<Option<String>> {
    let conn = db()?;

    let active: Option<i64> = conn
        .query_row("SELECT persona_id FROM active_persona", [], |row| {
            row.get(0)
        })
        .ok();

    let Some(persona_id) = active else {
        return Ok(None);
    };

    let persona_name: String = conn.query_row(
        "SELECT name FROM personas WHERE id = ?",
        params![persona_id],
        |row| row.get(0),
    )?;

    let mut stmt = conn.prepare(
        "SELECT d.name FROM domains d
         JOIN persona_domains pd ON pd.domain_id = d.id
         WHERE pd.persona_id = ?
         ORDER BY d.name",
    )?;
    let domains: Vec<String> = stmt
        .query_map(params![persona_id], |row| row.get(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut stmt = conn.prepare(
        "SELECT p.pattern_id, p.name, p.mitigations
         FROM patterns p
         JOIN domains d ON d.id = p.domain_id
         JOIN persona_domains pd ON pd.domain_id = d.id
         WHERE pd.persona_id = ?
         ORDER BY p.pattern_id",
    )?;
    let patterns: Vec<serde_json::Value> = stmt
        .query_map(params![persona_id], |row| {
            let id: String = row.get(0)?;
            let name: String = row.get(1)?;
            let mits_json: String = row.get(2)?;
            let mits: Vec<String> = serde_json::from_str(&mits_json).unwrap_or_default();
            Ok(serde_json::json!({"id": id, "name": name, "mitigations": mits}))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let output = serde_json::json!({
        "persona": persona_name,
        "active_domains": domains,
        "patterns": patterns,
    });
    Ok(Some(output.to_string()))
}

/// Get the active persona name as a plain string (for factory/hook consumption).
pub fn query_active_name() -> String {
    let conn = match db() {
        Ok(c) => c,
        Err(_) => return String::new(),
    };

    let active: Option<i64> = conn
        .query_row("SELECT persona_id FROM active_persona", [], |row| {
            row.get(0)
        })
        .ok();

    let Some(persona_id) = active else {
        return String::new();
    };

    conn.query_row(
        "SELECT name FROM personas WHERE id = ?",
        params![persona_id],
        |row| row.get(0),
    )
    .unwrap_or_default()
}

struct PatternRow {
    id: String,
    name: String,
    mitigations: Vec<String>,
}

/// Traverse the knowledge graph starting from a task description.
///
/// Finds the active persona, walks connected domains and patterns,
/// and prints nodes whose edge weight meets or exceeds `threshold`.
pub fn walk(task: &str, threshold: f64) -> Result<()> {
    let conn = db()?;

    let active: Option<i64> = conn
        .query_row("SELECT persona_id FROM active_persona", [], |row| {
            row.get(0)
        })
        .ok();

    let Some(persona_id) = active else {
        println!("No active persona — run 'ccb expert activate <name>' first.");
        return Ok(());
    };

    let persona_name: String = conn.query_row(
        "SELECT name FROM personas WHERE id = ?",
        params![persona_id],
        |row| row.get(0),
    )?;

    println!("Walk: {task}");
    println!("Active persona: {persona_name}");
    println!("Threshold: {:.2}", threshold);
    println!();

    // Walk: persona → domains → patterns
    let mut stmt = conn.prepare(
        "SELECT d.name, d.category, pd.weight
         FROM persona_domains pd
         JOIN domains d ON d.id = pd.domain_id
         WHERE pd.persona_id = ? AND pd.weight >= ?
         ORDER BY pd.weight DESC",
    )?;

    let domains: Vec<(String, String, f64)> = stmt
        .query_map(params![persona_id, threshold], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    if domains.is_empty() {
        println!("No domains activated above threshold.");
        return Ok(());
    }

    println!("{:<25} {:>12} {:>8}", "DOMAIN", "CATEGORY", "WEIGHT");
    println!("{}", "-".repeat(50));
    for (name, category, weight) in &domains {
        println!("{:<25} {:>12} {:>8.2}", name, category, weight);
    }
    println!();

    println!("Patterns:");
    for (domain_name, _, _) in &domains {
        let domain_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM domains WHERE name = ?",
                params![domain_name],
                |row| row.get(0),
            )
            .ok();

        let Some(domain_id) = domain_id else { continue };

        let mut stmt = conn.prepare(
            "SELECT pattern_id, name FROM patterns WHERE domain_id = ? ORDER BY pattern_id",
        )?;
        let patterns: Vec<(String, String)> = stmt
            .query_map(params![domain_id], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()?;

        for (pid, pname) in patterns {
            println!("  [{pid}] {pname} ({domain_name})");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------

/// Ingest a YAML dataset file into the knowledge graph.
/// The file must contain a top-level "personas" list.
/// Each persona has {name, description, domains: [{name, category, patterns: [{id, name, mitigations}]}]}.
pub fn ingest(dataset_path: &std::path::Path) -> Result<()> {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    struct YamlPattern {
        id: String,
        name: String,
        mitigations: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    struct YamlDomain {
        name: String,
        category: String,
        patterns: Vec<YamlPattern>,
    }

    #[derive(Debug, Deserialize)]
    struct YamlPersona {
        name: String,
        description: String,
        domains: Vec<YamlDomain>,
    }

    #[derive(Debug, Deserialize)]
    struct YamlDataset {
        personas: Vec<YamlPersona>,
    }

    let data = std::fs::read_to_string(dataset_path)
        .with_context(|| format!("reading dataset {path}", path = dataset_path.display()))?;
    let ds: YamlDataset = serde_yaml::from_str(&data).with_context(|| "parsing YAML dataset")?;

    let conn = db()?;
    let mut total_domains = 0;
    let mut total_patterns = 0;

    for persona in &ds.personas {
        conn.execute(
            "INSERT INTO personas (name, description) VALUES (?, ?)
             ON CONFLICT(name) DO UPDATE SET description = excluded.description",
            params![&persona.name, &persona.description],
        )?;
        let persona_id: i64 = conn.query_row(
            "SELECT id FROM personas WHERE name = ?",
            params![&persona.name],
            |row| row.get(0),
        )?;

        for domain in &persona.domains {
            conn.execute(
                "INSERT INTO domains (name, category) VALUES (?, ?)
                 ON CONFLICT(name) DO UPDATE SET category = excluded.category",
                params![&domain.name, &domain.category],
            )?;
            let domain_id: i64 = conn.query_row(
                "SELECT id FROM domains WHERE name = ?",
                params![&domain.name],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO persona_domains (persona_id, domain_id, weight) VALUES (?, ?, 1.0)
                 ON CONFLICT(persona_id, domain_id) DO NOTHING",
                params![persona_id, domain_id],
            )?;

            total_domains += 1;
            for pattern in &domain.patterns {
                let mitigations_json = serde_json::to_string(&pattern.mitigations)
                    .context("serialising mitigations")?;
                conn.execute(
                    "INSERT INTO patterns (domain_id, pattern_id, name, mitigations) VALUES (?, ?, ?, ?)
                     ON CONFLICT(domain_id, pattern_id) DO UPDATE SET
                       name = excluded.name, mitigations = excluded.mitigations",
                    params![domain_id, &pattern.id, &pattern.name, &mitigations_json],
                )?;
                total_patterns += 1;
            }
        }
    }

    println!(
        "Ingested {} persona(s), {} domain(s), {} pattern(s)",
        ds.personas.len(),
        total_domains,
        total_patterns
    );
    Ok(())
}

// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    /// Helper: run a SQL batch on an in-memory connection.
    fn init_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS personas (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, description TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS domains (
                id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, category TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS persona_domains (
                persona_id INTEGER NOT NULL REFERENCES personas(id) ON DELETE CASCADE,
                domain_id  INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
                weight     REAL NOT NULL DEFAULT 1.0, PRIMARY KEY (persona_id, domain_id)
            );
            CREATE TABLE IF NOT EXISTS patterns (
                id INTEGER PRIMARY KEY, domain_id INTEGER NOT NULL REFERENCES domains(id) ON DELETE CASCADE,
                pattern_id TEXT NOT NULL, name TEXT NOT NULL, mitigations TEXT NOT NULL, UNIQUE(domain_id, pattern_id)
            );
            CREATE TABLE IF NOT EXISTS active_persona (
                id INTEGER PRIMARY KEY CHECK (id = 1), persona_id INTEGER REFERENCES personas(id)
            );
            CREATE INDEX IF NOT EXISTS idx_persona_domains ON persona_domains(persona_id);
            CREATE INDEX IF NOT EXISTS idx_patterns_domain  ON patterns(domain_id);
            "#,
        )
        .unwrap();
    }

    #[test]
    fn test_schema_creates_tables() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn);

        conn.execute_batch("SELECT 1 FROM personas LIMIT 1")
            .unwrap();
        conn.execute_batch("SELECT 1 FROM domains LIMIT 1").unwrap();
        conn.execute_batch("SELECT 1 FROM persona_domains LIMIT 1")
            .unwrap();
        conn.execute_batch("SELECT 1 FROM patterns LIMIT 1")
            .unwrap();
        conn.execute_batch("SELECT 1 FROM active_persona LIMIT 1")
            .unwrap();
    }

    #[test]
    fn test_activate_unknown_persona_returns_error() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn);

        // `activate` opens its own db() — we can't easily inject the in-memory conn.
        // Test the logic path instead: query for a non-existent name returns Err.
        let result = conn.query_row(
            "SELECT id FROM personas WHERE name = ?",
            params!["nonexistent"],
            |_row| Ok(()),
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_query_active_no_persona_returns_empty() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn);

        let active: Option<i64> = conn
            .query_row("SELECT persona_id FROM active_persona", [], |row| {
                row.get(0)
            })
            .ok();
        assert!(active.is_none());
    }

    #[test]
    fn test_persona_lifecycle() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn);

        // Insert a persona
        conn.execute(
            "INSERT INTO personas (name, description) VALUES (?, ?)",
            params!["test-persona", "A test persona"],
        )
        .unwrap();

        let id: i64 = conn
            .query_row(
                "SELECT id FROM personas WHERE name = ?",
                params!["test-persona"],
                |row| row.get(0),
            )
            .unwrap();

        // Activate
        conn.execute("DELETE FROM active_persona", []).unwrap();
        conn.execute(
            "INSERT INTO active_persona (id, persona_id) VALUES (1, ?)",
            params![id],
        )
        .unwrap();

        let active: Option<i64> = conn
            .query_row("SELECT persona_id FROM active_persona", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(active, Some(id));

        // Deactivate
        conn.execute("DELETE FROM active_persona", []).unwrap();
        let active: Option<i64> = conn
            .query_row("SELECT persona_id FROM active_persona", [], |row| {
                row.get(0)
            })
            .ok();
        assert!(active.is_none());
    }

    #[test]
    fn test_pattern_upsert() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn);

        conn.execute(
            "INSERT INTO personas (name, description) VALUES ('sec', 'Security')",
            [],
        )
        .unwrap();
        let _pid: i64 = conn
            .query_row("SELECT id FROM personas WHERE name = 'sec'", [], |row| {
                row.get(0)
            })
            .unwrap();

        conn.execute(
            "INSERT INTO domains (name, category) VALUES ('path_traversal', 'security')",
            [],
        )
        .unwrap();
        let did: i64 = conn
            .query_row(
                "SELECT id FROM domains WHERE name = 'path_traversal'",
                [],
                |row| row.get(0),
            )
            .unwrap();

        conn.execute(
            "INSERT INTO patterns (domain_id, pattern_id, name, mitigations) VALUES (?, ?, ?, ?)",
            params![did, "CWE-22", "Path Traversal", r#"["validate input"]"#],
        )
        .unwrap();

        // Upsert same pattern (different mitigations) — should replace
        conn.execute(
            "INSERT INTO patterns (domain_id, pattern_id, name, mitigations) VALUES (?, ?, ?, ?)
             ON CONFLICT(domain_id, pattern_id) DO UPDATE SET mitigations = excluded.mitigations",
            params![
                did,
                "CWE-22",
                "Path Traversal",
                r#"["validate input","resolve then check root"]"#
            ],
        )
        .unwrap();

        let mitigations: String = conn
            .query_row(
                "SELECT mitigations FROM patterns WHERE pattern_id = 'CWE-22'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(mitigations.contains("resolve then check root"));
    }

    #[test]
    fn test_walk_threshold_filters_domains() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn);

        conn.execute(
            "INSERT INTO personas (name, description) VALUES ('test', 'T')",
            [],
        )
        .unwrap();
        let pid: i64 = conn
            .query_row("SELECT id FROM personas WHERE name = 'test'", [], |row| {
                row.get(0)
            })
            .unwrap();

        conn.execute(
            "INSERT INTO domains (name, category) VALUES ('high', 'cat'), ('low', 'cat')",
            [],
        )
        .unwrap();
        let hid: i64 = conn
            .query_row("SELECT id FROM domains WHERE name = 'high'", [], |row| {
                row.get(0)
            })
            .unwrap();
        let lid: i64 = conn
            .query_row("SELECT id FROM domains WHERE name = 'low'", [], |row| {
                row.get(0)
            })
            .unwrap();

        conn.execute(
            "INSERT INTO persona_domains (persona_id, domain_id, weight) VALUES (?, ?, ?), (?, ?, ?)",
            params![pid, hid, 0.9, pid, lid, 0.1],
        )
        .unwrap();

        // Threshold 0.5 should include 'high' only
        let mut stmt = conn
            .prepare(
                "SELECT d.name FROM persona_domains pd JOIN domains d ON d.id = pd.domain_id
                 WHERE pd.persona_id = ? AND pd.weight >= ? ORDER BY pd.weight DESC",
            )
            .unwrap();
        let rows: Vec<String> = stmt
            .query_map(params![pid, 0.5], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert_eq!(rows, vec!["high"]);
    }
}

// ---------------------------------------------------------------------------
// Active context query (used by trim/fade to annotate log events)
// ---------------------------------------------------------------------------

/// Returns (persona_name, domains_hit) if an expert persona is active, None otherwise.
/// Requires the expert feature to be compiled in.
#[cfg(feature = "expert")]
pub fn active_context() -> Option<(String, Vec<String>)> {
    let conn = db().ok()?;

    let persona_id: Option<i64> = conn
        .query_row("SELECT persona_id FROM active_persona", [], |row| {
            row.get(0)
        })
        .ok();

    let persona_id = persona_id?;

    let persona_name: String = conn
        .query_row(
            "SELECT name FROM personas WHERE id = ?",
            params![persona_id],
            |row| row.get(0),
        )
        .ok()?;

    let mut stmt = conn
        .prepare(
            "SELECT d.name FROM domains d
             JOIN persona_domains pd ON pd.domain_id = d.id
             WHERE pd.persona_id = ?",
        )
        .ok()?;

    let domains: Vec<String> = stmt
        .query_map(params![persona_id], |row| row.get(0))
        .ok()?
        .filter_map(|r| r.ok())
        .collect();

    Some((persona_name, domains))
}

#[cfg(not(feature = "expert"))]
pub fn active_context() -> Option<(String, Vec<String>)> {
    None
}
