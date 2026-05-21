use crate::cli::FadeArgs;
use std::path::PathBuf;

pub fn run(args: FadeArgs) -> anyhow::Result<()> {
    match args.resource {
        Some(name) => load_resource(&name),
        None       => list_index(),
    }
}

pub fn load_resource(name: &str) -> anyhow::Result<()> {
    let index_content = read_index()?;
    match lookup(&index_content, name) {
        Some(path) => {
            let content = std::fs::read_to_string(&path)
                .map_err(|e| anyhow::anyhow!("skill in index but file missing at {}: {}", path.display(), e))?;
            print!("{}", content);
            tracing::info!(resource = name, path = %path.display(), "fade: loaded");
            Ok(())
        }
        None => {
            eprintln!("ccb fade: '{}' not found in INDEX.md — run `ccb style index-build` to rebuild", name);
            std::process::exit(1);
        }
    }
}

fn list_index() -> anyhow::Result<()> {
    println!("{}", read_index()?);
    Ok(())
}

fn index_path() -> PathBuf {
    dirs::home_dir().unwrap_or_default().join(".claude").join("skills").join("INDEX.md")
}

fn read_index() -> anyhow::Result<String> {
    let path = index_path();
    if !path.exists() {
        anyhow::bail!("INDEX.md not found at {}. Run: ccb style index-build", path.display());
    }
    Ok(std::fs::read_to_string(path)?)
}

fn lookup(index: &str, name: &str) -> Option<PathBuf> {
    for line in index.lines() {
        if !line.starts_with('|') { continue; }
        let cols: Vec<&str> = line.split('|').map(str::trim).collect();
        if cols.len() < 5 || cols[1] != name { continue; }
        let rel = cols[4];
        if rel.is_empty() { continue; }
        return Some(dirs::home_dir().unwrap_or_default().join(".claude").join(rel));
    }
    None
}
