# CCB Statusline — Complete Parity Plan

**Generated:** 2026-05-27
**Reference:** `yet-another-statusline/claude/statusline_command.py` (2929 lines)
**Target:** `claude-code-barber/src/features/status/` (Rust)

---

## 1. What Exists in YAS (Python Reference)

### Core Data Layer (`statusline_command.py` lines 1–600)
- **SessionInfo**: Full deserialization of Claude Code session JSON
- **GitInfo**: Reads `.git/HEAD` directly + `git status --porcelain` for dirty counts (modified, untracked, deleted, renamed)
- **TranscriptUsage**: Parses JSONL for per-message token breakdowns (input, cache_creation, cache_read, output) with message-ID dedup
- **TokenLog**: Persistent daily token tracking (`~/.claude/statusline-tokens.log`), session-keyed upsert, day totals
- **TokenRate**: Rolling-window throughput (60s window, 300s keep), sparkline history (n_buckets), recently_active check
- **RunningSubagents**: Discovers subagent JSON+JSONL from filesystem, parses last activity (tool_use/thinking/text), token counts, model
- **TaskList**: Extracts TaskCreate/TaskUpdate lifecycle from transcript JSONL with freshness cap (120s) and grace period (20s)
- **LoadedSkills**: Regex extraction of Skill invocations from transcript
- **OpenSpec**: Walks up from cwd for `openspec/` directory, counts `- [x]`/`- [ ]` checkboxes in `tasks.md` files, skips `/archive/`
- **TokenAccounting**: Per-model cost rates from model_metadata, cache weighting (creation 1.25x, read 0.1x), session + day cost
- **burndown_delta**: Rate limit trend = actual% - ideal linear burn%

### Gradient Engine (`statusline_command.py` lines 600–900)
- **GradientEngine**: Multi-stop color interpolation for borders
- **gradient_bar**: Filled portion with per-cell gradient color
- **sparkline**: Two-row (top/bottom half-block) sparkline with spark_stops gradient
- **pill_gradient_fg**: Edge char (▐/▌) foreground gradient based on effort level
- **model_bg_pct**: Effort→percentage mapping (high=80, medium=50, low=20)
- **model_anchor_pair**: Resolves model family → (anchor_rgb, shift_rgb) from theme
- **empty_fade_colors**: 3-step brightness fade at fill boundary
- **spec_gradient_bar**: 3-stop gradient for OpenSpec progress bars
- **rainbow_step / rainbow_at**: Time-cycling rainbow palette for glyph coloring

### Border Renderer (`statusline_command.py` lines 900–1100)
- **border_top**: Top border with optional T-junction down connectors + pill overlay
- **border_bottom**: Bottom border with up connectors
- **border_separator**: Full-brightness separator with up connectors
- **border_separator_dim**: Dim separator with down/up connectors + optional pill overlay
- **border_line**: Content row with gradient left/right borders, optional pill flush, optional right_pill alignment
- **dim_for_col**: Per-column gradient dimming based on fill ratio

### Layout/Rendering (`statusline_command.py` lines 1100–2400)
- **RowSpec / LayoutSpec**: Declarative row assembly with kind (top_border/bottom_border/separator/separator_dim/content)
- **render_pill**: Model badge with ▐/▌ gradient edges, effort-based background blend, luminance-adaptive foreground
- **fit_path**: Progressive path degradation (full → abbreviated → middle-ellipsis → tail-only)
- **_middle_ellipsis**: Truncates middle of string with "…" to fit width
- **context_line**: Gradient bar + 3-step fade + token/limit label + rate limit percentages
- **context_line_compact**: Abbreviated version for medium layout
- **tokens_cost**: Two-row aligned layout with VSEPs: row1=session tokens+cost+sparkline, row2=day tokens+day cost+sparkline
- **model_right_section**: Helper glyph + rate limits + time-to-reset + 7d + burndown trend
- **model_right_section_compact**: Abbreviated for medium layout
- **model_section_compact**: Pill + tokens + bar + rate% + branch + cost (medium single-line)
- **path_git**: Full path line with folder glyph, branch, dirty indicator, commit hash, elapsed time
- **subagent_row**: Wide=two-line (header + metrics), narrow=single-line with rainbow markers
- **task_row**: Rainbow task glyph + completed/total + active task label
- **plugins_skills**: Rainbow glyphs + comma-separated names, truncated to fit
- **openspec_bar**: Per-story 3-stop gradient progress bar with name + done/total + percentage
- **burndown_trend**: Arrow glyph (▲/▼/→) colored by delta magnitude
- **helper**: Rate limit text with time-to-reset formatting

### Layout Modes
- **build_narrow** (<55 cols): Pill + tokens + rate% (single line, no border)
- **build_medium** (55–80 cols): Bordered, pill + tokens + bar + rate% + branch + cost + optional tasks + context bar
- **build_wide** (80+ cols): Full bordered layout with all sections — path, model pill + thinking + effort, helper, two-row tokens/cost with aligned VSEPs, context bar with 3-step fade, skills/plugins, tasks, subagents (two-line when >100 cols), openspec bars

### Themes (`themes.py` — 481 lines)
- 4 themes: claude-dark, claude-light, catppuccin-latte, catppuccin-mocha
- ~30 color slots per theme (border, pwd, branch, commit, session, skills, time, tok, cost, bar_fill, bar_empty, safe/warn/alert/yellow, etc.)
- Per-model ModelColors: opus, sonnet, haiku, minimax, qwopus, other — each with anchor, warm_shift, cool_shift, label
- pill_fg_dark/pill_fg_light for luminance-adaptive pill text
- grad_stops (5 stops), spark_stops (3 stops), spec_gradients (12×3 RGB), grey_rgb, bar_empty_rgb
- Resolution: CLI arg → env var → config file → claude-dark default

### Demo (`demo.py` — 710 lines)
- 7 hermetic scenarios: sonnet-thinking, opus-thinking, tasks, openspec, subagents, kitchen-sink, full-context
- Animation mode: 60 steps with interpolated token growth
- Snapshot mode: static renders at each scenario

### Monitor (`mon/` — 4 modules)
- **discovery**: Finds session JSON files under `~/.claude/projects/`
- **layout**: Multi-session TUI with header, session boxes, footer, height clipping
- **lifecycle**: Age-tier classification (bright/dim/removed), watcher thread
- **tui**: Alternate screen buffer, cursor control, refresh loop

---

## 2. What Exists in CCB Rust (Current State)

### session.rs — 1349 lines — COMPLETE
All YAS data structures and filesystem readers are ported:
- SessionInfo with all fields + derived helpers (total_tokens, billed_in, cache_read, soft_limit, context_fill, model_family, short_pwd, model_thinking, plugin_names)
- GitInfo with read_head + dirty_counts
- TranscriptUsage with message-ID dedup
- TokenLog with session-keyed upsert + day totals
- TokenRate with rolling window + sparkline history + recently_active
- RunningSubagent discovery + transcript parsing + SubagentActivity enum
- TaskList with freshness cap + grace period
- LoadedSkills with regex extraction
- OpenSpec discovery + checkbox counting
- TokenAccounting with cache weighting
- burndown_delta
- fmt_tok, fmt_dur, elapsed_from_transcript

**Status: FEATURE COMPLETE** — no gaps vs YAS.

### themes.rs — 481 lines — COMPLETE
All 4 themes ported with full color slot parity:
- claude-dark, claude-light, catppuccin-latte, catppuccin-mocha
- All ~30 color slots match YAS definitions
- Per-model ModelColors for all 6 families (opus, sonnet, haiku, minimax, qwopus, other)
- pill_fg_dark/pill_fg_light, grad_stops (5), spark_stops (3), spec_gradients (12×3), grey_rgb, bar_empty_rgb
- gradient_color, pill_fg, lerp_rgb helper functions
- resolve_theme function

**Status: FEATURE COMPLETE** — no gaps vs YAS.

### gradient.rs — 599 lines — COMPLETE
- GradientEngine with gradient_bar, sparkline (two-row half-block), dim_for_col
- Pill struct with start/end/anchor/shift/pct
- paint_bg_span for gradient background painting
- rainbow_step / rainbow_at for time-cycling palette
- pill_gradient_fg for edge char gradients
- empty_fade_colors for 3-step fade
- spec_gradient_bar for OpenSpec progress
- model_bg_pct (effort→percentage)
- model_anchor_pair (family→RGB pair)
- All Nerd Font PUA glyph constants
- terminal_width detection

**Status: FEATURE COMPLETE** — no gaps vs YAS.

### border.rs — 261 lines — COMPLETE
- border_top with T-junction connectors + pill overlay
- border_bottom with up connectors
- border_separator with up connectors
- border_separator_dim with down/up connectors + pill overlay
- border_line with gradient borders + pill flush + right_pill
- dim_for_col gradient dimming

**Status: FEATURE COMPLETE** — no gaps vs YAS.

### renderer.rs — 861 lines — ~85% COMPLETE
Has:
- RowSpec/LayoutSpec declarative system
- render_layout through BorderRenderer
- render_wide: path line, model pill + thinking/effort, helper with rate limits + time-to-reset, two-row token/cost with aligned VSEPs, context bar with 3-step fade, skills/plugins row, tasks row, subagent rows (two-line + single-line), openspec bars
- render_medium: pill + tokens + rate bar + branch + cost + optional tasks + context bar
- render_narrow: pill + tokens + rate%
- render_pill with gradient edges and effort blending
- visible_width, strip_ansi helpers
- day_cost_colour, helper_text functions

Missing vs YAS:
1. **fit_path** — progressive path degradation (full → abbreviated → middle-ellipsis → tail-only). Currently uses short_pwd() always.
2. **_middle_ellipsis** — helper for fit_path
3. **burndown_trend display** — burndown_delta() exists in session.rs but is never called/displayed in renderer. YAS shows ▲/▼/→ arrow with color coding in model_right_section.
4. **7d rate display in wide helper** — partially there (shows `| 7d X%`) but missing burndown trend arrow
5. **Day cost in wide layout** — exists in two-row layout but the day cost formatting could be more precise vs YAS (YAS formats as `$X.XX/d` with day_cost_colour)
6. **Sparkline in token rows** — sect_c1/sect_c2 build sparkline sections but they're not appended to the final tok_row1/tok_row2 strings

**Status: ~85% COMPLETE** — 6 specific gaps listed above.

### demo.rs — 196 lines — ~30% COMPLETE
Has:
- 6 scenarios (missing "openspec" scenario from YAS's 7)
- Basic make_session helper
- Static rendering via render()

Missing vs YAS:
1. **Animation mode** — YAS has 60-step interpolated animation. CCB only does static snapshots.
2. **openspec scenario** — not present
3. **Hermetic environment** — YAS synthesizes fake transcript files, token logs, git state so demos work without real sessions. CCB's demo renders use real filesystem readers that return empty data.

**Status: ~30% COMPLETE** — functional but missing animation and hermetic environment.

### mon.rs — 650 lines — ~70% COMPLETE
Has:
- Session discovery from `~/.claude/projects/`
- MiniSession deserialization with all fields
- render_session_box with pill, effort, thinking, branch, tokens, fill bar, rate limits, cost, skills, tasks
- gradient_color_hex for fill bar coloring
- Header/footer formatting
- Alternate screen buffer
- Age classification (bright/dim/removed)
- Height clipping
- Watcher thread (notify crate)
- Aggregate rate limits + day cost

Missing vs YAS:
1. **Age label rendering** — exists but simpler than YAS's styled version
2. **Subagent rows in session boxes** — YAS's monitor shows subagents per session
3. **Refresh from watcher events** — watcher thread exists but doesn't actually trigger redraws (the `for _ in rx.iter()` loop blocks forever in a separate thread without signaling the main loop)

**Status: ~70% COMPLETE** — renders and updates but missing subagent display and watcher integration.

### ccb_bridge.rs — 437 lines — COMPLETE
- StatusInput struct with all fields
- load() with model staleness guard (multi-session race protection)
- build_session_info() converting StatusInput → SessionInfo
- route_usage.jsonl and route_limits.json readers
- Session env file reader
- Git helpers (branch, dirty, commit hash)
- Factory story integration (feature-gated)

**Status: FEATURE COMPLETE** — no gaps.

### model_metadata.rs — 143 lines — COMPLETE
- TOML-driven model metadata with OnceLock singleton
- context_window_for() with family-based fallback
- rates_for() returning (input, output, thinking_multiplier)
- 16 models in config/model_metadata.toml

**Status: FEATURE COMPLETE** — no gaps.

### ccb-route.rs (bin) — 1264 lines — ~90% COMPLETE
Has:
- 4 backend kinds: Anthropic, OllamaCompat, OpenAiCompat (stub)
- Tier routing: explicit prefix (`qwopus:`, `minimax:`, `ollama:`, `anthropic:`), native model name matching, session env overrides, tier keyword fallback
- Model discovery: Ollama /api/tags with cloud detection, static backends (qwopus, minimax), Anthropic passthrough models
- /v1/messages handler with streaming + non-streaming for both Anthropic and OllamaCompat
- /v1/messages/count_tokens with char/4 estimation
- /v1/models and /v1/models/:id endpoints
- /health endpoint with full backend status
- Ollama unsupported field stripping (tool_choice, metadata, cache_control)
- Auto-pull for missing Ollama models
- Usage logging to route_usage.jsonl
- Rate limit fetching for Anthropic, Ollama Cloud, MiniMax
- Per-provider API key loading from env/$HOME/.secrets
- OpenAI→Anthropic format conversion (prepared but unused)

Missing vs YAS (and Jeremy's requirements):
1. **OpenAI-compat backends** — stub exists but not wired up. Needed for Together, Groq, vLLM, etc.
2. **Dynamic provider selection** — the routing table is config-driven but there's no runtime UI or API to switch providers
3. **Model availability checking** — /health shows backend status but there's no pre-request availability check that could failover

**Status: ~90% COMPLETE** — fully functional router with 4 backends, missing OpenAI-compat and failover.

---

## 3. What Works (Passing / Feature-Complete)

| Module | Lines | Status | Notes |
|--------|-------|--------|-------|
| session.rs | 1349 | COMPLETE | All data structures + filesystem readers |
| themes.rs | 481 | COMPLETE | All 4 themes, all color slots |
| gradient.rs | 599 | COMPLETE | GradientEngine, sparklines, pills, rainbow |
| border.rs | 261 | COMPLETE | All border types + pill overlay |
| model_metadata.rs | 143 | COMPLETE | TOML-driven, 16 models |
| ccb_bridge.rs | 437 | COMPLETE | StatusInput + session env + git + factory |
| ccb-route.rs | 1264 | ~90% | 4 backends, routing, discovery, usage logging |
| renderer.rs | 861 | ~85% | Wide/medium/narrow layouts, most features |
| mon.rs | 650 | ~70% | Multi-session TUI, discovery, rendering |
| demo.rs | 196 | ~30% | 6 static scenarios, no animation |

**Build status:** `cargo build --features full` — compiles successfully.

---

## 4. What Doesn't Work (Gaps)

### renderer.rs — 6 Gaps

**Gap R1: fit_path missing**
- YAS progressively degrades path display: full path → abbreviated (first-char segments) → middle-ellipsis → tail-only
- CCB always uses `short_pwd()` which only does first-char abbreviation
- Impact: Wide paths overflow or get truncated badly

**Gap R2: _middle_ellipsis missing**
- Helper for fit_path that truncates the middle of a string with "…"
- Needed by fit_path

**Gap R3: Burndown trend not displayed**
- `burndown_delta()` exists in session.rs but renderer never calls it
- YAS shows ▲/▼/→ arrow colored by delta magnitude in the helper/right section
- Impact: No rate limit trend visibility

**Gap R4: Sparkline sections not wired into token rows**
- `sect_c1`/`sect_c2` are built with sparkline data but never appended to `tok_row1`/`tok_row2`
- The sparkline renders but doesn't appear in output

**Gap R5: Wide layout right_pill alignment incomplete**
- When pill_pct > 0, the right section uses right_pill but the padding between content and right_pill may not match YAS's exact alignment math

**Gap R6: Medium layout missing burndown + sparkline**
- Medium layout shows basic pill + tokens + bar but lacks the compact burndown trend and sparkline that YAS includes

### demo.rs — 3 Gaps

**Gap D1: No animation mode**
- YAS interpolates 60 frames of token growth with sleep(0.05)
- CCB only renders static snapshots

**Gap D2: Missing openspec scenario**
- YAS has 7 scenarios, CCB has 6 (missing openspec)

**Gap D3: No hermetic environment**
- YAS creates synthetic transcript files, token logs, and git state for demos
- CCB's demo uses real filesystem readers that return empty/zero data
- Impact: Demo scenarios show correct pills/rates but empty token/cost/sparkline sections

### mon.rs — 3 Gaps

**Gap M1: No subagent rows in session boxes**
- YAS monitor shows subagent summary per session
- CCB monitor only shows skills and tasks

**Gap M2: Watcher doesn't trigger redraws**
- Watcher thread exists but blocks on `rx.iter()` without signaling the main tick loop
- Refresh only happens on the fixed interval timer

**Gap M3: Age label styling**
- CCB's age_label is simpler than YAS's styled version with dimmed separators

### ccb-route.rs — 3 Gaps

**Gap RT1: OpenAI-compat backend not wired**
- `to_openai()` and `oai_chunk_to_ant()` conversion functions exist but `BackendKind::OpenAiCompat` returns 501
- Needed for Together, Groq, vLLM, and other OpenAI-format providers

**Gap RT2: No runtime provider switching**
- Routing is config-file + env-var driven only
- No CLI command or API endpoint to switch a model's backend at runtime

**Gap RT3: No pre-request availability check / failover**
- If a backend is down, the request fails with 502
- No automatic failover to an alternate backend

---

## 5. Implementation Plan

### Phase 1: Renderer Parity (Priority: HIGH)
Fix the 6 renderer gaps to achieve visual parity with YAS.

**1a. Wire sparkline into token rows** (Gap R4)
- File: `renderer.rs` ~line 408
- Append `sect_c1` to `tok_row1` and `sect_c2` to `tok_row2` after the cost VSEP
- Estimated: 5 lines changed

**1b. Add burndown_trend display** (Gap R3)
- File: `renderer.rs`, add a `burndown_trend()` function
- Call `session::burndown_delta()` with fh_pct and resets_at
- Format as ▲ (alert, delta > 10), ▼ (safe, delta < -10), → (warn, otherwise)
- Insert into helper_text after the rate limit percentage
- Estimated: 20 lines added

**1c. Add fit_path + _middle_ellipsis** (Gaps R1, R2)
- File: `renderer.rs`, add `fit_path(path, git_info, theme, max_width)` and `middle_ellipsis(s, max_len)`
- Progressive degradation: try full path → abbreviated → middle-ellipsis → tail-only
- Replace `short_pwd()` call in render_wide's path_line with `fit_path()`
- Estimated: 40 lines added

**1d. Fix right_pill alignment** (Gap R5)
- File: `renderer.rs`, audit the padding math in render_wide when pill_pct > 0
- Ensure right section aligns to right edge minus pill width
- Estimated: 10 lines changed

**1e. Add burndown + sparkline to medium layout** (Gap R6)
- File: `renderer.rs` render_medium function
- Add compact burndown arrow after rate% display
- Add single-row sparkline if width permits
- Estimated: 15 lines added

### Phase 2: Demo Completeness (Priority: MEDIUM)

**2a. Add openspec scenario** (Gap D2)
- File: `demo.rs`
- Add a scenario with synthetic openspec_changes data
- Estimated: 20 lines added

**2b. Hermetic environment for demos** (Gap D3)
- File: `demo.rs`
- Create temp directory with synthetic transcript JSONL, token log, and git state
- Set env vars to redirect filesystem readers to temp dir
- Clean up after render
- Estimated: 80 lines added

**2c. Animation mode** (Gap D1)
- File: `demo.rs`
- Add `--animate` flag support
- 60-step loop with interpolated token counts and sleep(50ms)
- Use alternate screen buffer for clean animation
- Estimated: 60 lines added

### Phase 3: Monitor Polish (Priority: LOW)

**3a. Subagent rows in session boxes** (Gap M1)
- File: `mon.rs` render_session_box function
- Parse subagent JSON/JSONL from filesystem (similar to session.rs discover_subagents but using MiniSession data)
- Render 1-line summary per subagent
- Estimated: 30 lines added

**3b. Fix watcher→redraw signaling** (Gap M2)
- File: `mon.rs`
- Use `Arc<AtomicBool>` or `mpsc::channel` to signal the main loop when the watcher detects changes
- Estimated: 15 lines changed

**3c. Age label styling** (Gap M3)
- File: `mon.rs`
- Match YAS's dimmed separator style with gradient coloring
- Estimated: 10 lines changed

### Phase 4: Router Enhancement (Priority: HIGH — per Jeremy's requirement)

**4a. Wire OpenAI-compat backend** (Gap RT1)
- File: `ccb-route.rs`
- In `messages()` handler, add `BackendKind::OpenAiCompat` arm
- Use `to_openai()` to convert request body
- For streaming: use `preamble()` + `oai_chunk_to_ant()` to convert SSE chunks back to Anthropic format
- For non-streaming: convert OpenAI response JSON to Anthropic message format
- Add backend config for at least one OpenAI-compat provider (e.g., Together, Groq)
- Estimated: 80 lines added/changed

**4b. Runtime provider switching** (Gap RT2)
- File: `ccb-route.rs`
- Add `POST /config/route` endpoint accepting `{"model_family": "sonnet", "backend": "ollama"}`
- Store in `Arc<RwLock<Cfg>>` instead of `Arc<Cfg>`
- Persist changes back to `~/.claude/ccb.toml`
- Estimated: 50 lines added

**4c. Pre-request availability check** (Gap RT3)
- File: `ccb-route.rs`
- Before forwarding, send a lightweight check (HEAD or GET /health) to the target backend
- If unreachable, try fallback backends in order: same-tier alternate → anthropic
- Cache availability status with TTL (30s) to avoid check overhead
- Estimated: 60 lines added

---

## 6. Execution Order

| Step | Phase | Effort | Impact |
|------|-------|--------|--------|
| 1 | 1a: Wire sparklines | 10 min | Sparklines appear in output |
| 2 | 1b: Burndown trend | 20 min | Rate limit trend visible |
| 3 | 1c: fit_path | 30 min | Paths degrade gracefully |
| 4 | 1d: Right pill alignment | 15 min | Visual polish |
| 5 | 1e: Medium burndown+spark | 15 min | Medium layout parity |
| 6 | 4a: OpenAI-compat backend | 60 min | New provider support |
| 7 | 4b: Runtime provider switch | 40 min | Dynamic routing |
| 8 | 4c: Availability check | 45 min | Failover resilience |
| 9 | 2a: Openspec scenario | 15 min | Demo completeness |
| 10 | 2b: Hermetic demo env | 45 min | Demo accuracy |
| 11 | 2c: Animation mode | 40 min | Demo polish |
| 12 | 3a: Monitor subagents | 25 min | Monitor parity |
| 13 | 3b: Watcher signaling | 15 min | Monitor responsiveness |
| 14 | 3c: Age label styling | 10 min | Monitor polish |

**Total estimated effort:** ~6 hours

**Critical path:** Steps 1–5 (renderer parity, ~1.5 hours) then Steps 6–8 (router enhancement, ~2.5 hours).

---

## 7. What's NOT Needed

These YAS features are intentionally excluded or already handled differently:

- **alacritty.py** — Terminal width detection from Alacritty resize logs. CCB uses `terminal_width()` from gradient.rs which reads terminal size directly via ioctl/TIOCGWINSZ. Better approach.
- **Python virtual env / pip** — Rust binary, no runtime dependency management needed.
- **YAS test suite** — 40 Python test files. CCB has its own Rust tests. Parity is validated by visual comparison, not test porting.
- **model_metadata hardcoding** — YAS hardcodes context windows and rates in Python. CCB uses `config/model_metadata.toml` which is strictly better (no recompile to add models).

---

## 8. Router Architecture (Jeremy's Requirement)

Jeremy's requirement: "The router in CCB MUST allow for selecting any model from any provider and it must use the proper API for each provider to ensure maximum performance."

### Current state:
- 4 backend kinds already exist: Anthropic, OllamaCompat, OpenAiCompat (stub)
- 5 backend destinations: anthropic (api.anthropic.com), qwopus (aibox:8080), ollama (localhost:11434), minimax (api.minimax.io), and any explicit-prefix override
- Explicit prefix syntax works: `claude --model ollama:gemma4:31b-cloud`, `claude --model minimax:opus`, etc.
- Model discovery lists all available models from all backends via /v1/models

### What's needed:
1. **Wire OpenAI-compat** so Together/Groq/vLLM backends work (Phase 4a)
2. **Runtime switching** so you don't need to restart the router to change routes (Phase 4b)
3. **Availability check** so requests don't fail when a backend is down (Phase 4c)

### Per-provider API handling (already implemented):
- **Anthropic**: Full API passthrough with x-api-key, anthropic-version, anthropic-beta headers
- **Ollama**: Anthropic-compat endpoint with field stripping (tool_choice, metadata, cache_control removed)
- **MiniMax**: Anthropic-compat with anthropic-auth-token header
- **Qwopus**: Ollama-compat (same as Ollama but pointing at aibox)
- **OpenAI** (planned): Full format conversion via to_openai() with SSE chunk translation back to Anthropic format
