//! Border renderer — builds gradient ANSI borders around the statusline
//!
//! Ported from YAS's BorderRenderer class. Produces per-column gradient-colored
//! border lines with pill overlays, T-junction connectors, and dim separators.

use super::gradient::{GradientEngine, Pill, PILL_TL, RESET};
use super::themes::Theme;

// ---------------------------------------------------------------------------
// Visible-width helper (strips ANSI escapes)
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

// ---------------------------------------------------------------------------
// BorderRenderer
// ---------------------------------------------------------------------------

pub struct BorderRenderer {
    pub gradient: GradientEngine,
    session_color: String,
}

impl BorderRenderer {
    pub fn new(gradient: GradientEngine, theme: &Theme) -> Self {
        Self {
            gradient,
            session_color: theme.session.clone(),
        }
    }

    /// Top border: ╭─session_id─┬─────╮ with optional pill overlay and T-down connectors.
    pub fn border_top(
        &self,
        width: usize,
        session_id: &str,
        downs: &[usize],
        fill: f64,
        pill: Option<&Pill>,
    ) -> String {
        let downs_set: std::collections::HashSet<usize> = downs.iter().cloned().collect();
        let p = pill.cloned().unwrap_or_default();

        let mut parts = Vec::new();

        // Left corner
        if p.active() && p.start <= 1 {
            parts.push(p.gradient_fg(p.start));
            parts.push(PILL_TL.to_string());
        } else {
            parts.push(self.gradient.grad_at(0, width, 1.0, fill));
            parts.push('+'.to_string());
        }

        // Session ID or fill
        if !session_id.is_empty() {
            let avail = width.saturating_sub(4);
            let sid_max = if avail > 12 { avail } else { 0 };
            let sid_display = if session_id.len() <= sid_max {
                session_id.to_string()
            } else if sid_max > 1 {
                format!("{}…", &session_id[..sid_max.saturating_sub(1)])
            } else {
                String::new()
            };
            let sid_w = visible_width(&sid_display);
            if !sid_display.is_empty() {
                parts.push(self.gradient.grad_at(1, width, 1.0, fill));
                parts.push('─'.to_string());
                parts.push(self.gradient.grad_at(2, width, 1.0, fill));
                parts.push('─'.to_string());
                parts.push(self.session_color.clone());
                parts.push("\x1b[3m".to_string());
                parts.push(sid_display);
                parts.push("\x1b[23m".to_string());
            }

            let start = 3 + sid_w;
            let rest = width.saturating_sub(4).saturating_sub(sid_w);
            for i in 0..rest {
                let col = start + i + 1;
                if let Some(ch) = p.border_char(col, "top") {
                    parts.push(p.gradient_fg(col));
                    parts.push(ch.to_string());
                } else if downs_set.contains(&col) {
                    parts.push(self.gradient.grad_at(col - 1, width, 1.0, fill));
                    parts.push('+'.to_string());
                } else {
                    parts.push(self.gradient.grad_at(col - 1, width, 1.0, fill));
                    parts.push('-'.to_string());
                }
            }
        } else {
            for i in 1..(width - 1) {
                let col = i + 1;
                if let Some(ch) = p.border_char(col, "top") {
                    parts.push(p.gradient_fg(col));
                    parts.push(ch.to_string());
                } else if downs_set.contains(&col) {
                    parts.push(self.gradient.grad_at(i, width, 1.0, fill));
                    parts.push('+'.to_string());
                } else {
                    parts.push(self.gradient.grad_at(i, width, 1.0, fill));
                    parts.push('-'.to_string());
                }
            }
        }

        // Right corner
        if p.active() && p.start <= width && width <= p.end {
            parts.push(p.gradient_fg(width));
            parts.push(p.border_char(width, "top").unwrap_or('+').to_string());
        } else {
            parts.push(self.gradient.grad_at(width - 1, width, 1.0, fill));
            parts.push('+'.to_string());
        }
        parts.push(RESET.to_string());

        parts.join("")
    }

    /// Bottom border: +-+-----+ with optional T-up connectors.
    pub fn border_bottom(&self, width: usize, ups: &[usize], fill: f64) -> String {
        let mut parts = Vec::new();
        parts.push(self.gradient.grad_at(0, width, 0.6, fill));
        parts.push('+'.to_string());
        for i in 1..(width - 1) {
            let col = i + 1;
            if ups.contains(&col) {
                parts.push(self.gradient.grad_at(i, width, 0.6, fill));
                parts.push('+'.to_string());
            } else {
                parts.push(self.gradient.grad_at(i, width, 0.6, fill));
                parts.push('-'.to_string());
            }
        }
        parts.push(self.gradient.grad_at(width - 1, width, 0.6, fill));
        parts.push('+'.to_string());
        parts.push(RESET.to_string());
        parts.join("")
    }

    /// Solid separator: +---+ with T-up connectors.
    pub fn border_separator(&self, width: usize, ups: &[usize], fill: f64) -> String {
        let mut parts = Vec::new();
        parts.push(self.gradient.grad_at(0, width, 1.0, fill));
        parts.push('+'.to_string());
        for i in 1..(width - 1) {
            let col = i + 1;
            if ups.contains(&col) {
                parts.push(self.gradient.grad_at(i, width, 1.0, fill));
                parts.push('+'.to_string());
            } else {
                parts.push(self.gradient.grad_at(i, width, 1.0, fill));
                parts.push('-'.to_string());
            }
        }
        parts.push(self.gradient.grad_at(width - 1, width, 1.0, fill));
        parts.push('+'.to_string());
        parts.push(RESET.to_string());
        parts.join("")
    }

    /// Dim separator with ┄ characters, dimming away from elbows.
    pub fn border_separator_dim(
        &self,
        width: usize,
        downs: &[usize],
        ups: &[usize],
        fill: f64,
        pill: Option<&Pill>,
        pill_edge: &str,
    ) -> String {
        let downs_set: std::collections::HashSet<usize> = downs.iter().cloned().collect();
        let ups_set: std::collections::HashSet<usize> = ups.iter().cloned().collect();
        let mut elbow_cols: Vec<usize> = vec![1, width];
        elbow_cols.extend(downs.iter());
        elbow_cols.extend(ups.iter());

        let p = pill.cloned().unwrap_or_default();
        let mut parts = Vec::new();

        // Left corner
        if p.active() && p.start <= 1 {
            parts.push(p.gradient_fg(p.start));
            parts.push(p.border_char(p.start, pill_edge).unwrap_or('+').to_string());
        } else {
            parts.push(
                self.gradient
                    .grad_at(0, width, dim_for_col(1, &elbow_cols), fill),
            );
            parts.push('+'.to_string());
        }

        for i in 1..(width - 1) {
            let col = i + 1;
            if let Some(ch) = p.border_char(col, pill_edge) {
                parts.push(p.gradient_fg(col));
                parts.push(ch.to_string());
            } else if downs_set.contains(&col) || ups_set.contains(&col) {
                parts.push(
                    self.gradient
                        .grad_at(i, width, dim_for_col(col, &elbow_cols), fill),
                );
                parts.push('+'.to_string());
            } else {
                parts.push(
                    self.gradient
                        .grad_at(i, width, dim_for_col(col, &elbow_cols), fill),
                );
                parts.push('-'.to_string());
            }
        }

        // Right corner
        if p.active() && p.start <= width && width <= p.end {
            parts.push(p.gradient_fg(width));
            parts.push(p.border_char(width, pill_edge).unwrap_or('+').to_string());
        } else {
            parts.push(self.gradient.grad_at(
                width - 1,
                width,
                dim_for_col(width, &elbow_cols),
                fill,
            ));
            parts.push('+'.to_string());
        }
        parts.push(RESET.to_string());
        parts.join("")
    }

    /// Content line: │content│ with gradient-colored borders.
    #[allow(clippy::too_many_arguments)]
    pub fn border_line(
        &self,
        content: &str,
        width: usize,
        fill: f64,
        bg_lead: &str,
        bg_trail: &str,
        pill_flush: bool,
        right_pill: &str,
    ) -> String {
        let content_w = visible_width(content);
        let right_w = visible_width(right_pill);

        if !right_pill.is_empty() {
            let pad = width
                .saturating_sub(2)
                .saturating_sub(content_w)
                .saturating_sub(right_w);
            let left = self.gradient.grad_at(0, width, 1.0, fill);
            let lead = if !bg_lead.is_empty() {
                format!("{bg_lead} \x1b[49m")
            } else {
                " ".to_string()
            };
            return format!(
                "{left}|{RESET}{lead}{content}{}{right_pill}{RESET}",
                " ".repeat(pad)
            );
        }

        if pill_flush {
            let pad = width.saturating_sub(1).saturating_sub(content_w);
            let right = self.gradient.grad_at(width - 1, width, 1.0, fill);
            return format!("{content}{}{right}|{RESET}", " ".repeat(pad));
        }

        let pad = width.saturating_sub(3).saturating_sub(content_w);
        let left = self.gradient.grad_at(0, width, 1.0, fill);
        let right = self.gradient.grad_at(width - 1, width, 1.0, fill);
        let lead = if !bg_lead.is_empty() {
            format!("{bg_lead} \x1b[49m")
        } else {
            " ".to_string()
        };
        let pad_str = if !bg_trail.is_empty() && pad > 0 {
            format!("{} \x1b[49m{}", " ".repeat(pad.saturating_sub(1)), bg_trail)
        } else {
            " ".repeat(pad)
        };
        format!("{left}|{RESET}{lead}{content}{pad_str}{right}|{RESET}")
    }
}

/// Dimming factor for a column based on distance to nearest elbow.
fn dim_for_col(col: usize, elbow_cols: &[usize]) -> f64 {
    let d = elbow_cols
        .iter()
        .map(|e| (col as i32 - *e as i32).unsigned_abs())
        .min()
        .unwrap_or(5);
    if d == 0 {
        1.0
    } else if d >= 5 {
        0.6
    } else {
        1.0 - (1.0 - 0.6) * (d as f64 / 5.0)
    }
}
