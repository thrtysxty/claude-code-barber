mod analytics;
mod cli;
mod config;
mod log;
mod utils;

pub mod features {
    pub mod buzz;
    pub mod context;
    pub mod cut;
    #[cfg(feature = "fade")]
    pub mod fade;
    #[cfg(feature = "graph")]
    pub mod graph;
    #[cfg(feature = "expert")]
    pub mod expert;
    pub mod index;
    pub mod lineup;
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
        Command::Gain => analytics::gain(),
        #[cfg(feature = "graph")]
        Command::Graph(args) => graph_cmd(args),
        #[cfg(feature = "expert")]
        Command::Expert(args) => expert_cmd(args),
    }
}

fn trim_cmd(_args: cli::TrimArgs) -> anyhow::Result<()> {
    #[cfg(feature = "trim")]
    return features::trim::run(_args);
}

fn fade_cmd(_args: cli::FadeArgs) -> anyhow::Result<()> {
    #[cfg(feature = "fade")]
    return features::fade::run(_args);
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
        ExpertCmd::Activate { name } => features::expert::activate(&name),
        ExpertCmd::Deactivate => features::expert::deactivate(),
        ExpertCmd::List => features::expert::list(),
        ExpertCmd::Walk { task } => features::expert::walk(&task, 0.5),
        ExpertCmd::Query { tool: _, format } => features::expert::query_active(fmt(&format)),
    }
}
