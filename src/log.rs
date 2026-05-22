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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_empty() {
        assert_eq!(estimate_tokens(""), 0);
    }

    #[test]
    fn estimate_tokens_single_char() {
        assert_eq!(estimate_tokens("a"), 1); // (1+3)/4 = 1
    }

    #[test]
    fn estimate_tokens_four_chars() {
        assert_eq!(estimate_tokens("abcd"), 1); // (4+3)/4 = 1
    }

    #[test]
    fn estimate_tokens_five_chars() {
        assert_eq!(estimate_tokens("abcde"), 2); // (5+3)/4 = 2
    }

    #[test]
    fn estimate_tokens_400_bytes() {
        let s = "x".repeat(400);
        assert_eq!(estimate_tokens(&s), 100);
    }

    #[test]
    fn estimate_tokens_401_bytes() {
        let s = "x".repeat(401);
        assert_eq!(estimate_tokens(&s), 101);
    }

    #[test]
    fn estimate_tokens_multibyte_utf8() {
        // "héllo" is 6 bytes in UTF-8 (h=1, é=2, l=1, l=1, o=1)
        let s = "héllo";
        assert_eq!(estimate_tokens(s), 2); // (6+3)/4 = 2
    }

    #[test]
    fn reduction_pct_zero_in() {
        let event = CompressionEvent {
            timestamp: "2026-05-22T00:00:00Z".to_string(),
            feature: "test".to_string(),
            command: "test".to_string(),
            tokens_in: 0,
            tokens_out: 999,
            bytes_in: 0,
            bytes_out: 0,
        };
        assert_eq!(event.reduction_pct(), 0.0);
    }

    #[test]
    fn reduction_pct_no_reduction() {
        let event = CompressionEvent {
            timestamp: "2026-05-22T00:00:00Z".to_string(),
            feature: "test".to_string(),
            command: "test".to_string(),
            tokens_in: 100,
            tokens_out: 100,
            bytes_in: 0,
            bytes_out: 0,
        };
        assert_eq!(event.reduction_pct(), 0.0);
    }

    #[test]
    fn reduction_pct_full_reduction() {
        let event = CompressionEvent {
            timestamp: "2026-05-22T00:00:00Z".to_string(),
            feature: "test".to_string(),
            command: "test".to_string(),
            tokens_in: 100,
            tokens_out: 0,
            bytes_in: 0,
            bytes_out: 0,
        };
        assert_eq!(event.reduction_pct(), 100.0);
    }

    #[test]
    fn reduction_pct_sixty_percent() {
        let event = CompressionEvent {
            timestamp: "2026-05-22T00:00:00Z".to_string(),
            feature: "test".to_string(),
            command: "test".to_string(),
            tokens_in: 100,
            tokens_out: 40,
            bytes_in: 0,
            bytes_out: 0,
        };
        assert!((event.reduction_pct() - 60.0).abs() < 0.01);
    }

    #[test]
    fn reduction_pct_twentyseven_percent() {
        let event = CompressionEvent {
            timestamp: "2026-05-22T00:00:00Z".to_string(),
            feature: "test".to_string(),
            command: "test".to_string(),
            tokens_in: 100,
            tokens_out: 73,
            bytes_in: 0,
            bytes_out: 0,
        };
        assert!((event.reduction_pct() - 27.0).abs() < 0.01);
    }
}
