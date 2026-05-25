use anyhow::{Context, Result};
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

const HOOK_SCRIPTS: &[(&str, &str)] = &[
    (
        "context_monitor.sh",
        include_str!("../../hooks/context_monitor.sh"),
    ),
    (
        "skill_loader.sh",
        include_str!("../../hooks/skill_loader.sh"),
    ),
];

// Minimal JSON patch: add context_monitor PostToolUse hook + skill_loader PreToolUse hook.
// We parse and patch the existing settings.json rather than overwriting it.
pub fn run(auto: bool, dry_run: bool) -> Result<()> {
    let hooks_dir = dirs::home_dir()
        .context("cannot determine home dir")?
        .join(".claude")
        .join("hooks");

    let settings_path = dirs::home_dir()
        .context("cannot determine home dir")?
        .join(".claude")
        .join("settings.json");

    if dry_run {
        println!("DRY RUN — no changes will be written\n");
    }

    // ── 1. Hook scripts ────────────────────────────────────────────────────
    println!("Hook scripts → {}", hooks_dir.display());
    for (name, content) in HOOK_SCRIPTS {
        let dest = hooks_dir.join(name);
        let status = if dest.exists() { "exists" } else { "new" };
        println!("  {} [{}]", name, status);
        if !dry_run {
            if !hooks_dir.exists() {
                fs::create_dir_all(&hooks_dir)?;
            }
            fs::write(&dest, content)?;
            make_executable(&dest)?;
        }
    }

    // ── 2. settings.json patch ─────────────────────────────────────────────
    println!("\nSettings → {}", settings_path.display());

    let settings_text = if settings_path.exists() {
        fs::read_to_string(&settings_path)?
    } else {
        r#"{"hooks":{"PreToolUse":[],"PostToolUse":[]}}"#.to_string()
    };

    let mut settings: serde_json::Value =
        serde_json::from_str(&settings_text).context("settings.json is not valid JSON")?;

    let hooks_dir_str = hooks_dir.display().to_string();

    let context_hook = serde_json::json!({
        "hooks": [{"type": "command", "command": format!("{}/context_monitor.sh", hooks_dir_str), "async": true}]
    });
    let skill_hook = serde_json::json!({
        "matcher": "Skill",
        "hooks": [{"type": "command", "command": format!("{}/skill_loader.sh", hooks_dir_str)}]
    });

    let already_has_context = hook_already_present(&settings, "PostToolUse", "context_monitor");
    let already_has_skill = hook_already_present(&settings, "PreToolUse", "skill_loader");

    if already_has_context {
        println!("  context_monitor hook [already wired]");
    } else {
        println!("  context_monitor hook [will add → PostToolUse]");
    }
    if already_has_skill {
        println!("  skill_loader hook [already wired]");
    } else {
        println!("  skill_loader hook [will add → PreToolUse / Skill]");
    }

    if dry_run {
        println!("\nDry run complete. Run without --dry-run to apply.");
        return Ok(());
    }

    if !auto && (!already_has_context || !already_has_skill) {
        print!("\nApply changes to settings.json? [y/N] ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !answer.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    if !already_has_context {
        settings["hooks"]["PostToolUse"]
            .as_array_mut()
            .context("PostToolUse is not an array")?
            .push(context_hook);
    }
    if !already_has_skill {
        settings["hooks"]["PreToolUse"]
            .as_array_mut()
            .context("PreToolUse is not an array")?
            .push(skill_hook);
    }

    if !already_has_context || !already_has_skill {
        let patched = serde_json::to_string_pretty(&settings)?;
        fs::write(&settings_path, patched)?;
        println!("\n✓ settings.json updated");
    } else {
        println!("\nAll hooks already wired — nothing to do.");
    }

    println!("✓ context monitoring: warns at >70% (compact) and >85% (clear)");
    println!("✓ skill loading: /skill commands lazy-load via ccb fade");
    Ok(())
}

fn hook_already_present(settings: &serde_json::Value, phase: &str, script_fragment: &str) -> bool {
    let arr = match settings["hooks"][phase].as_array() {
        Some(a) => a,
        None => return false,
    };
    let needle = serde_json::json!(script_fragment);
    let s = serde_json::to_string(&needle).unwrap_or_default();
    let s = s.trim_matches('"');
    arr.iter().any(|entry| {
        let entry_str = entry.to_string();
        entry_str.contains(s)
    })
}

#[cfg(unix)]
fn make_executable(path: &PathBuf) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o755);
    fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn make_executable(_path: &PathBuf) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hook_already_present_returns_false_when_no_hooks() {
        let settings = serde_json::json!({"hooks": {"PreToolUse": [], "PostToolUse": []}});
        assert!(!hook_already_present(
            &settings,
            "PreToolUse",
            "context_monitor"
        ));
        assert!(!hook_already_present(
            &settings,
            "PostToolUse",
            "skill_loader"
        ));
    }

    #[test]
    fn hook_already_present_finds_matching_hook() {
        let settings = serde_json::json!({
            "hooks": {
                "PreToolUse": [
                    {"type": "command", "command": "/some/path/context_monitor.sh"}
                ]
            }
        });
        assert!(hook_already_present(
            &settings,
            "PreToolUse",
            "context_monitor"
        ));
        assert!(!hook_already_present(
            &settings,
            "PreToolUse",
            "skill_loader"
        ));
    }

    #[test]
    fn hook_already_present_returns_false_for_missing_phase() {
        let settings = serde_json::json!({"hooks": {}});
        assert!(!hook_already_present(
            &settings,
            "PreToolUse",
            "context_monitor"
        ));
    }

    #[test]
    fn hook_already_present_returns_false_null_array() {
        let settings = serde_json::json!({"hooks": {"PreToolUse": null}});
        assert!(!hook_already_present(
            &settings,
            "PreToolUse",
            "context_monitor"
        ));
    }
}
