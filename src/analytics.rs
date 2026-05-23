use crate::log::CompressionEvent;
use std::collections::HashMap;

pub fn gain() -> anyhow::Result<()> {
    let events = load_events();

    if events.is_empty() {
        println!("╭─────────────────────────────────────────╮");
        println!("│  No sessions logged yet.                │");
        println!("│  Run: ccb trim <command>                │");
        println!("╰─────────────────────────────────────────╯");
        return Ok(());
    }

    let mut by_feature: HashMap<&str, (usize, usize, usize)> = HashMap::new();

    for e in &events {
        let entry = by_feature.entry(&e.feature as &str).or_insert((0, 0, 0));
        entry.0 += e.tokens_in;
        entry.1 += e.tokens_out;
        entry.2 += 1;
    }

    let total_in: usize = events.iter().map(|e| e.tokens_in).sum();
    let total_out: usize = events.iter().map(|e| e.tokens_out).sum();
    let saved = total_in.saturating_sub(total_out);
    let pct = (saved * 100).checked_div(total_in).unwrap_or(0);

    println!("╭──────────────────────────────────────────────────╮");
    println!("│               CCB — Token Savings                │");
    println!("├──────────────┬──────────┬──────────┬────────────┤");
    println!("│ feature      │ tokens↓  │ tokens↑  │ saved      │");
    println!("├──────────────┼──────────┼──────────┼────────────┤");

    let mut rows: Vec<_> = by_feature.iter().collect();
    rows.sort_by_key(|(k, _)| *k);
    for (feat, (tin, tout, ops)) in &rows {
        let s = tin.saturating_sub(*tout);
        let p = if *tin > 0 { s * 100 / tin } else { 0 };
        println!(
            "│ {:<12} │ {:>8} │ {:>8} │ {:>7}  {}%  │",
            feat, tin, tout, s, p
        );
        let _ = ops;
    }

    println!("├──────────────┼──────────┼──────────┼────────────┤");
    println!(
        "│ {:<12} │ {:>8} │ {:>8} │ {:>7}  {}%  │",
        "TOTAL", total_in, total_out, saved, pct
    );
    println!("╰──────────────┴──────────┴──────────┴────────────╯");
    println!("  {} operations logged", events.len());

    Ok(())
}

fn load_events() -> Vec<CompressionEvent> {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("ccb_log.jsonl");

    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };

    content
        .lines()
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}
