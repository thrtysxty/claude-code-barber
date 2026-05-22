use crate::cli::TrimArgs;
use crate::log::{estimate_tokens, CompressionEvent};
use std::process::Command;

pub fn run(args: TrimArgs) -> anyhow::Result<()> {
    if args.cmd.is_empty() {
        anyhow::bail!("usage: ccb trim <command> [args...]");
    }
    let out = Command::new(&args.cmd[0]).args(&args.cmd[1..]).output()
        .map_err(|e| anyhow::anyhow!("ccb trim: command not found: {}: {}", &args.cmd[0], e))?;

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
    }
    .record();

    print!("{}", compressed);
    // NOTE: exit code is intentionally swallowed — trim is a context-optimization
    // pass, not a gate. The compressed output (or lack of it) is the signal.
    Ok(())
}

pub fn compress_str(input: &str) -> String {
    let lines: Vec<&str> = input.lines().filter(|l| !is_boilerplate(l)).collect();

    // deduplicate consecutive identical lines
    let mut deduped: Vec<&str> = Vec::with_capacity(lines.len());
    for line in lines {
        if deduped.last() != Some(&line) {
            deduped.push(line);
        }
    }
    deduped.join("\n").trim_matches('\n').to_string()
}

pub fn is_boilerplate(line: &str) -> bool {
    let t = line.trim_start();

    // ── Rust / Cargo ──────────────────────────────────────────────────────────
    // Strip progress/status lines — pure noise, never actionable
    if t.starts_with("Compiling ")
        || t.starts_with("Checking ")
        || t.starts_with("Finished")
        || t.starts_with("Downloading ")
        || t.starts_with("Updating ")
        || t.starts_with("Unpacking ")
        || t.starts_with("Resolving ")
        || t.starts_with("Locking ")
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
        || t == "-- Docs: https://docs.pytest.org/en/latest/how-to/capture-warnings.html"
    {
        return true;
    }
    // Strip test session header line — "=== test session starts ===" is pure delimiter
    if t.starts_with("=") && t.contains("test session starts") {
        return true;
    }
    // Strip "collected N items" — noise before results
    if t.starts_with("collected ") && t.contains(" item") {
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
        || t.starts_with("run `npm fund")
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
        || t.starts_with("---> Running in")
        || t.starts_with("--->")
        || (t.starts_with("Step ") && t.contains("/") && t.contains(" : "))
    {
        return true;
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── is_boilerplate ────────────────────────────────────────────────────────

    // Rust / Cargo
    #[test]
    fn cargo_compiling() {
        assert!(is_boilerplate("Compiling foo v1.0.0"));
    }
    #[test]
    fn cargo_compiling_indented() {
        assert!(is_boilerplate("   Compiling foo"));
    }
    #[test]
    fn cargo_checking() {
        assert!(is_boilerplate("    Checking mylib v0.1.0"));
    }
    #[test]
    fn cargo_finished() {
        assert!(is_boilerplate("Finished release [optimized]"));
    }
    #[test]
    fn cargo_downloading() {
        assert!(is_boilerplate("   Downloading crate foo"));
    }
    #[test]
    fn cargo_updating() {
        assert!(is_boilerplate("    Updating crates.io index"));
    }
    #[test]
    fn cargo_warning_summary() {
        assert!(is_boilerplate("warning: `ccb` generated 3 warnings"));
    }
    #[test]
    fn cargo_note_warn_default() {
        assert!(is_boilerplate(
            "= note: `#[warn(unused_variables)]` on by default"
        ));
    }
    #[test]
    fn cargo_hint() {
        assert!(is_boilerplate("hint: use --verbose for more info"));
    }
    #[test]
    fn cargo_help_fix() {
        assert!(is_boilerplate(
            "= help: run `cargo fix` to apply suggestion"
        ));
    }

    // Errors and real signal — must NOT be filtered
    #[test]
    fn cargo_error_kept() {
        assert!(!is_boilerplate("error[E0382]: use of moved value"));
    }
    #[test]
    fn cargo_file_line_kept() {
        assert!(!is_boilerplate("src/main.rs:10:5"));
    }
    #[test]
    fn empty_line_kept() {
        assert!(!is_boilerplate(""));
    }
    #[test]
    fn whitespace_only_kept() {
        assert!(!is_boilerplate("  "));
    }

    // Python / pytest
    #[test]
    fn pytest_cachedir() {
        assert!(is_boilerplate("cachedir: .pytest_cache"));
    }
    #[test]
    fn pytest_rootdir() {
        assert!(is_boilerplate("rootdir: /home/user/project"));
    }
    #[test]
    fn pytest_platform() {
        assert!(is_boilerplate("platform linux -- Python 3.11.2"));
    }
    #[test]
    fn pytest_collecting() {
        assert!(is_boilerplate("collecting ..."));
    }
    #[test]
    fn pytest_site_packages() {
        assert!(is_boilerplate("/usr/lib/python3/site-packages/foo.py:12"));
    }
    #[test]
    fn pytest_site_packages_with_failed_kept() {
        assert!(!is_boilerplate("/site-packages/bar.py: FAILED"));
    }

    #[test]
    fn pytest_session_header() {
        assert!(is_boilerplate(
            "============================= test session starts =============================="
        ));
    }
    #[test]
    fn pytest_session_header_kept_if_results() {
        assert!(!is_boilerplate("============================== 2 failed, 45 passed in 1.23s =============================="));
    }
    #[test]
    fn pytest_collected_items() {
        assert!(is_boilerplate("collected 47 items"));
    }
    #[test]
    fn pytest_collected_one_item() {
        assert!(is_boilerplate("collected 1 item"));
    }

    // npm / Node
    #[test]
    fn npm_warn_deprecated() {
        assert!(is_boilerplate("npm warn deprecated lodash@4.0.0"));
    }
    #[test]
    fn npm_notice() {
        assert!(is_boilerplate("npm notice created a lockfile"));
    }
    #[test]
    fn npm_audited() {
        assert!(is_boilerplate("added 120 packages, audited 300 packages"));
    }
    #[test]
    fn npm_up_to_date() {
        assert!(is_boilerplate("up to date, audited 120 packages in 1s"));
    }
    #[test]
    fn npm_funding() {
        assert!(is_boilerplate("153 packages are looking for funding"));
    }

    // Git
    #[test]
    fn git_counting() {
        assert!(is_boilerplate("remote: Counting objects: 5, done."));
    }
    #[test]
    fn git_compressing() {
        assert!(is_boilerplate("remote: Compressing objects: 100%"));
    }
    #[test]
    fn git_receiving() {
        assert!(is_boilerplate("Receiving objects: 100% (5/5)"));
    }
    #[test]
    fn git_resolving() {
        assert!(is_boilerplate("Resolving deltas: 100% (2/2)"));
    }

    // Docker
    #[test]
    fn docker_removing_container() {
        assert!(is_boilerplate("Removing intermediate container abc123"));
    }
    #[test]
    fn docker_running_in() {
        assert!(is_boilerplate(" ---> Running in def456"));
    }
    #[test]
    fn docker_step() {
        assert!(is_boilerplate("Step 3/10 : RUN apt-get install"));
    }

    // ── compress_str ──────────────────────────────────────────────────────────

    #[test]
    fn compress_empty() {
        assert_eq!(compress_str(""), "");
    }

    #[test]
    fn compress_single_signal_line() {
        assert_eq!(compress_str("actual error"), "actual error");
    }

    #[test]
    fn compress_filters_boilerplate() {
        let input = "Compiling foo\nactual error\nFinished release";
        assert_eq!(compress_str(input), "actual error");
    }

    #[test]
    fn compress_deduplicates_consecutive() {
        assert_eq!(compress_str("foo\nfoo\nfoo"), "foo");
    }

    #[test]
    fn compress_keeps_non_consecutive_dupes() {
        assert_eq!(compress_str("foo\nbar\nfoo"), "foo\nbar\nfoo");
    }

    #[test]
    fn compress_mixed_boilerplate_and_dupes() {
        let input = "Compiling foo\nerror: bad\nerror: bad\nFinished release";
        assert_eq!(compress_str(input), "error: bad");
    }
}

#[cfg(test)]
mod fixture_tests {
    use super::*;
    use crate::log::estimate_tokens;

    #[test]
    fn fixture_cargo_build_with_error() {
        let input = "\
   Compiling serde v1.0.197\n\
   Compiling serde_derive v1.0.197\n\
   Compiling anyhow v1.0.86\n\
   Compiling ccb v0.1.0 (/home/user/ccb)\n\
error[E0308]: mismatched types\n\
 --> src/main.rs:42:18\n\
  |\n42|     let x: u32 = \"hello\";\n\
  |            ---   ^^^^^^^ expected `u32`, found `&str`\n\
error: aborting due to 1 previous error\n\
   Finished dev [unoptimized + debuginfo] target(s) in 3.14s";

        let expected = "\
error[E0308]: mismatched types\n\
 --> src/main.rs:42:18\n\
  |\n42|     let x: u32 = \"hello\";\n\
  |            ---   ^^^^^^^ expected `u32`, found `&str`\n\
error: aborting due to 1 previous error";

        let output = compress_str(input);
        assert_eq!(output, expected);
        assert_eq!(estimate_tokens(input), 90); // before
        assert_eq!(estimate_tokens(&output), 45); // after — 50% reduction
    }

    #[test]
    fn fixture_npm_install_clean() {
        let input = "\
npm warn deprecated inflight@1.0.6: This module is not supported\n\
npm warn deprecated glob@7.2.3: Glob versions prior to v9 are no longer supported\n\
npm warn deprecated rimraf@3.0.2: Rimraf versions prior to v4 are no longer supported\n\
added 312 packages, audited 313 packages in 8s\n\
3 packages are looking for funding\n\
  run `npm fund` for details\n\
found 0 vulnerabilities";

        let expected = "found 0 vulnerabilities";

        let output = compress_str(input);
        assert_eq!(output, expected);
        assert_eq!(estimate_tokens(input), 92); // before
        assert_eq!(estimate_tokens(&output), 6); // after — 94% reduction
    }

    #[test]
    fn fixture_pytest_failures_surfaced() {
        let input = "\
============================= test session starts ==============================\n\
platform darwin -- Python 3.11.8, pytest-8.1.1, pluggy-1.4.0\n\
rootdir: /Users/user/project\n\
configfile: pyproject.toml\n\
plugins: anyio-4.3.0, cov-5.0.0\n\
collecting ...\n\
collected 47 items\n\
\n\
FAILED tests/test_api.py::test_create_story - AssertionError: 404\n\
FAILED tests/test_api.py::test_update_story - AssertionError: 500\n\
\n\
============================== 2 failed, 45 passed in 1.23s ==============================";

        let expected = "\
FAILED tests/test_api.py::test_create_story - AssertionError: 404\n\
FAILED tests/test_api.py::test_update_story - AssertionError: 500\n\
\n\
============================== 2 failed, 45 passed in 1.23s ==============================";

        let output = compress_str(input);
        assert_eq!(output, expected);
        assert_eq!(estimate_tokens(input), 122); // before
        assert_eq!(estimate_tokens(&output), 56); // after — 54% reduction
    }
}
