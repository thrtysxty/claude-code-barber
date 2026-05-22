use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "ccb",
    about = "Claude Code Barber — your AI's context, styled.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Compress noisy command output before it hits the context window
    Trim(TrimArgs),
    /// Lazy-load a skill, persona, or MCP resource on demand
    Fade(FadeArgs),
    /// Monitor context window usage, suggest /clear or /compact
    Context(ContextArgs),
    /// Run all active features at maximum compression
    Cut,
    /// Show what's currently loaded in the context budget
    Lineup,
    /// Configure ccb — build index, set conversation mode, toggle features
    Style(StyleArgs),
    /// Nuclear option — maximum token reduction across all features
    Buzz,
    /// Show token savings analytics
    Gain,
    /// Build and query a code symbol graph (requires --features graph)
    #[cfg(feature = "graph")]
    Graph(GraphArgs),
}

#[derive(Args)]
pub struct StyleArgs {
    #[command(subcommand)]
    pub cmd: StyleCmd,
}

#[derive(Subcommand)]
pub enum StyleCmd {
    /// Scan ~/.claude/skills/ and regenerate INDEX.md
    IndexBuild,
    /// Show current ccb config
    Show,
}

#[derive(Args)]
pub struct ContextArgs {
    #[command(subcommand)]
    pub cmd: ContextCmd,
}

#[derive(Subcommand)]
pub enum ContextCmd {
    /// Show current context window usage
    Show,
    /// Suggest /clear when context exceeds threshold
    Clear {
        #[arg(default_value = "80")]
        threshold: u8,
    },
    /// Suggest /compact when context exceeds threshold
    Compact {
        #[arg(default_value = "60")]
        threshold: u8,
    },
}

#[derive(Args)]
pub struct TrimArgs {
    pub cmd: Vec<String>,
}

#[derive(Args)]
pub struct FadeArgs {
    pub resource: Option<String>,
}

/// Build and query a code symbol graph (requires --features graph)
#[cfg(feature = "graph")]
#[derive(Subcommand)]
pub enum GraphCmd {
    /// Index a directory into the code graph
    Index {
        #[arg(default_value = ".")]
        path: std::path::PathBuf,
    },
    /// Search symbols by name pattern
    Search {
        pattern: String,
        #[arg(long, value_enum, default_value = "human")]
        format: OutputFormatArg,
    },
    /// Show all symbols in a file
    Show {
        file: std::path::PathBuf,
        #[arg(long, value_enum, default_value = "human")]
        format: OutputFormatArg,
    },
    /// Print graph statistics
    Stats {
        #[arg(long, value_enum, default_value = "human")]
        format: OutputFormatArg,
    },
}

/// Build and query a code symbol graph (requires --features graph)
#[cfg(feature = "graph")]
#[derive(Args)]
pub struct GraphArgs {
    #[command(subcommand)]
    pub cmd: GraphCmd,
}

#[cfg(feature = "graph")]
#[derive(clap::ValueEnum, Clone)]
pub enum OutputFormatArg {
    Human,
    Json,
}
