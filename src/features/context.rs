//! context — monitor context window usage, suggest /clear or /compact
//! Designed to run as a PostToolUse hook: reads usage % from env, advises action.

use crate::cli::ContextCmd;

pub fn run(cmd: ContextCmd) -> anyhow::Result<()> {
    match cmd {
        ContextCmd::Show        => show(),
        ContextCmd::Clear   { threshold } => advise(threshold, "clear"),
        ContextCmd::Compact { threshold } => advise(threshold, "compact"),
    }
}

fn current_pct() -> Option<u8> {
    // Claude Code exposes context usage via CCB_CONTEXT_PCT when hook is wired
    // Falls back to parsing CCB_CTX_TOKENS / CCB_CTX_MAX if available
    if let Ok(val) = std::env::var("CCB_CONTEXT_PCT") {
        return val.parse().ok();
    }
    let tokens: Option<u64> = std::env::var("CCB_CTX_TOKENS").ok().and_then(|v| v.parse().ok());
    let max: Option<u64>    = std::env::var("CCB_CTX_MAX").ok().and_then(|v| v.parse().ok());
    match (tokens, max) {
        (Some(t), Some(m)) if m > 0 => Some(((t * 100) / m) as u8),
        _ => None,
    }
}

fn show() -> anyhow::Result<()> {
    match current_pct() {
        Some(pct) => {
            let bar = progress_bar(pct);
            println!("context: {}% {}", pct, bar);
            tracing::info!(pct, "context: usage");
        }
        None => println!("context: usage unknown (set CCB_CONTEXT_PCT in hook)"),
    }
    Ok(())
}

fn advise(threshold: u8, action: &str) -> anyhow::Result<()> {
    let pct = match current_pct() {
        Some(p) => p,
        None => {
            tracing::debug!("context: usage unknown, skipping {} check", action);
            return Ok(());
        }
    };

    if pct >= threshold {
        // Output is read by PostToolUse hook and injected into Claude's context
        println!(
            "\n⚠️  ccb context: {}% used (threshold {}%) — consider /{}\n",
            pct, threshold, action
        );
        tracing::warn!(pct, threshold, action, "context: threshold exceeded");
    }
    Ok(())
}

fn progress_bar(pct: u8) -> String {
    let filled = (pct as usize * 20) / 100;
    let empty  = 20usize.saturating_sub(filled);
    let color  = if pct >= 80 { "🔴" } else if pct >= 60 { "🟡" } else { "🟢" };
    format!("[{}{}] {}", "█".repeat(filled), "░".repeat(empty), color)
}
