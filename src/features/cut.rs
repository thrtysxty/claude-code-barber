//! cut — run all active features at maximum compression

use crate::cli::ContextCmd;

pub fn run() -> anyhow::Result<()> {
    println!("─── ccb cut: maximum compression ───");

    // 1. Context check (compact at 50%)
    let _ = crate::features::context::run(ContextCmd::Compact { threshold: 50 });

    // 2. Show lineup
    println!();
    crate::features::lineup::run()?;

    println!();
    println!("tip: pipe commands through `ccb trim <cmd>` to compress output");
    println!("tip: run `ccb fade <skill>` to lazy-load a skill on demand");

    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn cut_run_completes_without_error() {
        // Smoke test: run() should succeed even if lineup/context fail gracefully
        let result = crate::features::cut::run();
        assert!(result.is_ok());
    }
}
