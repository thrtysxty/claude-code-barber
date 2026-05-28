//! Demo mode — renders sample sessions to showcase the statusline

use super::session::{
    ContextWindow, FiveHourLimit, GitState, Model, RateLimits, SessionInfo, SevenDayLimit, Skill,
    SubAgent, Task,
};
use super::themes::Theme;

/// Run demo mode with optional scenario filter
pub fn run(scenario: Option<&str>, theme: &Theme, columns: usize) -> anyhow::Result<()> {
    let scenarios = get_scenarios();
    let all_names: Vec<String> = scenarios.iter().map(|sc| sc.name.clone()).collect();

    let to_run: Vec<&Scenario> = match scenario {
        Some(s) => scenarios.iter().filter(|sc| sc.name == s).collect(),
        None => scenarios.iter().collect(),
    };

    if to_run.is_empty() {
        eprintln!("No scenarios found matching: {:?}", scenario);
        eprintln!("Available: {:?}", all_names);
        return Ok(());
    }

    for sc in &to_run {
        print!("\n\n=== {} ===\n\n", sc.name);
        let output = super::renderer::render(&sc.session, theme, columns, "wide");
        print!("{output}");
    }

    Ok(())
}

struct Scenario {
    name: String,
    session: SessionInfo,
}

fn make_session(
    model_id: &str,
    model_name: &str,
    total_in: u64,
    total_out: u64,
    five_pct: f64,
    seven_pct: f64,
) -> SessionInfo {
    SessionInfo {
        session_id: "demo-session-12345678".to_string(),
        transcript_path: Some("/home/user/.claude/projects/demo/12345678.jsonl".to_string()),
        cwd: Some("/home/user/my-project".to_string()),
        model: Model {
            id: model_id.to_string(),
            display_name: Some(model_name.to_string()),
        },
        workspace: None,
        current_date: Some("2026-05-24".to_string()),
        current_time: Some("14:32:01".to_string()),
        version: Some("2.1.117".to_string()),
        output_style: None,
        cost: Some(super::session::Cost {
            total_cost_usd: Some(0.23),
            total_duration_ms: Some(120_000),
            total_api_duration_ms: Some(2_500),
            total_lines_added: Some(342),
            total_lines_removed: Some(89),
        }),
        context_window: ContextWindow {
            total_input_tokens: total_in,
            total_output_tokens: total_out,
            context_window_size: Some(200_000),
            current_usage: None,
            used_percentage: Some((total_in as f64 / 200_000.0) * 100.0),
            remaining_percentage: Some(100.0 - (total_in as f64 / 200_000.0) * 100.0),
        },
        exceeds_200k_tokens: Some(false),
        rate_limits: Some(RateLimits {
            five_hour: FiveHourLimit {
                used_percentage: five_pct,
                resets_at: Some(1776870000),
            },
            seven_day: SevenDayLimit {
                used_percentage: seven_pct,
                resets_at: Some(1777035600),
            },
        }),
        skills: None,
        enabled_plugins: None,
        tasks: None,
        subagents: None,
        openspec_changes: None,
        git: Some(GitState {
            branch: Some("feat/new-feature".to_string()),
            is_dirty: Some(true),
            commit_hash: Some("a1b2c3d4e5f6".to_string()),
            commit_message: None,
            ahead: Some(2),
            behind: Some(0),
        }),
        sparkline_data: Some(vec![0.2, 0.35, 0.28, 0.45, 0.6, 0.55, 0.7, 0.65, 0.8, 0.75]),
        thinking: Some(super::session::Thinking {
            enabled: Some(true),
        }),
        effort: Some(super::session::Effort {
            level: Some("high".to_string()),
        }),
        fast_mode: None,
    }
}

fn get_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "sonnet-thinking".to_string(),
            session: {
                let mut s = make_session(
                    "claude-sonnet-4-6",
                    "Sonnet 4.6",
                    85_000,
                    12_000,
                    30.0,
                    20.0,
                );
                s.skills = Some(vec![
                    Skill {
                        skill: "grill-me".to_string(),
                    },
                    Skill {
                        skill: "caveman".to_string(),
                    },
                ]);
                s
            },
        },
        Scenario {
            name: "opus-thinking".to_string(),
            session: {
                let mut s =
                    make_session("claude-opus-4-7", "Opus 4.7", 180_000, 15_000, 52.0, 41.0);
                s.skills = Some(vec![
                    Skill {
                        skill: "grill-me".to_string(),
                    },
                    Skill {
                        skill: "caveman".to_string(),
                    },
                    Skill {
                        skill: "tdd".to_string(),
                    },
                ]);
                s.effort = Some(super::session::Effort {
                    level: Some("high".to_string()),
                });
                s.thinking = Some(super::session::Thinking {
                    enabled: Some(true),
                });
                s
            },
        },
        Scenario {
            name: "tasks".to_string(),
            session: {
                let mut s =
                    make_session("claude-sonnet-4-6", "Sonnet 4.6", 60_000, 8_000, 22.0, 15.0);
                s.tasks = Some(vec![
                    Task {
                        task_id: Some("1".to_string()),
                        subject: Some("Audit gradient palette".to_string()),
                        active_form: Some("Auditing gradient palette".to_string()),
                        status: Some("completed".to_string()),
                    },
                    Task {
                        task_id: Some("2".to_string()),
                        subject: Some("Wire alert-mode pill".to_string()),
                        active_form: Some("Wiring alert-mode pill".to_string()),
                        status: Some("completed".to_string()),
                    },
                    Task {
                        task_id: Some("3".to_string()),
                        subject: Some("Refactor border math".to_string()),
                        active_form: Some("Refactoring border math".to_string()),
                        status: Some("in_progress".to_string()),
                    },
                    Task {
                        task_id: Some("4".to_string()),
                        subject: Some("Update CONTEXT.md".to_string()),
                        active_form: Some("Updating CONTEXT.md".to_string()),
                        status: Some("pending".to_string()),
                    },
                ]);
                s
            },
        },
        Scenario {
            name: "subagents".to_string(),
            session: {
                let mut s =
                    make_session("claude-opus-4-7", "Opus 4.7", 190_000, 20_000, 46.0, 37.0);
                s.subagents = Some(vec![
                    SubAgent {
                        name: Some("explore".to_string()),
                        agent_type: Some("explore".to_string()),
                        description: Some("Search codebase for token tracking".to_string()),
                        billed_in: Some(3_200),
                        output_tokens: Some(420),
                        transcript_path: None,
                    },
                    SubAgent {
                        name: Some("general-purpose".to_string()),
                        agent_type: Some("general-purpose".to_string()),
                        description: Some("Fix sparkline bucket algorithm".to_string()),
                        billed_in: Some(8_700),
                        output_tokens: Some(1_850),
                        transcript_path: None,
                    },
                    SubAgent {
                        name: Some("claude".to_string()),
                        agent_type: Some("claude".to_string()),
                        description: Some("Review border math implementation".to_string()),
                        billed_in: Some(5_400),
                        output_tokens: Some(980),
                        transcript_path: None,
                    },
                ]);
                s
            },
        },
        Scenario {
            name: "kitchen-sink".to_string(),
            session: {
                let mut s =
                    make_session("claude-opus-4-7", "Opus 4.7", 195_000, 25_000, 58.0, 49.0);
                s.skills = Some(vec![
                    Skill {
                        skill: "grill-me".to_string(),
                    },
                    Skill {
                        skill: "caveman".to_string(),
                    },
                    Skill {
                        skill: "tdd".to_string(),
                    },
                    Skill {
                        skill: "rocky:rocky".to_string(),
                    },
                ]);
                s.tasks = Some(vec![
                    Task {
                        task_id: Some("1".to_string()),
                        subject: Some("Audit gradient palette".to_string()),
                        active_form: Some("Auditing gradient palette".to_string()),
                        status: Some("completed".to_string()),
                    },
                    Task {
                        task_id: Some("2".to_string()),
                        subject: Some("Wire alert-mode pill".to_string()),
                        active_form: Some("Wiring alert-mode pill".to_string()),
                        status: Some("completed".to_string()),
                    },
                    Task {
                        task_id: Some("3".to_string()),
                        subject: Some("Refactor border math".to_string()),
                        active_form: Some("Refactoring border math".to_string()),
                        status: Some("in_progress".to_string()),
                    },
                    Task {
                        task_id: Some("4".to_string()),
                        subject: Some("Update CONTEXT.md".to_string()),
                        active_form: Some("Updating CONTEXT.md".to_string()),
                        status: Some("pending".to_string()),
                    },
                ]);
                s.subagents = Some(vec![
                    SubAgent {
                        name: Some("explore".to_string()),
                        agent_type: Some("explore".to_string()),
                        description: Some("Search codebase for token tracking".to_string()),
                        billed_in: Some(3_200),
                        output_tokens: Some(420),
                        transcript_path: None,
                    },
                    SubAgent {
                        name: Some("general-purpose".to_string()),
                        agent_type: Some("general-purpose".to_string()),
                        description: Some("Fix sparkline bucket algorithm".to_string()),
                        billed_in: Some(8_700),
                        output_tokens: Some(1_850),
                        transcript_path: None,
                    },
                    SubAgent {
                        name: Some("claude".to_string()),
                        agent_type: Some("claude".to_string()),
                        description: Some("Review border math implementation".to_string()),
                        billed_in: Some(5_400),
                        output_tokens: Some(980),
                        transcript_path: None,
                    },
                ]);
                s.git = Some(GitState {
                    branch: Some("feat/kitchen-sink".to_string()),
                    is_dirty: Some(true),
                    commit_hash: Some("deadbeef123456".to_string()),
                    commit_message: Some("add everything including the kitchen sink".to_string()),
                    ahead: Some(5),
                    behind: Some(1),
                });
                s
            },
        },
        Scenario {
            name: "full-context".to_string(),
            session: {
                let mut s = make_session(
                    "claude-sonnet-4-6",
                    "Sonnet 4.6",
                    196_000,
                    30_000,
                    71.0,
                    62.0,
                );
                s.exceeds_200k_tokens = Some(true);
                s
            },
        },
    ]
}
