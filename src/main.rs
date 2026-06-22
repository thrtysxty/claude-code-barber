mod analytics;
mod cli;
mod config;
mod log;
mod utils;

#[cfg(any(feature = "hooks", feature = "context"))]
pub mod hooks;

pub mod features {
    pub mod buzz;
    #[cfg(feature = "classify")]
    pub mod classify;
    #[cfg(feature = "context")]
    pub mod context;
    pub mod cut;
    #[cfg(feature = "expert")]
    pub mod expert;
    #[cfg(feature = "factory")]
    pub mod factory;
    #[cfg(feature = "fade")]
    pub mod fade;
    #[cfg(feature = "graph")]
    pub mod graph;
    pub mod index;
    pub mod install;
    pub mod lineup;
    #[cfg(feature = "loop")]
    pub mod loop_cmd;
    pub mod model_metadata;
    #[cfg(feature = "plugins")]
    pub mod plugins;
    #[cfg(feature = "route")]
    pub mod providers;
    #[cfg(feature = "status")]
    pub mod rates;
    #[cfg(feature = "route")]
    pub mod route;
    #[cfg(feature = "status")]
    pub mod status;
    #[cfg(feature = "trim")]
    pub mod trim;
}

use clap::Parser;
use cli::{Cli, Command, StyleCmd};

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_env("CCB_LOG"))
        .json()
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Trim(args) => trim_cmd(args),
        Command::Fade(args) => fade_cmd(args),
        Command::Cut => features::cut::run(),
        Command::Lineup => features::lineup::run(),
        Command::Style(s) => style_cmd(s.cmd),
        #[cfg(feature = "context")]
        Command::Context(c) => features::context::run(c.cmd),
        Command::Buzz => features::buzz::run(),
        Command::Gain(args) => {
            let mode = if args.ab {
                analytics::GainMode::AbTest
            } else if args.expert {
                analytics::GainMode::ExpertDelta
            } else {
                analytics::GainMode::Default
            };
            analytics::gain(mode)
        }
        Command::Install(args) => features::install::run(args.auto, args.dry_run),
        #[cfg(feature = "plugins")]
        Command::Plugins => features::plugins::run(),
        Command::Models => models_cmd(),
        #[cfg(feature = "graph")]
        Command::Graph(args) => graph_cmd(args),
        #[cfg(feature = "expert")]
        Command::Expert(args) => expert_cmd(args),
        #[cfg(feature = "route")]
        Command::Route(args) => route_cmd(args),
        #[cfg(feature = "classify")]
        Command::Classify => features::classify::run(),
        #[cfg(feature = "factory")]
        Command::Factory(args) => factory_cmd(args),
        #[cfg(feature = "loop")]
        Command::Loop(args) => loop_cmd(args),
        #[cfg(feature = "status")]
        Command::Status(args) => status_cmd(args.cmd.unwrap_or(cli::StatusCmd::Show)),
    }
}

fn models_cmd() -> anyhow::Result<()> {
    let url =
        std::env::var("ANTHROPIC_BASE_URL").unwrap_or_else(|_| "http://localhost:9001".to_string());
    let endpoint = format!("{}/v1/models", url.trim_end_matches('/'));

    let client = reqwest::blocking::Client::new();
    let resp = client.get(&endpoint).send();
    match resp {
        Ok(resp) if resp.status().is_success() => {
            let body: serde_json::Value = resp.json()?;
            if let Some(data) = body["data"].as_array() {
                for m in data {
                    let id = m["id"].as_str().unwrap_or("");
                    let display = m["display_name"].as_str().unwrap_or(id);
                    if id.starts_with("──") {
                        println!("\n  \x1b[1m{}\x1b[0m", display);
                    } else {
                        println!("    {:<35} claude --model {}", display, id);
                    }
                }
                println!();
            }
            Ok(())
        }
        _ => {
            anyhow::bail!("ccb-route not reachable at {endpoint}\nStart it with: ccb-route &");
        }
    }
}

fn trim_cmd(_args: cli::TrimArgs) -> anyhow::Result<()> {
    #[cfg(feature = "trim")]
    return features::trim::run(_args);
    #[cfg(not(feature = "trim"))]
    anyhow::bail!("ccb was built without the 'trim' feature")
}

fn fade_cmd(_args: cli::FadeArgs) -> anyhow::Result<()> {
    #[cfg(feature = "fade")]
    return features::fade::run(_args);
    #[cfg(not(feature = "fade"))]
    anyhow::bail!("ccb was built without the 'fade' feature")
}

fn style_cmd(cmd: StyleCmd) -> anyhow::Result<()> {
    match cmd {
        StyleCmd::IndexBuild => {
            let skills_dir = dirs::home_dir()
                .unwrap_or_default()
                .join(".claude")
                .join("skills");
            let path = features::index::write(&skills_dir)?;
            println!("INDEX.md written to {}", path.display());
        }
        StyleCmd::Show => {
            let cfg = config::load()?;
            println!("{}", toml::to_string_pretty(&cfg)?);
        }
    }
    Ok(())
}

#[cfg(feature = "graph")]
fn graph_cmd(_args: cli::GraphArgs) -> anyhow::Result<()> {
    use cli::{GraphCmd, OutputFormatArg};
    use features::graph::OutputFormat;
    let fmt = |f: &OutputFormatArg| match f {
        OutputFormatArg::Human => OutputFormat::Human,
        OutputFormatArg::Json => OutputFormat::Json,
    };
    match _args.cmd {
        GraphCmd::Index { path } => features::graph::index(&path),
        GraphCmd::Search { pattern, format } => features::graph::search(&pattern, fmt(&format)),
        GraphCmd::Show { file, format } => features::graph::show(&file, fmt(&format)),
        GraphCmd::Stats { format } => features::graph::stats(fmt(&format)),
    }
}

#[cfg(feature = "expert")]
fn expert_cmd(_args: cli::ExpertArgs) -> anyhow::Result<()> {
    use cli::{ExpertCmd, ExpertOutputFormatArg};
    use features::expert::OutputFormat;
    let fmt = |f: &ExpertOutputFormatArg| match f {
        ExpertOutputFormatArg::Human => OutputFormat::Human,
        ExpertOutputFormatArg::Json => OutputFormat::Json,
    };
    match _args.cmd {
        ExpertCmd::Build { name, dataset } => features::expert::build(&name, &dataset),
        ExpertCmd::Ingest { dataset } => features::expert::ingest(&dataset),
        ExpertCmd::Activate { name } => features::expert::activate(&name),
        ExpertCmd::Deactivate => features::expert::deactivate(),
        ExpertCmd::List => features::expert::list(),
        ExpertCmd::Walk { task } => features::expert::walk(&task, 0.5),
        ExpertCmd::Query { tool: _, format } => features::expert::query_active(fmt(&format)),
        ExpertCmd::Export {
            persona,
            format,
            output,
        } => features::expert::export(&persona, format, &output),
    }
}

#[cfg(feature = "route")]
fn route_cmd(args: cli::RouteArgs) -> anyhow::Result<()> {
    use features::route::run_router;
    run_router(args.cmd).map_err(|e| anyhow::anyhow!("Router error: {}", e))?;
    Ok(())
}

#[cfg(feature = "factory")]
fn factory_cmd(_args: cli::FactoryArgs) -> anyhow::Result<()> {
    use cli::FactoryCmd;
    use features::factory;
    match &_args.cmd {
        FactoryCmd::New {
            title,
            loop_type,
            description,
        } => {
            let lt = match loop_type.as_str() {
                "planning" => factory::LoopType::Planning,
                "implementation" => factory::LoopType::Implementation,
                _ => anyhow::bail!("loop_type must be 'planning' or 'implementation'"),
            };
            let desc = description.as_deref().unwrap_or("");
            let story = factory::create_story(title, desc, lt)?;
            println!("Created story: {}", story.id);
            println!("  title: {}", story.title);
            println!("  loop: {}", story.loop_type);
            println!("  state: {}", story.state);
        }
        FactoryCmd::Advance { story_id, note } => {
            let story = factory::advance_story(story_id, note.as_ref().map(|s| s.as_str()))?;
            println!("Advanced {} → {}", story.id, story.state);
        }
        FactoryCmd::Kickback { story_id, note } => {
            let story = factory::kickback_story(story_id, note.as_ref().map(|s| s.as_str()))?;
            println!("Kicked back {} → {}", story.id, story.state);
        }
        FactoryCmd::Escalate {
            story_id,
            target,
            note,
        } => {
            let story =
                factory::escalate_story(story_id, target, note.as_ref().map(|s| s.as_str()))?;
            println!("Escalated {} → {} (@{})", story.id, story.state, target);
        }
        FactoryCmd::Approve { story_id, note } => {
            let story = factory::approve_story(story_id, note.as_ref().map(|s| s.as_str()))?;
            println!("Approved {} → {}", story.id, story.state);
        }
        FactoryCmd::Status { story_id } => {
            let story = factory::story_status(story_id)?;
            println!("{}", factory::format_story(&story));
            println!("\nHistory:");
            for h in &story.history {
                println!(
                    "  [{}] {} --[{}]--> {} @{}",
                    h.timestamp, h.from, h.trigger, h.to, h.expert
                );
                if let Some(n) = &h.note {
                    println!("    note: {}", n);
                }
            }
        }
        FactoryCmd::List { loop_type } => {
            let lt = match loop_type.as_deref() {
                Some("planning") => Some(factory::LoopType::Planning),
                Some("implementation") => Some(factory::LoopType::Implementation),
                Some(_) => anyhow::bail!("loop_type must be 'planning' or 'implementation'"),
                None => None,
            };
            let stories = factory::list_stories(lt)?;
            if stories.is_empty() {
                println!("No stories found.");
            } else {
                for s in &stories {
                    println!("{}", factory::format_story(s));
                }
            }
        }
        FactoryCmd::Show { loop_type } => {
            let loop_def = match loop_type.as_str() {
                "planning" => &factory::PLANNING_LOOP,
                "implementation" => &factory::IMPLEMENTATION_LOOP,
                _ => anyhow::bail!("loop_type must be 'planning' or 'implementation'"),
            };
            println!("{}", factory::format_state_machine(loop_def));
        }
    }
    Ok(())
}

#[cfg(feature = "loop")]
fn loop_cmd(args: cli::LoopArgs) -> anyhow::Result<()> {
    use cli::LoopCmd;
    match args.cmd {
        LoopCmd::Detect { format } => features::loop_cmd::cmd_detect(cli::DetectArgs { format }),
        LoopCmd::Plan { story_file, save } => {
            features::loop_cmd::cmd_plan(cli::PlanArgs { story_file, save })
        }
        LoopCmd::Build { plan, story } => {
            features::loop_cmd::cmd_build(cli::BuildArgs { plan, story })
        }
        LoopCmd::Lesson { description } => {
            features::loop_cmd::cmd_lesson(cli::LessonArgs { description })
        }
        LoopCmd::Gates { run } => features::loop_cmd::cmd_gates(cli::GatesArgs { run }),
    }
}

#[cfg(feature = "status")]
fn status_cmd(cmd: cli::StatusCmd) -> anyhow::Result<()> {
    use features::status::gradient::{set_nerd_font, terminal_width};
    use features::status::session::SessionInfo;
    use features::status::{build_session_info, resolve_theme, StatusInput};

    // Initialize Nerd Font mode from config
    let cfg = config::load().unwrap_or_default();
    set_nerd_font(cfg.nerd_font);

    // Use max of detected terminal width and 80 to ensure consistent layout
    // even in narrow terminals (MIN_WIDTH for statusline is 40, so 80 is safe)
    let width = terminal_width().max(80);
    let theme = resolve_theme("claude-dark");

    match cmd {
        cli::StatusCmd::Show => {
            // Try reading session JSON from stdin first (Claude Code pipes it).
            // If valid, use it as the primary data source — it has the real model,
            // real tokens, real rate limits from the live session.
            let stdin_json = {
                use std::io::Read;
                let mut buf = String::new();
                let _ = std::io::stdin().read_to_string(&mut buf);
                buf
            };

            if !stdin_json.trim().is_empty() {
                if let Ok(session) = serde_json::from_str::<SessionInfo>(&stdin_json) {
                    let ccb = StatusInput::load();
                    let session = enrich_from_ccb(session, &ccb);
                    update_session_env(&session.session_id, &session.model.id);
                    let output = features::status::render(&session, &theme, width);
                    println!("{}", output);
                    return Ok(());
                }
            }

            // Fallback: use CCB-only data (route usage, session env)
            let input = StatusInput::load();
            let session_info = build_session_info(&input);
            let output = features::status::render(&session_info, &theme, width);
            println!("{}", output);
            Ok(())
        }
        cli::StatusCmd::Demo { scenario } => features::status::demo::run(
            if scenario.is_empty() {
                None
            } else {
                Some(scenario.as_str())
            },
            &theme,
            width,
        ),
        cli::StatusCmd::Mon { directory } => {
            let dir = if directory.is_empty() {
                dirs::home_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."))
                    .join(".claude")
            } else {
                std::path::PathBuf::from(directory)
            };
            features::status::mon::run(&dir, 5, &theme, width)
        }
    }
}

#[cfg(feature = "status")]
fn enrich_from_ccb(
    mut session: features::status::SessionInfo,
    ccb: &features::status::StatusInput,
) -> features::status::SessionInfo {
    use features::status::session::GitInfo;

    // Fill in git details if missing from session JSON
    if session.git.is_none()
        || session
            .git
            .as_ref()
            .and_then(|g| g.branch.as_deref())
            .is_none()
    {
        let git_info = GitInfo::from_cwd(session.cwd.as_deref().unwrap_or(""));
        session.git = Some(features::status::session::GitState {
            branch: Some(git_info.branch),
            is_dirty: Some(git_info.modified > 0 || git_info.untracked > 0),
            commit_hash: Some(git_info.commit),
            commit_message: None,
            ahead: None,
            behind: None,
        });
    }

    // Fill in rate limits from CCB route data when Claude Code's JSON omits them
    if session.rate_limits.is_none() && (ccb.rate_5h_pct > 0.0 || ccb.rate_7d_pct > 0.0) {
        session.rate_limits = Some(features::status::session::RateLimits {
            five_hour: features::status::session::FiveHourLimit {
                used_percentage: ccb.rate_5h_pct,
                resets_at: ccb.rate_resets_at.as_ref().and_then(|s| {
                    chrono::DateTime::parse_from_rfc3339(s)
                        .map(|dt| dt.timestamp() as u64)
                        .ok()
                }),
            },
            seven_day: features::status::session::SevenDayLimit {
                used_percentage: ccb.rate_7d_pct,
                resets_at: None,
            },
        });
    }

    // If session JSON didn't provide a display name, derive one
    if session.model.display_name.is_none() {
        let id = &session.model.id;
        let display = if id.contains("qwopus") {
            "Qwopus 3.5"
        } else if id.contains("opus") {
            "Opus 4.7"
        } else if id.contains("sonnet") {
            "Sonnet 4.6"
        } else if id.contains("haiku") {
            "Haiku 4.5"
        } else if id.contains("minimax") {
            "MiniMax-M2.7"
        } else if id == "unknown" {
            "Unknown"
        } else {
            id
        };
        session.model.display_name = Some(display.to_string());
    }

    session
}

#[cfg(feature = "status")]
fn update_session_env(session_id: &str, model_id: &str) {
    use std::io::Write;
    let path = dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
        .join("ccb")
        .join("session_env.sh");

    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let updated = content
        .lines()
        .filter(|l| !l.starts_with("export CCB_MODEL=") && !l.starts_with("export CCB_SESSION_ID="))
        .chain(std::iter::once(""))
        .fold(String::new(), |mut acc, l| {
            if !l.is_empty() {
                acc.push_str(l);
                acc.push('\n');
            }
            acc
        });
    let mut lines: Vec<String> = updated.lines().map(|l| l.to_string()).collect();
    lines.push(format!("export CCB_SESSION_ID=\"{}\"", session_id));
    lines.push(format!("export CCB_MODEL=\"{}\"", model_id));
    let output = lines.join("\n") + "\n";

    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::File::create(&path) {
        let _ = f.write_all(output.as_bytes());
    }
}
