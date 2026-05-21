mod cli;
mod config;
mod analytics;
mod log;

pub mod features {
    pub mod context;
    pub mod lineup;
    pub mod cut;
    pub mod buzz;
    #[cfg(feature = "trim")] pub mod trim;
    #[cfg(feature = "fade")] pub mod fade;
    pub mod index;
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
        Command::Trim(args)  => trim_cmd(args),
        Command::Fade(args)  => fade_cmd(args),
        Command::Cut         => features::cut::run(),
        Command::Lineup      => features::lineup::run(),
        Command::Style(s)    => style_cmd(s.cmd),
        Command::Context(c)  => features::context::run(c.cmd),
        Command::Buzz        => features::buzz::run(),
        Command::Gain        => analytics::gain(),
    }
}

fn trim_cmd(_args: cli::TrimArgs) -> anyhow::Result<()> {
    #[cfg(feature = "trim")]
    return features::trim::run(_args);
    #[allow(unreachable_code)]
    anyhow::bail!("ccb built without 'trim'. Rebuild: cargo build --features trim")
}

fn fade_cmd(_args: cli::FadeArgs) -> anyhow::Result<()> {
    #[cfg(feature = "fade")]
    return features::fade::run(_args);
    #[allow(unreachable_code)]
    anyhow::bail!("ccb built without 'fade'. Rebuild: cargo build --features fade")
}

fn style_cmd(cmd: StyleCmd) -> anyhow::Result<()> {
    match cmd {
        StyleCmd::IndexBuild => {
            let skills_dir = dirs::home_dir().unwrap_or_default().join(".claude").join("skills");
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
