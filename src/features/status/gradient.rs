//! Gradient engine, pill rendering, and bar characters
//!
//! Ported from YAS's GradientEngine, Pill, paint_bg_span, and related functions.

use super::themes::{Theme, RGB};
use std::time::{SystemTime, UNIX_EPOCH};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

pub const RESET: &str = "\x1b[0m";
pub const BOLD: &str = "\x1b[1m";
pub const ITALIC: &str = "\x1b[3m";

pub const BG_LUM_THRESHOLD: i32 = 110;
pub const LIVE_DIM: f64 = 0.5;
pub const MIN_WIDTH: usize = 40;
pub const DEFAULT_MAX_WIDTH: usize = 140;
pub const NARROW_WIDTH: usize = 55;
pub const MEDIUM_WIDTH: usize = 80;
pub const SOFT_LIMIT: u64 = 150_000;

/// Unicode block/bar characters for progress bars
pub struct BarChars;
impl BarChars {
    pub const FILLED: char = '█';
    pub const HEAVY: char = '▆';
    pub const MID: char = '▗';
    pub const EMPTY: char = '░';
}

/// Unicode quadrant/half-block characters for pill borders
pub const PILL_TL: char = '▗';
pub const PILL_TOP: char = '▄';
pub const PILL_TR: char = '▖';
pub const PILL_LEFT: char = '▐';
pub const PILL_RIGHT: char = '▌';
pub const PILL_BL: char = '▝';
pub const PILL_BOT: char = '▀';
pub const PILL_BR: char = '▘';

/// Unicode "Symbols for Legacy Computing" slope characters for sparklines
pub const SPARK_RISE_SMALL: char = '\u{1fb48}';
pub const SPARK_FALL_SMALL: char = '\u{1fb3d}';
pub const SPARK_RISE_MED: char = '\u{1fb4a}';
pub const SPARK_FALL_MED: char = '\u{1fb3f}';
pub const SPARK_RISE_TALL: char = '\u{1fb45}';
pub const SPARK_FALL_TALL: char = '\u{1fb50}';
pub const SPARK_RISE_TOP: char = '\u{1fb4b}';
pub const SPARK_FALL_TOP: char = '\u{1fb40}';

// ---------------------------------------------------------------------------
// ANSI helpers
// ---------------------------------------------------------------------------

pub fn fg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

pub fn bg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[48;2;{r};{g};{b}m")
}

// ---------------------------------------------------------------------------
// Rainbow palette + animation (item 2.6)
// ---------------------------------------------------------------------------

/// 30-color 256-color palette cycling through the hue wheel.
/// Ported from YAS statusline_command.py RAINBOW_PALETTE.
pub const RAINBOW_PALETTE: [u8; 30] = [
    196, 202, 208, 214, 220, 226, 190, 154, 118, 82, 46, 47, 48, 49, 51, 45, 39, 33, 27, 21, 57,
    93, 129, 165, 201, 197, 198, 199, 200, 201,
];

/// Current rainbow animation step based on wall-clock seconds (0–29).
pub fn rainbow_step() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as usize % 30)
        .unwrap_or(0)
}

/// ANSI 256-color fg sequence for palette entry at `(step + offset) % 30`.
pub fn rainbow_at(step: usize, offset: usize) -> String {
    format!("\x1b[38;5;{}m", RAINBOW_PALETTE[(step + offset) % 30])
}

// ---------------------------------------------------------------------------
// Nerd Font glyph constants (item 2.10)
// ---------------------------------------------------------------------------

pub const GLYPH_FOLDER: &str = "\u{ef85}"; //  folder icon
pub const GLYPH_SUBAGENT: &str = "\u{25b6}"; // ▶ play
pub const GLYPH_CONTINUATION: &str = "\u{2514}"; // └ continuation
pub const GLYPH_ARROW_DOWN: &str = "\u{1f847}"; // ⇁ active down arrow
pub const GLYPH_ARROW_UP: &str = "\u{1f845}"; // ⇃ active up arrow
pub const GLYPH_VSEP: &str = "\u{2502}"; // │ vertical separator
pub const GLYPH_MEMBER: &str = "\u{2208}"; // ∈ element of
pub const GLYPH_HELPER: &str = "\u{f4cd}"; // star_circle
pub const GLYPH_THINKING: &str = "\u{1f53}"; // brain
pub const GLYPH_MODEL: &str = "\u{f08b9}"; // monitor-dashboard
pub const GLYPH_TASKS: &str = "\u{f0755}"; // format-list-checks
pub const GLYPH_SKILLS: &str = "\u{f07df}"; // skills
pub const GLYPH_PLUGINS: &str = "\u{f1e6}"; // plug
pub const GLYPH_COST: &str = "\u{efc8}"; // currency-usd
pub const GLYPH_TOK_RATE: &str = "\u{f18a7}"; // gauge
pub const GLYPH_BURN_FAST: &str = "\u{ef76}"; // nf-cod-zap (over-burn)
pub const GLYPH_BURN_SLOW: &str = "\u{f490}"; // nf-oct-flame (under-burn)

// ---------------------------------------------------------------------------
// GradientEngine
// ---------------------------------------------------------------------------

pub struct GradientEngine {
    grad_stops: Vec<(f64, RGB)>,
    spark_stops: Vec<(f64, RGB)>,
    grey_rgb: RGB,
    border_off: String,
}

impl GradientEngine {
    pub fn new(theme: &Theme) -> Self {
        Self {
            grad_stops: theme.grad_stops.clone(),
            spark_stops: theme.spark_stops.clone(),
            grey_rgb: theme.grey_rgb,
            border_off: theme.border_off.clone(),
        }
    }

    /// Interpolate a color from gradient stops at position t (0.0–1.0).
    pub fn gradient_rgb(&self, t: f64, dim: f64) -> RGB {
        let t = t.max(0.0).min(1.0);
        for i in 0..self.grad_stops.len() - 1 {
            let (t0, c0) = self.grad_stops[i];
            let (t1, c1) = self.grad_stops[i + 1];
            if t <= t1 {
                let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
                return (
                    ((c0.0 as f64 + (c1.0 as f64 - c0.0 as f64) * u) * dim).round() as u8,
                    ((c0.1 as f64 + (c1.1 as f64 - c0.1 as f64) * u) * dim).round() as u8,
                    ((c0.2 as f64 + (c1.2 as f64 - c0.2 as f64) * u) * dim).round() as u8,
                );
            }
        }
        let (r, g, b) = self
            .grad_stops
            .last()
            .map(|&(_, c)| c)
            .unwrap_or((100, 100, 100));
        (
            (r as f64 * dim).round() as u8,
            (g as f64 * dim).round() as u8,
            (b as f64 * dim).round() as u8,
        )
    }

    pub fn gradient_color(&self, t: f64, dim: f64) -> String {
        let (r, g, b) = self.gradient_rgb(t, dim);
        fg(r, g, b)
    }

    /// Color for a column on a border, fading to grey beyond fill ratio.
    pub fn grad_at(&self, col: usize, width: usize, dim: f64, fill: f64) -> String {
        let denom = (width - 1).max(1) as f64;
        let t = col as f64 / denom;
        if fill <= 0.0 {
            return self.border_off.clone();
        }
        let fade = 0.06;
        if t <= fill - fade {
            return self.gradient_color(t, dim);
        }
        if t >= fill + fade {
            return self.border_off.clone();
        }
        let (er, eg, eb) = self.gradient_rgb(t.min(fill), dim);
        let (gr, gg, gb) = self.grey_rgb;
        let u = ((t - (fill - fade)) / (2.0 * fade)).max(0.0).min(1.0);
        let r = (er as f64 + (gr as f64 - er as f64) * u).round() as u8;
        let g = (eg as f64 + (gg as f64 - eg as f64) * u).round() as u8;
        let b = (eb as f64 + (gb as f64 - eb as f64) * u).round() as u8;
        fg(r, g, b)
    }

    /// Sparkline color at position t.
    pub fn spark_rgb(&self, t: f64, dim: f64) -> RGB {
        let t = t.max(0.0).min(1.0);
        for i in 0..self.spark_stops.len() - 1 {
            let (t0, c0) = self.spark_stops[i];
            let (t1, c1) = self.spark_stops[i + 1];
            if t <= t1 {
                let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
                return (
                    ((c0.0 as f64 + (c1.0 as f64 - c0.0 as f64) * u) * dim).round() as u8,
                    ((c0.1 as f64 + (c1.1 as f64 - c0.1 as f64) * u) * dim).round() as u8,
                    ((c0.2 as f64 + (c1.2 as f64 - c0.2 as f64) * u) * dim).round() as u8,
                );
            }
        }
        let (r, g, b) = self
            .spark_stops
            .last()
            .map(|&(_, c)| c)
            .unwrap_or((100, 200, 100));
        (
            (r as f64 * dim).round() as u8,
            (g as f64 * dim).round() as u8,
            (b as f64 * dim).round() as u8,
        )
    }

    pub fn spark_color(&self, t: f64, dim: f64) -> String {
        let (r, g, b) = self.spark_rgb(t, dim);
        fg(r, g, b)
    }

    /// Render a filled gradient bar.
    pub fn gradient_bar(&self, filled: usize, bar_w: usize) -> String {
        if filled == 0 || bar_w == 0 {
            return String::new();
        }
        let denom = (bar_w - 1).max(1) as f64;
        let mut parts = Vec::with_capacity(filled + 2);
        for i in 0..filled {
            let (r, g, b) = self.gradient_rgb(i as f64 / denom, 1.0);
            parts.push(format!("\x1b[48;2;{r};{g};{b}m "));
        }
        if filled <= bar_w {
            parts.push(format!(
                "\x1b[49m{}{}",
                self.gradient_color(filled as f64 / denom, 1.0),
                BarChars::MID
            ));
        }
        parts.join("")
    }

    /// Two-row sparkline from history data.
    pub fn sparkline(&self, history: &[u64], live: bool) -> (String, String) {
        if history.is_empty() {
            return (String::new(), String::new());
        }
        let max_val = *history.iter().max().unwrap_or(&1).max(&1);
        let spark_chars = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

        let indices: Vec<usize> = history
            .iter()
            .map(|&v| (((v as f64 / max_val as f64) * 16.0) as usize).min(16))
            .collect();
        let last_i = indices.len() - 1;

        let mut top_parts = Vec::new();
        let mut bot_parts = Vec::new();

        for (i, &idx) in indices.iter().enumerate() {
            let prev_idx = if i > 0 { indices[i - 1] } else { 0 };
            let (top_ch, bot_ch, tint_idx) = if idx > prev_idx {
                let (t, b) = if idx <= 3 {
                    (' ', SPARK_RISE_SMALL)
                } else if idx <= 7 {
                    (' ', SPARK_RISE_MED)
                } else if idx <= 8 {
                    (' ', SPARK_RISE_TALL)
                } else {
                    (SPARK_RISE_TOP, SPARK_RISE_TALL)
                };
                (t, b, idx)
            } else if prev_idx > idx {
                let (t, b) = if prev_idx <= 3 {
                    (' ', SPARK_FALL_SMALL)
                } else if prev_idx <= 7 {
                    (' ', SPARK_FALL_MED)
                } else if prev_idx <= 8 {
                    (' ', SPARK_FALL_TALL)
                } else {
                    (SPARK_FALL_TOP, SPARK_FALL_TALL)
                };
                (t, b, prev_idx)
            } else {
                let (t, b) = if idx == 0 {
                    (' ', spark_chars[0])
                } else if idx <= 8 {
                    (' ', spark_chars[idx - 1])
                } else {
                    (spark_chars[idx - 9], '█')
                };
                (t, b, idx)
            };

            let ratio = tint_idx as f64 / 16.0;
            let ratio_bot = ratio * 0.5;
            let ratio_top = 0.5 + ratio * 0.5;

            let (bot_clr, top_clr) = if live && i == last_i {
                (
                    self.spark_color(ratio_bot, LIVE_DIM),
                    self.spark_color(ratio_top, LIVE_DIM),
                )
            } else {
                (
                    self.spark_color(ratio_bot, 1.0),
                    self.spark_color(ratio_top, 1.0),
                )
            };

            top_parts.push(format!("{top_clr}{top_ch}{RESET}"));
            bot_parts.push(format!("{bot_clr}{bot_ch}{RESET}"));
        }

        (top_parts.join(""), bot_parts.join(""))
    }
}

// ---------------------------------------------------------------------------
// Pill
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Pill {
    pub start: usize,
    pub end: usize,
    pub anchor: RGB,
    pub shift: RGB,
    pub pct: i32,
}

impl Default for Pill {
    fn default() -> Self {
        Self {
            start: 0,
            end: 0,
            anchor: (0, 0, 0),
            shift: (0, 0, 0),
            pct: 0,
        }
    }
}

impl Pill {
    pub fn active(&self) -> bool {
        self.pct > 0
    }

    fn scale(rgb: RGB, pct: i32) -> RGB {
        (
            (rgb.0 as i32 * pct / 100).min(255).max(0) as u8,
            (rgb.1 as i32 * pct / 100).min(255).max(0) as u8,
            (rgb.2 as i32 * pct / 100).min(255).max(0) as u8,
        )
    }

    pub fn gradient_fg(&self, col: usize) -> String {
        let span = (self.end - self.start).max(1) as f64;
        let t = ((col as f64 - self.start as f64) / span).max(0.0).min(1.0);
        let c0 = Self::scale(self.anchor, self.pct);
        let c1 = Self::scale(self.shift, self.pct);
        let r = (c0.0 as f64 + (c1.0 as f64 - c0.0 as f64) * t).round() as u8;
        let g = (c0.1 as f64 + (c1.1 as f64 - c0.1 as f64) * t).round() as u8;
        let b = (c0.2 as f64 + (c1.2 as f64 - c0.2 as f64) * t).round() as u8;
        fg(r, g, b)
    }

    pub fn border_char(&self, col: usize, edge: &str) -> Option<char> {
        if !self.active() || col < self.start || col > self.end {
            return None;
        }
        if edge == "top" {
            if col == self.start {
                Some(PILL_TL)
            } else if col == self.end {
                Some(PILL_TR)
            } else {
                Some(PILL_TOP)
            }
        } else {
            if col == self.start {
                Some(PILL_BL)
            } else if col == self.end {
                Some(PILL_BR)
            } else {
                Some(PILL_BOT)
            }
        }
    }
}

// ---------------------------------------------------------------------------
// paint_bg_span — per-cell gradient pill background
// ---------------------------------------------------------------------------

pub fn paint_bg_span(
    cells: &[(char, Option<RGB>, bool, bool)],
    anchor: RGB,
    shift: RGB,
    pct: i32,
    pill_fg_dark: RGB,
    pill_fg_light: Option<RGB>,
) -> String {
    let c0 = Pill::scale(anchor, pct);
    let c1 = Pill::scale(shift, pct);
    let n = (cells.len().max(2) - 1) as f64;
    let mut parts = Vec::new();
    let mut prev_bg: Option<RGB> = None;
    let mut prev_fg: Option<RGB> = None;
    let mut prev_bold = false;
    let mut prev_italic = false;

    for (i, (ch, cell_fg, bold, italic)) in cells.iter().enumerate() {
        let t = i as f64 / n;
        let r = (c0.0 as f64 + (c1.0 as f64 - c0.0 as f64) * t).round() as u8;
        let g = (c0.1 as f64 + (c1.1 as f64 - c0.1 as f64) * t).round() as u8;
        let b = (c0.2 as f64 + (c1.2 as f64 - c0.2 as f64) * t).round() as u8;
        let lum = (r as i32 * 299 + g as i32 * 587 + b as i32 * 114) / 1000;

        let cur_bg = (r, g, b);
        let fg_rgb = if lum >= BG_LUM_THRESHOLD {
            Some(pill_fg_dark)
        } else if let Some(fg_l) = pill_fg_light {
            Some(fg_l)
        } else {
            *cell_fg
        };

        if prev_bg != Some(cur_bg) {
            parts.push(bg(r, g, b));
            prev_bg = Some(cur_bg);
        }
        if prev_fg != fg_rgb {
            if let Some((fr, fg_, fb)) = fg_rgb {
                parts.push(format!("\x1b[38;2;{fr};{fg_};{fb}m"));
            } else {
                parts.push("\x1b[39m".to_string());
            }
            prev_fg = fg_rgb;
        }
        if *bold != prev_bold {
            parts.push(if *bold {
                BOLD.to_string()
            } else {
                "\x1b[22m".to_string()
            });
            prev_bold = *bold;
        }
        if *italic != prev_italic {
            parts.push(if *italic {
                ITALIC.to_string()
            } else {
                "\x1b[23m".to_string()
            });
            prev_italic = *italic;
        }
        parts.push(ch.to_string());
    }
    parts.push("\x1b[49m".to_string());
    if prev_bold {
        parts.push("\x1b[22m".to_string());
    }
    if prev_italic {
        parts.push("\x1b[23m".to_string());
    }
    parts.push("\x1b[39m".to_string());
    parts.join("")
}

// ---------------------------------------------------------------------------
// Model pill rendering
// ---------------------------------------------------------------------------

/// Compute the effort-based background percentage for a model pill.
pub fn model_bg_pct(effort: &str) -> i32 {
    match effort {
        "low" => 30,
        "medium" => 55,
        "high" => 80,
        "xhigh" => 100,
        "max" => 140,
        _ => 0,
    }
}

/// Model anchor/shift color pair for pill backgrounds.
pub fn model_anchor_pair(model_name: &str, theme: &Theme) -> (RGB, RGB) {
    let family = {
        let m = model_name.to_lowercase();
        if m.contains("qwopus") {
            "qwopus"
        } else if m.contains("opus") {
            "opus"
        } else if m.contains("sonnet") {
            "sonnet"
        } else if m.contains("haiku") {
            "haiku"
        } else if m.contains("minimax") {
            "minimax"
        } else {
            "other"
        }
    };

    let mc = theme
        .models
        .get(family)
        .unwrap_or_else(|| theme.models.get("sonnet").unwrap());
    (mc.anchor, mc.warm_shift)
}

// ---------------------------------------------------------------------------
// pill_gradient_fg — pill side edge color for content rows (item 2.7)
// ---------------------------------------------------------------------------

/// Linear interpolation between `anchor` and `shift` at column position
/// `t = col / (total_cols - 1)`, scaled by `pct / 100`.
/// Used to render the ▐/▌ pill side edges in content rows.
pub fn pill_gradient_fg(
    col: usize,
    total_cols: usize,
    anchor: RGB,
    shift: RGB,
    pct: i32,
) -> String {
    let denom = if total_cols > 1 {
        (total_cols - 1) as f64
    } else {
        1.0
    };
    let t = col as f64 / denom;
    let r = ((anchor.0 as f64 + (shift.0 as f64 - anchor.0 as f64) * t) * pct as f64 / 100.0)
        .round()
        .min(255.0)
        .max(0.0) as u8;
    let g = ((anchor.1 as f64 + (shift.1 as f64 - anchor.1 as f64) * t) * pct as f64 / 100.0)
        .round()
        .min(255.0)
        .max(0.0) as u8;
    let b = ((anchor.2 as f64 + (shift.2 as f64 - anchor.2 as f64) * t) * pct as f64 / 100.0)
        .round()
        .min(255.0)
        .max(0.0) as u8;
    format!("\x1b[38;2;{r};{g};{b}m")
}

// ---------------------------------------------------------------------------
// Context bar empty fade helpers (item 2.8)
// ---------------------------------------------------------------------------

/// Returns 3 RGB values at 0.3x, 0.5x, 0.7x of `bar_empty_rgb`.
/// Used to render the 3 fade cells at the fill/empty boundary.
pub fn empty_fade_colors(bar_empty_rgb: RGB) -> [RGB; 3] {
    let scale = |factor: f64| -> RGB {
        (
            (bar_empty_rgb.0 as f64 * factor).round().min(255.0) as u8,
            (bar_empty_rgb.1 as f64 * factor).round().min(255.0) as u8,
            (bar_empty_rgb.2 as f64 * factor).round().min(255.0) as u8,
        )
    };
    [scale(0.3), scale(0.5), scale(0.7)]
}

// ---------------------------------------------------------------------------
// Spec gradient bar helpers (item 2.9)
// ---------------------------------------------------------------------------

/// Linearly interpolate through 3 color stops at position `t` (0.0–1.0).
pub fn spec_rgb_at(t: f64, stops: &[(f64, RGB); 3]) -> RGB {
    let t = t.max(0.0).min(1.0);
    for i in 0..stops.len() - 1 {
        let (t0, c0) = stops[i];
        let (t1, c1) = stops[i + 1];
        if t <= t1 {
            let u = if t1 > t0 { (t - t0) / (t1 - t0) } else { 0.0 };
            return (
                (c0.0 as f64 + (c1.0 as f64 - c0.0 as f64) * u).round() as u8,
                (c0.1 as f64 + (c1.1 as f64 - c0.1 as f64) * u).round() as u8,
                (c0.2 as f64 + (c1.2 as f64 - c0.2 as f64) * u).round() as u8,
            );
        }
    }
    let (_, c) = stops[2];
    c
}

/// Render a gradient-filled bar using `spec_rgb_at` for filled cells (true-color
/// bg spaces), a 45%-brightness blend cell at the boundary, and `empty_ansi` +
/// `BarChars::HEAVY` for empty cells.
pub fn spec_gradient_bar(
    filled: usize,
    total: usize,
    stops: &[(f64, RGB); 3],
    empty_ansi: &str,
) -> String {
    if total == 0 {
        return String::new();
    }
    let denom = if total > 1 { (total - 1) as f64 } else { 1.0 };
    let mut parts = Vec::with_capacity(total + 2);

    for i in 0..total {
        if i < filled {
            let (r, g, b) = spec_rgb_at(i as f64 / denom, stops);
            parts.push(format!("\x1b[48;2;{r};{g};{b}m "));
        } else if i == filled && filled > 0 {
            // 45%-brightness blend at boundary
            let (r, g, b) = spec_rgb_at(i as f64 / denom, stops);
            let br = (r as f64 * 0.45).round() as u8;
            let bg_ = (g as f64 * 0.45).round() as u8;
            let bb = (b as f64 * 0.45).round() as u8;
            parts.push(format!("\x1b[48;2;{br};{bg_};{bb}m "));
        } else {
            parts.push(format!("{empty_ansi}{}", BarChars::HEAVY));
        }
    }
    parts.push("\x1b[49m".to_string());
    parts.join("")
}

// ---------------------------------------------------------------------------
// Terminal width detection
// ---------------------------------------------------------------------------

pub fn terminal_width() -> usize {
    let home = dirs::home_dir().unwrap_or_default();
    let tw_path = home.join(".claude").join("terminal-width");

    // 1. tmux
    if let Ok(p) = std::env::var("TMUX_PANE") {
        let output = std::process::Command::new("tmux")
            .args(["display-message", "-p", "-t", &p, "#{pane_width}"])
            .output();
        if let Ok(out) = output {
            if let Ok(w) = String::from_utf8_lossy(&out.stdout).trim().parse::<usize>() {
                if w > 0 {
                    let _ = std::fs::write(&tw_path, w.to_string());
                    return w;
                }
            }
        }
    }

    // 2. ~/.claude/terminal-width file (written by SessionStart hook or self-heal)
    if let Ok(data) = std::fs::read_to_string(&tw_path) {
        if let Ok(w) = data.trim().parse::<usize>() {
            if w > 0 {
                return w;
            }
        }
    }

    // 3. COLUMNS env var
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(w) = cols.trim().parse::<usize>() {
            if w > 0 {
                let _ = std::fs::write(&tw_path, w.to_string());
                return w;
            }
        }
    }

    // 4. libc ioctl on stderr/stdout/stdin
    #[cfg(unix)]
    {
        for fd in [2, 1, 0] {
            unsafe {
                let ws: libc::winsize = std::mem::zeroed();
                if libc::ioctl(fd, libc::TIOCGWINSZ, &ws) == 0 && ws.ws_col > 0 {
                    let w = ws.ws_col as usize;
                    let _ = std::fs::write(&tw_path, w.to_string());
                    return w;
                }
            }
        }
        // 5. Try /dev/tty — when running as a statusline command, stdin is a pipe
        // but /dev/tty still points to the controlling terminal (same as YAS)
        if let Ok(tty) = std::fs::File::open("/dev/tty") {
            use std::os::unix::io::AsRawFd;
            unsafe {
                let mut ws: libc::winsize = std::mem::zeroed();
                if libc::ioctl(tty.as_raw_fd(), libc::TIOCGWINSZ, &mut ws) == 0 && ws.ws_col > 0 {
                    let w = ws.ws_col as usize;
                    let _ = std::fs::write(&tw_path, w.to_string());
                    return w;
                }
            }
        }
    }

    DEFAULT_MAX_WIDTH
}
