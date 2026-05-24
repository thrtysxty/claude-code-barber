//! context — monitor context window usage and inject safe-stop directives.
//! Designed to run as a PostToolUse hook: reads usage % from env, emits
//! agent-readable directives that Claude Code injects into the next turn.

use crate::cli::ContextCmd;
use crate::utils::progress_bar;

pub fn run(cmd: ContextCmd) -> anyhow::Result<()> {
    match cmd {
        ContextCmd::Show => show(),
        ContextCmd::Clear { threshold } => advise(threshold, "clear"),
        ContextCmd::Compact { threshold } => advise(threshold, "compact"),
    }
}

fn current_pct() -> Option<u8> {
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

fn show() -> anyhow::Result<()> {
    match current_pct() {
        Some(pct) => {
            let bar = progress_bar(pct, 20);
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
        let directive = match action {
            "compact" => format!(
                "\n[CCB CONTEXT DIRECTIVE] Context at {}% (threshold {}%). \
COMPACT REQUIRED: Complete the current tool sequence to a safe boundary \
(file written, gate passed, or search complete). Do NOT compact mid-edit or mid-gate. \
Before running /compact, write one paragraph to /tmp/ccb-handoff.md: \
stories completed, current story state, next story. Then run /compact.\n",
                pct, threshold
            ),
            "clear" => format!(
                "\n[CCB CONTEXT DIRECTIVE] Context at {}% (threshold {}%). \
CONTEXT CRITICAL: Finish the current statement immediately — no new reads, \
no new searches. Write /tmp/ccb-handoff.md (completed, in-progress, next). \
Run /compact now. Resume from handoff note after compaction.\n",
                pct, threshold
            ),
            _ => format!(
                "\n[CCB CONTEXT DIRECTIVE] Context at {}% — run /{} at next safe boundary.\n",
                pct, action
            ),
        };
        print!("{}", directive);
        tracing::warn!(pct, threshold, action, "context: threshold exceeded");
    }
    Ok(())
}
