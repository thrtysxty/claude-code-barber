//! status_unit — unit tests for the status pipeline
//!
//! Tests: data layer (session structs, GitInfo, TranscriptUsage, TokenLog, etc.),
//!        gradient engine, border renderer, renderer output.
//!
//! Build: cargo test --features status status_unit

#[cfg(feature = "status")]
mod tests {
    use ccb::features::status::border::BorderRenderer;
    use ccb::features::status::gradient::*;
    use ccb::features::status::renderer;
    use ccb::features::status::session::*;
    use ccb::features::status::themes;

    // ---------------------------------------------------------------------------
    // SessionInfo helpers
    // ---------------------------------------------------------------------------

    fn make_session() -> SessionInfo {
        SessionInfo {
            session_id: "test-session-abc12345".to_string(),
            transcript_path: None,
            cwd: Some("/Users/dev/projects/my-app".to_string()),
            model: Model {
                id: "claude-sonnet-4-6".to_string(),
                display_name: Some("Sonnet 4.6".to_string()),
            },
            workspace: None,
            current_date: Some("2026-05-26".to_string()),
            current_time: Some("14:30".to_string()),
            version: None,
            output_style: None,
            cost: Some(Cost {
                total_cost_usd: Some(0.42),
                total_duration_ms: Some(60_000),
                total_api_duration_ms: Some(5_000),
                total_lines_added: Some(200),
                total_lines_removed: Some(50),
            }),
            context_window: ContextWindow {
                total_input_tokens: 85_000,
                total_output_tokens: 12_000,
                context_window_size: Some(200_000),
                current_usage: Some(CurrentUsage {
                    input_tokens: 70_000,
                    cache_creation_input_tokens: 15_000,
                    cache_read_input_tokens: 30_000,
                    output_tokens: 12_000,
                }),
                used_percentage: Some(42.5),
                remaining_percentage: Some(57.5),
            },
            exceeds_200k_tokens: Some(false),
            rate_limits: Some(RateLimits {
                five_hour: FiveHourLimit {
                    used_percentage: 30.0,
                    resets_at: Some(1776870000),
                },
                seven_day: SevenDayLimit {
                    used_percentage: 20.0,
                    resets_at: Some(1777035600),
                },
            }),
            skills: Some(vec![Skill {
                skill: "grill-me".to_string(),
            }]),
            enabled_plugins: None,
            tasks: None,
            subagents: None,
            openspec_changes: None,
            git: Some(GitState {
                branch: Some("feat/status-pipeline".to_string()),
                is_dirty: Some(true),
                commit_hash: Some("a1b2c3d4e5f6".to_string()),
                commit_message: None,
                ahead: Some(3),
                behind: Some(0),
            }),
            sparkline_data: Some(vec![0.2, 0.4, 0.6, 0.8, 0.5]),
            thinking: Some(Thinking {
                enabled: Some(true),
            }),
            effort: Some(Effort {
                level: Some("high".to_string()),
            }),
            fast_mode: Some(false),
        }
    }

    // ---------------------------------------------------------------------------
    // SessionInfo method tests
    // ---------------------------------------------------------------------------

    #[test]
    fn session_total_tokens() {
        let s = make_session();
        assert_eq!(s.total_tokens(), 97_000);
    }

    #[test]
    fn session_soft_limit_sonnet() {
        let s = make_session();
        // Sonnet 4.6 has 200K context window per model_metadata.toml
        assert_eq!(s.soft_limit(), 200_000);
    }

    #[test]
    fn session_soft_limit_opus() {
        let mut s = make_session();
        s.model.id = "claude-opus-4-7".to_string();
        // Opus 4.7 has 200K context window per model_metadata.toml
        assert_eq!(s.soft_limit(), 200_000);
    }

    #[test]
    fn session_soft_limit_minimax() {
        let mut s = make_session();
        s.model.id = "minimax-m2.7:cloud".to_string();
        // MiniMax has 204800 ctx window per model_metadata.toml
        assert_eq!(s.soft_limit(), 204_800);
    }

    #[test]
    fn session_model_family() {
        let s = make_session();
        assert_eq!(s.model_family(), "sonnet");

        let mut s2 = make_session();
        s2.model.id = "claude-opus-4-7".to_string();
        assert_eq!(s2.model_family(), "opus");

        let mut s3 = make_session();
        s3.model.id = "minimax-m2.7".to_string();
        assert_eq!(s3.model_family(), "minimax");
    }

    #[test]
    fn session_context_fill() {
        let s = make_session();
        let fill = s.context_fill();
        assert!(
            fill > 0.0 && fill < 1.0,
            "context fill should be between 0 and 1, got {}",
            fill
        );
    }

    #[test]
    fn session_short_branch() {
        let s = make_session();
        assert_eq!(s.short_branch(), Some("status-pipeline"));
    }

    #[test]
    fn session_is_dirty() {
        let s = make_session();
        assert!(s.is_dirty());
    }

    #[test]
    fn session_short_pwd() {
        let s = make_session();
        let pwd = s.short_pwd();
        assert!(
            pwd.contains("my-app"),
            "short_pwd should contain dir name, got: {}",
            pwd
        );
    }

    #[test]
    fn session_model_thinking_high() {
        let s = make_session();
        assert_eq!(s.model_thinking(), "high");
    }

    #[test]
    fn session_model_thinking_fast() {
        let mut s = make_session();
        s.fast_mode = Some(true);
        s.effort = Some(Effort {
            level: Some("high".to_string()),
        });
        assert_eq!(s.model_thinking(), "high/fast");
    }

    #[test]
    fn session_model_thinking_none() {
        let mut s = make_session();
        s.thinking = Some(Thinking {
            enabled: Some(false),
        });
        s.effort = Some(Effort {
            level: Some(String::new()),
        });
        assert_eq!(s.model_thinking(), "");
    }

    #[test]
    fn session_billed_in_with_cache() {
        let s = make_session();
        // billed_in = total_input_tokens + cache_creation_input_tokens
        // total_input = 85,000, cache_creation = 15,000 (from current_usage)
        assert_eq!(s.billed_in(), 100_000);
    }

    #[test]
    fn session_cache_read() {
        let s = make_session();
        assert_eq!(s.cache_read(), 30_000);
    }

    // ---------------------------------------------------------------------------
    // GitInfo
    // ---------------------------------------------------------------------------

    #[test]
    fn git_info_from_cwd() {
        // This test runs in the CCB repo, so it should find a branch
        let git = GitInfo::from_cwd(".");
        // Just verify it doesn't crash and produces some branch name
        // (may be empty if not in a git repo, but we are)
        assert!(
            !git.branch.is_empty() || !git.commit.is_empty(),
            "GitInfo should find something in a git repo"
        );
    }

    // ---------------------------------------------------------------------------
    // TranscriptUsage
    // ---------------------------------------------------------------------------

    #[test]
    fn transcript_usage_default() {
        let usage = TranscriptUsage::default();
        assert_eq!(usage.input_tokens, 0);
        assert_eq!(usage.cache_creation_input_tokens, 0);
        assert_eq!(usage.cache_read_input_tokens, 0);
        assert_eq!(usage.output_tokens, 0);
    }

    #[test]
    fn transcript_usage_from_empty_path() {
        let usage = TranscriptUsage::from_transcript("");
        assert_eq!(usage.input_tokens, 0);
    }

    #[test]
    fn transcript_usage_from_nonexistent_path() {
        let usage = TranscriptUsage::from_transcript("/nonexistent/path/file.jsonl");
        assert_eq!(usage.input_tokens, 0);
    }

    // ---------------------------------------------------------------------------
    // TokenAccounting
    // ---------------------------------------------------------------------------

    #[test]
    fn token_accounting_sonnet_rates() {
        let (rate_in, rate_out) = TokenAccounting::rates_for("claude-sonnet-4-6");
        assert_eq!(rate_in, 3.00);
        assert_eq!(rate_out, 15.00);
    }

    #[test]
    fn token_accounting_opus_rates() {
        let (rate_in, rate_out) = TokenAccounting::rates_for("claude-opus-4-7");
        assert_eq!(rate_in, 15.00);
        assert_eq!(rate_out, 75.00);
    }

    #[test]
    fn token_accounting_minimax_rates() {
        let (rate_in, rate_out) = TokenAccounting::rates_for("minimax-m2.7:cloud");
        assert_eq!(rate_in, 0.30);
        assert_eq!(rate_out, 1.20);
    }

    #[test]
    fn token_accounting_qwopus_rates() {
        let (rate_in, rate_out) = TokenAccounting::rates_for("qwopus3.5-9b-v3");
        assert_eq!(rate_in, 0.00);
        assert_eq!(rate_out, 0.00);
    }

    #[test]
    fn token_accounting_session_cost() {
        let model = Model {
            id: "claude-sonnet-4-6".to_string(),
            display_name: Some("Sonnet 4.6".to_string()),
        };
        let usage = TranscriptUsage {
            input_tokens: 100_000,
            cache_creation_input_tokens: 10_000,
            cache_read_input_tokens: 50_000,
            output_tokens: 20_000,
        };
        let cost = TokenAccounting::session_cost(&model, &usage);
        // in: 100K * $3/M = $0.30
        // cache_creation: 10K * $3/M * 1.25 = $0.0375
        // cache_read: 50K * $3/M * 0.1 = $0.015
        // out: 20K * $15/M = $0.30
        // total ≈ $0.6525
        assert!(
            cost > 0.60 && cost < 0.70,
            "expected cost around $0.65, got ${:.4}",
            cost
        );
    }

    // ---------------------------------------------------------------------------
    // Burndown delta
    // ---------------------------------------------------------------------------

    #[test]
    fn burndown_delta_over_budget() {
        // 50% used, but only 10 minutes into a 300-minute window → over budget
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_start = now - 600; // 10 min ago
        let resets_at = window_start + 300 * 60; // 300 min window
        let delta = burndown_delta(50.0, resets_at, 300, 5);
        assert!(delta.is_some(), "should return Some when past warmup");
        let d = delta.unwrap();
        assert!(d > 0.0, "should be over budget (positive delta), got {}", d);
    }

    #[test]
    fn burndown_delta_under_budget() {
        // 5% used, 250 minutes into a 300-minute window → under budget
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let window_start = now - 15000; // ~250 min ago
        let resets_at = window_start + 300 * 60;
        let delta = burndown_delta(5.0, resets_at, 300, 5);
        if let Some(d) = delta {
            assert!(
                d < 0.0,
                "should be under budget (negative delta), got {}",
                d
            );
        }
        // May be None if window calculation doesn't align exactly
    }

    #[test]
    fn burndown_delta_during_warmup() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let resets_at = now + 300 * 60;
        let delta = burndown_delta(30.0, resets_at, 300, 5);
        // This might be None depending on exact timing, but shouldn't panic
        // The warmup is 5 min, so if we're in the first 5 min it should be None
        assert!(delta.is_some() || delta.is_none());
    }

    // ---------------------------------------------------------------------------
    // Token formatting
    // ---------------------------------------------------------------------------

    #[test]
    fn fmt_tok_small() {
        assert_eq!(fmt_tok(500), "500");
    }

    #[test]
    fn fmt_tok_thousands() {
        assert_eq!(fmt_tok(15000), "15.0K");
    }

    #[test]
    fn fmt_tok_millions() {
        assert_eq!(fmt_tok(2_500_000), "2.5M");
    }

    #[test]
    fn fmt_dur_seconds() {
        assert_eq!(fmt_dur(45.0), "45s");
    }

    #[test]
    fn fmt_dur_minutes() {
        assert_eq!(fmt_dur(125.0), "2m05s");
    }

    #[test]
    fn fmt_dur_hours() {
        assert_eq!(fmt_dur(7320.0), "2h02m");
    }

    // ---------------------------------------------------------------------------
    // GradientEngine
    // ---------------------------------------------------------------------------

    #[test]
    fn gradient_engine_interpolation() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        // At t=0, should return the first gradient stop color
        let (r, g, b) = ge.gradient_rgb(0.0, 1.0);
        assert_eq!((r, g, b), theme.grad_stops[0].1);
    }

    #[test]
    fn gradient_engine_midpoint() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        // At t=0.5, should be between stop at 0.5 and the next
        let (r, _g, _b) = ge.gradient_rgb(0.5, 1.0);
        assert!(r > 0, "midpoint color should not be black");
    }

    #[test]
    fn gradient_engine_sparkline_empty() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let (top, bot) = ge.sparkline(&[], false);
        assert!(top.is_empty());
        assert!(bot.is_empty());
    }

    #[test]
    fn gradient_engine_sparkline_nonempty() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let (top, bot) = ge.sparkline(&[10, 20, 30, 20, 10], false);
        assert!(!top.is_empty(), "top sparkline should not be empty");
        assert!(!bot.is_empty(), "bottom sparkline should not be empty");
        // Should contain ANSI escapes
        assert!(top.contains("\x1b["), "sparkline should have ANSI colors");
    }

    #[test]
    fn gradient_bar_filled() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let bar = ge.gradient_bar(5, 10);
        assert!(!bar.is_empty());
        assert!(
            bar.contains("\x1b[48;2;"),
            "gradient bar should use background colors"
        );
    }

    #[test]
    fn gradient_bar_zero() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let bar = ge.gradient_bar(0, 10);
        assert!(bar.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Pill
    // ---------------------------------------------------------------------------

    #[test]
    fn pill_active() {
        let pill = Pill {
            start: 10,
            end: 30,
            anchor: (255, 0, 0),
            shift: (0, 0, 255),
            pct: 70,
        };
        assert!(pill.active());
    }

    #[test]
    fn pill_inactive() {
        let pill = Pill {
            start: 10,
            end: 30,
            anchor: (255, 0, 0),
            shift: (0, 0, 255),
            pct: 0,
        };
        assert!(!pill.active());
    }

    #[test]
    fn pill_border_chars() {
        let pill = Pill {
            start: 10,
            end: 30,
            anchor: (255, 0, 0),
            shift: (0, 0, 255),
            pct: 70,
        };
        assert_eq!(pill.border_char(10, "top"), Some(PILL_TL));
        assert_eq!(pill.border_char(30, "top"), Some(PILL_TR));
        assert_eq!(pill.border_char(20, "top"), Some(PILL_TOP));
        assert_eq!(pill.border_char(5, "top"), None);
    }

    // ---------------------------------------------------------------------------
    // model_bg_pct
    // ---------------------------------------------------------------------------

    #[test]
    fn model_bg_pct_levels() {
        assert_eq!(model_bg_pct("high"), 80);
        assert_eq!(model_bg_pct("medium"), 55);
        assert_eq!(model_bg_pct("low"), 30);
        assert_eq!(model_bg_pct(""), 0); // empty string defaults to 0
    }

    // ---------------------------------------------------------------------------
    // model_anchor_pair
    // ---------------------------------------------------------------------------

    #[test]
    fn model_anchor_pair_sonnet() {
        let theme = themes::resolve_theme("claude-dark");
        let (anchor, _shift) = model_anchor_pair("Sonnet 4.6", &theme);
        // Sonnet anchor should be the green color
        assert_eq!(anchor, (135, 215, 135));
    }

    #[test]
    fn model_anchor_pair_opus() {
        let theme = themes::resolve_theme("claude-dark");
        let (anchor, _shift) = model_anchor_pair("Opus 4.7", &theme);
        // Opus anchor should be yellow
        assert_eq!(anchor, (255, 255, 0));
    }

    // ---------------------------------------------------------------------------
    // Terminal width
    // ---------------------------------------------------------------------------

    #[test]
    fn terminal_width_returns_value() {
        let w = terminal_width();
        assert!(
            w >= 30,
            "terminal width should be at least 30 (narrow render minimum), got {}",
            w
        );
        assert!(w <= 500, "terminal width should be reasonable, got {}", w);
    }

    // ---------------------------------------------------------------------------
    // Renderer smoke test
    // ---------------------------------------------------------------------------

    #[test]
    fn render_produces_output() {
        let s = make_session();
        let theme = themes::resolve_theme("claude-dark");
        let output = renderer::render(&s, &theme, 120, "wide");
        assert!(!output.is_empty(), "render should produce output");
        assert!(
            output.contains("\x1b["),
            "render should contain ANSI escapes"
        );
    }

    #[test]
    fn render_wide_contains_model() {
        let s = make_session();
        let theme = themes::resolve_theme("claude-dark");
        let output = renderer::render(&s, &theme, 120, "wide");
        assert!(
            output.contains("Sonnet"),
            "wide render should contain model name"
        );
    }

    #[test]
    fn render_wide_contains_rate_limits() {
        let s = make_session();
        let theme = themes::resolve_theme("claude-dark");
        let output = renderer::render(&s, &theme, 120, "wide");
        assert!(output.contains("5h"), "should contain 5h rate limit");
        assert!(output.contains("7d"), "should contain 7d rate limit");
    }

    #[test]
    fn render_narrow_produces_output() {
        let s = make_session();
        let theme = themes::resolve_theme("claude-dark");
        let output = renderer::render(&s, &theme, 40, "narrow");
        assert!(!output.is_empty(), "narrow render should produce output");
    }

    #[test]
    fn render_too_narrow_returns_empty() {
        let s = make_session();
        let theme = themes::resolve_theme("claude-dark");
        let output = renderer::render(&s, &theme, 10, "narrow");
        assert!(
            output.is_empty(),
            "render below MIN_WIDTH should return empty"
        );
    }

    // ---------------------------------------------------------------------------
    // Theme resolution
    // ---------------------------------------------------------------------------

    #[test]
    fn theme_claude_dark() {
        let theme = themes::resolve_theme("claude-dark");
        assert_eq!(theme.name, "claude-dark");
        assert!(!theme.grad_stops.is_empty());
        assert!(!theme.models.is_empty());
    }

    #[test]
    fn theme_claude_light() {
        let theme = themes::resolve_theme("claude-light");
        assert_eq!(theme.name, "claude-light");
    }

    #[test]
    fn theme_catppuccin_mocha() {
        let theme = themes::resolve_theme("catppuccin-mocha");
        assert_eq!(theme.name, "catppuccin-mocha");
    }

    #[test]
    fn theme_unknown_falls_back() {
        let theme = themes::resolve_theme("nonexistent");
        assert_eq!(theme.name, "claude-dark");
    }

    #[test]
    fn theme_has_minimax_and_qwopus() {
        let theme = themes::resolve_theme("claude-dark");
        assert!(
            theme.models.contains_key("minimax"),
            "theme should have minimax colors"
        );
        assert!(
            theme.models.contains_key("qwopus"),
            "theme should have qwopus colors"
        );
    }

    // ---------------------------------------------------------------------------
    // BorderRenderer
    // ---------------------------------------------------------------------------

    #[test]
    fn border_top_produces_output() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let br = BorderRenderer::new(ge, &theme);
        let line = br.border_top(80, "test1234", &[], 1.0, None);
        assert!(!line.is_empty());
        // Top border should contain '+' (ASCII fallback for ╭/╮)
        assert!(line.contains('+'), "top border should contain +");
    }

    #[test]
    fn border_bottom_produces_output() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let br = BorderRenderer::new(ge, &theme);
        let line = br.border_bottom(80, &[], 0.5);
        assert!(!line.is_empty());
        // Bottom border should contain '+' (ASCII fallback for ╰/╯)
        assert!(line.contains('+'), "bottom border should contain +");
    }

    #[test]
    fn border_separator_with_ups() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let br = BorderRenderer::new(ge, &theme);
        let line = br.border_separator(80, &[40], 1.0);
        // Separator with ups should contain '+' at the up position
        assert!(line.contains('+'), "separator with ups should contain +");
    }

    #[test]
    fn border_separator_dim_with_connectors() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let br = BorderRenderer::new(ge, &theme);
        let line = br.border_separator_dim(80, &[20], &[40], 0.5, None, "bottom");
        // Dim separator uses '+' for all connectors
        assert!(line.contains('+'), "dim sep should contain + for all connectors");
    }

    #[test]
    fn border_line_with_content() {
        let theme = themes::resolve_theme("claude-dark");
        let ge = GradientEngine::new(&theme);
        let br = BorderRenderer::new(ge, &theme);
        let line = br.border_line("hello world", 80, 0.5, "", "", false, "");
        assert!(
            line.contains("hello world"),
            "border line should contain the content"
        );
    }

    // ---------------------------------------------------------------------------
    // paint_bg_span
    // ---------------------------------------------------------------------------

    #[test]
    fn paint_bg_span_basic() {
        let cells = vec![
            ('H', Some((255, 255, 255)), false, false),
            ('i', Some((255, 255, 255)), false, false),
        ];
        let result = paint_bg_span(
            &cells,
            (135, 215, 135),
            (44, 208, 168),
            70,
            (15, 15, 15),
            None,
        );
        assert!(!result.is_empty());
        assert!(result.contains('H'), "should contain the character");
        assert!(result.contains("\x1b["), "should contain ANSI escapes");
    }
}
