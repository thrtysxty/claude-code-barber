//! Model router — routes Claude Code API calls to local or Anthropic backends

#[cfg(feature = "route")]
use crate::cli::RouteCmd;
use anyhow::{Context, Result};

use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

const ROUTER_PID_FILE: &str = ".cache/ccb/router.pid";
const DEFAULT_PORT: u16 = 9001;

/// Locate the ccb-route binary: companion next to ccb → PATH → dev build dirs
fn find_ccb_route() -> Result<PathBuf> {
    // 1. Same directory as the current executable (e.g. ~/.local/bin/ccb-route)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let companion = dir.join("ccb-route");
            if companion.exists() {
                return Ok(companion);
            }
        }
    }

    // 2. Which / PATH lookup
    if let Ok(output) = Command::new("which").arg("ccb-route").output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let p = PathBuf::from(&path);
            if p.exists() {
                return Ok(p);
            }
        }
    }

    // 3. Dev build paths (release then debug)
    if let Some(home) = dirs::home_dir() {
        for profile in &["release", "debug"] {
            let dev_path = home
                .join("Projects")
                .join("claude-code-barber")
                .join("target")
                .join(profile)
                .join("ccb-route");
            if dev_path.exists() {
                return Ok(dev_path);
            }
        }
    }

    Err(anyhow::anyhow!(
        "ccb-route binary not found. Install to PATH or run: cargo build --features route"
    ))
}

/// Default routing configuration
#[derive(Debug, serde::Serialize)]
struct DefaultConfig {
    haiku: (String, String),
    sonnet: (String, String),
    opus: (String, String),
    minimax: (String, String),
    ollama: (String, String),
}

impl DefaultConfig {
    fn new() -> Self {
        let aibox = env::var("AIBOX_URL").unwrap_or_else(|_| "http://aibox:8080".to_string());
        let aibox_model = env::var("AIBOX_MODEL").unwrap_or_else(|_| "qwopus3.5-9b-v3".to_string());
        let minimax_url = env::var("MINIMAX_URL")
            .unwrap_or_else(|_| "https://api.minimax.io/anthropic".to_string());
        let minimax_model =
            env::var("MINIMAX_MODEL").unwrap_or_else(|_| "MiniMax-M2.7".to_string());
        let ollama_url =
            env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());

        Self {
            haiku: (aibox, aibox_model),
            sonnet: (ollama_url, "ollama".to_string()),
            opus: ("https://api.anthropic.com".to_string(), "opus".to_string()),
            minimax: (minimax_url, minimax_model),
            ollama: ("http://localhost:11434".to_string(), "ollama".to_string()),
        }
    }
}

/// Loads routing config from ~/.claude/ccb.toml with fallback to defaults
fn load_config() -> Result<Config> {
    let config_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("ccb.toml");

    if !config_path.exists() {
        let cfg = DefaultConfig::new();
        let toml = toml::to_string_pretty(&cfg)?;
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&config_path, &toml)?;
        Ok(Config::from_default(cfg))
    } else {
        let toml_str = fs::read_to_string(&config_path)
            .with_context(|| format!("reading config at {}", config_path.display()))?;
        let cfg: Result<Config, _> = toml::from_str(&toml_str);
        match cfg {
            Ok(cfg) => {
                let defaults = DefaultConfig::new();
                Ok(Config::from_defaults(defaults, cfg))
            }
            Err(_) => {
                // Config file format is outdated — regenerate from defaults
                fs::remove_file(&config_path).ok();
                let cfg = DefaultConfig::new();
                let toml = toml::to_string_pretty(&cfg)?;
                if let Some(parent) = config_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&config_path, &toml)?;
                Ok(Config::from_default(cfg))
            }
        }
    }
}

/// Full routing configuration
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Config {
    haiku: (String, String),
    sonnet: (String, String),
    opus: (String, String),
    minimax: (String, String),
    ollama: (String, String),
    port: u16,
}

impl Config {
    fn from_default(cfg: DefaultConfig) -> Self {
        Self {
            haiku: (cfg.haiku.0, cfg.haiku.1),
            sonnet: (cfg.sonnet.0, cfg.sonnet.1),
            opus: (cfg.opus.0, cfg.opus.1),
            minimax: (cfg.minimax.0, cfg.minimax.1),
            ollama: (cfg.ollama.0, cfg.ollama.1),
            port: DEFAULT_PORT,
        }
    }

    fn from_defaults(defaults: DefaultConfig, config: Config) -> Self {
        Self {
            haiku: (defaults.haiku.0, config.haiku.1),
            sonnet: (defaults.sonnet.0, config.sonnet.1),
            opus: (defaults.opus.0, config.opus.1),
            minimax: (defaults.minimax.0, config.minimax.1),
            ollama: (defaults.ollama.0, config.ollama.1),
            port: config.port,
        }
    }

    fn start(&self) -> Result<()> {
        let pid_file = PathBuf::from(ROUTER_PID_FILE);

        // Check if already running
        if pid_file.exists() {
            let pid_str = fs::read_to_string(&pid_file)
                .with_context(|| format!("reading PID file at {}", pid_file.display()))?;
            if let Ok(_pid) = pid_str.trim().parse::<u32>() {
                let process_exists = Command::new("ps")
                    .args(["-p", &pid_str])
                    .output()
                    .unwrap()
                    .status
                    .success();
                if process_exists {
                    return Err(anyhow::anyhow!("Router already running (PID {})", pid_str));
                }
            }
        }

        // Find the ccb-route binary: prefer companion next to ccb, then PATH, then dev paths
        let router_exe = find_ccb_route()?;

        let mut cmd = Command::new(&router_exe);
        cmd.env("CCB_ROUTE_PORT", self.port.to_string());
        cmd.env("AIBOX_URL", &self.haiku.0);
        cmd.env("AIBOX_MODEL", &self.haiku.1);
        cmd.env("OLLAMA_URL", &self.ollama.0);
        cmd.env("CCB_SONNET_BACKEND", "ollama");
        cmd.env("ANTHROPIC_API_KEY_REAL", load_real_key()?);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        let status = cmd
            .spawn()
            .with_context(|| format!("failed to spawn {}", router_exe.display()))?;

        // Wait for startup
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Write PID
        let pid = status.id();
        fs::create_dir_all(pid_file.parent().unwrap())?;
        fs::write(&pid_file, pid.to_string())
            .with_context(|| format!("writing PID {} to {}", pid, pid_file.display()))?;

        println!("Router started on :{}", self.port);
        println!("Run Claude Code with:");
        println!(
            "  ANTHROPIC_BASE_URL=http://localhost:{} ANTHROPIC_API_KEY=router claude",
            self.port
        );

        Ok(())
    }

    fn stop(&self) -> Result<()> {
        let pid_file = PathBuf::from(ROUTER_PID_FILE);
        if !pid_file.exists() {
            return Ok(());
        }
        let pid_str = fs::read_to_string(ROUTER_PID_FILE)
            .with_context(|| format!("reading PID file at {}", ROUTER_PID_FILE))?;

        let pid: u32 = pid_str.trim().parse().with_context(|| {
            format!("invalid PID in {}: {:?}", ROUTER_PID_FILE, pid_str.trim())
        })?;

        // Verify the process is actually ccb-route before sending kill
        let ps_output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "comm="])
            .output();
        match ps_output {
            Ok(out) if out.status.success() => {
                let comm = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !comm.contains("ccb-route") {
                    return Err(anyhow::anyhow!(
                        "PID {} is '{}' not ccb-route — refusing to kill",
                        pid,
                        comm
                    ));
                }
            }
            _ => {
                // Process doesn't exist or ps failed — clean up stale PID file
                let _ = fs::remove_file(&pid_file);
                return Ok(());
            }
        }

        let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();

        fs::remove_file(ROUTER_PID_FILE)
            .with_context(|| format!("removing PID file at {}", ROUTER_PID_FILE))?;

        Ok(())
    }

    fn status(&self) -> Result<()> {
        let pid_str = fs::read_to_string(ROUTER_PID_FILE)
            .with_context(|| format!("reading PID file at {}", ROUTER_PID_FILE))?;

        let running = Command::new("ps")
            .args(["-p", &pid_str])
            .output()
            .unwrap()
            .status
            .success();

        println!(
            "Router status: {}",
            if running { "running" } else { "stopped" }
        );
        println!("  PID: {}", pid_str);
        println!("  Port: {}", self.port);
        println!("  Routes:");
        println!("    haiku  → {} → {}", self.haiku.0, self.haiku.1);
        println!("    sonnet → {} → {}", self.sonnet.0, self.sonnet.1);
        println!("    opus   → {} → {}", self.opus.0, self.opus.1);
        println!("    minimax → {} → {}", self.minimax.0, self.minimax.1);
        println!("    ollama  → {} → {}", self.ollama.0, self.ollama.1);

        if !running {
            fs::remove_file(ROUTER_PID_FILE)
                .with_context(|| format!("removing PID file at {}", ROUTER_PID_FILE))?;
        }

        Ok(())
    }

    fn env(&self) -> Result<()> {
        println!("export ANTHROPIC_BASE_URL=http://localhost:{}", self.port);
        println!("export ANTHROPIC_API_KEY=router");
        Ok(())
    }
}

/// Loads the real Anthropic API key from env or ~/.secrets
fn load_real_key() -> Result<String> {
    let key = env::var("ANTHROPIC_API_KEY_REAL");
    if let Ok(k) = key {
        if !k.is_empty() {
            return Ok(k);
        }
    }
    let home = dirs::home_dir().unwrap_or_default();
    let secrets = home.join(".secrets");
    if secrets.exists() {
        let content = fs::read_to_string(&secrets)
            .with_context(|| format!("reading {}", secrets.display()))?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("ANTHROPIC_API_KEY=") && !line.contains("router") {
                return Ok(line
                    .split_once('=')
                    .map(|(_, v)| v.trim().to_string())
                    .unwrap_or_default());
            }
        }
    }
    Ok("".to_string())
}

/// Runs the router binary directly (for testing)
#[cfg(test)]
fn run_router_binary() -> Result<std::process::Child> {
    let router_exe = dirs::home_dir()
        .unwrap_or_default()
        .join("Projects")
        .join("claude-code-barber")
        .join("target")
        .join("debug")
        .join("ccb-route");

    if !router_exe.exists() {
        return Err(anyhow::anyhow!(
            "ccb-route binary not found at {}",
            router_exe.display()
        ));
    }

    let mut cmd = Command::new(router_exe);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::null());

    Ok(cmd.spawn()?)
}

// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_loads_defaults() {
        let cfg = load_config().unwrap();
        assert_eq!(cfg.haiku.0, "http://aibox:8080");
        assert_eq!(cfg.haiku.1, "qwopus3.5-9b-v3");
        assert_eq!(cfg.sonnet.0, "http://localhost:11434");
        assert_eq!(cfg.minimax.0, "https://api.minimax.io/anthropic");
        assert_eq!(cfg.minimax.1, "MiniMax-M2.7");
        assert_eq!(cfg.ollama.0, "http://localhost:11434");
    }

    #[test]
    #[ignore]
    fn test_start_stop() {
        let cfg = load_config().unwrap();
        cfg.start().unwrap();
        cfg.status().unwrap();
        cfg.stop().unwrap();
        // status after stop may fail if PID file was cleaned up — that's fine
        let _ = cfg.status();
    }

    #[test]
    fn test_env_output() {
        let cfg = load_config().unwrap();
        cfg.env().unwrap();
    }

    #[test]
    #[ignore]
    fn test_router_binary() {
        let cfg = load_config().unwrap();
        let mut child = run_router_binary().unwrap();

        // Give it time to start
        std::thread::sleep(std::time::Duration::from_secs(2));

        // Check health endpoint
        let response =
            reqwest::blocking::get(format!("http://localhost:{}/health", cfg.port)).unwrap();
        let json: serde_json::Value = response.json().unwrap();
        assert_eq!(json["status"], "ok");

        child.kill().unwrap();
    }
}

pub fn run_router(args: RouteCmd) -> Result<()> {
    let mut cfg = load_config()?;

    match args {
        RouteCmd::Start { port } => {
            cfg.port = port;
            cfg.start()?;
        }
        RouteCmd::Stop => cfg.stop()?,
        RouteCmd::Status => cfg.status()?,
        RouteCmd::Env => cfg.env()?,
        RouteCmd::Tiers { test } => show_tier_routing(&cfg, test.as_deref())?,
    }
    Ok(())
}

/// Show the tier routing table with resolved model → provider mappings.
/// Implements: ccb route tiers (AC18, AC19)
fn show_tier_routing(_cfg: &Config, test_tier: Option<&str>) -> Result<()> {
    use crate::features::providers::{ProviderConfig, Tier};

    let pcfg = ProviderConfig::get();

    // Override: if set, show it and skip individual tier lists
    if let Some(ref override_model) = pcfg.tier_routing.override_all {
        println!("  override_all: {}", override_model);
        if let Some((pname, _, entry)) = pcfg.resolve_model(override_model) {
            println!("    → {} ({})\n", pname, entry.id);
        } else {
            println!("    [WARNING: override model not found in any provider]\n");
        }
    }

    if let Some(tier_str) = test_tier {
        // AC19: --test flag shows which model would handle a specific tier
        let tier = match tier_str.to_lowercase().as_str() {
            "opus" => Tier::Opus,
            "sonnet" => Tier::Sonnet,
            "haiku" => Tier::Haiku,
            _ => {
                anyhow::bail!("Unknown tier: {}. Use opus, sonnet, or haiku.", tier_str);
            }
        };

        let models = pcfg.tier_routing.models_for_tier(&tier);
        if models.is_empty() {
            println!(
                "No tier_routing config for {:?}. Using fallback (first available).",
                tier
            );
            // Fall back to first matching provider
            for (name, provider) in &pcfg.providers {
                for entry in &provider.models {
                    if provider.effective_tier(entry) == tier {
                        println!("  {} → {} ({})", tier, name, entry.backend_model());
                        return Ok(());
                    }
                }
            }
        } else {
            println!("{} tier preference list ({} models):", tier, models.len());
            for (i, model_id) in models.iter().enumerate() {
                let pos = i + 1;
                if let Some((pname, _, entry)) = pcfg.resolve_model(model_id) {
                    let tag = if pos == 1 { " → " } else { "   " };
                    println!(
                        "  {}{} via {} [{}]",
                        tag,
                        model_id,
                        pname,
                        entry.backend_model()
                    );
                } else {
                    println!("  [INVALID] {} — not found in any provider", model_id);
                }
            }
        }
        return Ok(());
    }

    // AC18: show full tier routing table
    println!("Tier Routing Table");
    println!("=================");
    println!();

    for tier in [Tier::Opus, Tier::Sonnet, Tier::Haiku] {
        let models = pcfg.tier_routing.models_for_tier(&tier);
        println!("{:>6} tier:", tier);
        if models.is_empty() {
            println!("  (none — fallback to first available)");
        } else {
            for (i, model_id) in models.iter().enumerate() {
                let pos = i + 1;
                if let Some((pname, _, entry)) = pcfg.resolve_model(model_id) {
                    println!(
                        "  [{}/{}] {} via {} [{}]",
                        pos,
                        models.len(),
                        model_id,
                        pname,
                        entry.backend_model()
                    );
                } else {
                    println!(
                        "  [{}/{}] {} — [INVALID: not in any provider]",
                        pos,
                        models.len(),
                        model_id
                    );
                }
            }
        }
        println!();
    }

    println!("Note: tier_routing section in providers.toml controls these preferences.");
    println!("Router must be restarted for config changes to take effect.");
    Ok(())
}
