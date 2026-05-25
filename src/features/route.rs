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

/// Default routing configuration
#[derive(Debug, serde::Serialize)]
struct DefaultConfig {
    haiku: (String, String),
    sonnet: (String, String),
    opus: (String, String),
}

impl DefaultConfig {
    fn new() -> Self {
        let aibox = env::var("AIBOX_URL").unwrap_or_else(|_| "http://aibox:8080".to_string());
        let aibox_model = env::var("AIBOX_MODEL").unwrap_or_else(|_| "qwopus3.5-9b-v3".to_string());
        let ollama =
            env::var("OLLAMA_URL").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let ollama_model = env::var("OLLAMA_MODEL").unwrap_or_else(|_| "glm-5.1:cloud".to_string());

        Self {
            haiku: (aibox, aibox_model),
            sonnet: (ollama, ollama_model),
            opus: ("https://api.anthropic.com".to_string(), "opus".to_string()),
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
    port: u16,
}

impl Config {
    fn from_default(cfg: DefaultConfig) -> Self {
        Self {
            haiku: (cfg.haiku.0, cfg.haiku.1),
            sonnet: (cfg.sonnet.0, cfg.sonnet.1),
            opus: (cfg.opus.0, cfg.opus.1),
            port: DEFAULT_PORT,
        }
    }

    fn from_defaults(defaults: DefaultConfig, config: Config) -> Self {
        Self {
            haiku: (defaults.haiku.0, config.haiku.1),
            sonnet: (defaults.sonnet.0, config.sonnet.1),
            opus: (defaults.opus.0, config.opus.1),
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

        // Load real Anthropic key
        let real_key = load_real_key()?;

        // Determine API key for each route
        let opus_key = if real_key.is_empty() {
            "MISSING".to_string()
        } else {
            real_key.to_string()
        };

        // Shell out to the Python model-router (the working implementation)
        let python_router =
            std::path::PathBuf::from("/Users/dadmin/Projects/scripts/model-router.py");
        if !python_router.exists() {
            return Err(anyhow::anyhow!(
                "router script not found at {}",
                python_router.display()
            ));
        }

        let mut cmd = Command::new("python3");
        cmd.arg(&python_router);
        cmd.env("AIBOX_URL", &self.haiku.0);
        cmd.env("AIBOX_MODEL", &self.haiku.1);
        cmd.env("OLLAMA_URL", &self.sonnet.0);
        cmd.env("OLLAMA_MODEL", &self.sonnet.1);
        cmd.env("ANTHROPIC_API_KEY", &opus_key);
        cmd.env("PORT", self.port.to_string());
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::null());
        cmd.stderr(Stdio::null());

        let status = cmd
            .spawn()
            .with_context(|| "failed to spawn python3 model-router.py")?;

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

        if let Ok(_pid) = pid_str.trim().parse::<u32>() {
            let _ = Command::new("kill").args(["-9", &pid_str]).output();

            fs::remove_file(ROUTER_PID_FILE)
                .with_context(|| format!("removing PID file at {}", ROUTER_PID_FILE))?;
        }

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
        assert_eq!(cfg.sonnet.1, "glm-5.1:cloud");
    }

    #[test]
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
    }
    Ok(())
}
