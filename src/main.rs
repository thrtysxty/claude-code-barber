mod analytics;
mod cli;
mod config;
mod log;
mod utils;

pub mod features {
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
    pub mod model_metadata;
    #[cfg(feature = "route")]
    pub mod providers;
    #[cfg(feature = "route")]
    pub mod route;
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

// status feature (WIP) — status_cmd, enrich_from_ccb, update_session_env
// will be added back when the status feature is stable.
