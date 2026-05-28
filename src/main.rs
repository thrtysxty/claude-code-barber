mod analytics;
mod cli;
mod config;
mod log;
mod utils;

pub mod features {
    #[cfg(feature = "bench")]
    pub mod bench;
    pub mod buzz;
    #[cfg(feature = "classify")]
    pub mod classify;
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
    #[cfg(feature = "memory")]
    pub mod session_mem;
    pub mod model_metadata;
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
        Command::Context(c) => features::context::run(c.cmd),
        Command::Buzz => features::buzz::run(),
        Command::Gain(args) => {
            let mode = if args.locomo {
                #[cfg(feature = "bench")]
                {
                    analytics::GainMode::Locomo {
                        dataset_path: args.dataset.clone(),
                        compression_level: args.compression_level.clone(),
                        format: args.format.clone(),
                    }
                }
                #[cfg(not(feature = "bench"))]
                    {
                        anyhow::bail!("ccb was built without the 'bench' feature (see Cargo.toml)")
                    }
            } else if args.ab {
                analytics::GainMode::AbTest
            } else if args.expert {
                analytics::GainMode::ExpertDelta
            } else {
                analytics::GainMode::Default
            };
            analytics::gain(mode)
        }
        Command::Install(args) => features::install::run(args.auto, args.dry_run),
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
        #[cfg(feature = "status")]
        Command::Status => status_cmd(),
        #[cfg(feature = "memory")]
        Command::Memory(args) => memory_cmd(args),
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
            eprintln!("ccb-route not reachable at {}", endpoint);
            eprintln!("Start it with: ccb-route &");
            std::process::exit(1);
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
        GraphCmd::Watch { path, once } => features::graph::watch(&path, once),
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

#[cfg(feature = "status")]
fn status_cmd() -> anyhow::Result<()> {
    use features::status::gradient::terminal_width;
    use features::status::session::SessionInfo;
    use features::status::{build_session_info, resolve_theme, StatusInput};

    // Try reading session JSON from stdin first (Claude Code pipes it).
    // If valid, use it as the primary data source — it has the real model,
    // real tokens, real rate limits from the live session.
    let stdin_json = {
        use std::io::Read;
        let mut buf = String::new();
        let _ = std::io::stdin().read_to_string(&mut buf);
        buf
    };

    let width = terminal_width();
    let theme = resolve_theme("claude-dark");

    if !stdin_json.trim().is_empty() {
        if let Ok(session) = serde_json::from_str::<SessionInfo>(&stdin_json) {
            let ccb = StatusInput::load();
            let session = enrich_from_ccb(session, &ccb);
            update_session_env(&session.session_id, &session.model.id);
            let output = features::status::render(&session, &theme, width, "wide");
            println!("{}", output);
            return Ok(());
        }
    }

    // Fallback: use CCB-only data (route usage, session env)
    let input = StatusInput::load();
    let session_info = build_session_info(&input);
    let output = features::status::render(&session_info, &theme, width, "wide");
    println!("{}", output);
    Ok(())
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

#[cfg(feature = "memory")]
fn memory_cmd(args: cli::MemoryArgs) -> anyhow::Result<()> {
    use cli::{MemoryCmd, OutputFormatArg};
    use features::memory::{recall, search};

    match args.cmd {
        MemoryCmd::Search {
            query,
            project,
            limit,
            format,
        } => {
            let proj_filter = project.as_deref();
            let results = search(&query, proj_filter, limit)?;

            // AC12: no results → "No matching sessions found"
            if results.is_empty() {
                println!("No matching sessions found.");
                return Ok(());
            }

            match format {
                OutputFormatArg::Human => {
                    println!("Found {} results for '{}':", results.len(), query);
                    for r in &results {
                        println!("  {}", r.human());
                    }
                }
                OutputFormatArg::Json => {
                    let json = serde_json::json!({
                        "query": query,
                        "results": results.iter().map(|r| {
                            serde_json::json!({
                                "session_id": r.session_id,
                                "name": r.name,
                                "kind": r.kind,
                                "metadata": r.metadata,
                                "timestamp": r.timestamp,
                                "score": r.score,
                            })
                        }).collect::<Vec<_>>()
                    });
                    println!("{}", json);
                }
            }
            Ok(())
        }
        MemoryCmd::Recall {
            project,
            persona,
            task,
            max_tokens,
        } => {
            let proj_filter = project.as_deref();
            let pers_filter = persona.as_deref();
            let task_filter = task.as_deref();
            let output = recall(proj_filter, pers_filter, task_filter, max_tokens)?;
            println!("{}", output);
            Ok(())
        }
        MemoryCmd::Mine { min_frequency, dry_run } => {
            use features::memory::mine::{self, run_mine};
            use features::memory::db;
            let conn = db::init()?;
            let (stats, patterns) = run_mine(&conn, min_frequency, dry_run)?;

            if dry_run {
                println!("[DRY RUN] Would generate {} patterns:", stats.tool_sequences + stats.file_clusters + stats.error_fixes + stats.persona_domains);
                for p in &patterns {
                    println!("  [{}] {} (freq={})", p.pattern_type, p.description, p.frequency);
                }
                println!("\nNo files written. Run without --dry-run to generate skill files.");
            } else {
                use features::memory::skills;
                let skill_paths = skills::generate_all_skills(&patterns)?;
                for (p, path) in patterns.iter().zip(skill_paths.iter()) {
                    skills::link_skill_path(p, path, &conn)?;
                }
                if !skill_paths.is_empty() {
                    skills::rebuild_index()?;
                    println!("✓ Generated {} skill file(s) in ~/.claude/skills/auto/", skill_paths.len());
                    println!("  tool_sequences={}, file_clusters={}, error_fixes={}, persona_domains={}",
                        stats.tool_sequences, stats.file_clusters, stats.error_fixes, stats.persona_domains);
                    println!("  INDEX.md rebuilt.");
                } else {
                    println!("No new patterns above threshold.");
                }
            }
            Ok(())
        }
        MemoryCmd::Patterns { pattern_type } => {
            use features::memory::db::{self, PatternType};
            let conn = db::init()?;
            let filter = pattern_type.as_ref().and_then(|t| PatternType::from_str(t));
            db::list_patterns(&conn, filter)
        }
        MemoryCmd::Suppress { id } => {
            use features::memory::db;
            let conn = db::init()?;
            db::suppress_pattern(&conn, id)?;
            println!("Suppressed pattern {}", id);
            Ok(())
        }
    }
}
