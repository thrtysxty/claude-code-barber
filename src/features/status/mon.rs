//! Monitor mode — watches a directory of session JSON files and renders
//! an aggregated statusline per session
//!
//! NOTE: This module is not yet wired into the `ccb status` command. The TUI
//! monitor will be activated once the statusline renderer is stable.

#![allow(dead_code, unused_variables)]

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use super::themes::Theme;

const CURSOR_HOME: &str = "\x1b[H";
const DIM: &str = "\x1b[38;5;240m";
const RESET: &str = "\x1b[0m";

// ---------------------------------------------------------------------------
// Session file discovery
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct SessionFile {
    path: PathBuf,
    modified: SystemTime,
}

fn find_sessions(claude_dir: &Path) -> Vec<SessionFile> {
    let mut sessions = Vec::new();
    let projects = claude_dir.join("projects");

    if !projects.is_dir() {
        return sessions;
    }

    let entries = match fs::read_dir(&projects) {
        Ok(e) => e,
        Err(_) => return sessions,
    };

    for entry in entries.filter_map(Result::ok) {
        let project_path = entry.path();
        if !project_path.is_dir() {
            continue;
        }

        // Look for sessions in project/sessions/
        let sessions_dir = project_path.join("sessions");
        if !sessions_dir.is_dir() {
            continue;
        }

        if let Ok(dir_entries) = fs::read_dir(&sessions_dir) {
            for session_entry in dir_entries.filter_map(Result::ok) {
                let file_path = session_entry.path();
                if file_path.extension().is_some_and(|e| e == "json") {
                    if let Ok(metadata) = fs::metadata(&file_path) {
                        let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
                        sessions.push(SessionFile {
                            path: file_path,
                            modified,
                        });
                    }
                }
            }
        }
    }

    sessions
}

// ---------------------------------------------------------------------------
// Lightweight session deserialization
// ---------------------------------------------------------------------------

#[derive(Debug, serde::Deserialize)]
struct MiniSession {
    session_id: String,
    model: ModelMini,
    cwd: Option<String>,
    current_date: Option<String>,
    current_time: Option<String>,
    context_window: ContextMini,
    rate_limits: Option<RateLimitsMini>,
    cost: Option<CostMini>,
    skills: Option<Vec<SkillMini>>,
    tasks: Option<Vec<TaskMini>>,
    subagents: Option<Vec<SubAgentMini>>,
    git: Option<GitMini>,
    thinking: Option<ThinkingMini>,
    effort: Option<EffortMini>,
}

#[derive(Debug, serde::Deserialize)]
struct ModelMini {
    id: String,
    display_name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ContextMini {
    #[serde(rename = "total_input_tokens")]
    total_input_tokens: u64,
    #[serde(rename = "total_output_tokens")]
    total_output_tokens: u64,
}

#[derive(Debug, serde::Deserialize)]
struct RateLimitsMini {
    #[serde(rename = "five_hour")]
    five_hour: FiveHourMini,
    #[serde(rename = "seven_day")]
    seven_day: SevenDayMini,
}

#[derive(Debug, serde::Deserialize)]
struct FiveHourMini {
    #[serde(rename = "used_percentage")]
    used_percentage: f64,
    #[serde(rename = "resets_at")]
    resets_at: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
struct SevenDayMini {
    #[serde(rename = "used_percentage")]
    used_percentage: f64,
    #[serde(rename = "resets_at")]
    resets_at: Option<u64>,
}

#[derive(Debug, serde::Deserialize)]
struct CostMini {
    #[serde(rename = "total_cost_usd")]
    total_cost_usd: Option<f64>,
}

#[derive(Debug, serde::Deserialize)]
struct SkillMini {
    skill: String,
}

#[derive(Debug, serde::Deserialize)]
struct TaskMini {
    #[serde(rename = "taskId")]
    task_id: Option<String>,
    subject: Option<String>,
    #[serde(rename = "activeForm")]
    active_form: Option<String>,
    status: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct SubAgentMini {
    name: Option<String>,
    #[serde(rename = "agent_type")]
    agent_type: Option<String>,
    description: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct GitMini {
    branch: Option<String>,
    #[serde(rename = "is_dirty")]
    is_dirty: Option<bool>,
    #[serde(rename = "commit_hash")]
    commit_hash: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct ThinkingMini {
    enabled: Option<bool>,
}

#[derive(Debug, serde::Deserialize)]
struct EffortMini {
    level: Option<String>,
}

impl MiniSession {
    fn total_tokens(&self) -> u64 {
        self.context_window.total_input_tokens + self.context_window.total_output_tokens
    }

    fn soft_limit(&self) -> u64 {
        crate::features::model_metadata::ModelMetadata::get().context_window_for(&self.model.id)
    }

    fn context_fill(&self) -> f64 {
        let limit = self.soft_limit() as f64;
        (self.total_tokens() as f64 / limit).min(1.0)
    }

    fn model_family(&self) -> &'static str {
        let id = self.model.id.to_lowercase();
        if id.contains("opus") {
            "opus"
        } else if id.contains("sonnet") {
            "sonnet"
        } else if id.contains("haiku") {
            "haiku"
        } else {
            "other"
        }
    }

    fn short_branch(&self) -> Option<&str> {
        self.git
            .as_ref()
            .and_then(|g| g.branch.as_deref())
            .and_then(|b| b.rsplit('/').next())
    }

    fn is_dirty(&self) -> bool {
        self.git
            .as_ref()
            .map(|g| g.is_dirty.unwrap_or(false))
            .unwrap_or(false)
    }

    fn load(path: &Path) -> Option<Self> {
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn fmt_age(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{m}m")
        } else {
            format!("{m}m{s:02}s")
        }
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        if m == 0 {
            format!("{h}h")
        } else {
            format!("{h}h{m:02}m")
        }
    }
}

fn age_label(age_secs: u64, width: usize) -> String {
    let age = fmt_age(age_secs);
    // Brighter text for the age value, dim for the decorative dashes
    let text = format!(" {}─{} \x1b[38;5;248m{} ago{} ", DIM, RESET, age, RESET);
    let vis_len = 4 + age.len() + 5; // " ─ " + age + " ago "
    let fill = width.saturating_sub(vis_len);
    // Gradient fade on trailing dashes: start at 244 → 238
    let mut dashes = String::new();
    for i in 0..fill {
        let brightness = 244u8.saturating_sub((i as u8 * 3).min(20));
        dashes.push_str(&format!("\x1b[38;5;{}m─", brightness));
    }
    format!("{}{}{}", text, dashes, RESET)
}

fn format_tokens(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.0}K", n as f64 / 1_000.0)
    } else {
        format!("{}", n)
    }
}

// ---------------------------------------------------------------------------
// Classify session age tier
// ---------------------------------------------------------------------------

fn classify(
    modified: SystemTime,
    now: SystemTime,
    idle_after: u64,
    remove_after: u64,
) -> &'static str {
    let age_secs = now.duration_since(modified).unwrap_or_default().as_secs();
    if age_secs >= remove_after {
        "removed"
    } else if age_secs >= idle_after {
        "dim"
    } else {
        "bright"
    }
}

// ---------------------------------------------------------------------------
// Layout rendering
// ---------------------------------------------------------------------------

fn render_session_box(s: &MiniSession, width: usize, theme: &Theme) -> Option<String> {
    let model_name = s.model.display_name.as_deref().unwrap_or(&s.model.id);
    let family = s.model_family();

    // Model pill
    let colors = theme.models.get(family);
    let bg = colors.map(|c| c.anchor).unwrap_or((108, 108, 108));
    let luminance = (bg.0 as f64 * 0.299 + bg.1 as f64 * 0.587 + bg.2 as f64 * 0.114) / 255.0;
    let (fr, fg_col, fb) = if luminance > 0.5 {
        theme.pill_fg_dark
    } else {
        theme.pill_fg_light
    };
    let pill = format!(
        "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m {} {RESET}",
        bg.0, bg.1, bg.2, fr, fg_col, fb, model_name
    );

    // Effort + thinking
    let mut extras = String::new();
    if let Some(ref eff) = s.effort {
        if let Some(ref level) = eff.level {
            if !level.is_empty() {
                let (bg_r, bg_g, bg_b) = match level.as_str() {
                    "high" => (220, 40, 50),
                    "medium" => (255, 140, 20),
                    "low" => (40, 210, 80),
                    _ => (108, 108, 108),
                };
                extras.push_str(&format!(
                    " \x1b[48;2;{bg_r};{bg_g};{bg_b}m\x1b[38;2;255;255;255m {} {RESET}",
                    level.to_uppercase()
                ));
            }
        }
    }
    if let Some(ref th) = s.thinking {
        if th.enabled.unwrap_or(false) {
            extras.push_str(&format!("{} thinking ", theme.label));
        }
    }

    // Branch + dirty
    let branch_str = s
        .short_branch()
        .map(|b| format!("{}{}", theme.branch, b))
        .unwrap_or_default();
    let dirty_str = if s.is_dirty() {
        format!("{} ±", theme.dirty)
    } else {
        String::new()
    };

    // Tokens + fill bar
    let total = s.total_tokens();
    let fill = s.context_fill();
    let grad_color = gradient_color_hex(theme, fill);
    let bar_width = 15.min((width / 3).max(8));
    let filled = (fill * bar_width as f64).round() as usize;

    // Rate limits
    let fh = s
        .rate_limits
        .as_ref()
        .map(|r| r.five_hour.used_percentage)
        .unwrap_or(0.0);
    let sd = s
        .rate_limits
        .as_ref()
        .map(|r| r.seven_day.used_percentage)
        .unwrap_or(0.0);
    let rate_color = |pct: f64| -> String {
        if pct >= 90.0 {
            theme.alert.clone()
        } else if pct >= 70.0 {
            theme.warn.clone()
        } else {
            theme.safe.clone()
        }
    };

    // Build output string using format! (String)
    let mut out = String::new();

    // Top line: pill + extras
    out.push_str("  ");
    out.push_str(&pill);
    out.push_str(&extras);
    out.push_str(RESET);
    out.push('\n');

    // Second line: branch + tokens + context bar + rate limits + cost
    out.push_str("  ");
    out.push_str(&branch_str);
    out.push_str(&dirty_str);
    out.push_str(RESET);
    out.push_str("  ");
    out.push_str(&theme.tok_icon);
    out.push_str(&theme.tok);
    out.push_str(&format_tokens(total));
    out.push_str("  [");
    out.push_str(&grad_color);
    out.push_str(&"█".repeat(filled));
    out.push_str(&theme.bar_empty);
    out.push_str(&"░".repeat(bar_width - filled));
    out.push_str(&format!(
        "]{}  5h {}{:.0}%{}  7d {}{:.0}%{}",
        RESET,
        rate_color(fh),
        fh,
        RESET,
        rate_color(sd),
        sd,
        RESET
    ));

    if let Some(ref cost) = s.cost {
        if let Some(c) = cost.total_cost_usd {
            out.push_str(&format!("  {}${:.2}", theme.cost, c));
        }
    }
    out.push('\n');

    // Skills row
    if let Some(ref skills) = s.skills {
        if !skills.is_empty() {
            out.push_str("  ");
            for sk in skills {
                out.push_str(&theme.skills);
                out.push_str(&sk.skill);
                out.push(',');
            }
            out.push_str(RESET);
            out.push_str("  ");
            out.push('\n');
        }
    }

    // Tasks row
    if let Some(ref tasks) = s.tasks {
        if !tasks.is_empty() {
            out.push_str("  ");
            for task in tasks.iter().take(5) {
                let status = task.status.as_deref().unwrap_or("pending");
                let (icon, color) = match status {
                    "completed" => ("●", &theme.safe),
                    "in_progress" => ("○", &theme.warn),
                    _ => ("·", &theme.label),
                };
                let label = task
                    .subject
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(15)
                    .collect::<String>();
                out.push_str(color);
                out.push_str(icon);
                out.push(' ');
                out.push_str(&label);
                out.push_str(RESET);
                out.push_str("  ");
            }
            out.push('\n');
        }
    }

    // Subagent rows (Gap M1)
    if let Some(ref subs) = s.subagents {
        if !subs.is_empty() {
            out.push_str("  ");
            for sub in subs.iter().take(4) {
                let agent_type = sub.agent_type.as_deref().unwrap_or("agent");
                let desc = sub.description.as_deref().unwrap_or("");
                let desc_short: String = desc.chars().take(20).collect();
                out.push_str(&format!("{}▶ {}{}", theme.label, agent_type, RESET));
                if !desc_short.is_empty() {
                    out.push_str(&format!(" {}{}{}", DIM, desc_short, RESET));
                }
                out.push_str("  ");
            }
            out.push('\n');
        }
    }

    Some(out)
}

fn gradient_color_hex(theme: &Theme, ratio: f64) -> String {
    let ratio = ratio.clamp(0.0, 1.0);
    let stops = &theme.grad_stops;
    if stops.is_empty() {
        return "\x1b[38;5;240m".to_string();
    }

    // Find surrounding stops
    let mut lower = stops[0];
    let mut upper = stops[stops.len() - 1];

    for i in 0..stops.len() - 1 {
        if ratio >= stops[i].0 && ratio <= stops[i + 1].0 {
            lower = stops[i];
            upper = stops[i + 1];
            break;
        }
    }

    let t = if upper.0 > lower.0 {
        (ratio - lower.0) / (upper.0 - lower.0)
    } else {
        0.0
    };

    let r = (lower.1 .0 as f64 + t * (upper.1 .0 as f64 - lower.1 .0 as f64)) as u8;
    let g = (lower.1 .1 as f64 + t * (upper.1 .1 as f64 - lower.1 .1 as f64)) as u8;
    let b = (lower.1 .2 as f64 + t * (upper.1 .2 as f64 - lower.1 .2 as f64)) as u8;
    format!("\x1b[38;2;{r};{g};{b}m")
}

// ---------------------------------------------------------------------------
// Header / footer
// ---------------------------------------------------------------------------

fn format_header(n_sessions: usize, cols: usize) -> String {
    let title = format!(
        " YASR — {} session{} ",
        n_sessions,
        if n_sessions == 1 { "" } else { "s" }
    );
    let inner = cols.saturating_sub(4);
    let title_len = title.len().min(inner);
    let dashes = inner.saturating_sub(title_len);
    format!(
        "\x1b[38;5;244m┌{}{}┐\x1b[0m",
        &title[..title_len],
        "─".repeat(dashes)
    )
}

fn format_footer(n_hidden: usize, cols: usize) -> String {
    let info = format!(" {} hidden ", n_hidden);
    let inner = cols.saturating_sub(4);
    let info_len = info.len().min(inner);
    let dashes = inner.saturating_sub(info_len);
    format!(
        "\x1b[38;5;244m└{}{}┘\x1b[0m",
        &info[..info_len],
        "─".repeat(dashes)
    )
}

fn format_empty_body(cols: usize, rows: usize) -> String {
    let inner = cols.saturating_sub(4);
    let msg = " No active sessions ";
    let _msg_padded = format!("{:^width$}", msg, width = inner);
    let mut out = String::new();
    out.push_str(&format!("\x1b[38;5;244m┌{}┐\x1b[0m", "─".repeat(inner)));
    out.push('\n');
    for _ in 0..rows.saturating_sub(2) {
        out.push_str(&format!(
            "\x1b[38;5;240m│{:^width$}│\x1b[0m",
            "",
            width = inner
        ));
        out.push('\n');
    }
    out.push_str(&format!("\x1b[38;5;244m└{}┘\x1b[0m", "─".repeat(inner)));
    out
}

// ---------------------------------------------------------------------------
// Aggregates
// ---------------------------------------------------------------------------

fn aggregate_rate_limits(sessions: &[&MiniSession]) -> (f64, f64) {
    if sessions.is_empty() {
        return (0.0, 0.0);
    }
    let n = sessions.len() as f64;
    let sum_fh: f64 = sessions
        .iter()
        .map(|s| {
            s.rate_limits
                .as_ref()
                .map(|r| r.five_hour.used_percentage)
                .unwrap_or(0.0)
        })
        .sum();
    let sum_sd: f64 = sessions
        .iter()
        .map(|s| {
            s.rate_limits
                .as_ref()
                .map(|r| r.seven_day.used_percentage)
                .unwrap_or(0.0)
        })
        .sum();
    ((sum_fh / n).min(100.0), (sum_sd / n).min(100.0))
}

fn aggregate_day_cost(sessions: &[&MiniSession]) -> f64 {
    sessions
        .iter()
        .filter_map(|s| s.cost.as_ref()?.total_cost_usd)
        .sum()
}

// ---------------------------------------------------------------------------
// Clip to height
// ---------------------------------------------------------------------------

fn clip_to_height(boxes: Vec<String>, max_height: usize) -> (Vec<String>, usize) {
    let heights: usize = boxes.iter().map(|b| b.lines().count()).sum();
    if heights <= max_height {
        return (boxes, 0);
    }

    let mut result = boxes;
    let mut hidden = 0;
    while result.iter().map(|b| b.lines().count()).sum::<usize>() > max_height && !result.is_empty()
    {
        hidden += 1;
        result.pop();
    }
    (result, hidden)
}

// ---------------------------------------------------------------------------
// Main run loop
// ---------------------------------------------------------------------------

pub fn run(directory: &Path, interval: u64, theme: &Theme, columns: usize) -> anyhow::Result<()> {
    let directory = directory.to_path_buf();
    let running = Arc::new(AtomicBool::new(true));
    let dirty = Arc::new(AtomicBool::new(true)); // start dirty to force initial render

    // Enter alternate screen buffer
    io::stdout().write_all(b"\x1b[?1049h")?;
    io::stdout().flush()?;

    // Spawn watcher thread — signals dirty flag on filesystem changes (Gap M2)
    let dirty_clone = dirty.clone();
    let dir_clone = directory.clone();
    let _watcher_handle = thread::spawn(move || {
        let (tx, rx) = std::sync::mpsc::channel();

        let tx_clone = tx.clone();
        let mut watcher: RecommendedWatcher = Watcher::new(
            move |_res: Result<notify::Event, notify::Error>| {
                let _ = tx_clone.send(());
            },
            Config::default(),
        )
        .unwrap();

        // Watch the projects directory recursively
        let projects_dir = dir_clone.join("projects");
        if watcher
            .watch(&projects_dir, RecursiveMode::Recursive)
            .is_ok()
        {
            // Signal dirty on every filesystem event
            for _ in rx.iter() {
                dirty_clone.store(true, Ordering::SeqCst);
            }
        }
    });

    let idle_after = 120u64;
    let remove_after = 3600u64;

    loop {
        // Always render on timer tick, but prioritize watcher-triggered redraws
        if dirty.swap(false, Ordering::SeqCst) {
            tick(&directory, theme, columns, idle_after, remove_after);
        }

        // Sleep in 100ms increments, break early on watcher signal
        let deadline = std::time::Instant::now() + Duration::from_secs(interval);
        loop {
            if dirty.load(Ordering::SeqCst) {
                break; // watcher fired — redraw immediately
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            if !running.load(Ordering::SeqCst) {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        if !running.load(Ordering::SeqCst) {
            break;
        }
    }

    // Exit alternate screen buffer
    io::stdout().write_all(b"\x1b[?1049l")?;
    io::stdout().flush()?;

    Ok(())
}

fn tick(directory: &Path, theme: &Theme, columns: usize, idle_after: u64, remove_after: u64) {
    let now = SystemTime::now();
    let cols = columns.max(60);

    // Discover + filter + load sessions
    let session_files = find_sessions(directory);
    let mut active: Vec<(SessionFile, MiniSession, &'static str)> = Vec::new();

    for sf in &session_files {
        let _age_secs = now
            .duration_since(sf.modified)
            .unwrap_or_default()
            .as_secs();
        let tier = classify(sf.modified, now, idle_after, remove_after);
        if tier == "removed" {
            continue;
        }

        if let Some(session) = MiniSession::load(&sf.path) {
            active.push((sf.clone(), session, tier));
        }
    }

    // Sort by modified time (newest first)
    active.sort_by(|a, b| b.0.modified.cmp(&a.0.modified));

    let n = active.len();
    let width = (cols - 8).max(40);

    // Header
    let visible: Vec<&MiniSession> = active.iter().map(|(_, s, _)| s).collect();
    let (avg_fh, avg_sd) = aggregate_rate_limits(&visible);
    let day_cost = aggregate_day_cost(&visible);

    let mut out = String::new();
    out.push_str(CURSOR_HOME);

    // Header line
    out.push_str(&format_header(n, cols));
    out.push('\n');

    // Rate limits summary in header area
    if n > 0 {
        let rate_color = |pct: f64| -> String {
            if pct >= 90.0 {
                theme.alert.clone()
            } else if pct >= 70.0 {
                theme.warn.clone()
            } else {
                theme.safe.clone()
            }
        };
        out.push_str(&format!(
            "  5h {}{:.0}%{}  7d {}{:.0}%{}",
            rate_color(avg_fh),
            avg_fh,
            RESET,
            rate_color(avg_sd),
            avg_sd,
            RESET
        ));
        if day_cost > 0.0 {
            out.push_str(&format!("  ${:.2}", day_cost));
        }
        out.push('\n');
    }

    // Session boxes
    let term_rows: usize = std::env::var("LINES")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(24);

    let available_body = term_rows.saturating_sub(4);

    let boxes: Vec<String> = active
        .iter()
        .map(|(sf, s, _)| {
            let age_secs = now
                .duration_since(sf.modified)
                .unwrap_or_default()
                .as_secs();
            let label = age_label(age_secs, width);
            if let Some(box_str) = render_session_box(s, width, theme) {
                format!("{}{}", label, box_str)
            } else {
                String::new()
            }
        })
        .collect();

    let (visible_boxes, hidden) = clip_to_height(boxes, available_body);

    for box_str in &visible_boxes {
        out.push_str(box_str);
    }

    out.push('\n');
    out.push_str(&format_footer(hidden, cols));
    out.push_str("\x1b[J"); // Clear rest of screen

    io::stdout().write_all(out.as_bytes()).unwrap();
    io::stdout().flush().unwrap();
}
