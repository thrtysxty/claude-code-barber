# CCB Story 003: Expert CLI Subcommands

**Status:** READY
**Priority:** P0 — Layer 3 usability
**Sprint:** CCB-1 (Expert/Persona Graph)

## Narrative
**As a** CCB user,
**I want** `ccb expert build/activate/deactivate/list` commands,
**So that** I can manage the expert knowledge graph from the shell.

## Context

Follow the exact wiring pattern used by `Graph` in `src/cli.rs` and `src/main.rs` (qwopus added this in Sprint 0c). The `expert` feature gate mirrors `graph`.

## Acceptance Criteria

### STEP ZERO
1. **Read `src/cli.rs` and `src/main.rs`** as modified by Sprint 0c — use `Graph`/`GraphArgs`/`GraphCmd` as the exact template for `Expert`/`ExpertArgs`/`ExpertCmd`.

### `src/cli.rs`
2. **`Command::Expert(ExpertArgs)` variant added** — feature-gated with `#[cfg(feature = "expert")]`.
3. **`ExpertArgs` struct** with `#[command(subcommand)] pub cmd: ExpertCmd`.
4. **`ExpertCmd` enum** with four variants:
```rust
pub enum ExpertCmd {
    /// Build knowledge graph from a dataset file
    Build {
        name: String,
        #[arg(long)]
        dataset: std::path::PathBuf,
    },
    /// Activate a persona — makes it available to hooks
    Activate { name: String },
    /// Deactivate the current persona
    Deactivate,
    /// List all registered experts and active status
    List,
}
```

### `src/main.rs`
5. **`pub mod expert` added** to `pub mod features` block, gated `#[cfg(feature = "expert")]`.
6. **`Command::Expert(args) => expert_cmd(args)` arm** added to `match cli.command`.
7. **`expert_cmd` dispatch function** mirrors `graph_cmd` pattern exactly:
```rust
fn expert_cmd(_args: cli::ExpertArgs) -> anyhow::Result<()> {
    #[cfg(feature = "expert")]
    {
        use cli::ExpertCmd;
        return match _args.cmd {
            ExpertCmd::Build { name, dataset }  => features::expert::build(&name, &dataset),
            ExpertCmd::Activate { name }        => features::expert::activate(&name),
            ExpertCmd::Deactivate               => features::expert::deactivate(),
            ExpertCmd::List                     => features::expert::list(),
        };
    }
    #[allow(unreachable_code)]
    anyhow::bail!("ccb built without 'expert'. Rebuild: cargo build --features expert")
}
```

### Smoke Tests
8. **`ccb expert list`** — prints expert roster (empty is fine if no dataset built yet).
9. **`ccb expert build sentinel --dataset <path>`** — exits 0 when dataset file exists.
10. **`ccb expert activate sentinel`** — exits 0 after build.
11. **`ccb expert deactivate`** — exits 0.

### Gate
12. **`cargo build --features expert`** — zero errors, zero panics on smoke tests.

## Files in Scope
- `src/cli.rs` — add Expert variants
- `src/main.rs` — add mod, match arm, dispatch function

## Frozen Surfaces
- All existing commands — do not modify `Trim`, `Fade`, `Cut`, `Lineup`, `Style`, `Context`, `Buzz`, `Gain`, `Graph`

## Blocked By
- Story CCB-002

## Blocks
- Story CCB-004

## Definition of Done
- [ ] `ExpertArgs`, `ExpertCmd` in `src/cli.rs`
- [ ] `expert_cmd` in `src/main.rs`
- [ ] All 4 smoke tests pass
- [ ] `cargo build --features expert` clean
