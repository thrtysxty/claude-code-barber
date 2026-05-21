use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct Config {
    pub terse: bool,
    pub conversation_style: bool,
    pub index_path: Option<PathBuf>,
    pub features: FeatureConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct FeatureConfig {
    pub trim: bool,
    pub fade: bool,
    pub sandbox: bool,
    pub terse: bool,
    pub graph: bool,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("ccb.toml")
}

pub fn load() -> anyhow::Result<Config> {
    let path = config_path();
    if !path.exists() {
        return Ok(Config::default());
    }
    let raw = std::fs::read_to_string(path)?;
    Ok(toml::from_str(&raw)?)
}
