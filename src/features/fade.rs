use crate::cli::FadeArgs;
#[cfg(feature = "expert")]
use crate::features::expert::active_context;
use crate::log::{CompressionEvent, estimate_tokens};
use std::path::PathBuf;

pub fn run(args: FadeArgs) -> anyhow::Result<()> {
    // Bypass mode: log with mode:bypass and return
    if std::env::var("CCB_BYPASS").is_ok() {
        let content = match &args.resource {
            Some(name) => {
                let index_content = read_index()?;
                match lookup(&index_content, name) {
                    Some(path) => std::fs::read_to_string(&path).unwrap_or_default(),
                    None => String::new(),
                }
            }
            None => read_index()?,
        };
        let (persona, domains_hit) = {
        #[cfg(feature = "expert")]
        { active_context().unzip() }
        #[cfg(not(feature = "expert"))]
        { (None, Some(vec![])) }
    };
        CompressionEvent {
            timestamp: chrono::Utc::now().to_rfc3339(),
            feature: "fade".to_string(),
            command: args.resource.clone().unwrap_or_else(|| "list".to_string()),
            tokens_in: estimate_tokens(&content),
            tokens_out: estimate_tokens(&content),
            bytes_in: content.len(),
            bytes_out: content.len(),
            mode: Some("bypass".to_string()),
            persona,
            domains_hit,
        }
        .record();
        print!("{}", content);
        return Ok(());
    }

    match args.resource {
        Some(name) => load_resource(&name),
        None => list_index(),
    }
}

pub fn load_resource(name: &str) -> anyhow::Result<()> {
    let index_content = read_index()?;
    match lookup(&index_content, name) {
        Some(path) => {
            let content = std::fs::read_to_string(&path).map_err(|e| {
                anyhow::anyhow!(
                    "skill in index but file missing at {}: {}",
                    path.display(),
                    e
                )
            })?;
            print!("{}", content);
            tracing::info!(resource = name, path = %path.display(), "fade: loaded");
            Ok(())
        }
        None => {
            eprintln!(
                "ccb fade: '{}' not found in INDEX.md — run `ccb style index-build` to rebuild",
                name
            );
            std::process::exit(1);
        }
    }
}

fn list_index() -> anyhow::Result<()> {
    println!("{}", read_index()?);
    Ok(())
}

fn index_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("skills")
        .join("INDEX.md")
}

fn read_index() -> anyhow::Result<String> {
    let path = index_path();
    if !path.exists() {
        anyhow::bail!(
            "INDEX.md not found at {}. Run: ccb style index-build",
            path.display()
        );
    }
    Ok(std::fs::read_to_string(path)?)
}

fn lookup(index: &str, name: &str) -> Option<PathBuf> {
    for line in index.lines() {
        if !line.starts_with('|') {
            continue;
        }
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        if cols.len() < 5 || cols[1] != name {
            continue;
        }
        let rel = cols[5];
        if rel.is_empty() {
            continue;
        }
        return Some(
            dirs::home_dir()
                .unwrap_or_default()
                .join(".claude")
                .join(rel),
        );
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lookup_finds_exact_name() {
        let index = "| name | type | description | tags | path |
|------|------|-------------|------|------|
| trim | fade | trim feature | fade | skills/trim.md |
";
        let expected = dirs::home_dir().unwrap().join(".claude/skills/trim.md");
        assert_eq!(lookup(index, "trim"), Some(expected));
    }

    #[test]
    fn lookup_returns_none_for_missing_name() {
        let index = "| name | type | description | tags | path |
|------|------|-------------|------|------|
| trim | fade | trim feature | fade | skills/trim.md |
";
        assert_eq!(lookup(index, "fade"), None);
    }

    #[test]
    fn lookup_skips_non_table_lines() {
        let index = "# INDEX.md
| name | type | description | tags | path |
|------|------|-------------|------|------|
| trim | fade | trim feature | fade | skills/trim.md |
";
        assert_eq!(
            lookup(index, "trim"),
            Some(dirs::home_dir().unwrap().join(".claude/skills/trim.md"))
        );
    }

    #[test]
    fn lookup_returns_none_on_empty_path_col() {
        let index = "| name | type | description | tags | path |
|------|------|-------------|------|------|
| trim | fade | trim feature | fade | |
";
        assert_eq!(lookup(index, "trim"), None);
    }

    #[test]
    fn lookup_separator_row_skipped() {
        let index = "| name | type | description | tags | path |
|------|------|-------------|------|------|
| --- | --- | ----------- | ------| ------ |
| trim | fade | trim feature | fade | skills/trim.md |
";
        assert_eq!(
            lookup(index, "trim"),
            Some(dirs::home_dir().unwrap().join(".claude/skills/trim.md"))
        );
    }

    #[test]
    fn lookup_partial_name_no_match() {
        let index = "| name | type | description | tags | path |
|------|------|-------------|------|------|
| trim | fade | trim feature | fade | skills/trim.md |
";
        assert_eq!(lookup(index, "tri"), None);
    }

    #[test]
    fn index_path_ends_with_skills_index() {
        let path = index_path();
        assert!(path.to_string_lossy().ends_with(".claude/skills/INDEX.md"));
    }
}
