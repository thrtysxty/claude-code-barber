use crate::cli::TrimArgs;
use crate::log::{CompressionEvent, estimate_tokens};
use std::process::Command;

pub fn run(args: TrimArgs) -> anyhow::Result<()> {
    if args.cmd.is_empty() {
        anyhow::bail!("usage: ccb trim <command> [args...]");
    }
    let out = Command::new(&args.cmd[0])
        .args(&args.cmd[1..])
        .output()?;

    // merge stderr — most build tools write noise to stderr
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let combined = if stderr.is_empty() {
        stdout.into_owned()
    } else if stdout.is_empty() {
        stderr.into_owned()
    } else {
        format!("{}{}", stdout, stderr)
    };

    let compressed = compress_str(&combined);

    CompressionEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        feature: "trim".to_string(),
        command: args.cmd.join(" "),
        tokens_in: estimate_tokens(&combined),
        tokens_out: estimate_tokens(&compressed),
        bytes_in: combined.len(),
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

pub fn is_boilerplate(line: &str) -> bool {
    let t = line.trim_start();

    // ── Rust / Cargo ──────────────────────────────────────────────────────────
    // Strip progress/status lines — pure noise, never actionable
    if t.starts_with("Compiling ")
        || t.starts_with("   Compiling")
        || t.starts_with("    Checking")
        || t.starts_with("Finished")
        || t.starts_with("   Downloading")
        || t.starts_with("    Updating")
        || t.starts_with("   Unpacking")
        || t.starts_with("   Resolving")
        || t.starts_with("     Locking")
    {
        return true;
    }
    // Strip the warning summary count ("warning: `ccb` generated 5 warnings")
    // but NOT specific warning messages — those name the problem and are signal
    if t.starts_with("warning: `") && t.contains("generated") && t.contains("warning") {
        return true;
    }
    // Strip "= note: `#[warn(...)]` on by default" — explains the lint is enabled,
    // never tells you anything useful about the actual problem
    if t.starts_with("= note: `#[warn(") {
        return true;
    }
    // Strip git-style "hint:" suggestion lines from cargo
    if t.starts_with("hint:") {
        return true;
    }
    // Strip "run `cargo fix`" suggestion lines
    if t.starts_with("= help: ") && t.contains("cargo fix") {
        return true;
    }

    // ── Python / pytest ───────────────────────────────────────────────────────
    // Strip session header lines — tell you nothing about test results
    if t.starts_with("cachedir:")
        || t.starts_with("rootdir:")
        || t.starts_with("configfile:")
        || t.starts_with("plugins:")
        || t.starts_with("collecting ...")
        || (t.starts_with("platform ") && t.contains(" -- Python "))
        || t == "-- Docs: https://docs.pytest.org/en/stable/how-to/capture-warnings.html"
    {
        return true;
    }
    // Strip site-packages paths in warnings summary — these are install paths, not your code
    // Guard: keep lines that also reference FAILED or ERROR (could be signal in rare cases)
    if t.contains("/site-packages/") && !t.contains("FAILED") && !t.contains("ERROR") {
        return true;
    }
    // Strip bare `warnings.warn(` call line — the warning text above it is the signal
    if t == "warnings.warn(" || t.starts_with("warnings.warn(") && t.ends_with("warn(") {
        return true;
    }

    // ── pip ───────────────────────────────────────────────────────────────────
    // Strip download/install progress — only the final "Successfully installed" matters
    if t.starts_with("Requirement already satisfied:")
        || t.starts_with("Using cached ")
        || t.starts_with("Installing collected packages:")
        || (t.starts_with("Downloading ") && (t.contains(".whl") || t.contains(".tar.gz")))
        || (t.starts_with("  Downloading") && t.contains(".whl"))
        || (t.starts_with("  Using cached") && t.contains(".whl"))
        || (t.starts_with("Obtaining ") && t.contains("(from"))
        // progress bar lines (━━━━ or -----)
        || (t.contains("━━━━") && t.contains("MB/s"))
        || (t.contains("----") && t.contains("kB"))
    {
        return true;
    }

    // ── npm / Node ────────────────────────────────────────────────────────────
    // Strip deprecation spam and audit noise — vulnerabilities count is kept
    if t.starts_with("npm warn deprecated")
        || t.starts_with("npm warn")
        || t.starts_with("npm notice")
        || (t.starts_with("added ") && t.contains("packages") && t.contains("audited"))
        || t.starts_with("up to date, audited")
        || (t.starts_with("audited ") && t.contains("packages"))
        || t.starts_with("Run `npm audit")
        || t.starts_with("  run `npm fund")
        || (t.starts_with("153 ") && t.contains("funding"))  // "N packages are looking for funding"
        || t.contains("packages are looking for funding")
    {
        return true;
    }

    // ── Git ───────────────────────────────────────────────────────────────────
    // Strip transfer progress — pure network noise
    if t.starts_with("remote: Counting objects")
        || t.starts_with("remote: Compressing objects")
        || t.starts_with("remote: Total")
        || t.starts_with("remote: Enumerating objects")
        || t.starts_with("Receiving objects:")
        || t.starts_with("Resolving deltas:")
        || t.starts_with("Updating files:")
    {
        return true;
    }

    // ── Docker ────────────────────────────────────────────────────────────────
    // Strip intermediate layer noise — final image lines are kept
    if t.starts_with("Removing intermediate container")
        || t.starts_with(" ---> Running in")
        || t.starts_with(" --->")
        || (t.starts_with("Step ") && t.contains("/") && t.contains(" : "))
    {
        return true;
    }

    false
}
