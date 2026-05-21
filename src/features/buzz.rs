//! buzz — nuclear option: maximum token reduction across all features

use crate::log::{CompressionEvent, estimate_tokens};

pub fn run() -> anyhow::Result<()> {
    println!("⚡ ccb buzz: nuclear mode");
    println!();

    let home = dirs::home_dir().unwrap_or_default();
    let claude_dir = home.join(".claude");

    let mut total_saved = 0usize;

    // Strip INDEX.md of everything except the table (comments, blank lines)
    let index_path = claude_dir.join("skills").join("INDEX.md");
    if index_path.exists() {
        let content = std::fs::read_to_string(&index_path)?;
        let before = content.len();
        let stripped: String = content.lines()
            .filter(|l| l.starts_with('|') || l.starts_with("# Skills"))
            .collect::<Vec<_>>()
            .join("\n") + "\n";
        let after = stripped.len();
        let saved = before.saturating_sub(after);
        if saved > 0 {
            std::fs::write(&index_path, &stripped)?;
            println!("  ✂  INDEX.md  -{} bytes (~{} tokens)", saved, estimate_tokens(&" ".repeat(saved)));
            total_saved += saved;

            CompressionEvent {
                timestamp: chrono::Utc::now().to_rfc3339(),
                feature: "buzz".to_string(),
                command: "INDEX.md strip".to_string(),
                tokens_in: estimate_tokens(&content),
                tokens_out: estimate_tokens(&stripped),
                bytes_in: before,
                bytes_out: after,
            }.record();
        }
    }

    // Prune ccb_log.jsonl to last 500 events
    let log_path = claude_dir.join("ccb_log.jsonl");
    if log_path.exists() {
        let content = std::fs::read_to_string(&log_path)?;
        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        if total > 500 {
            let keep = &lines[total - 500..];
            let pruned = keep.join("\n") + "\n";
            std::fs::write(&log_path, &pruned)?;
            println!("  ✂  ccb_log.jsonl  pruned {} old events", total - 500);
        }
    }

    println!();
    if total_saved > 0 {
        println!("buzz complete — reclaimed ~{} tokens from overhead", estimate_tokens(&" ".repeat(total_saved)));
    } else {
        println!("buzz complete — nothing to prune");
    }

    Ok(())
}
