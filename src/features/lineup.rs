use std::path::PathBuf;

pub fn run() -> anyhow::Result<()> {
    let home = dirs::home_dir().unwrap_or_default();
    let claude_dir = home.join(".claude");

    println!("╭─────────────────────────────────────────────────────╮");
    println!("│               CCB — Context Budget                  │");
    println!("├─────────────────────────────────────────────────────┤");

    let pct = context_pct();
    match pct {
        Some(p) => {
            let bar = progress_bar(p);
            println!(
                "│  window  {} {}%{pad}│",
                bar,
                p,
                pad = " ".repeat(16usize.saturating_sub(format!("{}", p).len()))
            );
        }
        None => println!("│  window  unknown (set CCB_CONTEXT_PCT in hook)       │"),
    }

    println!("├──────────────────┬─────────┬───────────────────────┤");
    println!("│ resource         │  tokens │ path                  │");
    println!("├──────────────────┼─────────┼───────────────────────┤");

    let mut rows: Vec<(String, usize, String)> = Vec::new();

    let index_path = claude_dir.join("skills").join("INDEX.md");
    if index_path.exists() {
        let content = std::fs::read_to_string(&index_path).unwrap_or_default();
        let skill_count = content
            .lines()
            .filter(|l| l.starts_with('|') && !l.contains("name") && !l.contains("---"))
            .count();
        rows.push((
            format!("INDEX ({} skills)", skill_count),
            estimate_file_tokens(&index_path),
            "~/.claude/skills/INDEX.md".to_string(),
        ));
    }

    let claude_md = claude_dir.join("CLAUDE.md");
    if claude_md.exists() {
        rows.push((
            "CLAUDE.md".to_string(),
            estimate_file_tokens(&claude_md),
            "~/.claude/CLAUDE.md".to_string(),
        ));
    }

    let rules_dir = claude_dir.join("rules");
    if rules_dir.exists() {
        let mut rule_tokens = 0usize;
        let mut rule_count = 0usize;
        if let Ok(entries) = std::fs::read_dir(&rules_dir) {
            for e in entries.flatten() {
                if e.path().extension().and_then(|s| s.to_str()) == Some("md") {
                    rule_tokens += estimate_file_tokens(&e.path());
                    rule_count += 1;
                }
            }
        }
        if rule_count > 0 {
            rows.push((
                format!("rules ({} files)", rule_count),
                rule_tokens,
                "~/.claude/rules/".to_string(),
            ));
        }
    }

    let log_path = claude_dir.join("ccb_log.jsonl");
    if log_path.exists() {
        let events = std::fs::read_to_string(&log_path)
            .map(|c| c.lines().count())
            .unwrap_or(0);
        rows.push((
            format!("ccb_log ({} events)", events),
            0,
            "~/.claude/ccb_log.jsonl".to_string(),
        ));
    }

    let total: usize = rows.iter().map(|(_, t, _)| t).sum();

    for (name, tokens, path) in &rows {
        let short_path: String = path.chars().take(21).collect();
        if *tokens > 0 {
            println!(
                "│ {:<16} │ {:>7} │ {:<21} │",
                truncate(name, 16),
                tokens,
                short_path
            );
        } else {
            println!(
                "│ {:<16} │    —    │ {:<21} │",
                truncate(name, 16),
                short_path
            );
        }
    }

    println!("├──────────────────┼─────────┼───────────────────────┤");
    println!("│ {:<16} │ {:>7} │ {:<21} │", "ESTIMATED TOTAL", total, "");
    println!("╰──────────────────┴─────────┴───────────────────────╯");

    Ok(())
}

fn estimate_file_tokens(path: &PathBuf) -> usize {
    std::fs::read_to_string(path)
        .map(|s| s.len().div_ceil(4))
        .unwrap_or(0)
}

fn context_pct() -> Option<u8> {
    if let Ok(val) = std::env::var("CCB_CONTEXT_PCT") {
        return val.parse().ok();
    }
    let tokens: Option<u64> = std::env::var("CCB_CTX_TOKENS")
        .ok()
        .and_then(|v| v.parse().ok());
    let max: Option<u64> = std::env::var("CCB_CTX_MAX")
        .ok()
        .and_then(|v| v.parse().ok());
    match (tokens, max) {
        (Some(t), Some(m)) if m > 0 => Some(((t * 100) / m) as u8),
        _ => None,
    }
}

fn progress_bar(pct: u8) -> String {
    let filled = (pct as usize * 10) / 100;
    let empty = 10usize.saturating_sub(filled);
    format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
}

fn truncate(s: &str, max: usize) -> &str {
    let end = s.char_indices().nth(max).map(|(i, _)| i).unwrap_or(s.len());
    &s[..end]
}
