use crate::log::CompressionEvent;
use std::collections::HashMap;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy)]
pub enum GainMode {
    Default,
    AbTest,
    ExpertDelta,
}

#[allow(dead_code)]
pub fn gain(mode: GainMode) -> anyhow::Result<()> {
    let events = load_events();

    if events.is_empty() {
        println!("╭─────────────────────────────────────────╮");
        println!("│  No sessions logged yet.                │");
        println!("│  Run: ccb trim <command>                │");
        println!("╰─────────────────────────────────────────╯");
        return Ok(());
    }

    match mode {
        GainMode::AbTest => gain_ab(&events),
        GainMode::ExpertDelta => gain_expert_delta(&events),
        GainMode::Default => gain_default(&events),
    }
}

#[allow(dead_code)]
fn gain_default(events: &[CompressionEvent]) -> anyhow::Result<()> {
    let mut by_feature: HashMap<&str, (usize, usize, usize)> = HashMap::new();

    for e in events {
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

#[allow(dead_code)]
fn gain_ab(events: &[CompressionEvent]) -> anyhow::Result<()> {
    let ccb_events: Vec<_> = events
        .iter()
        .filter(|e| e.mode.as_deref() != Some("bypass"))
        .collect();
    let bypass_events: Vec<_> = events
        .iter()
        .filter(|e| e.mode.as_deref() == Some("bypass"))
        .collect();

    let ccb_total_in: usize = ccb_events.iter().map(|e| e.tokens_in).sum();
    let ccb_total_out: usize = ccb_events.iter().map(|e| e.tokens_out).sum();
    let ccb_saved = ccb_total_in.saturating_sub(ccb_total_out);
    #[allow(unknown_lints, clippy::manual_checked_ops)]
    let ccb_pct = if ccb_total_in > 0 {
        ccb_saved * 100 / ccb_total_in
    } else {
        0
    };
    let ccb_avg_in = if !ccb_events.is_empty() {
        ccb_total_in / ccb_events.len()
    } else {
        0
    };
    let ccb_avg_out = if !ccb_events.is_empty() {
        ccb_total_out / ccb_events.len()
    } else {
        0
    };

    let bp_total_in: usize = bypass_events.iter().map(|e| e.tokens_in).sum();
    let bp_total_out: usize = bypass_events.iter().map(|e| e.tokens_out).sum();
    let bp_saved = bp_total_in.saturating_sub(bp_total_out);
    #[allow(unknown_lints, clippy::manual_checked_ops)]
    let _bp_pct = if bp_total_in > 0 {
        bp_saved * 100 / bp_total_in
    } else {
        0
    };
    let bp_avg_in = if !bypass_events.is_empty() {
        bp_total_in / bypass_events.len()
    } else {
        0
    };
    let bp_avg_out = if !bypass_events.is_empty() {
        bp_total_out / bypass_events.len()
    } else {
        0
    };

    println!("╭──────────────────────────────────────────────────────────────╮");
    println!("│                 CCB — A/B Comparison                      │");
    println!("├────────────┬──────────────┬──────────────┬───────────────┤");
    println!("│ mode       │ avg tokens↓  │ avg tokens↑  │ avg saved     │");
    println!("├────────────┼──────────────┼──────────────┼───────────────┤");
    println!(
        "│ {:<10} │ {:>12} │ {:>12} │ {:>10}  {}%   │",
        "ccb", ccb_avg_in, ccb_avg_out, ccb_saved, ccb_pct
    );
    println!(
        "│ {:<10} │ {:>12} │ {:>12} │ {:>10}   0%   │",
        "bypass", bp_avg_in, bp_avg_out, bp_saved
    );
    println!("╰────────────┴──────────────┴──────────────┴───────────────╯");
    println!(
        "  ccb: {} events   bypass: {} events",
        ccb_events.len(),
        bypass_events.len()
    );

    if bypass_events.len() < 2 {
        println!("  Not enough bypass sessions — run with CCB_BYPASS=1 to generate baseline.");
    }

    Ok(())
}

#[allow(dead_code)]
fn gain_expert_delta(events: &[CompressionEvent]) -> anyhow::Result<()> {
    let expert_events: Vec<_> = events.iter().filter(|e| e.persona.is_some()).collect();
    let no_expert_events: Vec<_> = events.iter().filter(|e| e.persona.is_none()).collect();

    let exp_total_in: usize = expert_events.iter().map(|e| e.tokens_in).sum();
    let exp_total_out: usize = expert_events.iter().map(|e| e.tokens_out).sum();
    let exp_saved = exp_total_in.saturating_sub(exp_total_out);
    #[allow(unknown_lints, clippy::manual_checked_ops)]
    let exp_pct = if exp_total_in > 0 {
        exp_saved * 100 / exp_total_in
    } else {
        0
    };
    let exp_avg_in = if !expert_events.is_empty() {
        exp_total_in / expert_events.len()
    } else {
        0
    };
    let exp_avg_out = if !expert_events.is_empty() {
        exp_total_out / expert_events.len()
    } else {
        0
    };

    let no_exp_total_in: usize = no_expert_events.iter().map(|e| e.tokens_in).sum();
    let no_exp_total_out: usize = no_expert_events.iter().map(|e| e.tokens_out).sum();
    let no_exp_saved = no_exp_total_in.saturating_sub(no_exp_total_out);
    #[allow(unknown_lints, clippy::manual_checked_ops)]
    let no_exp_pct = if no_exp_total_in > 0 {
        no_exp_saved * 100 / no_exp_total_in
    } else {
        0
    };
    let no_exp_avg_in = if !no_expert_events.is_empty() {
        no_exp_total_in / no_expert_events.len()
    } else {
        0
    };
    let no_exp_avg_out = if !no_expert_events.is_empty() {
        no_exp_total_out / no_expert_events.len()
    } else {
        0
    };

    println!("╭─────────────────────────────────────────────────────────────╮");
    println!("│             CCB — Expert Injection Delta                   │");
    println!("├──────────────────┬────────────┬────────────┬─────────────┤");
    println!("│ condition       │ avg tok↓   │ avg tok↑   │ avg saved  │");
    println!("├──────────────────┼────────────┼────────────┼─────────────┤");
    println!(
        "│ {:<16} │ {:>10} │ {:>10} │ {:>9}  {}% │",
        "expert active", exp_avg_in, exp_avg_out, exp_saved, exp_pct
    );
    println!(
        "│ {:<16} │ {:>10} │ {:>10} │ {:>9}  {}% │",
        "no expert", no_exp_avg_in, no_exp_avg_out, no_exp_saved, no_exp_pct
    );
    println!("╰──────────────────┴────────────┴────────────┴─────────────╯");
    println!(
        "  expert: {} events   no expert: {} events",
        expert_events.len(),
        no_expert_events.len()
    );

    // Top 3 domains
    let mut domain_counts: HashMap<&str, usize> = HashMap::new();
    for e in &expert_events {
        if let Some(ref domains) = e.domains_hit {
            for d in domains {
                *domain_counts.entry(d.as_str()).or_insert(0) += 1;
            }
        }
    }
    let mut top_domains: Vec<_> = domain_counts.iter().collect();
    top_domains.sort_by(|a, b| b.1.cmp(a.1));
    if !top_domains.is_empty() {
        println!(
            "
Top domains:"
        );
        for (domain, count) in top_domains.iter().take(3) {
            println!("  {} — {} hits", domain, count);
        }
    }

    Ok(())
}

#[allow(dead_code)]
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
