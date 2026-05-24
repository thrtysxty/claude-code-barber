//! Classify — two-tier tool call safety classifier
//!
//! Tier 1: instant local pattern matching (no API call)
//! Tier 2: LLM evaluation via OpenRouter (ambiguous actions only)
//!
//! Reads PreToolUse hook JSON from stdin. Exits silently to allow,
//! prints deny JSON to block.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::process::Command;



const TRANSCRIPT_CHAR_LIMIT: usize = 4000;

static SKIP_TOOLS: &[&str] = &["Read", "Glob", "Grep", "WebSearch", "LS"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Decision {
    Allow,
    Deny,
    Uncertain,
}

#[derive(Debug, Deserialize)]
struct HookPayload {
    tool_name: Option<String>,
    tool_input: Option<serde_json::Value>,
    session_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct DenyOutput {
    #[serde(rename = "hookSpecificOutput")]
    hook_specific_output: DenyInner,
}

#[derive(Debug, Serialize)]
struct DenyInner {
    #[serde(rename = "hookEventName")]
    hook_event_name: String,
    #[serde(rename = "permissionDecision")]
    permission_decision: String,
    #[serde(rename = "permissionDecisionReason")]
    permission_decision_reason: String,
}

#[derive(Debug, Deserialize)]
struct AutoModeConfig {
    #[serde(default)]
    allow: Vec<String>,
    #[serde(default)]
    soft_deny: Vec<String>,
    #[serde(default)]
    environment: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ClassifyEvent {
    timestamp: String,
    feature: String,
    tool_name: String,
    tier: String,
    decision: String,
    reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    llm_tokens: Option<usize>,
}

fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

fn projects_root() -> PathBuf {
    home().join("Projects")
}

fn load_secrets_key(key: &str) -> Option<String> {
    let path = home().join(".secrets");
    if !path.exists() {
        return None;
    }
    std::fs::read_to_string(&path).ok().and_then(|content| {
        content.lines().find_map(|line| {
            let line = line.trim();
            if line.starts_with('#') || !line.contains('=') {
                return None;
            }
            let (k, v) = line.split_once('=')?;
            if k.trim() == key {
                Some(v.trim().trim_matches('"').trim_matches('\'').to_string())
            } else {
                None
            }
        })
    })
}

fn load_automode_config() -> Option<AutoModeConfig> {
    let path = home().join(".claude").join("settings.json");
    let content = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    serde_json::from_value(val.get("autoMode")?.clone()).ok()
}

// ---------------------------------------------------------------------------
// Tier 1 — local pattern matching
// ---------------------------------------------------------------------------

fn tier1_classify(tool_name: &str, tool_input: &serde_json::Value) -> (Decision, &'static str) {
    // Skip tools are always allowed
    if SKIP_TOOLS.contains(&tool_name) {
        return (Decision::Allow, "skip tool");
    }

    // Memory writes — always allowed
    if matches!(tool_name, "Write" | "Edit") {
        if let Some(fp) = tool_input
            .get("file_path")
            .or_else(|| tool_input.get("path"))
            .and_then(|v| v.as_str())
        {
            let memory_prefix = home().join(".claude").join("projects");
            if fp.starts_with(memory_prefix.to_str().unwrap_or("")) && fp.contains("/memory/") {
                return (Decision::Allow, "memory write");
            }
        }
    }

    // File writes within project root — allowed
    if matches!(tool_name, "Write" | "Edit") {
        if let Some(fp) = tool_input
            .get("file_path")
            .or_else(|| tool_input.get("path"))
            .and_then(|v| v.as_str())
        {
            let root = projects_root();
            if fp.starts_with(root.to_str().unwrap_or("")) {
                // But not secrets files
                let lower = fp.to_lowercase();
                if lower.ends_with(".env")
                    || lower.contains("credentials")
                    || lower.contains(".secrets")
                {
                    return (Decision::Deny, "writing to secrets file");
                }
                return (Decision::Allow, "file write in project");
            }
        }
    }

    // Bash commands — pattern match
    if tool_name == "Bash" {
        if let Some(cmd) = tool_input.get("command").and_then(|v| v.as_str()) {
            return tier1_bash(cmd);
        }
    }

    (Decision::Uncertain, "needs LLM evaluation")
}

fn tier1_bash(cmd: &str) -> (Decision, &'static str) {
    // Fast-deny: obviously dangerous
    if cmd.contains("curl") && cmd.contains("| bash")
        || cmd.contains("| sh")
        || cmd.contains("| zsh")
    {
        return (Decision::Deny, "pipe to shell");
    }
    if cmd.starts_with("rm -rf ~/")
        || cmd.starts_with("rm -rf /")
        || cmd.contains("rm -rf $HOME")
    {
        return (Decision::Deny, "recursive delete of home/root");
    }
    if cmd.contains(".ssh/authorized_keys") && !cmd.starts_with("cat ") {
        return (Decision::Deny, "modifying SSH authorized_keys");
    }

    // Fast-allow: routine dev commands
    let safe_prefixes = [
        "git status", "git log", "git diff", "git branch", "git fetch",
        "git add", "git commit", "git checkout", "git stash", "git merge",
        "git remote", "git rev-parse", "git symbolic-ref", "git ls-files",
        "cargo build", "cargo test", "cargo check", "cargo clippy", "cargo fmt",
        "npm install", "npm run", "npm test", "npx ",
        "swift build", "swift test",
        "python3 -m pytest", "pytest", "ruff ",
        "make ", "ls ", "wc ", "cat ", "head ", "tail ", "grep ", "find ",
        "which ", "echo ", "printf ", "basename ", "dirname ",
        "gh api", "gh repo", "gh pr", "gh auth",
        "cd ", "pwd",
    ];
    for prefix in safe_prefixes {
        if cmd.starts_with(prefix) || cmd.contains(&format!("&& {prefix}")) {
            // But check for chained dangerous commands
            if cmd.contains("| bash") || cmd.contains("rm -rf") {
                return (Decision::Uncertain, "safe prefix but suspicious chain");
            }
            return (Decision::Allow, "routine dev command");
        }
    }

    // Git push — check specifics
    if cmd.contains("git") && cmd.contains("push") {
        // Force push to main — check if initial repo
        if cmd.contains("--force") || cmd.contains("-f ") {
            if is_initial_repo_push() {
                return (Decision::Allow, "initial repo push");
            }
            return (Decision::Uncertain, "force push needs review");
        }
        // Regular push — uncertain, let LLM check branch
        return (Decision::Uncertain, "git push needs branch check");
    }

    // Write to known infra paths (when explicitly in the allow list)
    if cmd.contains("~/.zshrc")
        || cmd.contains("~/.bashrc")
        || cmd.contains("~/.gitignore_global")
    {
        return (Decision::Uncertain, "shell profile edit");
    }

    (Decision::Uncertain, "unrecognized command")
}

fn is_initial_repo_push() -> bool {
    let output = Command::new("git")
        .args(["log", "--oneline", "origin/main"])
        .output();
    match output {
        Ok(o) => {
            let lines: Vec<&str> = std::str::from_utf8(&o.stdout)
                .unwrap_or("")
                .lines()
                .filter(|l| !l.trim().is_empty())
                .collect();
            lines.len() <= 1
        }
        Err(_) => false,
    }
}

// ---------------------------------------------------------------------------
// Tier 2 — LLM evaluation
// ---------------------------------------------------------------------------

fn tier2_classify(
    tool_name: &str,
    tool_input: &serde_json::Value,
    session_id: &str,
    api_key: &str,
) -> Result<(Decision, String)> {
    let cfg = load_automode_config().unwrap_or(AutoModeConfig {
        allow: vec![],
        soft_deny: vec![],
        environment: vec![],
    });

    let project_root = extract_session_cwd(session_id).unwrap_or_default();
    let system_prompt = build_system_prompt(&cfg, &project_root);
    let transcript = build_transcript(session_id);
    let action = format_action(tool_name, tool_input, &project_root);

    let user_content = format!(
        "<transcript>\n{}\n</transcript>\n\n<action>\n{}\n</action>",
        transcript, action
    );

    let model = std::env::var("CLASSIFIER_MODEL")
        .unwrap_or_else(|_| "mistralai/devstral-small".to_string());
    let timeout_secs: u64 = std::env::var("CLASSIFIER_TIMEOUT")
        .unwrap_or_else(|_| "20".to_string())
        .parse()
        .unwrap_or(20);

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(timeout_secs))
        .build()?;

    let body = serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt},
            {"role": "user", "content": user_content},
        ],
        "temperature": 0,
        "max_tokens": 128,
        "stream": false,
    });

    let resp = client
        .post("https://openrouter.ai/api/v1/chat/completions")
        .header("Authorization", format!("Bearer {api_key}"))
        .header("Content-Type", "application/json")
        .header("HTTP-Referer", "https://github.com/thrtysxty/claude-code-barber")
        .header("X-Title", "CCB classifier")
        .json(&body)
        .send()
        .context("classifier API call failed")?;

    let json: serde_json::Value = resp.json().context("failed to parse classifier response")?;
    let raw = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    parse_llm_decision(&raw)
}

fn parse_llm_decision(text: &str) -> Result<(Decision, String)> {
    let block_re = regex_lite::Regex::new(r"(?i)<block>(yes|no)</block>").unwrap();
    let reason_re = regex_lite::Regex::new(r"(?is)<reason>(.*?)</reason>").unwrap();

    match block_re.captures(text) {
        Some(caps) if caps[1].eq_ignore_ascii_case("yes") => {
            let reason = reason_re
                .captures(text)
                .map(|r| r[1].trim().to_string())
                .unwrap_or_else(|| "blocked by classifier".to_string());
            Ok((Decision::Deny, reason))
        }
        Some(_) => Ok((Decision::Allow, "LLM approved".to_string())),
        None => Ok((Decision::Deny, "unparseable classifier response".to_string())),
    }
}

fn extract_session_cwd(session_id: &str) -> Option<String> {
    if session_id.is_empty() {
        return None;
    }
    let projects_dir = home().join(".claude").join("projects");
    if !projects_dir.exists() {
        return None;
    }
    let target = format!("{session_id}.jsonl");
    for entry in std::fs::read_dir(&projects_dir).ok()? {
        let entry = entry.ok()?;
        if !entry.file_type().ok()?.is_dir() {
            continue;
        }
        let candidate = entry.path().join(&target);
        if candidate.exists() {
            // Read first line for cwd
            let content = std::fs::read_to_string(&candidate).ok()?;
            for line in content.lines() {
                if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                    if let Some(cwd) = event.get("cwd").and_then(|v| v.as_str()) {
                        return Some(cwd.to_string());
                    }
                }
            }
        }
    }
    None
}

fn build_transcript(session_id: &str) -> String {
    if session_id.is_empty() {
        return "(no transcript)".to_string();
    }
    let projects_dir = home().join(".claude").join("projects");
    let target = format!("{session_id}.jsonl");

    for entry in std::fs::read_dir(&projects_dir).into_iter().flatten() {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let candidate = entry.path().join(&target);
        if !candidate.exists() {
            continue;
        }

        let content = match std::fs::read_to_string(&candidate) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut turns = Vec::new();
        for line in content.lines() {
            let event: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if event.get("type").and_then(|v| v.as_str()) != Some("message") {
                continue;
            }
            let msg = match event.get("message") {
                Some(m) => m,
                None => continue,
            };
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            let content_arr = msg.get("content").and_then(|v| v.as_array());

            if role == "human" {
                if let Some(blocks) = content_arr {
                    let text: String = blocks
                        .iter()
                        .filter_map(|b| {
                            if b.get("type").and_then(|v| v.as_str()) == Some("text") {
                                b.get("text").and_then(|v| v.as_str())
                            } else {
                                None
                            }
                        })
                        .collect::<Vec<_>>()
                        .join("");
                    if !text.is_empty() {
                        let truncated: String = text.chars().take(500).collect();
                        turns.push(format!("User: {truncated}"));
                    }
                }
            }
        }

        let mut transcript = turns.join("\n");
        if transcript.len() > TRANSCRIPT_CHAR_LIMIT {
            let start = transcript.len() - TRANSCRIPT_CHAR_LIMIT;
            transcript = format!("...(truncated)\n{}", &transcript[start..]);
        }
        return if transcript.is_empty() {
            "(empty transcript)".to_string()
        } else {
            transcript
        };
    }

    "(session not found)".to_string()
}

fn format_action(tool_name: &str, tool_input: &serde_json::Value, project_root: &str) -> String {
    let scope_note = if project_root.is_empty() {
        "\nProject root: (unknown)".to_string()
    } else {
        format!("\nProject root: {project_root}")
    };

    match tool_name {
        "Bash" => {
            let cmd = tool_input
                .get("command")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if cmd.contains("git") && cmd.contains("push") {
                let diff = Command::new("git")
                    .args(["diff", "origin/main..HEAD", "--stat"])
                    .output()
                    .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                    .unwrap_or_else(|_| "(could not get diff)".to_string());
                let diff_trimmed: String = diff.chars().take(2000).collect();
                format!("Tool: Bash\nCommand: {cmd}\nContent being pushed:\n{diff_trimmed}{scope_note}")
            } else {
                format!("Tool: Bash\nCommand: {cmd}{scope_note}")
            }
        }
        "Write" | "Edit" => {
            let fp = tool_input
                .get("file_path")
                .or_else(|| tool_input.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let scope = if !project_root.is_empty() && fp.starts_with(project_root) {
                " (within project)"
            } else if !project_root.is_empty() {
                " (outside project)"
            } else {
                ""
            };
            format!("Tool: {tool_name}\nFile: {fp}{scope}{scope_note}")
        }
        "Agent" => {
            let prompt = tool_input
                .get("prompt")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let truncated: String = prompt.chars().take(600).collect();
            format!("Tool: Agent\nPrompt: {truncated}{scope_note}")
        }
        _ => {
            let input_str = serde_json::to_string(tool_input).unwrap_or_default();
            let truncated: String = input_str.chars().take(400).collect();
            format!("Tool: {tool_name}\nInput: {truncated}{scope_note}")
        }
    }
}

fn build_system_prompt(cfg: &AutoModeConfig, project_root: &str) -> String {
    let allow_rules: String = cfg.allow.iter().map(|r| format!("- {r}\n")).collect();
    let deny_rules: String = cfg.soft_deny.iter().map(|r| format!("- {r}\n")).collect();
    let env_rules: String = cfg.environment.iter().map(|r| format!("- {r}\n")).collect();

    let scope_rule = if project_root.is_empty() {
        String::new()
    } else {
        format!(
            "\n## Project Scope\n\
             The current project root is: {project_root}\n\
             This is a multi-repo workspace. Sub-directories are independent git repos:\n\
             copernicus/, Atlas/, high-plains-hipster/, shxtlxst/, claude-code-barber/, docmap/\n\
             File ops within ANY of these repos are routine development, NOT scope escalation.\n\
             Writing to ~/.claude/projects/*/memory/ is the agent memory system — always ALLOWED.\n"
        )
    };

    format!(
        "## Default Rule\n\
         **By default, actions are ALLOWED.** Only block if a BLOCK condition matches AND no ALLOW exception applies.\n\
         \n## Scope\n\
         Security-relevant harm only. Loops, wasted tokens, bad output = NOT blockable.\n\
         {scope_rule}\
         \n## User Intent Rule\n\
         1. Distinguish user request from agent action.\n\
         2. Scope escalation = autonomous — evaluate against BLOCK.\n\
         3. High-severity actions require precise intent.\n\
         4. Agent-inferred parameters are not user-intended.\n\
         \n## Evaluation Rules\n\
         - COMPOSITE ACTIONS: blocked part -> block the whole chain.\n\
         - FILE OPS IN PROJECT: within project root = routine dev, NOT exfiltration.\n\
         - PREEMPTIVE BLOCK: bash comments revealing blocked intent -> block.\n\
         \n## Environment\n\
         {env_rules}\
         \n## BLOCK if action does ANY of these\n\
         {deny_rules}\
         \n## ALLOW (exceptions) if ANY apply\n\
         {allow_rules}\
         \n## Output\n\
         Blocked: <block>yes</block><reason>one short sentence</reason>\n\
         Allowed: <block>no</block>\n\
         Response MUST begin with <block>. No preamble."
    )
}

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

fn log_decision(tool_name: &str, tier: &str, decision: Decision, reason: &str) {
    let event = ClassifyEvent {
        timestamp: chrono::Utc::now().to_rfc3339(),
        feature: "classify".to_string(),
        tool_name: tool_name.to_string(),
        tier: tier.to_string(),
        decision: match decision {
            Decision::Allow => "allow",
            Decision::Deny => "deny",
            Decision::Uncertain => "uncertain",
        }
        .to_string(),
        reason: reason.to_string(),
        llm_tokens: None,
    };

    let log_path = home().join(".claude").join("ccb_log.jsonl");
    if let Ok(line) = serde_json::to_string(&event) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            let _ = writeln!(f, "{}", line);
        }
    }
}

fn deny_output(reason: &str) {
    let output = DenyOutput {
        hook_specific_output: DenyInner {
            hook_event_name: "PreToolUse".to_string(),
            permission_decision: "deny".to_string(),
            permission_decision_reason: format!("[ccb] {reason}"),
        },
    };
    if let Ok(json) = serde_json::to_string(&output) {
        println!("{json}");
    }
}


// ---------------------------------------------------------------------------
// Expert context injection (when expert feature is enabled)
// ---------------------------------------------------------------------------

#[cfg(feature = "expert")]
fn inject_expert_context(tool_name: &str) {
    use crate::features::expert;
    match expert::query_active_json() {
        Ok(Some(json)) => {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&json) {
                let persona = parsed.get("persona").and_then(|v| v.as_str()).unwrap_or("unknown");
                let domains = parsed.get("active_domains").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                let patterns = parsed.get("patterns").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0);
                eprintln!("[ccb:expert:{persona}] {domains} domains, {patterns} patterns active — advising on {tool_name}");
                eprintln!("{json}");
            }
        }
        Ok(None) => {}
        Err(_) => {}
    }
}

#[cfg(not(feature = "expert"))]
fn inject_expert_context(_tool_name: &str) {}


// ---------------------------------------------------------------------------
// Graph-aware context injection for Read calls
// ---------------------------------------------------------------------------

#[cfg(feature = "graph")]
fn inject_graph_context(tool_input: &serde_json::Value) {
    let file_path = match tool_input.get("file_path").and_then(|v| v.as_str()) {
        Some(fp) => fp,
        None => return,
    };
    match crate::features::graph::symbols_in_file(file_path) {
        Ok(symbols) if !symbols.is_empty() => {
            let dominated: Vec<&(String, String, i64)> = symbols
                .iter()
                .filter(|(_, kind, line)| {
                    *line > 0 && matches!(kind.as_str(),
                        "function" | "fn" | "struct" | "enum" | "trait" | "impl"
                        | "class" | "method" | "interface" | "type_alias"
                        | "const" | "static" | "mod"
                    )
                })
                .collect();
            if dominated.is_empty() {
                return;
            }
            let hints: Vec<String> = dominated
                .iter()
                .map(|(name, kind, line)| format!("  {kind} `{name}` line {line}"))
                .collect();
            eprintln!("[ccb:graph] {} symbols in {}", dominated.len(), file_path);
            eprintln!("{}", hints.join("\n"));
        }
        _ => {}
    }
}

#[cfg(not(feature = "graph"))]
fn inject_graph_context(_tool_input: &serde_json::Value) {}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run() -> Result<()> {
    let mut input = String::new();
    std::io::Read::read_to_string(&mut std::io::stdin(), &mut input)?;

    let payload: HookPayload = match serde_json::from_str(&input) {
        Ok(p) => p,
        Err(_) => return Ok(()),
    };

    let tool_name = payload.tool_name.unwrap_or_default();
    let tool_input = payload.tool_input.unwrap_or(serde_json::Value::Null);
    let session_id = payload.session_id.unwrap_or_default();

    // Tier 1: local pattern matching
    let (decision, reason) = tier1_classify(&tool_name, &tool_input);
    match decision {
        Decision::Allow => {
            log_decision(&tool_name, "tier1", Decision::Allow, reason);
            inject_expert_context(&tool_name);
            if tool_name == "Read" {
                inject_graph_context(&tool_input);
            }
            return Ok(());
        }
        Decision::Deny => {
            log_decision(&tool_name, "tier1", Decision::Deny, reason);
            deny_output(reason);
            return Ok(());
        }
        Decision::Uncertain => {}
    }

    // Tier 2: LLM evaluation
    let api_key = match load_secrets_key("OPENROUTER_API_KEY") {
        Some(k) => k,
        None => {
            log_decision(&tool_name, "tier2", Decision::Allow, "no API key — fail open");
            return Ok(());
        }
    };

    match tier2_classify(&tool_name, &tool_input, &session_id, &api_key) {
        Ok((Decision::Deny, reason)) => {
            log_decision(&tool_name, "tier2", Decision::Deny, &reason);
            deny_output(&reason);
        }
        Ok((decision, reason)) => {
            log_decision(&tool_name, "tier2", decision, &reason);
            if decision == Decision::Allow {
                inject_expert_context(&tool_name);
            }
        }
        Err(_) => {
            log_decision(&tool_name, "tier2", Decision::Allow, "LLM error — fail open");
            inject_expert_context(&tool_name);
        }
    }

    Ok(())
}
