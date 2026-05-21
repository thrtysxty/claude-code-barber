use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
pub struct CompressionEvent {
    pub timestamp: String,
    pub feature: String,
    pub command: String,
    pub tokens_in: usize,
    pub tokens_out: usize,
    pub bytes_in: usize,
    pub bytes_out: usize,
}

impl CompressionEvent {
    pub fn reduction_pct(&self) -> f64 {
        if self.tokens_in == 0 { return 0.0; }
        (1.0 - self.tokens_out as f64 / self.tokens_in as f64) * 100.0
    }

    pub fn record(&self) {
        tracing::info!(
            feature = self.feature,
            command = %self.command,
            tokens_in = self.tokens_in,
            tokens_out = self.tokens_out,
            reduction_pct = format!("{:.1}%", self.reduction_pct()),
            "compression"
        );
        if let Ok(path) = log_path() {
            append_jsonl(&path, self);
        }
    }
}

pub fn estimate_tokens(s: &str) -> usize {
    (s.len() + 3) / 4
}

fn log_path() -> anyhow::Result<PathBuf> {
    Ok(dirs::home_dir()
        .unwrap_or_default()
        .join(".claude")
        .join("ccb_log.jsonl"))
}

fn append_jsonl<T: Serialize>(path: &PathBuf, event: &T) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(path) {
        if let Ok(line) = serde_json::to_string(event) {
            let _ = writeln!(f, "{}", line);
        }
    }
}
