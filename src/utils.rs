/// Render a progress bar for context window usage.
/// `pct` is 0–100, `width` is the number of block characters (10 for lineup, 20 for context).
pub fn progress_bar(pct: u8, width: usize) -> String {
    let filled = (pct as usize * width) / 100;
    let empty = width.saturating_sub(filled);
    let color = if pct >= 80 {
        "🔴"
    } else if pct >= 60 {
        "🟡"
    } else {
        "🟢"
    };
    format!("[{}{}] {}", "█".repeat(filled), "░".repeat(empty), color)
}
