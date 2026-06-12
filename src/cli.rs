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
    #[cfg(feature = "context")]
    Context(ContextArgs),
    /// Run all active features at maximum compression
    Cut,
    /// Check plugin auth status (GitHub MCP, Cloudflare OAuth)
    #[cfg(feature = "plugins")]
    Plugins,
    /// Show what's currently loaded in the context budget
    Lineup,
    /// Configure ccb — build index, set conversation mode, toggle features
    Style(StyleArgs),
    /// Nuclear option — maximum token reduction across all features
    Buzz,
    /// Show token savings analytics
    Gain(GainArgs),
    /// Wire ccb hooks into ~/.claude/settings.json (use --auto to apply without prompting)
    Install(InstallArgs),
    /// Manage expert personas and the knowledge graph (requires --features expert)
    #[cfg(feature = "expert")]
    Expert(ExpertArgs),
    /// Build and query a code symbol graph (requires --features graph)
    #[cfg(feature = "graph")]
    Graph(GraphArgs),
    /// List all available models from the running router
    Models,
    /// Model router — routes Claude Code API calls to local or Anthropic backends
    #[cfg(feature = "route")]
    Route(RouteArgs),
    /// Classify tool calls for safety (reads hook JSON from stdin)
    #[cfg(feature = "classify")]
    Classify,
    /// Render the status line for Claude Code
    #[cfg(feature = "status")]
    Status(StatusArgs),
    /// Run deterministic story loops through planning and implementation phases
    #[cfg(feature = "factory")]
    Factory(FactoryArgs),
    /// Plan, build, and verify story implementation
    #[cfg(feature = "loop")]
    Loop(LoopArgs),
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
    /// Inject structured context at a hook point (session-start | pre-tool)
    Inject {
        /// Hook type: session-start or pre-tool
        #[arg(long)]
        hook: String,
        /// Tool name (for pre-tool hook)
        #[arg(long)]
        tool: Option<String>,
        /// Tool input as JSON string (for pre-tool hook)
        #[arg(long)]
        input: Option<String>,
        /// Read hook payload from stdin instead of --tool/--input flags
        #[arg(long)]
        stdin: bool,
    },
    /// Trace a tool call result (PostToolUse hook)
    Trace,
    /// Tune context node weights from session traces (EMA update)
    Tune {
        /// Dry run: show proposed changes without applying them
        #[arg(long)]
        dry_run: bool,
        /// Run LoCoMo validation after applying changes
        #[arg(long)]
        validate: bool,
        /// Override the validation threshold (percentage points)
        #[arg(long)]
        threshold: Option<f64>,
        /// Override the EMA decay factor (default 0.7)
        #[arg(long)]
        alpha: Option<f64>,
    },
    /// Detect gaps: built-but-unused experts, missing coverage for active domains
    Gaps {
        /// Minimum sessions before flagging as gap (default 3)
        #[arg(long)]
        min_sessions: Option<i64>,
        /// Apply a suggestion by gap ID
        #[arg(long)]
        apply: Option<i64>,
    },
    /// Show weight distribution, top gainers/losers, active gaps, and LoCoMo trend
    Report {
        /// Output format: human or json (default human)
        #[arg(long, value_enum, default_value = "human")]
        format: ContextReportFormat,
        /// Filter by node name
        #[arg(long)]
        node: Option<String>,
    },
}

#[derive(clap::ValueEnum, Clone, Default, Debug)]
pub enum ContextReportFormat {
    #[default]
    Human,
    Json,
}

#[derive(Args)]
pub struct TrimArgs {
    pub cmd: Vec<String>,
}

#[derive(Args)]
pub struct GainArgs {
    #[arg(long)]
    pub ab: bool,
    #[arg(long)]
    pub expert: bool,
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

// ---------------------------------------------------------------------------
// Expert / knowledge graph commands
// ---------------------------------------------------------------------------

/// Manage expert personas and the knowledge graph (requires --features expert)
#[cfg(feature = "expert")]
#[derive(Subcommand)]
pub enum ExpertCmd {
    /// Build knowledge graph from a dataset file
    Build {
        name: String,
        #[arg(long)]
        dataset: std::path::PathBuf,
    },
    /// Ingest YAML dataset file into the knowledge graph
    Ingest {
        #[arg(long)]
        dataset: std::path::PathBuf,
    },
    /// Activate a persona — makes it available to hooks
    Activate { name: String },
    /// Deactivate the current persona
    Deactivate,
    /// List all registered experts and active status
    List,
    /// Traverse the graph from a task description
    Walk { task: String },
    /// Query active persona — for hook consumption
    Query {
        #[arg(long)]
        tool: Option<String>,
        #[arg(long, value_enum, default_value = "json")]
        format: ExpertOutputFormatArg,
    },
    /// Export expert graph as instruction-tuning pairs for LoRA training
    Export {
        persona: String,
        #[arg(long, default_value = "alpaca")]
        format: ExportFormat,
        #[arg(long)]
        output: std::path::PathBuf,
    },
}

/// Output format for expert commands
#[cfg(feature = "expert")]
#[derive(clap::ValueEnum, Clone)]
pub enum ExpertOutputFormatArg {
    Human,
    Json,
}

/// Export format for instruction-tuning datasets
#[cfg(feature = "expert")]
#[derive(clap::ValueEnum, Clone)]
pub enum ExportFormat {
    Alpaca,
    Sharegpt,
}

/// Manage expert personas and the knowledge graph (requires --features expert)
#[cfg(feature = "expert")]
#[derive(Args)]
pub struct ExpertArgs {
    #[command(subcommand)]
    pub cmd: ExpertCmd,
}

#[derive(Args)]
pub struct InstallArgs {
    /// Apply changes without interactive confirmation
    #[arg(long)]
    pub auto: bool,
    /// Dry run: show what would be installed without applying
    #[arg(long)]
    pub dry_run: bool,
}

/// Route arguments (requires --features route)
#[cfg(feature = "route")]
#[derive(Args)]
pub struct RouteArgs {
    #[command(subcommand)]
    pub cmd: RouteCmd,
}

/// Route subcommands
#[cfg(feature = "route")]
#[derive(Subcommand)]
pub enum RouteCmd {
    /// Start the router on a specific port
    Start {
        #[arg(default_value = "9001")]
        port: u16,
    },
    /// Stop the router
    Stop,
    /// Show router status
    Status,
    /// Print export block for shell
    Env,
    /// Show tier routing table with resolved model → provider mappings
    Tiers {
        /// Test which model would handle a specific tier (opus, sonnet, haiku)
        #[arg(long)]
        test: Option<String>,
    },
}

// ─────────────────────────────────────────────────────────────────────────────
// Factory / story loop commands
// ─────────────────────────────────────────────────────────────────────────────

/// Factory subcommands
#[cfg(feature = "factory")]
#[derive(Subcommand)]
pub enum FactoryCmd {
    /// Create a new story in the backlog
    New {
        /// Story title
        title: String,
        /// Planning or implementation loop
        #[arg(short, long, default_value = "planning")]
        loop_type: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Advance story to next state (requires matching expert active)
    Advance {
        /// Story ID
        story_id: String,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Kick story back to previous state (sentinel/architect authority)
    Kickback {
        /// Story ID
        story_id: String,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Escalate story for architect/sentinel review
    Escalate {
        /// Story ID
        story_id: String,
        /// Escalation target
        target: String,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Approve story and advance to next state
    Approve {
        /// Story ID
        story_id: String,
        /// Optional note
        #[arg(short, long)]
        note: Option<String>,
    },
    /// Show story status and history
    Status {
        /// Story ID
        story_id: String,
    },
    /// List all stories (optionally filtered by loop)
    List {
        /// Filter by loop type: planning | implementation
        #[arg(short, long)]
        loop_type: Option<String>,
    },
    /// Show the state machine for a loop
    Show {
        /// planning | implementation
        #[arg(default_value = "planning")]
        loop_type: String,
    },
}

/// Factory args (requires --features factory)
#[cfg(feature = "factory")]
#[derive(Args)]
pub struct FactoryArgs {
    #[command(subcommand)]
    pub cmd: FactoryCmd,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loop / story implementation commands
// ─────────────────────────────────────────────────────────────────────────────

/// Loop subcommands — plan, build, and verify story implementation
#[cfg(feature = "loop")]
#[derive(Subcommand)]
pub enum LoopCmd {
    /// Detect repo type from CWD
    Detect {
        #[arg(long, value_enum, default_value = "human")]
        format: DetectFormat,
    },
    /// Parse story file, extract ACs, output phased JSON plan
    Plan {
        /// Path to the story markdown file
        story_file: std::path::PathBuf,
        /// Save plan to ~/.cache/ccb/plans/
        #[arg(long)]
        save: bool,
    },
    /// Implement story phases with gate checks
    Build {
        /// Path to a plan JSON file (omits LLM call, uses saved plan)
        #[arg(long)]
        plan: Option<std::path::PathBuf>,
        /// Story file path (extracts plan inline)
        #[arg(long)]
        story: Option<std::path::PathBuf>,
    },
    /// Capture a lesson to the cache for future recall
    Lesson {
        /// What was learned
        description: String,
    },
    /// Show gate sequence for detected repo type
    Gates {
        /// Actually run the gates instead of just listing
        #[arg(long)]
        run: bool,
    },
}

/// Output format for detect command
#[cfg(feature = "loop")]
#[derive(clap::ValueEnum, Clone)]
pub enum DetectFormat {
    Human,
    Json,
}

/// Loop args (requires --features loop)
#[cfg(feature = "loop")]
#[derive(Args)]
pub struct LoopArgs {
    #[command(subcommand)]
    pub cmd: LoopCmd,
}

/// Detect args
#[cfg(feature = "loop")]
#[derive(Args)]
pub struct DetectArgs {
    #[arg(long, value_enum, default_value = "human")]
    pub format: DetectFormat,
}

/// Plan args
#[cfg(feature = "loop")]
#[derive(Args)]
pub struct PlanArgs {
    /// Path to the story markdown file
    pub story_file: std::path::PathBuf,
    /// Save plan to ~/.cache/ccb/plans/
    #[arg(long)]
    pub save: bool,
}

/// Build args
#[cfg(feature = "loop")]
#[derive(Args)]
pub struct BuildArgs {
    /// Path to a plan JSON file (omits LLM call, uses saved plan)
    #[arg(long)]
    pub plan: Option<std::path::PathBuf>,
    /// Story file path (extracts plan inline)
    #[arg(long)]
    pub story: Option<std::path::PathBuf>,
}

/// Lesson args
#[cfg(feature = "loop")]
#[derive(Args)]
pub struct LessonArgs {
    /// What was learned
    pub description: String,
}

/// Gates args
#[cfg(feature = "loop")]
#[derive(Args)]
pub struct GatesArgs {
    /// Actually run the gates instead of just listing
    #[arg(long)]
    pub run: bool,
}

/// Status subcommands
#[cfg(feature = "status")]
#[derive(Subcommand)]
pub enum StatusCmd {
    /// Render the statusline once to stdout (live session data or CCB fallback)
    Show,
    /// Render with mock/demo data — no live session needed
    Demo {
        /// Scenario name to run (e.g. sonnet-thinking, kitchen-sink)
        #[arg(default_value = "")]
        scenario: String,
    },
    /// Enter live-refresh monitoring mode
    Mon {
        /// Directory to watch (defaults to ~/.claude)
        #[arg(default_value = "")]
        directory: String,
    },
}

/// Status args — StatusCmd with default to Show
#[cfg(feature = "status")]
#[derive(Args)]
pub struct StatusArgs {
    #[command(subcommand)]
    pub cmd: Option<StatusCmd>,
}

#[cfg(feature = "status")]
#[allow(clippy::derivable_impls)]
impl Default for StatusArgs {
    fn default() -> Self {
        Self { cmd: None }
    }
}
