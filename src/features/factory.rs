//! Factory — deterministic story loop state machine
//!
//! Manages story lifecycles through planning and implementation loops.
//! Each story tracks: state, assigned expert, ACs, history.
//! State transitions are gated by expert activation matching required role.

use crate::features::expert;
use anyhow::{bail, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DB_PATH: &str = "/.cache/ccb/factory.db";

// ─────────────────────────────────────────────────────────────────────────────
// Types
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopType {
    Planning,
    Implementation,
}

impl std::fmt::Display for LoopType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoopType::Planning => write!(f, "planning"),
            LoopType::Implementation => write!(f, "implementation"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateTransition {
    pub from: String,
    pub to: String,
    pub trigger: String,
    pub expert: String,
    pub timestamp: String,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcceptanceCriterion {
    pub id: String,
    pub statement: String,
    pub status: String, // pending | met | failed
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Story {
    pub id: String,
    pub title: String,
    pub description: String,
    pub loop_type: LoopType,
    pub state: String,
    pub assigned_expert: Option<String>,
    pub acs: Vec<AcceptanceCriterion>,
    pub history: Vec<StateTransition>,
    pub created_at: String,
    pub updated_at: String,
}

// ─────────────────────────────────────────────────────────────────────────────
// State machine definition
// ─────────────────────────────────────────────────────────────────────────────

pub struct LoopDef {
    pub initial: &'static str,
    pub final_states: &'static [&'static str],
    pub states: &'static [(&'static str, &'static str, &'static str)], // (name, required_expert, description)
    pub transitions: &'static [(&'static str, &'static str, &'static str, &'static str)], // (from, to, trigger, expert)
}

pub static PLANNING_LOOP: LoopDef = LoopDef {
    initial: "backlog",
    final_states: &["approved"],
    states: &[
        ("backlog", "planner", "Idea captured, awaiting research"),
        ("research", "researcher", "Exploring codebase and context"),
        (
            "interview",
            "interviewer",
            "Eliciting and refining requirements",
        ),
        (
            "sentinel_review",
            "sentinel",
            "Security and risk assessment",
        ),
        (
            "architect_review",
            "architect",
            "Infra and system design review",
        ),
        ("approved", "planner", "Story ready for implementation"),
    ],
    transitions: &[
        ("backlog", "research", "advance", "planner"),
        ("research", "interview", "advance", "researcher"),
        ("interview", "research", "kickback", "sentinel"),
        ("interview", "sentinel_review", "advance", "interviewer"),
        ("sentinel_review", "interview", "kickback", "sentinel"),
        ("sentinel_review", "architect_review", "advance", "sentinel"),
        ("architect_review", "interview", "kickback", "architect"),
        ("architect_review", "approved", "approve", "architect"),
    ],
};

pub static IMPLEMENTATION_LOOP: LoopDef = LoopDef {
    initial: "queued",
    final_states: &["done"],
    states: &[
        ("queued", "architect", "Story queued for implementation"),
        ("coding", "coder_backend", "Backend implementation"),
        (
            "test_verification",
            "test_verifier",
            "Test analysis and verification",
        ),
        (
            "validation",
            "implementation_validator",
            "Final validation and gain check",
        ),
        ("done", "architect", "Story complete"),
    ],
    transitions: &[
        ("queued", "coding", "start", "architect"),
        ("coding", "test_verification", "advance", "architect"),
        ("test_verification", "coding", "kickback", "test_verifier"),
        (
            "test_verification",
            "validation",
            "advance",
            "test_verifier",
        ),
        (
            "validation",
            "coding",
            "escalate",
            "implementation_validator",
        ),
        ("validation", "done", "approve", "architect"),
    ],
};

// ─────────────────────────────────────────────────────────────────────────────
// Database
// ─────────────────────────────────────────────────────────────────────────────

fn db() -> Result<Connection> {
    let path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .to_str()
        .unwrap()
        .to_string()
        + DB_PATH;
    let conn =
        Connection::open(&path).map_err(|e| anyhow::anyhow!("failed to open factory.db: {}", e))?;
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS stories (
            id          TEXT PRIMARY KEY,
            title       TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            loop_type   TEXT NOT NULL,
            state       TEXT NOT NULL,
            assigned_expert TEXT,
            acs         TEXT NOT NULL DEFAULT '[]',
            history     TEXT NOT NULL DEFAULT '[]',
            created_at  TEXT NOT NULL,
            updated_at  TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS factory_log (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            story_id    TEXT NOT NULL REFERENCES stories(id),
            timestamp   TEXT NOT NULL,
            event       TEXT NOT NULL,
            from_state  TEXT,
            to_state    TEXT,
            trigger     TEXT,
            expert      TEXT,
            note        TEXT
        );
        "#,
    )?;
    Ok(conn)
}

// ─────────────────────────────────────────────────────────────────────────────
// Story CRUD
// ─────────────────────────────────────────────────────────────────────────────

pub fn create_story(title: &str, description: &str, loop_type: LoopType) -> Result<Story> {
    let id = format!("{:x}", md5_hash(title));
    let now = chrono::Utc::now().to_rfc3339();
    let state = match loop_type {
        LoopType::Planning => PLANNING_LOOP.initial,
        LoopType::Implementation => IMPLEMENTATION_LOOP.initial,
    };

    let story = Story {
        id: id.clone(),
        title: title.to_string(),
        description: description.to_string(),
        loop_type,
        state: state.to_string(),
        assigned_expert: None,
        acs: vec![],
        history: vec![],
        created_at: now.clone(),
        updated_at: now,
    };

    let conn = db()?;
    conn.execute(
        "INSERT INTO stories (id, title, description, loop_type, state, assigned_expert, acs, history, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            story.id,
            story.title,
            story.description,
            story.loop_type.to_string(),
            story.state,
            story.assigned_expert,
            serde_json::to_string(&story.acs)?,
            serde_json::to_string(&story.history)?,
            story.created_at,
            story.updated_at,
        ],
    )?;

    log_event(
        &conn,
        &id,
        "created",
        None,
        Some(&state),
        Some("create"),
        None,
        None,
    )?;
    Ok(story)
}

pub fn get_story(id: &str) -> Result<Option<Story>> {
    let conn = db()?;
    let mut stmt = conn.prepare("SELECT * FROM stories WHERE id = ?1")?;
    let mut rows = stmt.query(params![id])?;
    if let Some(row) = rows.next()? {
        Ok(Some(row_to_story(row)?))
    } else {
        Ok(None)
    }
}

pub fn list_stories(loop_type: Option<LoopType>) -> Result<Vec<Story>> {
    let conn = db()?;
    let query = match loop_type {
        Some(lt) => format!(
            "SELECT * FROM stories WHERE loop_type = '{}' ORDER BY created_at DESC",
            lt
        ),
        None => "SELECT * FROM stories ORDER BY created_at DESC".to_string(),
    };
    let mut stmt = conn.prepare(&query)?;
    let mut stories = vec![];
    let mut rows = stmt.query([])?;
    while let Some(row) = rows.next()? {
        stories.push(row_to_story(row)?);
    }
    Ok(stories)
}

pub fn story_status(id: &str) -> Result<Story> {
    match get_story(id)? {
        Some(s) => Ok(s),
        None => bail!("story '{}' not found", id),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// State machine transitions
// ─────────────────────────────────────────────────────────────────────────────

pub fn advance_story(id: &str, note: Option<&str>) -> Result<Story> {
    let story = get_story(id)?.ok_or_else(|| anyhow::anyhow!("story '{}' not found", id))?;

    let loop_def = match story.loop_type {
        LoopType::Planning => &PLANNING_LOOP,
        LoopType::Implementation => &IMPLEMENTATION_LOOP,
    };

    let current_expert = expert::query_active_name();

    let (next_state, required_expert) = find_next_state(loop_def, &story.state, "advance")
        .ok_or_else(|| anyhow::anyhow!("no advance transition from state '{}'", story.state))?;

    // Verify active expert matches required
    if !current_expert.contains(required_expert) && !required_expert.is_empty() {
        bail!(
            "wrong expert: '{}' required, '{}' active (run 'ccb expert activate {}' first)",
            required_expert,
            current_expert,
            required_expert
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let transition = StateTransition {
        from: story.state.clone(),
        to: next_state.to_string(),
        trigger: "advance".to_string(),
        expert: current_expert.clone(),
        timestamp: now.clone(),
        note: note.map(|s| s.to_string()),
    };

    let conn = db()?;
    update_story_state(
        &conn,
        id,
        next_state,
        current_expert.clone(),
        Some(&transition),
        &now,
    )?;
    log_event(
        &conn,
        id,
        "advance",
        Some(&story.state),
        Some(next_state),
        Some("advance"),
        Some(&current_expert),
        note,
    )?;

    get_story(id)?.ok_or_else(|| anyhow::anyhow!("story disappeared"))
}

pub fn kickback_story(id: &str, note: Option<&str>) -> Result<Story> {
    let story = get_story(id)?.ok_or_else(|| anyhow::anyhow!("story '{}' not found", id))?;

    let loop_def = match story.loop_type {
        LoopType::Planning => &PLANNING_LOOP,
        LoopType::Implementation => &IMPLEMENTATION_LOOP,
    };

    let current_expert = expert::query_active_name();

    let (next_state, required_expert) = find_next_state(loop_def, &story.state, "kickback")
        .ok_or_else(|| anyhow::anyhow!("no kickback transition from state '{}'", story.state))?;

    if !current_expert.contains(required_expert) && !required_expert.is_empty() {
        bail!(
            "wrong expert: '{}' required, '{}' active (run 'ccb expert activate {}' first)",
            required_expert,
            current_expert,
            required_expert
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let transition = StateTransition {
        from: story.state.clone(),
        to: next_state.to_string(),
        trigger: "kickback".to_string(),
        expert: current_expert.clone(),
        timestamp: now.clone(),
        note: note.map(|s| s.to_string()),
    };

    let conn = db()?;
    update_story_state(
        &conn,
        id,
        next_state,
        current_expert.clone(),
        Some(&transition),
        &now,
    )?;
    log_event(
        &conn,
        id,
        "kickback",
        Some(&story.state),
        Some(next_state),
        Some("kickback"),
        Some(&current_expert),
        note,
    )?;

    get_story(id)?.ok_or_else(|| anyhow::anyhow!("story disappeared"))
}

pub fn escalate_story(id: &str, target: &str, note: Option<&str>) -> Result<Story> {
    let story = get_story(id)?.ok_or_else(|| anyhow::anyhow!("story '{}' not found", id))?;

    let loop_def = match story.loop_type {
        LoopType::Planning => &PLANNING_LOOP,
        LoopType::Implementation => &IMPLEMENTATION_LOOP,
    };

    let current_expert = expert::query_active_name();

    let (next_state, required_expert) = find_next_state(loop_def, &story.state, "escalate")
        .ok_or_else(|| anyhow::anyhow!("no escalate transition from state '{}'", story.state))?;

    if !current_expert.contains(required_expert) && !required_expert.is_empty() {
        bail!(
            "wrong expert: '{}' required, '{}' active (run 'ccb expert activate {}' first)",
            required_expert,
            current_expert,
            required_expert
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let transition = StateTransition {
        from: story.state.clone(),
        to: next_state.to_string(),
        trigger: format!("escalate:{}", target),
        expert: current_expert.clone(),
        timestamp: now.clone(),
        note: note.map(|s| s.to_string()),
    };

    let conn = db()?;
    update_story_state(
        &conn,
        id,
        next_state,
        current_expert.clone(),
        Some(&transition),
        &now,
    )?;
    log_event(
        &conn,
        id,
        "escalate",
        Some(&story.state),
        Some(next_state),
        Some("escalate"),
        Some(&current_expert),
        note,
    )?;

    get_story(id)?.ok_or_else(|| anyhow::anyhow!("story disappeared"))
}

pub fn approve_story(id: &str, note: Option<&str>) -> Result<Story> {
    let story = get_story(id)?.ok_or_else(|| anyhow::anyhow!("story '{}' not found", id))?;

    let loop_def = match story.loop_type {
        LoopType::Planning => &PLANNING_LOOP,
        LoopType::Implementation => &IMPLEMENTATION_LOOP,
    };

    let current_expert = expert::query_active_name();

    let (next_state, required_expert) = find_next_state(loop_def, &story.state, "approve")
        .ok_or_else(|| anyhow::anyhow!("no approve transition from state '{}'", story.state))?;

    if !current_expert.contains(required_expert) && !required_expert.is_empty() {
        bail!(
            "wrong expert: '{}' required, '{}' active (run 'ccb expert activate {}' first)",
            required_expert,
            current_expert,
            required_expert
        );
    }

    let now = chrono::Utc::now().to_rfc3339();
    let transition = StateTransition {
        from: story.state.clone(),
        to: next_state.to_string(),
        trigger: "approve".to_string(),
        expert: current_expert.clone(),
        timestamp: now.clone(),
        note: note.map(|s| s.to_string()),
    };

    let conn = db()?;
    update_story_state(
        &conn,
        id,
        next_state,
        current_expert.clone(),
        Some(&transition),
        &now,
    )?;
    log_event(
        &conn,
        id,
        "approve",
        Some(&story.state),
        Some(next_state),
        Some("approve"),
        Some(&current_expert),
        note,
    )?;

    get_story(id)?.ok_or_else(|| anyhow::anyhow!("story disappeared"))
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn find_next_state(
    loop_def: &LoopDef,
    from: &str,
    trigger: &str,
) -> Option<(&'static str, &'static str)> {
    for (f, t, tri, exp) in loop_def.transitions {
        if *f == from && *tri == trigger {
            return Some((*t, exp));
        }
    }
    None
}

fn update_story_state(
    conn: &Connection,
    id: &str,
    next_state: &str,
    assigned_expert: String,
    transition: Option<&StateTransition>,
    now: &str,
) -> Result<()> {
    let story = get_story(id)?.ok_or_else(|| anyhow::anyhow!("story not found"))?;
    let mut history = story.history;
    if let Some(t) = transition {
        history.push(t.clone());
    }

    conn.execute(
        "UPDATE stories SET state = ?1, assigned_expert = ?2, history = ?3, updated_at = ?4 WHERE id = ?5",
        params![next_state, assigned_expert, serde_json::to_string(&history)?, now, id],
    )?;
    Ok(())
}

fn log_event(
    conn: &Connection,
    story_id: &str,
    event: &str,
    from_state: Option<&str>,
    to_state: Option<&str>,
    trigger: Option<&str>,
    expert: Option<&str>,
    note: Option<&str>,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO factory_log (story_id, timestamp, event, from_state, to_state, trigger, expert, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![story_id, now, event, from_state, to_state, trigger, expert, note],
    )?;
    Ok(())
}

fn row_to_story(row: &rusqlite::Row) -> rusqlite::Result<Story> {
    Ok(Story {
        id: row.get("id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        loop_type: if row.get::<_, String>("loop_type")? == "implementation" {
            LoopType::Implementation
        } else {
            LoopType::Planning
        },
        state: row.get("state")?,
        assigned_expert: row.get("assigned_expert")?,
        acs: serde_json::from_str(&row.get::<_, String>("acs")?).unwrap_or_default(),
        history: serde_json::from_str(&row.get::<_, String>("history")?).unwrap_or_default(),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// md5 hash helper (simple, no external dep needed)
// ─────────────────────────────────────────────────────────────────────────────

fn md5_hash(input: &str) -> u32 {
    let mut hash: u32 = 0;
    for byte in input.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as u32);
    }
    hash
}

// ─────────────────────────────────────────────────────────────────────────────
// Display formatters
// ─────────────────────────────────────────────────────────────────────────────

pub fn format_story(s: &Story) -> String {
    let acs_summary = format!("{} ACs", s.acs.len());
    let loop_indicator = match s.loop_type {
        LoopType::Planning => "[P]",
        LoopType::Implementation => "[I]",
    };
    let assigned = s.assigned_expert.as_deref().unwrap_or("-");
    format!(
        "{} {:8} {:15} {:30} | {} | history:{}",
        loop_indicator,
        s.state,
        assigned,
        &s.title[..s.title.len().min(30)],
        acs_summary,
        s.history.len(),
    )
}

pub fn format_state_machine(loop_def: &LoopDef) -> String {
    let mut out = vec![];
    out.push(format!(
        "Loop: {} → {}",
        loop_def.initial,
        &loop_def.final_states.join("|")
    ));

    for (name, expert, desc) in loop_def.states {
        let is_initial = *name == loop_def.initial;
        let is_final = loop_def.final_states.contains(name);
        let flag = if is_initial {
            " (start)"
        } else if is_final {
            " (end)"
        } else {
            ""
        };
        out.push(format!("  {} {} — {} @{}", name, flag, desc, expert));
    }

    out.push(String::from("\nTransitions:"));
    for (f, t, tri, _exp) in loop_def.transitions {
        out.push(format!("  {} --[{}]--> {} (@{})", f, tri, t, _exp));
    }

    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_hash_deterministic() {
        let h1 = md5_hash("test story");
        let h2 = md5_hash("test story");
        assert_eq!(h1, h2);
    }

    #[test]
    fn find_next_state_advance() {
        let (next, expert) = find_next_state(&PLANNING_LOOP, "backlog", "advance").unwrap();
        assert_eq!(next, "research");
        assert_eq!(expert, "planner"); // transition expert, not state expert
    }

    #[test]
    fn find_next_state_kickback() {
        let (next, expert) = find_next_state(&PLANNING_LOOP, "interview", "kickback").unwrap();
        assert_eq!(next, "research");
        assert_eq!(expert, "sentinel");
    }

    #[test]
    fn find_next_state_no_transition() {
        let result = find_next_state(&PLANNING_LOOP, "approved", "advance");
        assert!(result.is_none());
    }

    #[test]
    fn loop_type_display() {
        assert_eq!(LoopType::Planning.to_string(), "planning");
        assert_eq!(LoopType::Implementation.to_string(), "implementation");
    }
}
