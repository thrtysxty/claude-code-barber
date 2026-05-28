//! CCB statusline bridge — aggregates data from CCB sources into a SessionInfo

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct StatusInput {
    pub session_id: String,
    pub cwd: Option<String>,
    pub model: String,
    pub model_display: Option<String>,
    pub thinking: bool,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub context_window_size: u64,
    pub used_percentage: f64,
    pub remaining_percentage: f64,
    pub rate_5h_pct: f64,
    pub rate_7d_pct: f64,
    pub rate_resets_at: Option<String>,
    pub cost_usd: f64,
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub commit_hash: Option<String>,
    pub story_id: Option<String>,
    pub story_title: Option<String>,
    pub story_state: Option<String>,
}

impl Default for StatusInput {
    fn default() -> Self {
        Self {
            session_id: uuid_v4(),
            cwd: std::env::var("PWD").ok(),
            model: std::env::var("CCB_MODEL").unwrap_or_else(|_| "unknown".to_string()),
            model_display: None,
            thinking: std::env::var("CCB_THINKING").is_ok(),
            input_tokens: 0,
            output_tokens: 0,
            context_window_size: crate::features::model_metadata::ModelMetadata::get()
                .context_window_for(&std::env::var("CCB_MODEL").unwrap_or_default()),
            used_percentage: 0.0,
            remaining_percentage: 100.0,
            rate_5h_pct: 0.0,
            rate_7d_pct: 0.0,
            rate_resets_at: None,
            cost_usd: 0.0,
            branch: git_branch(),
            is_dirty: git_is_dirty(),
            commit_hash: git_commit_hash(),
            story_id: None,
            story_title: None,
            story_state: None,
        }
    }
}

impl StatusInput {
    /// Load from all available CCB data sources.
    pub fn load() -> Self {
        // Read session env file first — written by SessionStart hook
        let session_env = read_session_env();

        // Model staleness guard: CCB_MODEL in session_env.sh is written by
        // update_session_env() during each statusline tick. With multiple
        // concurrent Claude Code sessions, the file is a race zone -- any
        // session's statusline can overwrite it. We can only trust the model
        // when we can verify it belongs to the current session via
        // CLAUDE_SESSION_ID (which Claude Code may set for statusline commands).
        // When CLAUDE_SESSION_ID is absent, the model from session_env.sh is
        // untrustworthy -- show "unknown" instead of a potentially wrong model.
        let env_session_id = session_env
            .get("CCB_SESSION_ID")
            .cloned()
            .unwrap_or_default();
        let active_session = std::env::var("CLAUDE_SESSION_ID").unwrap_or_default();
        let model_from_env_var = std::env::var("CCB_MODEL")
            .ok()
            .filter(|s| !s.is_empty() && s != "unknown");
        let model = if !active_session.is_empty() && !env_session_id.is_empty() {
            // Both IDs present -- compare them
            if env_session_id == active_session {
                session_env
                    .get("CCB_MODEL")
                    .cloned()
                    .filter(|s| !s.is_empty() && s != "unknown")
                    .or(model_from_env_var)
                    .unwrap_or_else(|| "unknown".to_string())
            } else {
                "unknown".to_string()
            }
        } else if !active_session.is_empty() {
            // Have CLAUDE_SESSION_ID but no CCB_SESSION_ID in file --
            // file was overwritten by another session's hook.
            model_from_env_var.unwrap_or_else(|| "unknown".to_string())
        } else {
            // No CLAUDE_SESSION_ID -- can't verify. Don't trust shared file.
            model_from_env_var.unwrap_or_else(|| "unknown".to_string())
        };

        let ctx_window =
            crate::features::model_metadata::ModelMetadata::get().context_window_for(&model);

        let mut input = Self {
            session_id: uuid_v4(),
            cwd: std::env::var("PWD").ok(),
            model,
            model_display: None,
            thinking: session_env
                .get("CCB_THINKING")
                .map(|s| s == "true")
                .unwrap_or_else(|| std::env::var("CCB_THINKING").is_ok()),
            input_tokens: 0,
            output_tokens: 0,
            context_window_size: ctx_window,
            used_percentage: 0.0,
            remaining_percentage: 100.0,
            rate_5h_pct: 0.0,
            rate_7d_pct: 0.0,
            rate_resets_at: None,
            cost_usd: 0.0,
            branch: session_env
                .get("CCB_GIT_BRANCH")
                .cloned()
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    std::env::var("CCB_GIT_BRANCH")
                        .ok()
                        .filter(|s| !s.is_empty())
                })
                .or_else(git_branch),
            is_dirty: session_env
                .get("CCB_GIT_DIRTY")
                .and_then(|s| s.parse::<bool>().ok())
                .unwrap_or_else(git_is_dirty),
            commit_hash: session_env
                .get("CCB_GIT_HASH")
                .cloned()
                .filter(|s| !s.is_empty())
                .or_else(|| std::env::var("CCB_GIT_HASH").ok().filter(|s| !s.is_empty()))
                .or_else(git_commit_hash),
            story_id: None,
            story_title: None,
            story_state: None,
        };

        if let Some(path) = cache_file("route_usage.jsonl") {
            let (in_toks, out_toks) = read_route_usage(&path);
            input.input_tokens = in_toks;
            input.output_tokens = out_toks;
        }

        if let Some(path) = cache_file("route_limits.json") {
            if let Some(rl) = read_route_limits(&path) {
                let (five_pct, seven_pct, resets) = rl.into_parts();
                input.rate_5h_pct = five_pct;
                input.rate_7d_pct = seven_pct;
                input.rate_resets_at = resets;
            }
        }

        if let (Some(toks), Some(max)) = (
            std::env::var("CCB_CTX_TOKENS")
                .ok()
                .and_then(|v| v.parse::<u64>().ok()),
            std::env::var("CCB_CTX_MAX")
                .ok()
                .and_then(|v| v.parse::<u64>().ok()),
        ) {
            input.input_tokens = toks;
            input.context_window_size = max;
            input.used_percentage = if max > 0 {
                (toks as f64 / max as f64) * 100.0
            } else {
                0.0
            };
            input.remaining_percentage = 100.0 - input.used_percentage;
        }

        if let Some(path) = cache_file("route_usage.jsonl") {
            input.cost_usd =
                crate::features::rates::session_cost(&input.model, input.thinking, &path);
        }

        #[cfg(feature = "factory")]
        {
            use crate::features::factory::{self, LoopType};
            if let Ok(stories) = factory::list_stories(Some(LoopType::Implementation)) {
                if let Some(s) = stories.into_iter().find(|s| s.state != "done") {
                    input.story_id = Some(s.id);
                    input.story_title = Some(s.title);
                    input.story_state = Some(s.state);
                }
            }
        }

        input
    }
}

// ── yasr SessionInfo builder ────────────────────────────────────────────────────

pub fn build_session_info(input: &StatusInput) -> super::SessionInfo {
    use super::session::*;
    use crate::features::rates::model_rates::ModelRate;

    let model = Model {
        id: input.model.clone(),
        display_name: input.model_display.clone(),
    };

    let context_window = ContextWindow {
        total_input_tokens: input.input_tokens,
        total_output_tokens: input.output_tokens,
        context_window_size: Some(input.context_window_size),
        current_usage: None,
        used_percentage: Some(input.used_percentage),
        remaining_percentage: Some(input.remaining_percentage),
    };

    let rate_limits = Some(RateLimits {
        five_hour: FiveHourLimit {
            used_percentage: input.rate_5h_pct,
            resets_at: input.rate_resets_at.as_ref().map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map(|dt| dt.timestamp() as u64)
                    .unwrap_or(0)
            }),
        },
        seven_day: SevenDayLimit {
            used_percentage: input.rate_7d_pct,
            resets_at: None,
        },
    });

    let git = GitState {
        branch: input.branch.clone(),
        is_dirty: Some(input.is_dirty),
        commit_hash: input.commit_hash.clone(),
        commit_message: None,
        ahead: None,
        behind: None,
    };

    let rate = ModelRate::for_model(&input.model);
    let base_cost = (input.input_tokens as f64 / 1_000_000.0) * rate.input_per_million
        + (input.output_tokens as f64 / 1_000_000.0)
            * rate.output_per_million
            * if input.thinking {
                rate.thinking_multiplier
            } else {
                1.0
            };
    let cost = Some(Cost {
        total_cost_usd: Some(base_cost),
        total_duration_ms: None,
        total_api_duration_ms: None,
        total_lines_added: None,
        total_lines_removed: None,
    });

    let thinking = Some(Thinking {
        enabled: Some(input.thinking),
    });
    let effort = None;

    let openspec_changes = input.story_state.clone().map(|state| {
        vec![OpenSpecChange {
            name: input.story_title.clone(),
            story_id: input.story_id.clone(),
            status: Some(state),
            tasks_total: None,
            tasks_completed: None,
        }]
    });

    super::SessionInfo {
        session_id: input.session_id.clone(),
        transcript_path: None,
        cwd: input.cwd.clone(),
        model,
        workspace: None,
        current_date: None,
        current_time: Some(chrono::Local::now().format("%H:%M").to_string()),
        version: None,
        output_style: None,
        cost,
        context_window,
        exceeds_200k_tokens: Some(input.input_tokens + input.output_tokens > 200_000),
        rate_limits,
        skills: None,
        enabled_plugins: None,
        tasks: None,
        subagents: None,
        openspec_changes,
        git: Some(git),
        sparkline_data: None,
        thinking,
        effort,
        fast_mode: None,
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

fn cache_file(name: &str) -> Option<PathBuf> {
    let base = dirs::home_dir()?.join(".cache").join("ccb");
    let path = base.join(name);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

fn read_route_usage(path: &PathBuf) -> (u64, u64) {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(_) => return (0, 0),
    };
    let mut in_total = 0u64;
    let mut out_total = 0u64;
    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<RouteUsageEntry>(line) {
            in_total += entry.in_tokens;
            out_total += entry.out_tokens;
        }
    }
    (in_total, out_total)
}

#[derive(Debug, serde::Deserialize)]
struct RouteUsageEntry {
    #[serde(rename = "t")]
    _timestamp: String,
    #[serde(rename = "mdl")]
    _model: String,
    #[serde(rename = "in")]
    in_tokens: u64,
    #[serde(rename = "out")]
    out_tokens: u64,
    #[serde(rename = "be")]
    _backend: String,
}

fn read_route_limits(path: &PathBuf) -> Option<RouteLimitsEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

#[derive(Debug, serde::Deserialize)]
struct RouteLimitsEntry {
    #[serde(rename = "five_hour")]
    five_hour: FiveHourEntry,
    #[serde(rename = "seven_day")]
    seven_day: SevenDayEntry,
    #[serde(rename = "resets_at", default)]
    resets_at: Option<String>,
}

impl RouteLimitsEntry {
    fn into_parts(self) -> (f64, f64, Option<String>) {
        (
            self.five_hour.utilization,
            self.seven_day.utilization,
            self.resets_at,
        )
    }
}

#[derive(Debug, serde::Deserialize)]
struct FiveHourEntry {
    #[serde(rename = "utilization")]
    utilization: f64,
}

#[derive(Debug, serde::Deserialize)]
struct SevenDayEntry {
    #[serde(rename = "utilization")]
    utilization: f64,
}

fn read_session_env() -> std::collections::HashMap<String, String> {
    let mut env = std::collections::HashMap::new();
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".cache")
        .join("ccb")
        .join("session_env.sh");
    if path.exists() {
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        for line in content.lines() {
            if let Some(stripped) = line.strip_prefix("export ") {
                if let Some((key, val)) = stripped.split_once('=') {
                    let val = val.trim_matches('"').trim_matches('\'');
                    env.insert(key.to_string(), val.to_string());
                }
            }
        }
    }
    env
}

fn git_branch() -> Option<String> {
    std::env::var("CCB_GIT_BRANCH")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() || s == "HEAD" {
                        None
                    } else {
                        Some(s)
                    }
                })
        })
}

fn git_is_dirty() -> bool {
    std::env::var("CCB_GIT_DIRTY")
        .ok()
        .and_then(|v| v.parse::<bool>().ok())
        .unwrap_or_else(|| {
            std::process::Command::new("git")
                .args(["status", "-s"])
                .output()
                .map(|o| !o.stdout.is_empty())
                .unwrap_or(false)
        })
}

fn git_commit_hash() -> Option<String> {
    std::env::var("CCB_GIT_HASH")
        .ok()
        .filter(|s| !s.is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "--short", "HEAD"])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() {
                        None
                    } else {
                        Some(s)
                    }
                })
        })
}

fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!(
        "{:x}{:x}{:x}{:x}",
        now as u64,
        now as u64,
        now as u64,
        std::process::id()
    )
}

pub fn resolve_theme(name: Option<&str>) -> super::Theme {
    super::themes::resolve_theme(name.unwrap_or("claude-dark"))
}
