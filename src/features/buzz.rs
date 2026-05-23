//! buzz — nuclear option: maximum token reduction across all features

use crate::log::{estimate_tokens, CompressionEvent};

pub(crate) fn strip_index(content: &str) -> String {
    content
        .lines()
        .filter(|l| l.starts_with('|') || l.starts_with("# Skills"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub(crate) fn prune_log(content: &str, keep: usize) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if keep == 0 || lines.is_empty() {
        return String::new();
    }
    if lines.len() <= keep {
        return content.to_string();
    }
    lines[lines.len() - keep..].join("\n") + "\n"
}

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
        let stripped = strip_index(&content);
        let after = stripped.len();
        let saved = before.saturating_sub(after);
        if saved > 0 {
            std::fs::write(&index_path, &stripped)?;
            println!(
                "  ✂  INDEX.md  -{} bytes (~{} tokens)",
                saved,
                estimate_tokens(&" ".repeat(saved))
            );
            total_saved += saved;

            CompressionEvent {
                timestamp: chrono::Utc::now().to_rfc3339(),
                feature: "buzz".to_string(),
                command: "INDEX.md strip".to_string(),
                tokens_in: estimate_tokens(&content),
                tokens_out: estimate_tokens(&stripped),
                bytes_in: before,
                bytes_out: after,
            }
            .record();
        }
    }

    // Prune ccb_log.jsonl to last 500 events
    let log_path = claude_dir.join("ccb_log.jsonl");
    if log_path.exists() {
        let content = std::fs::read_to_string(&log_path)?;
        let pruned = prune_log(&content, 500);
        let total = content.lines().count();
        let new_total = pruned.lines().count();
        if new_total < total {
            std::fs::write(&log_path, &pruned)?;
            println!(
                "  ✂  ccb_log.jsonl  pruned {} old events",
                total - new_total
            );
        }
    }

    println!();
    if total_saved > 0 {
        println!(
            "buzz complete — reclaimed ~{} tokens from overhead",
            estimate_tokens(&" ".repeat(total_saved))
        );
    } else {
        println!("buzz complete — nothing to prune");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_index_keeps_table_rows() {
        let input = "Some prose\n| Skill | Description |\n|------|-------------|\n| foo | bar |\nMore prose";
        let out = strip_index(input);
        assert!(out.contains("| Skill | Description |"));
        assert!(out.contains("| foo | bar |"));
        assert!(!out.contains("Some prose"));
        assert!(!out.contains("More prose"));
    }

    #[test]
    fn strip_index_keeps_skills_header() {
        let input = "# Skills\n| x | y |\n";
        let out = strip_index(input);
        assert!(out.contains("# Skills"));
        assert!(out.contains("| x | y |"));
    }

    #[test]
    fn strip_index_removes_blank_lines() {
        let input = "\n\n| a | b |\n\n";
        let out = strip_index(input);
        assert!(!out.contains("\n\n"));
        assert!(out.contains("| a | b |"));
    }

    #[test]
    fn strip_index_removes_comments() {
        let input = "<!-- comment -->\n| x | y |\n<!-- another -->";
        let out = strip_index(input);
        assert!(!out.contains("<!-- comment -->"));
        assert!(!out.contains("<!-- another -->"));
        assert!(out.contains("| x | y |"));
    }

    #[test]
    fn strip_index_empty_input() {
        let out = strip_index("");
        assert_eq!(out, "\n");
    }

    #[test]
    fn prune_log_under_limit_unchanged() {
        let ten_lines: String = (0..10)
            .map(|i| format!("{{\"line\":{}}}", i))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        let out = prune_log(&ten_lines, 500);
        assert_eq!(out, ten_lines);
    }

    #[test]
    fn prune_log_over_limit_keeps_tail() {
        let lines: Vec<String> = (0..600).map(|i| format!("{{\"line\":{}}}", i)).collect();
        let content: String = lines.join("\n") + "\n";
        let out = prune_log(&content, 500);
        assert!(out.contains("\"line\":100"));
        assert!(!out.contains("\"line\":99"));
        assert_eq!(out.lines().count(), 500);
    }

    #[test]
    fn prune_log_exact_limit_unchanged() {
        let lines: Vec<String> = (0..500).map(|i| format!("{{\"line\":{}}}", i)).collect();
        let content: String = lines.join("\n") + "\n";
        let out = prune_log(&content, 500);
        assert_eq!(out.lines().count(), 500);
    }

    #[test]
    fn prune_log_empty_input() {
        let out = prune_log("", 500);
        assert_eq!(out, "");
    }

    #[test]
    fn prune_log_keep_zero() {
        let content = "# log entry 1
# log entry 2
";
        let out = prune_log(content, 0);
        assert_eq!(out, "");
    }
}
