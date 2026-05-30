//! Session data structures — deserialized from Claude Code session JSON
//!
//! Also contains filesystem-reading functions ported from YAS's Python:
//! - GitInfo: reads .git/HEAD and git status --porcelain for dirty detail
//! - TranscriptUsage: parses JSONL for token breakdowns (cache creation/read)
//! - TokenLog: persistent daily token tracking
//! - TokenRate: rolling-window throughput rate for sparklines
//! - RunningSubagents: discovers subagent metadata + transcripts from filesystem
//! - TaskList: extracts task lifecycle from transcript JSONL
//! - LoadedSkills: extracts skill invocations from transcript JSONL
//! - TokenAccounting: per-model cost rates with cache token weighting
//! - OpenSpec: discovers openspec/ directory and counts task checkbox progress

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Home directory helper
// ---------------------------------------------------------------------------

fn home_dir() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

fn claude_dir() -> PathBuf {
    std::env::var("CLAUDE_CONFIG_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| home_dir().join(".claude"))
}

// ---------------------------------------------------------------------------
// Session JSON structs (deserialized from Claude Code)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct SessionInfo {
    pub session_id: String,

    #[serde(rename = "transcript_path")]
    pub transcript_path: Option<String>,

    pub cwd: Option<String>,
    pub model: Model,
    pub workspace: Option<Workspace>,

    #[serde(rename = "current_date")]
    pub current_date: Option<String>,

    #[serde(rename = "current_time")]
    pub current_time: Option<String>,

    pub version: Option<String>,

    #[serde(rename = "output_style")]
    pub output_style: Option<OutputStyle>,

    pub cost: Option<Cost>,

    #[serde(rename = "context_window")]
    pub context_window: ContextWindow,

    #[serde(rename = "exceeds_200k_tokens")]
    pub exceeds_200k_tokens: Option<bool>,

    #[serde(rename = "rate_limits")]
    pub rate_limits: Option<RateLimits>,

    pub skills: Option<Vec<Skill>>,

    #[serde(rename = "enabled_plugins")]
    pub enabled_plugins: Option<Vec<EnabledPlugin>>,

    pub tasks: Option<Vec<Task>>,

    pub subagents: Option<Vec<SubAgent>>,

    #[serde(rename = "openspec_changes")]
    pub openspec_changes: Option<Vec<OpenSpecChange>>,

    pub git: Option<GitState>,

    #[serde(rename = "sparkline_data")]
    pub sparkline_data: Option<Vec<f64>>,

    pub thinking: Option<Thinking>,
    pub effort: Option<Effort>,

    #[serde(rename = "fast_mode")]
    pub fast_mode: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Model {
    pub id: String,
    #[serde(rename = "display_name")]
    pub display_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Workspace {
    #[serde(rename = "current_dir")]
    pub current_dir: Option<String>,

    #[serde(rename = "project_dir")]
    pub project_dir: Option<String>,

    #[serde(rename = "added_dirs")]
    pub added_dirs: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct OutputStyle {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Cost {
    #[serde(rename = "total_cost_usd")]
    pub total_cost_usd: Option<f64>,

    #[serde(rename = "total_duration_ms")]
    pub total_duration_ms: Option<u64>,

    #[serde(rename = "total_api_duration_ms")]
    pub total_api_duration_ms: Option<u64>,

    #[serde(rename = "total_lines_added")]
    pub total_lines_added: Option<u64>,

    #[serde(rename = "total_lines_removed")]
    pub total_lines_removed: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct ContextWindow {
    #[serde(rename = "total_input_tokens")]
    pub total_input_tokens: u64,

    #[serde(rename = "total_output_tokens")]
    pub total_output_tokens: u64,

    #[serde(rename = "context_window_size")]
    pub context_window_size: Option<u64>,

    #[serde(rename = "current_usage")]
    pub current_usage: Option<CurrentUsage>,

    #[serde(rename = "used_percentage")]
    pub used_percentage: Option<f64>,

    #[serde(rename = "remaining_percentage")]
    pub remaining_percentage: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct CurrentUsage {
    #[serde(rename = "input_tokens")]
    pub input_tokens: u64,

    #[serde(rename = "output_tokens")]
    pub output_tokens: u64,

    #[serde(rename = "cache_creation_input_tokens")]
    pub cache_creation_input_tokens: u64,

    #[serde(rename = "cache_read_input_tokens")]
    pub cache_read_input_tokens: u64,
}

#[derive(Debug, Deserialize)]
pub struct RateLimits {
    #[serde(rename = "five_hour")]
    pub five_hour: FiveHourLimit,

    #[serde(rename = "seven_day")]
    pub seven_day: SevenDayLimit,
}

#[derive(Debug, Deserialize)]
pub struct FiveHourLimit {
    #[serde(rename = "used_percentage")]
    pub used_percentage: f64,

    pub resets_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct SevenDayLimit {
    #[serde(rename = "used_percentage")]
    pub used_percentage: f64,

    pub resets_at: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub struct Skill {
    pub skill: String,
}

#[derive(Debug, Deserialize)]
pub struct EnabledPlugin {
    #[serde(rename = "plugin_id")]
    pub plugin_id: String,

    pub version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    #[serde(rename = "taskId")]
    pub task_id: Option<String>,

    pub subject: Option<String>,

    #[serde(rename = "activeForm")]
    pub active_form: Option<String>,

    pub status: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SubAgent {
    pub name: Option<String>,

    #[serde(rename = "agent_type")]
    pub agent_type: Option<String>,

    pub description: Option<String>,

    #[serde(rename = "billed_in")]
    pub billed_in: Option<u64>,

    #[serde(rename = "output_tokens")]
    pub output_tokens: Option<u64>,

    #[serde(rename = "transcript_path")]
    pub transcript_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct OpenSpecChange {
    pub name: Option<String>,

    #[serde(rename = "story_id")]
    pub story_id: Option<String>,

    pub status: Option<String>,

    #[serde(rename = "tasks_total")]
    pub tasks_total: Option<u32>,

    #[serde(rename = "tasks_completed")]
    pub tasks_completed: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct GitState {
    pub branch: Option<String>,

    #[serde(rename = "is_dirty")]
    pub is_dirty: Option<bool>,

    #[serde(rename = "commit_hash")]
    pub commit_hash: Option<String>,

    #[serde(rename = "commit_message")]
    pub commit_message: Option<String>,

    #[serde(rename = "ahead")]
    pub ahead: Option<u32>,

    #[serde(rename = "behind")]
    pub behind: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct Thinking {
    pub enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct Effort {
    pub level: Option<String>,
}

// ---------------------------------------------------------------------------
// Derived helpers on SessionInfo
// ---------------------------------------------------------------------------

impl SessionInfo {
    /// Total tokens (input + output)
    pub fn total_tokens(&self) -> u64 {
        self.context_window.total_input_tokens + self.context_window.total_output_tokens
    }

    /// Billed input tokens (input + cache_creation)
    pub fn billed_in(&self) -> u64 {
        self.context_window.total_input_tokens
            + self
                .context_window
                .current_usage
                .as_ref()
                .map(|u| u.cache_creation_input_tokens)
                .unwrap_or(0)
    }

    /// Cache read tokens
    pub fn cache_read(&self) -> u64 {
        self.context_window
            .current_usage
            .as_ref()
            .map(|u| u.cache_read_input_tokens)
            .unwrap_or(0)
    }

    /// Soft limit for fill bar — reads model_metadata.toml for accurate per-model context windows.
    pub fn soft_limit(&self) -> u64 {
        crate::features::model_metadata::ModelMetadata::get().context_window_for(&self.model.id)
    }

    /// Context fill ratio (0.0–1.0)
    pub fn context_fill(&self) -> f64 {
        let limit = self.soft_limit() as f64;
        let total = self.total_tokens() as f64;
        (total / limit).min(1.0)
    }

    /// Model family key: "opus" | "sonnet" | "haiku" | "minimax" | "qwopus" | "other"
    pub fn model_family(&self) -> &'static str {
        let id = self.model.id.to_lowercase();
        if id.contains("qwopus") {
            "qwopus"
        } else if id.contains("opus") {
            "opus"
        } else if id.contains("sonnet") {
            "sonnet"
        } else if id.contains("haiku") {
            "haiku"
        } else if id.contains("minimax") {
            "minimax"
        } else {
            "other"
        }
    }

    /// Short branch name (last path component)
    pub fn short_branch(&self) -> Option<&str> {
        self.git
            .as_ref()
            .and_then(|g| g.branch.as_deref())
            .and_then(|b| b.rsplit('/').next())
    }

    /// Dirty indicator
    pub fn is_dirty(&self) -> bool {
        self.git
            .as_ref()
            .map(|g| g.is_dirty.unwrap_or(false))
            .unwrap_or(false)
    }

    /// Abbreviated path: ~/P/t/thrift-skynet style
    pub fn short_pwd(&self) -> String {
        let home = home_dir();
        let home_str = home.to_string_lossy();
        let p = self.cwd.as_deref().unwrap_or("");
        let p = if p.starts_with(home_str.as_ref()) {
            format!("~/{}", &p[home_str.len()..].trim_start_matches('/'))
        } else {
            p.to_string()
        };
        let parts: Vec<&str> = p.split('/').collect();
        let last = parts.len().saturating_sub(1);
        parts
            .iter()
            .enumerate()
            .map(|(i, seg)| {
                if i == last || seg.is_empty() || *seg == "~" {
                    seg.to_string()
                } else {
                    seg.chars().next().unwrap_or('_').to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("/")
    }

    /// Model thinking display string
    pub fn model_thinking(&self) -> String {
        let thinking = self
            .thinking
            .as_ref()
            .and_then(|t| t.enabled)
            .unwrap_or(false);
        let effort = self
            .effort
            .as_ref()
            .and_then(|e| e.level.as_deref())
            .unwrap_or("");
        let fast = self.fast_mode.unwrap_or(false);

        if thinking && !effort.is_empty() {
            if fast {
                format!("{}/fast", effort)
            } else {
                effort.to_string()
            }
        } else if fast {
            "fast".to_string()
        } else {
            String::new()
        }
    }

    /// Plugin names from workspace settings
    pub fn plugin_names(&self) -> String {
        let mut names = Vec::new();
        let mut seen = HashSet::new();
        let global_dir = claude_dir();
        let project_dir = self
            .workspace
            .as_ref()
            .and_then(|w| w.project_dir.as_deref())
            .unwrap_or("");
        let paths: Vec<PathBuf> = vec![
            global_dir.join("settings.json"),
            PathBuf::from(project_dir)
                .join(".claude")
                .join("settings.json"),
        ];
        for sf in &paths {
            if let Ok(data) = std::fs::read_to_string(sf) {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                    if let Some(obj) = json.as_object() {
                        if let Some(enabled) = obj.get("enabledPlugins").and_then(|v| v.as_object())
                        {
                            for key in enabled.keys() {
                                let name = key.split('@').next().unwrap_or(key);
                                if seen.insert(name.to_string()) {
                                    names.push(name.to_string());
                                }
                            }
                        }
                    }
                }
            }
        }
        names.join(",")
    }
}

// ---------------------------------------------------------------------------
// GitInfo — reads .git/HEAD and git status --porcelain directly
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct GitInfo {
    pub branch: String,
    pub commit: String,
    pub modified: u32,
    pub untracked: u32,
    pub deleted: u32,
    pub renamed: u32,
}

impl GitInfo {
    /// Read git info from the working directory (no git subprocess for branch/commit,
    /// only `git status --porcelain` for dirty counts since that's the reliable way).
    pub fn from_cwd(cwd: &str) -> Self {
        let (branch, commit) = Self::read_head(cwd);
        let (modified, untracked, deleted, renamed) = Self::dirty_counts(cwd);
        Self {
            branch,
            commit,
            modified,
            untracked,
            deleted,
            renamed,
        }
    }

    fn read_head(cwd: &str) -> (String, String) {
        let curr = Path::new(cwd);
        let mut dir = curr;
        loop {
            let git_dir = dir.join(".git");
            if git_dir.is_dir() {
                let head_path = git_dir.join("HEAD");
                let head = match std::fs::read_to_string(&head_path) {
                    Ok(h) => h.trim().to_string(),
                    Err(_) => return (String::new(), String::new()),
                };
                let branch = if head.starts_with("ref:") {
                    head.rsplit('/').next().unwrap_or(&head).to_string()
                } else if !head.is_empty() {
                    format!("d:{}", &head[..7.min(head.len())])
                } else {
                    String::new()
                };
                let commit = if !branch.is_empty() && !branch.starts_with("d:") {
                    let ref_path = git_dir.join("refs").join("heads").join(&branch);
                    match std::fs::read_to_string(&ref_path) {
                        Ok(c) => c.trim()[..9.min(c.trim().len())].to_string(),
                        Err(_) => {
                            let orig = git_dir.join("ORIG_HEAD");
                            match std::fs::read_to_string(&orig) {
                                Ok(c) => c.trim()[..9.min(c.trim().len())].to_string(),
                                Err(_) => String::new(),
                            }
                        }
                    }
                } else {
                    String::new()
                };
                return (branch, commit);
            }
            dir = match dir.parent() {
                Some(p) => p,
                None => return (String::new(), String::new()),
            };
        }
    }

    fn dirty_counts(cwd: &str) -> (u32, u32, u32, u32) {
        let output = match std::process::Command::new("git")
            .args([
                "-C",
                cwd,
                "status",
                "--porcelain=v1",
                "-z",
                "--untracked-files=normal",
            ])
            .output()
        {
            Ok(o) => o,
            Err(_) => return (0, 0, 0, 0),
        };
        let stdout = String::from_utf8_lossy(&output.stdout);
        let entries: Vec<&str> = stdout.split('\0').filter(|e| !e.is_empty()).collect();
        let mut modified = 0u32;
        let mut untracked = 0u32;
        let mut deleted = 0u32;
        let mut renamed = 0u32;
        let mut i = 0;
        while i < entries.len() {
            let entry = entries[i];
            if entry.len() < 2 {
                i += 1;
                continue;
            }
            let (x, y) = (entry.as_bytes()[0], entry.as_bytes()[1]);
            if x == b'R' || y == b'R' {
                renamed += 1;
                i += 2; // rename consumes a second NUL-separated field
                continue;
            }
            if (x == b'?' && y == b'?') || x == b'A' || y == b'A' {
                untracked += 1;
            } else if x == b'D' || y == b'D' {
                deleted += 1;
            } else if x == b'M' || y == b'M' {
                modified += 1;
            }
            i += 1;
        }
        (modified, untracked, deleted, renamed)
    }
}

// ---------------------------------------------------------------------------
// OpenSpec directory discovery — walks up from cwd looking for openspec/
// ---------------------------------------------------------------------------

/// Progress summary for a single OpenSpec story's tasks.md.
/// Separate from `OpenSpecChange` (which deserializes session JSON) to avoid
/// mixing filesystem-derived data with API-derived data.
#[derive(Debug, Clone)]
pub struct OpenSpecProgress {
    pub name: String,
    pub done: u32,
    pub total: u32,
}

/// Walk up from `cwd` looking for an `openspec/` directory.
/// When found, recursively find `tasks.md` files (skip `/archive/` paths),
/// count `- [x]` (done) and `- [ ]` (open) checkboxes,
/// and return per-story progress summaries sorted by path.
/// Ported from YAS's `OpenSpec.from_cwd()`.
pub fn discover_openspec(cwd: &str) -> Vec<OpenSpecProgress> {
    let mut p = Path::new(cwd);
    loop {
        let openspec_dir = p.join("openspec");
        if openspec_dir.is_dir() {
            return collect_openspec_progress(&openspec_dir);
        }
        p = match p.parent() {
            Some(parent) => parent,
            None => return Vec::new(),
        };
    }
}

fn collect_openspec_progress(openspec_dir: &Path) -> Vec<OpenSpecProgress> {
    let mut tasks_files: Vec<PathBuf> = Vec::new();
    find_tasks_md(openspec_dir, &mut tasks_files);
    tasks_files.sort();

    let mut results = Vec::new();
    for path in tasks_files {
        // Skip any tasks.md inside an /archive/ path component
        let path_str = path.to_string_lossy();
        if path_str.contains("/archive/") {
            continue;
        }
        if let Ok(content) = std::fs::read_to_string(&path) {
            let done = content.matches("- [x]").count() as u32;
            let open = content.matches("- [ ]").count() as u32;
            let total = done + open;
            if total > 0 {
                let name = path
                    .parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string();
                results.push(OpenSpecProgress { name, done, total });
            }
        }
    }
    results
}

/// Recursively collect all `tasks.md` files under `dir`.
fn find_tasks_md(dir: &Path, results: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                find_tasks_md(&path, results);
            } else if path.file_name().and_then(|n| n.to_str()) == Some("tasks.md") {
                results.push(path);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// TranscriptUsage — parses JSONL for token breakdowns
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct TranscriptUsage {
    pub input_tokens: u64,
    pub cache_creation_input_tokens: u64,
    pub cache_read_input_tokens: u64,
    pub output_tokens: u64,
}

impl TranscriptUsage {
    /// Parse a Claude Code transcript JSONL for token usage.
    /// Each line is a JSON object with `message.usage` containing token counts.
    pub fn from_transcript(path: &str) -> Self {
        if path.is_empty() {
            return Self::default();
        }
        let p = Path::new(path);
        if !p.is_file() {
            return Self::default();
        }
        let mut ti = 0u64;
        let mut cc = 0u64;
        let mut cr = 0u64;
        let mut to = 0u64;
        let mut seen = HashSet::new();

        if let Ok(content) = std::fs::read_to_string(p) {
            for line in content.lines() {
                if !line.contains("\"usage\"") || !line.contains("\"assistant\"") {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    let msg = match json.get("message") {
                        Some(m) => m,
                        None => continue,
                    };
                    let mid = match msg.get("id") {
                        Some(id) => id.as_str().unwrap_or(""),
                        None => continue,
                    };
                    if !seen.insert(mid.to_string()) {
                        continue;
                    }
                    let u = match msg.get("usage") {
                        Some(u) => u,
                        None => continue,
                    };
                    ti += u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                    cc += u
                        .get("cache_creation_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    cr += u
                        .get("cache_read_input_tokens")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    to += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);
                }
            }
        }

        Self {
            input_tokens: ti,
            cache_creation_input_tokens: cc,
            cache_read_input_tokens: cr,
            output_tokens: to,
        }
    }

    pub fn billed_in(&self) -> u64 {
        self.input_tokens + self.cache_creation_input_tokens
    }
}

// ---------------------------------------------------------------------------
// TokenLog — persistent daily token tracking
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TokenLog {
    pub day_in: u64,
    pub day_cache_read: u64,
    pub day_out: u64,
}

impl TokenLog {
    const LOG_FILE: &'static str = "statusline-tokens.log";

    /// Update the daily log with this session's usage and return day totals.
    pub fn update(session_id: &str, total_in: u64, cache_read: u64, total_out: u64) -> Self {
        let log_path = claude_dir().join(Self::LOG_FILE);
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut lines = Vec::new();

        if log_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                for ln in content.lines() {
                    let parts: Vec<&str> = ln.split_whitespace().collect();
                    if parts.len() >= 2 && parts[1] == session_id {
                        continue; // remove old entry for this session
                    }
                    lines.push(ln.to_string());
                }
            }
        }

        if !session_id.is_empty() && (total_in > 0 || cache_read > 0 || total_out > 0) {
            lines.push(format!(
                "{} {} {} {} {}",
                today, session_id, total_in, cache_read, total_out
            ));
            if let Some(parent) = log_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&log_path, lines.join("\n") + "\n");
        }

        // Sum today's totals
        let mut day_in = 0u64;
        let mut day_cache_read = 0u64;
        let mut day_out = 0u64;
        for ln in &lines {
            let parts: Vec<&str> = ln.split_whitespace().collect();
            if parts.len() < 5 || parts[0] != today {
                continue;
            }
            if let Ok(in_t) = parts[2].parse::<u64>() {
                day_in += in_t;
            }
            if let Ok(cr) = parts[3].parse::<u64>() {
                day_cache_read += cr;
            }
            if let Ok(out_t) = parts[4].parse::<u64>() {
                day_out += out_t;
            }
        }

        Self {
            day_in,
            day_cache_read,
            day_out,
        }
    }
}

// ---------------------------------------------------------------------------
// TokenRate — rolling-window throughput for sparklines
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TokenRate;

impl TokenRate {
    const LOG_FILE: &'static str = "statusline-token-rate.log";
    const WINDOW: f64 = 60.0;
    const KEEP: f64 = 300.0;

    /// Append a rate sample and return total throughput (tokens in the window).
    pub fn update(session_id: &str, total_in: u64, total_out: u64) -> u64 {
        let log_path = claude_dir().join(Self::LOG_FILE);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut rows = Vec::new();

        if log_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                for ln in content.lines() {
                    let parts: Vec<&str> = ln.split_whitespace().collect();
                    if parts.len() < 4 {
                        continue;
                    }
                    let ts = parts[0].parse::<f64>().unwrap_or(0.0);
                    if now - ts > Self::KEEP {
                        continue;
                    }
                    rows.push(ln.to_string());
                }
            }
        }

        rows.push(format!(
            "{:.3} {} {} {}",
            now, session_id, total_in, total_out
        ));

        if let Some(parent) = log_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&log_path, rows.join("\n") + "\n");

        // Compute throughput: delta tokens in the window
        let mut samples: Vec<(f64, u64, u64)> = Vec::new();
        for ln in &rows {
            let parts: Vec<&str> = ln.split_whitespace().collect();
            if parts.len() < 4 {
                continue;
            }
            let ts = parts[0].parse::<f64>().unwrap_or(0.0);
            let sid = parts[1];
            if sid != session_id {
                continue;
            }
            if now - ts > Self::WINDOW {
                continue;
            }
            let ti = parts[2].parse::<u64>().unwrap_or(0);
            let to = parts[3].parse::<u64>().unwrap_or(0);
            samples.push((ts, ti, to));
        }
        if samples.len() < 2 {
            return 0;
        }
        samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let (_, ti0, to0) = samples[0];
        let (_, ti1, to1) = samples[samples.len() - 1];
        (ti1 + to1).saturating_sub(ti0 + to0)
    }

    /// Return sparkline history: n_buckets values for the last `window` seconds.
    pub fn history(session_id: &str, n_buckets: usize, window: f64) -> Vec<u64> {
        if n_buckets == 0 || session_id.is_empty() {
            return vec![0; n_buckets];
        }
        let log_path = claude_dir().join(Self::LOG_FILE);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut samples: Vec<(f64, u64, u64)> = Vec::new();

        if log_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                for ln in content.lines() {
                    let parts: Vec<&str> = ln.split_whitespace().collect();
                    if parts.len() < 4 {
                        continue;
                    }
                    let ts = parts[0].parse::<f64>().unwrap_or(0.0);
                    let sid = parts[1];
                    if sid != session_id {
                        continue;
                    }
                    if now - ts > window + window / n_buckets as f64 {
                        continue;
                    }
                    let ti = parts[2].parse::<u64>().unwrap_or(0);
                    let to = parts[3].parse::<u64>().unwrap_or(0);
                    samples.push((ts, ti, to));
                }
            }
        }

        if samples.len() < 2 {
            return vec![0; n_buckets];
        }
        samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let bucket_size = window / n_buckets as f64;
        let last_bucket = (now / bucket_size) as usize;
        let first_bucket = last_bucket.saturating_sub(n_buckets - 1);
        let mut buckets = vec![0u64; n_buckets];

        for i in 0..samples.len().saturating_sub(1) {
            let (ts0, ti0, to0) = samples[i];
            let (ts1, ti1, to1) = samples[i + 1];
            let delta = (ti1 + to1).saturating_sub(ti0 + to0);
            if delta == 0 {
                continue;
            }
            let midpoint = (ts0 + ts1) / 2.0;
            let abs_bucket = (midpoint / bucket_size) as usize;
            if abs_bucket >= first_bucket && abs_bucket <= last_bucket {
                let idx = abs_bucket.saturating_sub(first_bucket);
                if idx < n_buckets {
                    buckets[idx] += delta;
                }
            }
        }
        buckets
    }

    /// Check if token counts have grown in the last `window` seconds.
    pub fn recently_active(session_id: &str, window: f64) -> (bool, bool) {
        if session_id.is_empty() {
            return (false, false);
        }
        let log_path = claude_dir().join(Self::LOG_FILE);
        if !log_path.exists() {
            return (false, false);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let mut samples: Vec<(f64, u64, u64)> = Vec::new();

        if let Ok(content) = std::fs::read_to_string(&log_path) {
            for ln in content.lines() {
                let parts: Vec<&str> = ln.split_whitespace().collect();
                if parts.len() < 4 {
                    continue;
                }
                let ts = parts[0].parse::<f64>().unwrap_or(0.0);
                let sid = parts[1];
                if sid != session_id {
                    continue;
                }
                if now - ts > window {
                    continue;
                }
                let ti = parts[2].parse::<u64>().unwrap_or(0);
                let to = parts[3].parse::<u64>().unwrap_or(0);
                samples.push((ts, ti, to));
            }
        }

        if samples.len() < 2 {
            return (false, false);
        }
        samples.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let (ti0, to0) = (samples[0].1, samples[0].2);
        let (ti1, to1) = (samples[samples.len() - 1].1, samples[samples.len() - 1].2);
        (ti1 > ti0, to1 > to0)
    }
}

// ---------------------------------------------------------------------------
// TokenAccounting — per-model cost rates with cache weighting
// ---------------------------------------------------------------------------

pub struct TokenAccounting;

impl TokenAccounting {
    /// Per-model pricing: (input_per_million, output_per_million)
    pub fn rates_for(model_name: &str) -> (f64, f64) {
        let (inp, out, _) =
            crate::features::model_metadata::ModelMetadata::get().rates_for(model_name);
        (inp, out)
    }

    /// Session cost with cache token weighting.
    /// cache_creation: 1.25x input rate, cache_read: 0.1x input rate
    pub fn session_cost(model: &Model, usage: &TranscriptUsage) -> f64 {
        let (rate_in, rate_out) =
            Self::rates_for(model.display_name.as_deref().unwrap_or(&model.id));
        (usage.input_tokens as f64 * rate_in
            + usage.cache_creation_input_tokens as f64 * rate_in * 1.25
            + usage.cache_read_input_tokens as f64 * rate_in * 0.1
            + usage.output_tokens as f64 * rate_out)
            / 1_000_000.0
    }

    /// Day cost from TokenLog with cache weighting
    pub fn day_cost(model: &Model, log: &TokenLog) -> f64 {
        let (rate_in, rate_out) =
            Self::rates_for(model.display_name.as_deref().unwrap_or(&model.id));
        (log.day_in as f64 * rate_in
            + log.day_cache_read as f64 * rate_in * 0.1
            + log.day_out as f64 * rate_out)
            / 1_000_000.0
    }
}

// ---------------------------------------------------------------------------
// RunningSubagent — discovers subagent metadata from filesystem
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RunningSubagent {
    pub agent_type: String,
    pub description: String,
    pub billed_in: u64,
    pub cache_read_in: u64,
    pub total_input: u64,
    pub output: u64,
    pub first_timestamp: f64,
    pub model: String,
    pub last_activity: SubagentActivity,
}

#[derive(Debug, Clone, Default)]
pub enum SubagentActivity {
    ToolUse {
        name: String,
        input_key: String,
        input_value: String,
    },
    Thinking,
    Text,
    #[default]
    None,
}

/// Discover running subagents from the filesystem.
/// Matches Claude Code's projects/ directory convention.
pub fn discover_subagents(session_id: &str, project_dir: &str) -> Vec<RunningSubagent> {
    if session_id.is_empty() || project_dir.is_empty() {
        return Vec::new();
    }
    let project_slug: String = project_dir
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect();
    let subagents_dir = claude_dir()
        .join("projects")
        .join(&project_slug)
        .join(session_id)
        .join("subagents");
    if !subagents_dir.is_dir() {
        return Vec::new();
    }

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64();
    let mut subagents = Vec::new();

    if let Ok(entries) = std::fs::read_dir(&subagents_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                let meta_path = &path;
                let jsonl_path = path.with_extension("").with_extension("jsonl");
                if !jsonl_path.is_file() {
                    continue;
                }

                let mtime = jsonl_path
                    .metadata()
                    .ok()
                    .and_then(|m| m.modified().ok())
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .unwrap_or_default()
                    .as_secs_f64();
                if now - mtime > 20.0 {
                    continue;
                } // stale

                let (agent_type, description) = match std::fs::read_to_string(meta_path) {
                    Ok(content) => {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            let at = json
                                .get("agentType")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let desc = json
                                .get("description")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            (at, desc)
                        } else {
                            continue;
                        }
                    }
                    Err(_) => continue,
                };

                let (billed_in, cache_read_in, output, first_ts, model, last_activity) =
                    parse_subagent_transcript(&jsonl_path);

                subagents.push(RunningSubagent {
                    agent_type,
                    description,
                    billed_in,
                    cache_read_in,
                    total_input: billed_in + cache_read_in,
                    output,
                    first_timestamp: first_ts,
                    model,
                    last_activity,
                });
            }
        }
    }

    subagents.sort_by(|a, b| {
        a.first_timestamp
            .partial_cmp(&b.first_timestamp)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    subagents
}

fn parse_subagent_transcript(path: &Path) -> (u64, u64, u64, f64, String, SubagentActivity) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (0, 0, 0, 0.0, String::new(), SubagentActivity::None),
    };

    let mut billed_in = 0u64;
    let mut cache_read_in = 0u64;
    let mut output = 0u64;
    let mut first_ts = 0.0f64;
    let mut model = String::new();
    let mut last_activity = SubagentActivity::None;
    let mut seen = HashSet::new();

    for line in content.lines() {
        if first_ts == 0.0 && line.contains("\"timestamp\"") {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                if let Some(ts) = json.get("timestamp").and_then(|v| v.as_str()) {
                    first_ts = parse_iso_to_epoch(ts);
                }
            }
        }
        if !line.contains("\"usage\"") || !line.contains("\"assistant\"") {
            continue;
        }
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
            let msg = match json.get("message") {
                Some(m) => m,
                None => continue,
            };
            let mid = match msg.get("id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => continue,
            };
            if !seen.insert(mid) {
                continue;
            }

            if model.is_empty() {
                model = msg
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
            }

            let u = match msg.get("usage") {
                Some(u) => u,
                None => continue,
            };
            billed_in += u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0)
                + u.get("cache_creation_input_tokens")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0);
            cache_read_in += u
                .get("cache_read_input_tokens")
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            output += u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0);

            // Parse last activity
            if let Some(content) = msg.get("content").and_then(|c| c.as_array()) {
                if let Some(item) = content.last() {
                    let kind = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
                    match kind {
                        "tool_use" => {
                            let name = item
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let input = item
                                .get("input")
                                .and_then(|v| v.as_object())
                                .and_then(|o| {
                                    // Find the first key in TOOL_ARG_KEY map
                                    for key in &["command", "file_path", "pattern"] {
                                        if let Some(val) = o.get(*key) {
                                            return Some(val.as_str().unwrap_or("").to_string());
                                        }
                                    }
                                    o.iter()
                                        .next()
                                        .map(|(_, v)| v.as_str().unwrap_or("").to_string())
                                })
                                .unwrap_or_default();
                            let input_key = item
                                .get("input")
                                .and_then(|v| v.as_object())
                                .and_then(|o| o.keys().next().cloned())
                                .unwrap_or_default();
                            let display_val = if input.len() > 36 {
                                format!("{}…", &input[..36.min(input.len())])
                            } else {
                                input.clone()
                            };
                            last_activity = SubagentActivity::ToolUse {
                                name,
                                input_key,
                                input_value: display_val,
                            };
                        }
                        "thinking" => {
                            last_activity = SubagentActivity::Thinking;
                        }
                        "text" => {
                            last_activity = SubagentActivity::Text;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    (
        billed_in,
        cache_read_in,
        output,
        first_ts,
        model,
        last_activity,
    )
}

fn parse_iso_to_epoch(ts: &str) -> f64 {
    // Handle ISO timestamps like "2026-05-26T21:07:00.000Z" or "2026-05-26T21:07:00+00:00"
    let ts = ts.trim();
    let ts = if let Some(ts) = ts.strip_suffix('Z') {
        ts
    } else {
        ts
    };
    // Try parsing with chrono
    chrono::NaiveDateTime::parse_from_str(ts, "%Y-%m-%dT%H:%M:%S%.f")
        .ok()
        .and_then(|dt| dt.and_utc().timestamp().try_into().ok())
        .unwrap_or(0) as f64
}

// ---------------------------------------------------------------------------
// TaskList — extracts task lifecycle from transcript JSONL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TaskEntry {
    pub id: usize,
    pub subject: String,
    pub active_form: String,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct TaskList {
    pub tasks: Vec<TaskEntry>,
    pub last_event_ts: f64,
}

const TASK_FRESHNESS_CAP: f64 = 120.0; // seconds
const TASK_GRACE_SECONDS: f64 = 20.0;

impl TaskList {
    /// Parse task lifecycle from a transcript JSONL.
    pub fn from_transcript(transcript_path: &str) -> Self {
        if transcript_path.is_empty() {
            return Self::default();
        }
        let path = Path::new(transcript_path);
        if !path.is_file() {
            return Self::default();
        }

        let mut by_id: std::collections::HashMap<usize, TaskEntry> =
            std::collections::HashMap::new();
        let mut next_id = 1usize;
        let mut last_ts = 0.0f64;

        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if !line.contains("\"TaskCreate\"") && !line.contains("\"TaskUpdate\"") {
                    continue;
                }
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(line) {
                    let ts = json
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .map(parse_iso_to_epoch)
                        .unwrap_or(0.0);
                    let content = match json
                        .get("message")
                        .and_then(|m| m.get("content"))
                        .and_then(|c| c.as_array())
                    {
                        Some(c) => c,
                        None => continue,
                    };
                    for item in content {
                        if item.get("type").and_then(|v| v.as_str()) != Some("tool_use") {
                            continue;
                        }
                        let name = item.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        let input = match item.get("input").and_then(|v| v.as_object()) {
                            Some(i) => i,
                            None => continue,
                        };
                        if name == "TaskCreate" {
                            let subj = input
                                .get("subject")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let af = input
                                .get("activeForm")
                                .and_then(|v| v.as_str())
                                .unwrap_or(&subj)
                                .to_string();
                            by_id.insert(
                                next_id,
                                TaskEntry {
                                    id: next_id,
                                    subject: subj,
                                    active_form: af,
                                    status: "pending".to_string(),
                                },
                            );
                            next_id += 1;
                            if ts > last_ts {
                                last_ts = ts;
                            }
                        } else if name == "TaskUpdate" {
                            let tid = input
                                .get("taskId")
                                .and_then(|v| v.as_str())
                                .and_then(|s| s.parse::<usize>().ok())
                                .unwrap_or(0);
                            let t = match by_id.get_mut(&tid) {
                                Some(t) => t,
                                None => continue,
                            };
                            if let Some(status) = input.get("status").and_then(|v| v.as_str()) {
                                if ["pending", "in_progress", "completed"].contains(&status) {
                                    t.status = status.to_string();
                                }
                            }
                            if let Some(af) = input.get("activeForm").and_then(|v| v.as_str()) {
                                if !af.is_empty() {
                                    t.active_form = af.to_string();
                                }
                            }
                            if let Some(subj) = input.get("subject").and_then(|v| v.as_str()) {
                                if !subj.is_empty() {
                                    t.subject = subj.to_string();
                                }
                            }
                            if ts > last_ts {
                                last_ts = ts;
                            }
                        }
                    }
                }
            }
        }

        let mut tasks: Vec<TaskEntry> = by_id.into_values().collect();
        tasks.sort_by_key(|t| t.id);
        Self {
            tasks,
            last_event_ts: last_ts,
        }
    }

    pub fn is_visible(&self, now: Option<f64>) -> bool {
        if self.tasks.is_empty() || self.last_event_ts <= 0.0 {
            return false;
        }
        let now = now.unwrap_or_else(|| {
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
        });
        let age = now - self.last_event_ts;
        if age > TASK_FRESHNESS_CAP {
            return false;
        }
        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status == "completed")
            .count();
        if completed == self.tasks.len() {
            return age <= TASK_GRACE_SECONDS;
        }
        true
    }

    pub fn active(&self) -> Option<&TaskEntry> {
        self.tasks.iter().rev().find(|t| t.status == "in_progress")
    }
}

impl Default for TaskList {
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            last_event_ts: 0.0,
        }
    }
}

// ---------------------------------------------------------------------------
// LoadedSkills — extracts skill invocations from transcript JSONL
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct LoadedSkills {
    pub names: Vec<String>,
}

impl LoadedSkills {
    pub fn from_transcript(transcript_path: &str) -> Self {
        if transcript_path.is_empty() {
            return Self::default();
        }
        let path = Path::new(transcript_path);
        if !path.is_file() {
            return Self::default();
        }

        let skill_re =
            regex_lite::Regex::new(r#""name"\s*:\s*"Skill"[^}]*?"skill"\s*:\s*"([^"]+)""#)
                .unwrap_or_else(|_| regex_lite::Regex::new("").unwrap());
        let mut names = Vec::new();
        let mut seen = HashSet::new();

        if let Ok(content) = std::fs::read_to_string(path) {
            for line in content.lines() {
                if line.contains("\"Skill\"") {
                    for cap in skill_re.captures_iter(line) {
                        if let Some(name) = cap.get(1) {
                            let n = name.as_str().to_string();
                            if seen.insert(n.clone()) {
                                names.push(n);
                            }
                        }
                    }
                }
            }
        }

        Self { names }
    }
}

// ---------------------------------------------------------------------------
// Plugin enumeration from settings.json
// ---------------------------------------------------------------------------

/// Read enabled plugin names from ~/.claude/settings.json and project .claude/settings.json
pub fn load_enabled_plugins(project_dir: Option<&str>) -> Vec<String> {
    let mut names = Vec::new();
    let mut seen = HashSet::new();
    let paths: Vec<PathBuf> = vec![
        claude_dir().join("settings.json"),
        project_dir
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(".claude")
            .join("settings.json"),
    ];

    for sf in &paths {
        if let Ok(data) = std::fs::read_to_string(sf) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(enabled) = json.get("enabledPlugins").and_then(|v| v.as_object()) {
                    for key in enabled.keys() {
                        let name = key.split('@').next().unwrap_or(key);
                        if seen.insert(name.to_string()) {
                            names.push(name.to_string());
                        }
                    }
                }
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Elapsed time from transcript modification
// ---------------------------------------------------------------------------

/// Compute elapsed time string (e.g. "2h15m") from transcript file mtime
pub fn elapsed_from_transcript(transcript_path: &str) -> String {
    if transcript_path.is_empty() {
        return String::new();
    }
    let path = Path::new(transcript_path);
    if !path.is_file() {
        return String::new();
    }
    let mtime = match path.metadata().and_then(|m| m.modified()) {
        Ok(t) => t,
        Err(_) => return String::new(),
    };
    let secs = SystemTime::now()
        .duration_since(mtime)
        .unwrap_or_default()
        .as_secs();
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    if h > 0 {
        format!("{}h{}m", h, m)
    } else {
        format!("{}m", m)
    }
}

// ---------------------------------------------------------------------------
// Burndown delta — rate limit trend calculation
// ---------------------------------------------------------------------------

/// Compute the difference between actual usage percentage and ideal linear burn.
/// Positive = over budget, negative = under budget.
pub fn burndown_delta(
    used_pct: f64,
    resets_at: u64,
    window_minutes: u32,
    warmup_minutes: u32,
) -> Option<f64> {
    if resets_at == 0 {
        return None;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    if now >= resets_at {
        return None;
    }
    let window_start_ts = resets_at as f64 - window_minutes as f64 * 60.0;
    let elapsed_minutes = (now as f64 - window_start_ts) / 60.0;
    if elapsed_minutes < warmup_minutes as f64 {
        return None;
    }
    let ideal_pct = (elapsed_minutes / window_minutes as f64) * 100.0;
    Some(used_pct - ideal_pct)
}

// ---------------------------------------------------------------------------
// Token formatting
// ---------------------------------------------------------------------------

pub fn fmt_tok(n: u64) -> String {
    if n >= 999_950_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 999_950 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1000 {
        format!("{:.1}K", n as f64 / 1000.0)
    } else {
        format!("{}", n)
    }
}

pub fn fmt_dur(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s < 60 {
        format!("{}s", s)
    } else if s < 3600 {
        format!("{}m{:02}s", s / 60, s % 60)
    } else {
        format!("{}h{:02}m", s / 3600, (s % 3600) / 60)
    }
}
