//! Renderer — builds the ANSI statusline string from a SessionInfo
//!
//! Uses the GradientEngine for gradient borders, Pill overlays, and the
//! BorderRenderer for per-column gradient coloring. Layout is built via
//! RowSpec/LayoutSpec declarative rows, then rendered through BorderRenderer.

use std::fmt::Write as FmtWrite;

use super::border::BorderRenderer;
use super::gradient::{
    self, empty_fade_colors, pill_gradient_fg, rainbow_at, rainbow_step, spec_gradient_bar,
    GradientEngine, Pill, GLYPH_ARROW_DOWN, GLYPH_ARROW_UP, GLYPH_BURN_FAST, GLYPH_BURN_SLOW,
    GLYPH_CONTINUATION, GLYPH_COST, GLYPH_FOLDER, GLYPH_HELPER, GLYPH_MEMBER, GLYPH_MODEL,
    GLYPH_PLUGINS, GLYPH_SKILLS, GLYPH_SUBAGENT, GLYPH_TASKS, GLYPH_THINKING, GLYPH_TOK_RATE,
    GLYPH_VSEP, MEDIUM_WIDTH, MIN_WIDTH, NARROW_WIDTH, RESET,
};
use super::session::{
    self, discover_openspec, fmt_tok, GitInfo, LoadedSkills, SessionInfo, TaskList,
    TokenAccounting, TokenLog, TokenRate, TranscriptUsage,
};
use super::themes::Theme;

// ---------------------------------------------------------------------------
// RowSpec / LayoutSpec — declarative row assembly
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RowSpec {
    pub kind: String, // top_border, bottom_border, separator, separator_dim, content
    pub content: String,
    pub bg_lead: String,
    pub bg_trail: String,
    pub pill_flush: bool,
    pub ups: Vec<usize>,
    pub downs: Vec<usize>,
    pub pill: Option<Pill>,
    pub pill_edge: String,
    pub right_pill: String,
}

impl Default for RowSpec {
    fn default() -> Self {
        Self {
            kind: "content".to_string(),
            content: String::new(),
            bg_lead: String::new(),
            bg_trail: String::new(),
            pill_flush: false,
            ups: Vec::new(),
            downs: Vec::new(),
            pill: None,
            pill_edge: "bottom".to_string(),
            right_pill: String::new(),
        }
    }
}

#[derive(Debug)]
pub struct LayoutSpec {
    pub width: usize,
    pub fill: f64,
    pub session_id: String,
    pub rows: Vec<RowSpec>,
}

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const BOLD: &str = "\x1b[1m";
const ITALIC: &str = "\x1b[3m";

// Default spec gradient palettes (12 palettes, 3 RGB stops each)
const DEFAULT_SPEC_GRADIENTS: &[(RGB, RGB, RGB); 12] = &[
    ((70, 130, 180), (100, 149, 237), (30, 144, 255)), // Ocean
    ((255, 99, 71), (255, 165, 0), (255, 215, 0)),     // Sunset
    ((34, 139, 34), (50, 205, 50), (144, 238, 144)),   // Forest
    ((138, 43, 226), (186, 85, 211), (221, 160, 221)), // Lavender
    ((220, 20, 60), (255, 69, 0), (255, 140, 0)),      // Inferno
    ((0, 191, 255), (65, 105, 225), (135, 206, 235)),  // Sky
    ((160, 82, 45), (210, 105, 30), (244, 164, 96)),   // Earth
    ((199, 21, 133), (255, 20, 147), (255, 182, 193)), // Rose
    ((0, 128, 128), (0, 206, 209), (127, 255, 212)),   // Teal
    ((75, 0, 130), (148, 0, 211), (216, 191, 216)),    // Violet
    ((107, 142, 35), (154, 205, 50), (240, 230, 140)), // Olive
    ((178, 34, 34), (233, 150, 122), (255, 228, 181)), // Salmon
];

type RGB = (u8, u8, u8);

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Day cost color coding (YAS: day_cost_colour)
fn day_cost_colour(cost: f64, theme: &Theme) -> &str {
    if cost > 50.0 {
        &theme.alert
    } else if cost >= 25.0 {
        &theme.yellow
    } else {
        &theme.safe
    }
}

/// Helper text: rate limit with time-to-reset
fn helper_text(fh_pct: f64, fh_resets_at: Option<u64>, theme: &Theme) -> String {
    let rate_color = if fh_pct >= 90.0 {
        &theme.alert
    } else if fh_pct >= 70.0 {
        &theme.warn
    } else {
        &theme.safe
    };
    let mut s = String::new();
    if let Some(resets) = fh_resets_at {
        if resets > 0 {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            if resets > now {
                let delta = resets - now;
                let h = delta / 3600;
                let m = (delta % 3600) / 60;
                let time_str = if h > 0 {
                    format!("{}h{}m", h, m)
                } else {
                    format!("{}m", m)
                };
                write!(
                    s,
                    "{}{:.0}%{} {}T-{}",
                    rate_color, fh_pct, RESET, theme.commit, time_str
                )
                .unwrap();
                return s;
            }
        }
    }
    if fh_pct > 0.0 {
        write!(s, "{}{:.0}%{}", rate_color, fh_pct, RESET).unwrap();
    } else {
        write!(s, "{}∞{}", theme.safe, RESET).unwrap();
    }
    s
}

const FIVE_HOUR_MINUTES: u32 = 300;
const FIVE_HOUR_WARMUP_MINUTES: u32 = 5;

/// Burndown trend glyph with gradient color (YAS: burndown_trend)
fn burndown_trend(fh_pct: f64, resets_at: Option<u64>, ge: &GradientEngine) -> String {
    let resets = match resets_at {
        Some(r) if r > 0 => r,
        _ => return String::new(),
    };
    let delta = match session::burndown_delta(
        fh_pct,
        resets,
        FIVE_HOUR_MINUTES,
        FIVE_HOUR_WARMUP_MINUTES,
    ) {
        Some(d) => d,
        None => return String::new(),
    };
    let abs_delta = delta.abs();
    let t = (0.5 + delta / 50.0).clamp(0.0, 1.0);
    let colour = ge.gradient_color(t, 1.0);
    let glyph = if delta > 0.0 {
        GLYPH_BURN_FAST
    } else {
        GLYPH_BURN_SLOW
    };
    let sign = if delta < 0.0 { '-' } else { '+' };
    format!("{}{} {}{:05.2}%{}", colour, glyph, sign, abs_delta, RESET)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn render(session: &SessionInfo, theme: &Theme, columns: usize, _layout_str: &str) -> String {
    // Require minimum viable width; if terminal is too narrow fall back to empty
    if columns < NARROW_WIDTH {
        if columns < 20 {
            return String::new();
        }
    }

    let ge = GradientEngine::new(theme);
    let br = BorderRenderer::new(ge, theme);

    let total_tokens = session.total_tokens();
    let fill = (total_tokens as f64 / session.soft_limit() as f64).min(1.0);
    let width = columns;

    // Determine layout based on width
    if width < NARROW_WIDTH {
        render_narrow(session, theme, &br, width, fill)
    } else if width < MEDIUM_WIDTH {
        render_medium(session, theme, &br, width, fill)
    } else {
        render_wide(session, theme, &br, width, fill)
    }
}

// ---------------------------------------------------------------------------
// Render layout spec through BorderRenderer
// ---------------------------------------------------------------------------

fn render_layout(spec: &LayoutSpec, br: &BorderRenderer) -> String {
    let mut lines = Vec::new();
    for row in &spec.rows {
        let line = match row.kind.as_str() {
            "top_border" => br.border_top(
                spec.width,
                &row.content,
                &row.downs,
                spec.fill,
                row.pill.as_ref(),
            ),
            "bottom_border" => br.border_bottom(spec.width, &row.ups, spec.fill),
            "separator" => br.border_separator(spec.width, &row.ups, spec.fill),
            "separator_dim" => br.border_separator_dim(
                spec.width,
                &row.downs,
                &row.ups,
                spec.fill,
                row.pill.as_ref(),
                &row.pill_edge,
            ),
            "content" => br.border_line(
                &row.content,
                spec.width,
                spec.fill,
                &row.bg_lead,
                &row.bg_trail,
                row.pill_flush,
                &row.right_pill,
            ),
            _ => String::new(),
        };
        lines.push(line);
    }
    lines.join("\n")
}

// ---------------------------------------------------------------------------
// Wide layout (80+ columns)
// ---------------------------------------------------------------------------

fn render_wide(
    s: &SessionInfo,
    theme: &Theme,
    br: &BorderRenderer,
    width: usize,
    fill: f64,
) -> String {
    let model_name = s.model.display_name.as_deref().unwrap_or(&s.model.id);
    let family = s.model_family();

    // Data enrichment from filesystem
    let transcript_path = s.transcript_path.as_deref().unwrap_or("");
    let usage = TranscriptUsage::from_transcript(transcript_path);
    let token_log = TokenLog::update(
        &s.session_id,
        usage.billed_in(),
        usage.cache_read_input_tokens,
        usage.output_tokens,
    );
    let tok_rate = TokenRate::update(&s.session_id, usage.billed_in(), usage.output_tokens);
    let git = GitInfo::from_cwd(s.cwd.as_deref().unwrap_or(""));
    let sess_cost = TokenAccounting::session_cost(&s.model, &usage);
    let day_cost = TokenAccounting::day_cost(&s.model, &token_log);
    let subagents = session::discover_subagents(
        &s.session_id,
        s.workspace
            .as_ref()
            .and_then(|w| w.project_dir.as_deref())
            .unwrap_or(""),
    );
    let tasks = TaskList::from_transcript(transcript_path);
    let skills = LoadedSkills::from_transcript(transcript_path);
    let elapsed = session::elapsed_from_transcript(transcript_path);
    let (down_active, up_active) = TokenRate::recently_active(&s.session_id, 60.0);

    // Build the layout
    let mut spec = LayoutSpec {
        width,
        fill,
        session_id: s.session_id.chars().take(8).collect(),
        rows: Vec::new(),
    };

    // Effort/thinking for pill background
    let effort_level = s
        .effort
        .as_ref()
        .and_then(|e| e.level.as_deref())
        .unwrap_or("");
    let thinking = s.thinking.as_ref().and_then(|t| t.enabled).unwrap_or(false);
    let effort_for_bg = if thinking { effort_level } else { "" };
    let pill_pct = gradient::model_bg_pct(effort_for_bg);
    let (pill_anchor, pill_shift) = gradient::model_anchor_pair(model_name, theme);

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
    let rate_color = |pct: f64| -> &str {
        if pct >= 90.0 {
            &theme.alert
        } else if pct >= 70.0 {
            &theme.warn
        } else {
            &theme.safe
        }
    };

    // Helper text: model pill + thinking + effort
    let mut helper = String::new();
    write!(
        helper,
        "{}",
        render_pill(model_name, family, theme, effort_for_bg)
    )
    .unwrap();
    if thinking {
        write!(
            helper,
            " {}{}{}{}{} thinking {}",
            theme.label, BOLD, GLYPH_THINKING, RESET, ITALIC, RESET
        )
        .unwrap();
    }
    let model_think = s.model_thinking();
    if !model_think.is_empty() && !thinking {
        write!(
            helper,
            " {}{}{}{}{} {}",
            theme.label, BOLD, GLYPH_MODEL, RESET, theme.model, model_think
        )
        .unwrap();
    }

    // Right section: rate limits with helper glyph + time-to-reset
    let step = rainbow_step();
    let c_helper = rainbow_at(step, 9);
    let mut right = String::new();
    write!(
        right,
        "{}{}{}{}  {}",
        c_helper, BOLD, GLYPH_HELPER, RESET, theme.white_brt
    )
    .unwrap();
    write!(
        right,
        "{}",
        helper_text(
            fh,
            s.rate_limits.as_ref().and_then(|r| r.five_hour.resets_at),
            theme
        )
    )
    .unwrap();
    // Burndown trend arrow
    let trend = burndown_trend(
        fh,
        s.rate_limits.as_ref().and_then(|r| r.five_hour.resets_at),
        &br.gradient,
    );
    if !trend.is_empty() {
        write!(right, " {}", trend).unwrap();
    }
    // Add 7d if non-zero
    if sd > 0.0
        || s.rate_limits
            .as_ref()
            .and_then(|r| r.seven_day.resets_at)
            .is_some()
    {
        let sd_color = rate_color(sd);
        write!(right, " {}| {}{:.0}%{}", theme.label, sd_color, sd, RESET).unwrap();
    }
    let right_w = visible_width(&right);

    // Path line — progressive degradation via fit_path
    let path_avail = width.saturating_sub(right_w).saturating_sub(6);
    let path_line = fit_path(s, &git, theme, &elapsed, path_avail);

    // Pill for model badge
    let pill = if pill_pct > 0 {
        let pill_end = width;
        let pill_start = pill_end.saturating_sub(right_w + 2);
        Some(Pill {
            start: pill_start,
            end: pill_end,
            anchor: pill_anchor,
            shift: pill_shift,
            pct: pill_pct,
        })
    } else {
        None
    };

    // Row 1: top border with path vertical separator down connector
    let path_w = visible_width(&path_line);
    let sep_col = 3 + path_w + 2;
    let content = format!("{}{}  {}{}", path_line, theme.label, RESET, helper);
    if pill_pct > 0 {
        spec.rows.push(RowSpec {
            kind: "top_border".to_string(),
            downs: vec![sep_col],
            pill: pill.clone(),
            ..Default::default()
        });
        spec.rows.push(RowSpec {
            kind: "content".to_string(),
            content,
            right_pill: right.clone(),
            ..Default::default()
        });
    } else {
        let pad = width
            .saturating_sub(3)
            .saturating_sub(visible_width(&content))
            .saturating_sub(right_w);
        let full = format!("{}{}{}{}", content, " ".repeat(pad), RESET, right);
        spec.rows.push(RowSpec {
            kind: "top_border".to_string(),
            downs: vec![sep_col],
            ..Default::default()
        });
        spec.rows.push(RowSpec {
            kind: "content".to_string(),
            content: full,
            ..Default::default()
        });
    }

    // Row 2: separator with path vsep up connector
    spec.rows.push(RowSpec {
        kind: "separator_dim".to_string(),
        ups: vec![sep_col],
        pill: pill.clone(),
        ..Default::default()
    });

    // Row 3-4: Two-row token/cost layout with aligned VSEPs
    let total = s.total_tokens();
    let ctx_fill = s.context_fill();
    let bar_w = (width as f64 * 0.35) as usize;
    let filled = (ctx_fill * bar_w as f64).round() as usize;
    let filled = filled.min(bar_w);
    let ge = &br.gradient;

    // Sparkline data
    let spark_history = TokenRate::history(&s.session_id, 20, 300.0);
    let (spark_top, spark_bot) = ge.sparkline(&spark_history, true);

    // Active arrow glyphs
    let down_arrow = if down_active { GLYPH_ARROW_DOWN } else { "↓" };
    let up_arrow = if up_active { GLYPH_ARROW_UP } else { "↑" };

    // Build sections separately for alignment
    let in_tok = s.billed_in();
    let cache_read = s.cache_read();
    let out_tok = s.context_window.total_output_tokens;

    // Section A: tokens — inputs on row 1, outputs on row 2
    // Left: session tokens  |  Right: daily tokens (when present)
    let has_daily = token_log.day_in > 0 || token_log.day_out > 0 || day_cost > 0.0;

    // Row 1 — all inputs: session ↓in (cache)  daily ↓in (cache)
    let mut sect_a1 = String::new();
    write!(sect_a1, "{}{}{}{}", theme.tok_icon, BOLD, down_arrow, RESET).unwrap();
    write!(sect_a1, " {}{}{}", theme.tok, fmt_tok(in_tok), RESET).unwrap();
    if cache_read > 0 {
        write!(
            sect_a1,
            "{}({}){}",
            theme.tok_dim,
            fmt_tok(cache_read),
            RESET
        )
        .unwrap();
    }
    if has_daily {
        write!(
            sect_a1,
            "  {}{}{}{}",
            theme.tok_day_dim, BOLD, down_arrow, RESET
        )
        .unwrap();
        write!(
            sect_a1,
            " {}{}{}",
            theme.tok_day,
            fmt_tok(token_log.day_in),
            RESET
        )
        .unwrap();
        if token_log.day_cache_read > 0 {
            write!(
                sect_a1,
                "{}({}){}",
                theme.tok_day_dim,
                fmt_tok(token_log.day_cache_read),
                RESET
            )
            .unwrap();
        }
    }

    // Row 2 — all outputs: session ↑out  daily ↑out
    let mut sect_a2 = String::new();
    write!(sect_a2, "{}{}{}{}", theme.tok_icon, BOLD, up_arrow, RESET).unwrap();
    write!(sect_a2, " {}{}{}", theme.tok, fmt_tok(out_tok), RESET).unwrap();
    if has_daily {
        write!(
            sect_a2,
            "  {}{}{}{}",
            theme.tok_day_dim, BOLD, up_arrow, RESET
        )
        .unwrap();
        write!(
            sect_a2,
            " {}{}{}",
            theme.tok_day,
            fmt_tok(token_log.day_out),
            RESET
        )
        .unwrap();
    }

    // Section B: cost — daily cost/d on row 1, session cost on row 2
    let mut sect_b1 = String::new();
    if has_daily {
        let dc_col = day_cost_colour(day_cost, theme);
        write!(
            sect_b1,
            " {}{}{}${:.2}/d{}",
            dc_col, GLYPH_COST, BOLD, day_cost, RESET
        )
        .unwrap();
    }

    let mut sect_b2 = String::new();
    if sess_cost > 0.0 {
        write!(
            sect_b2,
            " {}{}{}${:.2}{}",
            theme.cost, GLYPH_COST, BOLD, sess_cost, RESET
        )
        .unwrap();
    } else if let Some(ref cost) = s.cost {
        if let Some(c) = cost.total_cost_usd {
            if c > 0.0 {
                write!(
                    sect_b2,
                    " {}{}{}${:.2}{}",
                    theme.cost, GLYPH_COST, BOLD, c, RESET
                )
                .unwrap();
            }
        }
    }

    // Section C: sparkline + rate
    let mut sect_c1 = String::new();
    if !spark_top.is_empty() {
        write!(sect_c1, " {}{}{}", theme.tok_dim, spark_top, RESET).unwrap();
    }
    if tok_rate > 0 {
        write!(
            sect_c1,
            " {}{}{}{}{}/m{}",
            theme.tok,
            GLYPH_TOK_RATE,
            RESET,
            theme.tok,
            fmt_tok(tok_rate),
            RESET
        )
        .unwrap();
    }

    let mut sect_c2 = String::new();
    if !spark_bot.is_empty() {
        write!(sect_c2, " {}{}{}", theme.tok_dim, spark_bot, RESET).unwrap();
    }

    // Pad sections to align VSEPs between rows
    let wa1 = visible_width(&sect_a1);
    let wa2 = visible_width(&sect_a2);
    let max_a = wa1.max(wa2);
    if wa1 < max_a {
        sect_a1.push_str(&" ".repeat(max_a - wa1));
    }
    if wa2 < max_a {
        sect_a2.push_str(&" ".repeat(max_a - wa2));
    }

    let wb1 = visible_width(&sect_b1);
    let wb2 = visible_width(&sect_b2);
    let max_b = wb1.max(wb2);
    if wb1 < max_b {
        sect_b1.push_str(&" ".repeat(max_b - wb1));
    }
    if wb2 < max_b {
        sect_b2.push_str(&" ".repeat(max_b - wb2));
    }

    // Assemble rows with aligned VSEPs
    let vsep = format!(" {}{}{}", theme.label, GLYPH_VSEP, RESET);

    let tok_row1 = format!("{}{}{}{}{}", sect_a1, vsep, sect_b1, vsep, sect_c1);
    let tok_row2 = if !sect_a2.is_empty() {
        format!("{}{}{}{}{}", sect_a2, vsep, sect_b2, vsep, sect_c2)
    } else {
        String::new()
    };

    spec.rows.push(RowSpec {
        kind: "content".to_string(),
        content: tok_row1,
        ..Default::default()
    });

    if !tok_row2.is_empty() {
        spec.rows.push(RowSpec {
            kind: "content".to_string(),
            content: tok_row2,
            ..Default::default()
        });
    }

    // Row 5: Context bar with 3-step fade at fill boundary
    let mut ctx_line = String::new();
    // Context fill bar using gradient engine
    write!(ctx_line, " {}", ge.gradient_bar(filled, bar_w)).unwrap();

    // 3-step fade at fill boundary using empty_fade_colors
    let empty_count = bar_w.saturating_sub(filled);
    if empty_count > 0 {
        let fade = empty_fade_colors(theme.bar_empty_rgb);
        let fade_cells = std::cmp::min(3, empty_count);
        for i in 0..fade_cells {
            let (r, g, b) = fade[i];
            write!(ctx_line, "\x1b[48;2;{};{};{}m░", r, g, b).unwrap();
        }
        if empty_count > 3 {
            write!(
                ctx_line,
                "{}{}",
                theme.bar_empty,
                "░".repeat(empty_count - 3)
            )
            .unwrap();
        }
    }

    write!(
        ctx_line,
        "{}{}/{}{}",
        theme.bar_empty,
        fmt_tok(total),
        fmt_tok(s.soft_limit()),
        RESET
    )
    .unwrap();

    // Rate limits inline
    write!(
        ctx_line,
        "  5h {}{:.0}%{}  7d {}{:.0}%{}",
        rate_color(fh),
        fh,
        RESET,
        rate_color(sd),
        sd,
        RESET
    )
    .unwrap();

    spec.rows.push(RowSpec {
        kind: "content".to_string(),
        content: ctx_line,
        ..Default::default()
    });

    // Static/dynamic seam separator
    let mut pending_ups: Vec<usize> = vec![sep_col];
    let mut seam_pending = true;

    let mut sep_kind = || -> String {
        if seam_pending {
            seam_pending = false;
            "separator".to_string()
        } else {
            "separator_dim".to_string()
        }
    };

    // Skills/plugins row — rainbow-colored Nerd Font glyphs, truncated to fit
    let skill_names: Vec<String> = skills
        .names
        .iter()
        .map(|s| s.split(':').last().unwrap_or(s).to_string())
        .collect();
    let plugin_names = s.plugin_names();
    if !skill_names.is_empty() || !plugin_names.is_empty() {
        let max_content_w = width.saturating_sub(4); // 2 for left border, 2 for right border
        let mut line = String::new();
        let step = rainbow_step();
        if !skill_names.is_empty() {
            write!(
                line,
                "{}{}{}{}{} ",
                rainbow_at(step, 14),
                BOLD,
                GLYPH_SKILLS,
                RESET,
                theme.skills
            )
            .unwrap();
            let skills_text = skill_names.join(",");
            write!(line, "{}", skills_text).unwrap();
        }
        if !plugin_names.is_empty() {
            if !skill_names.is_empty() {
                write!(line, " ").unwrap();
            }
            write!(
                line,
                "{}{}{}{}{} ",
                rainbow_at(step, 7),
                BOLD,
                GLYPH_PLUGINS,
                RESET,
                theme.label
            )
            .unwrap();
            write!(line, "{}", plugin_names).unwrap();
        }
        // Truncate if wider than the content area
        let line_vis = visible_width(&line);
        if line_vis > max_content_w {
            let stripped = strip_ansi(&line);
            if stripped.len() > max_content_w {
                let truncated = format!("{}…", &stripped[..max_content_w.saturating_sub(1)]);
                line = truncated;
            }
        }
        spec.rows.push(RowSpec {
            kind: sep_kind(),
            ups: pending_ups.clone(),
            ..Default::default()
        });
        spec.rows.push(RowSpec {
            kind: "content".to_string(),
            content: line,
            ..Default::default()
        });
        pending_ups.clear();
    }

    // Tasks row — rainbow-colored task glyph
    if tasks.is_visible(None) {
        let mut line = String::new();
        let completed = tasks
            .tasks
            .iter()
            .filter(|t| t.status == "completed")
            .count();
        let total_tasks = tasks.tasks.len();
        let step = rainbow_step();
        write!(
            line,
            "{}{}{}{} {}✓{}/{}{}",
            rainbow_at(step, 9),
            BOLD,
            GLYPH_TASKS,
            RESET,
            theme.safe,
            completed,
            total_tasks,
            RESET
        )
        .unwrap();
        if let Some(active) = tasks.active() {
            let label = if !active.active_form.is_empty() {
                &active.active_form
            } else {
                &active.subject
            };
            let label = label.chars().take(30).collect::<String>();
            write!(line, " {}{}{}", theme.warn, label, RESET).unwrap();
        }
        spec.rows.push(RowSpec {
            kind: sep_kind(),
            ups: pending_ups.clone(),
            ..Default::default()
        });
        spec.rows.push(RowSpec {
            kind: "content".to_string(),
            content: line,
            ..Default::default()
        });
        pending_ups.clear();
    }

    // Subagent rows — two-line format when width > 100, single-line otherwise
    if !subagents.is_empty() {
        spec.rows.push(RowSpec {
            kind: sep_kind(),
            ups: pending_ups.clone(),
            ..Default::default()
        });
        let session_inout =
            (usage.billed_in() + usage.cache_read_input_tokens + usage.output_tokens) as f64;

        for sub in &subagents {
            let sub_step = rainbow_step();
            let marker_color = rainbow_at(sub_step, 3);

            if width > 100 {
                // Two-line format per subagent
                let mut line1 = String::new();
                write!(
                    line1,
                    "{}{}{}{}{}  {}{}{} · {}",
                    marker_color,
                    BOLD,
                    GLYPH_SUBAGENT,
                    RESET,
                    theme.white_brt,
                    BOLD,
                    sub.agent_type,
                    RESET,
                    theme.label
                )
                .unwrap();
                if !sub.description.is_empty() {
                    let desc = sub.description.chars().take(40).collect::<String>();
                    write!(line1, "{}", desc).unwrap();
                }
                write!(line1, "{}", RESET).unwrap();

                // Line 2: metrics cluster
                let mut line2 = String::new();
                write!(line2, "   {}{}{}", theme.label, GLYPH_CONTINUATION, RESET).unwrap();
                // Activity
                match &sub.last_activity {
                    session::SubagentActivity::ToolUse {
                        name,
                        input_key,
                        input_value,
                    } => {
                        write!(
                            line2,
                            " {}{}{}:{}{}{}",
                            theme.yellow, name, theme.dim_green, input_key, input_value, RESET
                        )
                        .unwrap();
                    }
                    session::SubagentActivity::Thinking => {
                        write!(line2, " {}{}thinking{}", theme.label, ITALIC, RESET).unwrap();
                    }
                    session::SubagentActivity::Text => {}
                    session::SubagentActivity::None => {}
                }

                // Metrics
                write!(
                    line2,
                    " · {}{}↓{}↑{}",
                    theme.tok_dim,
                    fmt_tok(sub.billed_in + sub.cache_read_in),
                    fmt_tok(sub.output),
                    RESET
                )
                .unwrap();

                // Share
                if session_inout > 0.0 {
                    let share = (sub.billed_in + sub.cache_read_in + sub.output) as f64
                        / session_inout
                        * 100.0;
                    if share >= 1.0 {
                        write!(line2, " {}{:.0}%{}", theme.tok_day, share, RESET).unwrap();
                    }
                }

                // Model
                if !sub.model.is_empty() {
                    let short_model = sub.model.split('-').next().unwrap_or(&sub.model);
                    write!(line2, " {}{}{}", theme.dim_green, short_model, RESET).unwrap();
                }

                // Rate
                if tok_rate > 0 {
                    write!(
                        line2,
                        " {}{}{}/m{}",
                        theme.tok,
                        GLYPH_TOK_RATE,
                        fmt_tok(tok_rate / subagents.len() as u64),
                        RESET
                    )
                    .unwrap();
                }

                spec.rows.push(RowSpec {
                    kind: "content".to_string(),
                    content: line1,
                    ..Default::default()
                });
                spec.rows.push(RowSpec {
                    kind: "content".to_string(),
                    content: line2,
                    ..Default::default()
                });
            } else {
                // Single-line format with rainbow marker
                let mut line = String::new();
                write!(
                    line,
                    "{}{}{}{}{} {}{}{}",
                    marker_color,
                    BOLD,
                    GLYPH_SUBAGENT,
                    RESET,
                    theme.white_brt,
                    theme.label,
                    sub.agent_type,
                    RESET
                )
                .unwrap();
                if !sub.description.is_empty() {
                    let desc = sub.description.chars().take(25).collect::<String>();
                    write!(line, " {}", desc).unwrap();
                }
                write!(
                    line,
                    " {}↓{}↑{}",
                    theme.tok_dim,
                    fmt_tok(sub.billed_in + sub.cache_read_in),
                    fmt_tok(sub.output)
                )
                .unwrap();

                // Share
                if session_inout > 0.0 {
                    let share = (sub.billed_in + sub.cache_read_in + sub.output) as f64
                        / session_inout
                        * 100.0;
                    if share >= 1.0 {
                        write!(line, " {}{:.0}%{}", theme.tok_day, share, RESET).unwrap();
                    }
                }

                // Model
                if !sub.model.is_empty() {
                    let short_model = sub.model.split('-').next().unwrap_or(&sub.model);
                    write!(line, " {}{}{}", theme.dim_green, short_model, RESET).unwrap();
                }

                // Last activity
                match &sub.last_activity {
                    session::SubagentActivity::ToolUse {
                        name,
                        input_key,
                        input_value,
                    } => {
                        write!(
                            line,
                            " {}{}{}:{}{}{}",
                            theme.yellow, name, theme.dim_green, input_key, input_value, RESET
                        )
                        .unwrap();
                    }
                    session::SubagentActivity::Thinking => {
                        write!(line, " {}thinking{}", theme.label, RESET).unwrap();
                    }
                    session::SubagentActivity::Text => {}
                    session::SubagentActivity::None => {}
                }

                spec.rows.push(RowSpec {
                    kind: "content".to_string(),
                    content: line,
                    ..Default::default()
                });
            }
        }
        pending_ups.clear();
    }

    // OpenSpec bars — after subagent rows, before bottom border
    let openspec = discover_openspec(s.cwd.as_deref().unwrap_or(""));
    for (idx, os) in openspec.iter().enumerate() {
        if os.total > 0 {
            let stops = theme
                .spec_gradients
                .get(idx % theme.spec_gradients.len())
                .copied()
                .unwrap_or_else(|| DEFAULT_SPEC_GRADIENTS[idx % DEFAULT_SPEC_GRADIENTS.len()]);
            // Convert to the (f64, RGB) 3-stop format expected by spec_gradient_bar
            let stops_fmt = [(0.0, stops.0), (0.5, stops.1), (1.0, stops.2)];
            let bar = spec_gradient_bar(
                os.done as usize,
                os.total as usize,
                &stops_fmt,
                &theme.spec_empty_ansi,
            );
            let mut line = String::new();
            let pct = if os.total > 0 {
                os.done * 100 / os.total
            } else {
                0
            };
            write!(
                line,
                "{}{}{}{}{} {} {}/{} {}{:>3}%",
                theme.white_brt, ITALIC, os.name, RESET, RESET, bar, os.done, os.total, BOLD, pct
            )
            .unwrap();
            spec.rows.push(RowSpec {
                kind: sep_kind(),
                ups: pending_ups.clone(),
                ..Default::default()
            });
            spec.rows.push(RowSpec {
                kind: "content".to_string(),
                content: line,
                ..Default::default()
            });
            pending_ups.clear();
        }
    }

    // Bottom border
    spec.rows.push(RowSpec {
        kind: "bottom_border".to_string(),
        ups: pending_ups,
        ..Default::default()
    });

    render_layout(&spec, br)
}

// ---------------------------------------------------------------------------
// Medium layout (55-80 columns)
// ---------------------------------------------------------------------------

fn render_medium(
    s: &SessionInfo,
    theme: &Theme,
    br: &BorderRenderer,
    width: usize,
    fill: f64,
) -> String {
    let model_name = s.model.display_name.as_deref().unwrap_or(&s.model.id);
    let family = s.model_family();
    let git = GitInfo::from_cwd(s.cwd.as_deref().unwrap_or(""));

    let total = s.total_tokens();
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
    let rate_color = |pct: f64| -> &str {
        if pct >= 90.0 {
            &theme.alert
        } else if pct >= 70.0 {
            &theme.warn
        } else {
            &theme.safe
        }
    };

    // Build content
    let mut content = String::new();
    write!(content, "{}", render_pill(model_name, family, theme, "")).unwrap();

    // Tokens
    write!(
        content,
        "  {}{}{}",
        theme.tok_icon,
        theme.tok,
        fmt_tok(total)
    )
    .unwrap();

    // Rate bar using gradient
    let bar_w = 15;
    let filled = ((fh / 100.0) * bar_w as f64).round() as usize;
    let filled = filled.min(bar_w);
    write!(content, " {}", br.gradient.gradient_bar(filled, bar_w)).unwrap();
    write!(content, "{}{:.0}%{}", rate_color(fh), fh, RESET).unwrap();

    // Burndown trend arrow (compact)
    let trend = burndown_trend(
        fh,
        s.rate_limits.as_ref().and_then(|r| r.five_hour.resets_at),
        &br.gradient,
    );
    if !trend.is_empty() {
        write!(content, "{}", trend).unwrap();
    }

    // Branch
    if !git.branch.is_empty() {
        write!(content, " {}{}", theme.branch, git.branch).unwrap();
        if git.modified > 0 || git.untracked > 0 {
            write!(content, "{} ±", theme.dirty).unwrap();
        }
    }

    // Cost
    if let Some(ref cost) = s.cost {
        if let Some(c) = cost.total_cost_usd {
            if c > 0.0 {
                write!(content, "  {}${:.2}", theme.cost, c).unwrap();
            }
        }
    }

    let tasks = TaskList::from_transcript(s.transcript_path.as_deref().unwrap_or(""));
    let sid = s.session_id.chars().take(8).collect::<String>();

    let mut spec = LayoutSpec {
        width,
        fill,
        session_id: sid,
        rows: Vec::new(),
    };

    spec.rows.push(RowSpec {
        kind: "top_border".to_string(),
        ..Default::default()
    });
    spec.rows.push(RowSpec {
        kind: "content".to_string(),
        content,
        ..Default::default()
    });

    if tasks.is_visible(None) {
        let completed = tasks
            .tasks
            .iter()
            .filter(|t| t.status == "completed")
            .count();
        let total_tasks = tasks.tasks.len();
        let mut task_line = String::new();
        write!(
            task_line,
            "{}✓{}/{}{}",
            theme.safe, completed, total_tasks, RESET
        )
        .unwrap();
        if let Some(active) = tasks.active() {
            let label = if !active.active_form.is_empty() {
                &active.active_form
            } else {
                &active.subject
            };
            write!(
                task_line,
                " {}{}{}",
                theme.warn,
                label.chars().take(30).collect::<String>(),
                RESET
            )
            .unwrap();
        }
        spec.rows.push(RowSpec {
            kind: "separator_dim".to_string(),
            ..Default::default()
        });
        spec.rows.push(RowSpec {
            kind: "content".to_string(),
            content: task_line,
            ..Default::default()
        });
    }

    // Context bar + sparkline
    let bar_w_ctx = (width as f64 * 0.35) as usize;
    let filled_ctx = (s.context_fill() * bar_w_ctx as f64).round() as usize;
    let mut ctx_line = String::new();
    write!(
        ctx_line,
        "{}{}{}/{}{}",
        theme.tok,
        fmt_tok(total),
        theme.bar_empty,
        fmt_tok(s.soft_limit()),
        RESET
    )
    .unwrap();
    write!(
        ctx_line,
        " {}",
        br.gradient
            .gradient_bar(filled_ctx.min(bar_w_ctx), bar_w_ctx)
    )
    .unwrap();
    write!(
        ctx_line,
        "{}7d {}{:.0}%{}",
        theme.bar_empty,
        rate_color(sd),
        sd,
        RESET
    )
    .unwrap();

    // Single-row sparkline if width permits (need ~10 chars for a useful sparkline)
    let ctx_vis_w = visible_width(&ctx_line);
    let spark_avail = width.saturating_sub(4).saturating_sub(ctx_vis_w); // 4 = border overhead
    if spark_avail >= 10 {
        let spark_history = TokenRate::history(&s.session_id, spark_avail.min(20), 300.0);
        let (spark_top, _) = br.gradient.sparkline(&spark_history, true);
        if !spark_top.is_empty() {
            write!(ctx_line, " {}{}{}", theme.tok_dim, spark_top, RESET).unwrap();
        }
    }

    spec.rows.push(RowSpec {
        kind: "separator_dim".to_string(),
        ..Default::default()
    });
    spec.rows.push(RowSpec {
        kind: "content".to_string(),
        content: ctx_line,
        ..Default::default()
    });
    spec.rows.push(RowSpec {
        kind: "bottom_border".to_string(),
        ..Default::default()
    });

    render_layout(&spec, br)
}

// ---------------------------------------------------------------------------
// Narrow layout (< 55 columns)
// ---------------------------------------------------------------------------

fn render_narrow(
    s: &SessionInfo,
    theme: &Theme,
    _br: &BorderRenderer,
    _width: usize,
    _fill: f64,
) -> String {
    let mut out = String::new();

    let model_name = s.model.display_name.as_deref().unwrap_or(&s.model.id);
    let family = s.model_family();

    // Model pill
    write!(out, "{}", render_pill(model_name, family, theme, "")).unwrap();
    out.push(' ');

    // Tokens
    let total = s.total_tokens();
    write!(out, "{}{}{} ", theme.tok_icon, theme.tok, fmt_tok(total)).unwrap();

    // Rate pct
    let pct = s
        .rate_limits
        .as_ref()
        .map(|r| r.five_hour.used_percentage)
        .unwrap_or(0.0);
    let rate_color = if pct >= 90.0 {
        &theme.alert
    } else if pct >= 70.0 {
        &theme.warn
    } else {
        &theme.safe
    };
    write!(out, "{}{:.0}%{}", rate_color, pct, RESET).unwrap();

    out.push_str(RESET);
    out.push('\n');
    out
}

// ---------------------------------------------------------------------------
// Model pill — with ▐/▌ edge chars and gradient coloring
// ---------------------------------------------------------------------------

fn render_pill(name: &str, model_family: &str, theme: &Theme, effort: &str) -> String {
    let colors = theme.models.get(model_family);
    let bg = match colors {
        Some(c) => c.anchor,
        None => (108, 108, 108),
    };

    // Effort-based background scaling
    let pct = gradient::model_bg_pct(effort);
    let (anchor, shift) = gradient::model_anchor_pair(name, theme);
    let eff_bg = if pct > 0 {
        let t = 0.5; // center of gradient
        let r = (anchor.0 as f64 + (shift.0 as f64 - anchor.0 as f64) * t) as i32;
        let g = (anchor.1 as f64 + (shift.1 as f64 - anchor.1 as f64) * t) as i32;
        let b = (anchor.2 as f64 + (shift.2 as f64 - anchor.2 as f64) * t) as i32;
        // Blend with model anchor at pct%
        let r = (bg.0 as f64 * (100 - pct) as f64 / 100.0 + r as f64 * pct as f64 / 100.0) as u8;
        let g = (bg.1 as f64 * (100 - pct) as f64 / 100.0 + g as f64 * pct as f64 / 100.0) as u8;
        let b = (bg.2 as f64 * (100 - pct) as f64 / 100.0 + b as f64 * pct as f64 / 100.0) as u8;
        (r, g, b)
    } else {
        bg
    };

    let luminance =
        (eff_bg.0 as f64 * 0.299 + eff_bg.1 as f64 * 0.587 + eff_bg.2 as f64 * 0.114) / 255.0;
    let fg_rgb = if luminance > 0.5 {
        theme.pill_fg_dark
    } else {
        theme.pill_fg_light
    };

    if pct > 0 {
        // Pill with gradient [ / ] edge chars
        let pill_l = pill_gradient_fg(0, name.len() + 4, anchor, shift, pct);
        let pill_r = pill_gradient_fg(name.len() + 4, name.len() + 4, anchor, shift, pct);
        format!(
            "{}[\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m {} {}]\x1b[0m",
            pill_l, eff_bg.0, eff_bg.1, eff_bg.2, fg_rgb.0, fg_rgb.1, fg_rgb.2, name, pill_r
        )
    } else {
        format!(
            "\x1b[48;2;{};{};{}m\x1b[38;2;{};{};{}m {} \x1b[0m",
            eff_bg.0, eff_bg.1, eff_bg.2, fg_rgb.0, fg_rgb.1, fg_rgb.2, name
        )
    }
}

// ---------------------------------------------------------------------------
// Visible-width helper
// ---------------------------------------------------------------------------

fn visible_width(s: &str) -> usize {
    let mut width = 0usize;
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if ch == '\x1b' {
            in_escape = true;
        } else {
            width += 1;
        }
    }
    width
}

/// Strip ANSI escape sequences from a string, returning only visible characters.
fn strip_ansi(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut in_escape = false;
    for ch in s.chars() {
        if in_escape {
            if ch.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else if ch == '\x1b' {
            in_escape = true;
        } else {
            result.push(ch);
        }
    }
    result
}

/// Truncate the middle of a plain string with "…" to fit max_len visible chars.
fn middle_ellipsis(text: &str, max_len: usize) -> String {
    if max_len <= 1 {
        return "…".to_string();
    }
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    let left = (max_len - 1) / 2;
    let right = max_len - 1 - left;
    let mut out = String::with_capacity(max_len);
    out.extend(&chars[..left]);
    out.push('…');
    out.extend(&chars[chars.len() - right..]);
    out
}

/// Build a compact path line: icon + pwd + ∈ + branch (no commit, dirty, elapsed).
fn path_git_compact(pwd: &str, branch: &str, theme: &Theme) -> String {
    let mut s = String::new();
    write!(
        s,
        "{}{}{}  {}{}{}",
        theme.icon_path, GLYPH_FOLDER, RESET, theme.pwd, pwd, RESET
    )
    .unwrap();
    if !branch.is_empty() {
        write!(
            s,
            " {}{}{}{}{}",
            theme.label, BOLD, theme.arrow, GLYPH_MEMBER, RESET
        )
        .unwrap();
        write!(s, " {}{}{}", theme.branch, branch, RESET).unwrap();
    }
    s
}

/// Progressive path degradation (YAS: fit_path).
/// Tries: full path_line → drop commit → drop elapsed → drop dirty → compact → ellipsis pwd → ellipsis both.
fn fit_path(
    s: &SessionInfo,
    git: &GitInfo,
    theme: &Theme,
    elapsed: &str,
    target_w: usize,
) -> String {
    let pwd = s.short_pwd();

    let build_full = |show_commit: bool, show_dirty: bool, show_elapsed: bool| -> String {
        let mut line = String::new();
        write!(
            line,
            "{}{}{}  {}{}",
            theme.icon_path, GLYPH_FOLDER, RESET, theme.pwd, pwd
        )
        .unwrap();
        if !git.branch.is_empty() {
            write!(
                line,
                " {}{}{}{}{}",
                theme.label, BOLD, theme.arrow, GLYPH_MEMBER, RESET
            )
            .unwrap();
            write!(line, " {}{}", theme.branch, git.branch).unwrap();
            if show_commit && !git.commit.is_empty() {
                write!(
                    line,
                    "{}/{}{}{}",
                    theme.label,
                    theme.commit,
                    &git.commit[..7.min(git.commit.len())],
                    RESET
                )
                .unwrap();
            }
            if show_dirty && (git.modified > 0 || git.untracked > 0) {
                write!(line, "{} ±", theme.dirty).unwrap();
            }
        }
        if show_elapsed && !elapsed.is_empty() && elapsed != "0m" {
            write!(line, " {}{}{}", theme.time, elapsed, RESET).unwrap();
        }
        write!(line, "{}", RESET).unwrap();
        line
    };

    // Try progressively shorter full-format versions
    for &(commit, dirty, elapsed_show) in &[
        (true, true, true),
        (false, true, true),
        (false, false, true),
        (false, false, false),
    ] {
        let candidate = build_full(commit, dirty, elapsed_show);
        if visible_width(&candidate) <= target_w {
            return candidate;
        }
    }

    // Try compact format (icon + pwd + branch only)
    let compact = path_git_compact(&pwd, &git.branch, theme);
    if visible_width(&compact) <= target_w {
        return compact;
    }

    // Ellipsis on pwd only
    for pwd_w in (1..target_w).rev() {
        let trunc = middle_ellipsis(&pwd, pwd_w);
        let candidate = path_git_compact(&trunc, &git.branch, theme);
        if visible_width(&candidate) <= target_w {
            return candidate;
        }
    }

    // Ellipsis on both pwd and branch
    let overhead = 10; // icon + spaces + ∈ + separators
    let half = ((target_w.saturating_sub(overhead)) / 2).max(1);
    let trunc_pwd = middle_ellipsis(&pwd, half);
    let trunc_branch = middle_ellipsis(&git.branch, half);
    path_git_compact(&trunc_pwd, &trunc_branch, theme)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn middle_ellipsis_short_text_unchanged() {
        assert_eq!(middle_ellipsis("hello", 10), "hello");
    }

    #[test]
    fn middle_ellipsis_exact_fit() {
        assert_eq!(middle_ellipsis("hello", 5), "hello");
    }

    #[test]
    fn middle_ellipsis_truncates_middle() {
        let result = middle_ellipsis("abcdefghij", 5);
        assert_eq!(result.chars().count(), 5);
        assert!(result.contains('…'));
        assert_eq!(result, "ab…ij");
    }

    #[test]
    fn middle_ellipsis_single_char_max() {
        assert_eq!(middle_ellipsis("abcdef", 1), "…");
    }

    #[test]
    fn visible_width_plain_text() {
        assert_eq!(visible_width("hello"), 5);
    }

    #[test]
    fn visible_width_with_ansi() {
        assert_eq!(visible_width("\x1b[31mhello\x1b[0m"), 5);
    }

    #[test]
    fn visible_width_empty() {
        assert_eq!(visible_width(""), 0);
    }

    #[test]
    fn strip_ansi_removes_escapes() {
        assert_eq!(strip_ansi("\x1b[38;2;255;0;0mred\x1b[0m text"), "red text");
    }

    #[test]
    fn strip_ansi_plain_text_unchanged() {
        assert_eq!(strip_ansi("plain text"), "plain text");
    }

    #[test]
    fn burndown_trend_no_resets() {
        let theme = super::super::themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let result = burndown_trend(50.0, None, &ge);
        assert!(result.is_empty());
    }

    #[test]
    fn burndown_trend_zero_resets() {
        let theme = super::super::themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let result = burndown_trend(50.0, Some(0), &ge);
        assert!(result.is_empty());
    }
}
