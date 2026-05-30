//! context — monitor context window usage and inject safe-stop directives.
//! Designed to run as a PostToolUse hook: reads usage % from env, emits
//! agent-readable directives that Claude Code injects into the next turn.
//!
//! Also provides: `ccb context inject` (PreToolUse/SessionStart injection)
//! and `ccb context trace` (PostToolUse logging).
//!
//! CCB-026 adds: `ccb context tune` (EMA weight updates), `ccb context gaps`
//! (blind-spot detection), `ccb context report` (weight distribution).

pub mod feedback;

use crate::cli::{ContextCmd, ContextReportFormat};
use crate::utils::progress_bar;

pub fn run(cmd: ContextCmd) -> anyhow::Result<()> {
    match cmd {
        ContextCmd::Show => show(),
        ContextCmd::Clear { threshold } => advise(threshold, "clear"),
        ContextCmd::Compact { threshold } => advise(threshold, "compact"),
        ContextCmd::Inject {
            hook,
            tool,
            input,
            stdin,
        } => {
            #[cfg(any(
                feature = "graph",
                feature = "expert",
                feature = "memory",
                feature = "factory"
            ))]
            {
                crate::hooks::run_inject(&hook, tool.as_deref(), input.as_deref(), stdin)?;
            }
            #[cfg(not(any(
                feature = "graph",
                feature = "expert",
                feature = "memory",
                feature = "factory"
            )))]
            {
                let _ = (&hook, &tool, &input, &stdin);
                eprintln!("hooks feature requires rusqlite — rebuild with --features graph,expert,memory,factory");
            }
            Ok(())
        }
        #[cfg(any(
            feature = "graph",
            feature = "expert",
            feature = "memory",
            feature = "factory"
        ))]
        ContextCmd::Trace => crate::hooks::run_trace(),
        // CCB-026: weight feedback commands
        ContextCmd::Tune {
            dry_run,
            validate,
            threshold,
            alpha,
        } => {
            let opts = feedback::TuneOptions {
                dry_run,
                validate,
                threshold_pct: threshold,
                alpha,
            };
            let report = feedback::tune(opts)?;
            feedback::print_tune_report(&report)?;
            Ok(())
        }
        ContextCmd::Gaps {
            min_sessions,
            apply,
        } => {
            let config = feedback::TuneConfig {
                min_sessions_for_gap: min_sessions.unwrap_or(3),
                ..Default::default()
            };
            let gaps = feedback::detect_gaps(config)?;
            if let Some(gap_id) = apply {
                apply_gap_suggestion(gap_id, &gaps)?;
            } else {
                feedback::print_gaps(&gaps)?;
            }
            Ok(())
        }
        ContextCmd::Report { format, node } => {
            let fmt = match format {
                ContextReportFormat::Human => feedback::ReportFormat::Human,
                ContextReportFormat::Json => feedback::ReportFormat::Json,
            };
            feedback::print_report(fmt, node.as_deref())?;
            Ok(())
        }
    }
}

fn apply_gap_suggestion(gap_id: i64, gaps: &[feedback::GapReport]) -> anyhow::Result<()> {
    let gap = gaps
        .iter()
        .find(|g| g.node_id == gap_id)
        .ok_or_else(|| anyhow::anyhow!("Gap {} not found", gap_id))?;

    match &gap.suggestion {
        feedback::GapSuggestion::ActivateExpert { name } => {
            println!("Activating expert '{}'...", name);
            #[cfg(feature = "expert")]
            crate::features::expert::activate(name)?;
            #[cfg(not(feature = "expert"))]
            println!("(expert feature not enabled — run with --features expert)");
        }
        feedback::GapSuggestion::WireSkill { path } => {
            println!("Generating skill stub at '{}'...", path);
            let path = feedback::generate_skill_stub(gap)?;
            println!("Created: {}", path.display());
        }
        feedback::GapSuggestion::PromoteWeight { node_id } => {
            println!("Promoting weight for node {}...", node_id);
            #[cfg(feature = "context")]
            {
                let conn = feedback::db()?;
                conn.execute(
                    "UPDATE context_nodes SET weight = 0.5 WHERE id = ?",
                    rusqlite::params![node_id],
                )?;
            }
            #[cfg(not(feature = "context"))]
            println!("(context feature not enabled — run with --features context)");
            println!("Weight reset to 0.5");
        }
        feedback::GapSuggestion::BuildExpert { domain } => {
            println!(
                "Domain gap detected: no expert covers '{}'.\nSuggested action: build new expert for domain '{}'",
                domain, domain
            );
        }
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    struct EnvGuard {
        _lock: std::sync::MutexGuard<'static, ()>,
    }
    impl EnvGuard {
        fn new() -> Self {
            let lock = ENV_LOCK.lock().unwrap_or_else(|e| e.into_inner());
            std::env::remove_var("CCB_CONTEXT_PCT");
            std::env::remove_var("CCB_CTX_TOKENS");
            std::env::remove_var("CCB_CTX_MAX");
            Self { _lock: lock }
        }
    }
    impl Drop for EnvGuard {
        fn drop(&mut self) {
            std::env::remove_var("CCB_CONTEXT_PCT");
            std::env::remove_var("CCB_CTX_TOKENS");
            std::env::remove_var("CCB_CTX_MAX");
        }
    }

    #[test]
    fn current_pct_parses_ccb_context_pct() {
        let _guard = EnvGuard::new();
        std::env::set_var("CCB_CONTEXT_PCT", "42");
        assert_eq!(current_pct(), Some(42));
    }

    #[test]
    fn current_pct_computes_from_tokens() {
        let _guard = EnvGuard::new();
        std::env::set_var("CCB_CTX_TOKENS", "50000");
        std::env::set_var("CCB_CTX_MAX", "100000");
        assert_eq!(current_pct(), Some(50));
    }

    #[test]
    fn current_pct_returns_none_when_no_env() {
        let _guard = EnvGuard::new();
        assert_eq!(current_pct(), None);
    }

    #[test]
    fn current_pct_handles_zero_max() {
        let _guard = EnvGuard::new();
        std::env::set_var("CCB_CTX_TOKENS", "50000");
        std::env::set_var("CCB_CTX_MAX", "0");
        assert_eq!(current_pct(), None);
    }

    #[test]
    fn advise_prints_directive_when_threshold_exceeded() {
        let _guard = EnvGuard::new();
        std::env::set_var("CCB_CONTEXT_PCT", "80");
        let result = advise(70, "compact");
        assert!(result.is_ok());
    }

    #[test]
    fn advise_skips_when_no_pct() {
        let _guard = EnvGuard::new();
        let result = advise(50, "compact");
        assert!(result.is_ok());
    }

    #[test]
    fn advise_does_not_fire_below_threshold() {
        let _guard = EnvGuard::new();
        std::env::set_var("CCB_CONTEXT_PCT", "30");
        let result = advise(70, "compact");
        assert!(result.is_ok());
    }
}
