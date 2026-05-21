use crate::cli::TrimArgs;
use crate::log::{CompressionEvent, estimate_tokens};
use std::process::Command;

pub fn run(args: TrimArgs) -> anyhow::Result<()> {
    if args.cmd.is_empty() {
        anyhow::bail!("usage: ccb trim <command> [args...]");
    }
    let output = Command::new(&args.cmd[0])
        .args(&args.cmd[1..])
        .output()?;

    let raw = String::from_utf8_lossy(&output.stdout);
    let compressed = compress_str(&raw);

    CompressionEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        feature: "trim".to_string(),
        command: args.cmd.join(" "),
        tokens_in: estimate_tokens(&raw),
        tokens_out: estimate_tokens(&compressed),
        bytes_in: raw.len(),
        bytes_out: compressed.len(),
    }.record();

    print!("{}", compressed);
    Ok(())
}

pub fn compress_str(input: &str) -> String {
    let lines: Vec<&str> = input.lines()
        .filter(|l| !is_boilerplate(l))
        .collect();

    // deduplicate consecutive identical lines
    let mut deduped: Vec<&str> = Vec::with_capacity(lines.len());
    for line in lines {
        if deduped.last() != Some(&line) {
            deduped.push(line);
        }
    }
    deduped.join("\n")
}

fn is_boilerplate(line: &str) -> bool {
    let t = line.trim_start();
    t.starts_with("hint:")
        || t.starts_with("warning: unused")
        || t.starts_with("Compiling ")
        || t.starts_with("   Compiling")
        || t.starts_with("    Finished")
        || t.starts_with("hint: use")
}
