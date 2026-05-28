//! model_metadata — loads model context windows and pricing from config/model_metadata.toml
//!
//! Shared by ccb-route (/v1/models endpoint) and ccb statusline (soft_limit, rates).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

static METADATA: OnceLock<ModelMetadata> = OnceLock::new();

pub struct ModelMetadata {
    context_windows: HashMap<String, u64>,
    rates: HashMap<String, [f64; 3]>,
}

impl ModelMetadata {
    fn load() -> Self {
        let mut context_windows = HashMap::new();
        let mut rates = HashMap::new();

        let paths = [
            dirs::home_dir()
                .unwrap_or_default()
                .join("Projects")
                .join("claude-code-barber")
                .join("config")
                .join("model_metadata.toml"),
            PathBuf::from("config/model_metadata.toml"),
        ];

        let toml_str = paths.iter().find_map(|p| std::fs::read_to_string(p).ok());

        if let Some(toml_str) = toml_str {
            if let Ok(doc) = toml::from_str::<toml::Value>(&toml_str) {
                if let Some(cw) = doc.get("context_windows").and_then(|v| v.as_table()) {
                    for (k, v) in cw {
                        if let Some(n) = v.as_integer() {
                            context_windows.insert(k.clone(), n as u64);
                        }
                    }
                }
                if let Some(r) = doc.get("rates").and_then(|v| v.as_table()) {
                    for (k, v) in r {
                        if let Some(arr) = v.as_array() {
                            let vals: Vec<f64> = arr.iter().filter_map(|x| x.as_float()).collect();
                            if vals.len() == 3 {
                                rates.insert(k.clone(), [vals[0], vals[1], vals[2]]);
                            }
                        }
                    }
                }
            }
        }

        Self {
            context_windows,
            rates,
        }
    }

    pub fn get() -> &'static Self {
        METADATA.get_or_init(Self::load)
    }

    /// Context window for a model ID, with family-based fallback.
    pub fn context_window_for(&self, model_id: &str) -> u64 {
        if let Some(&cw) = self.context_windows.get(model_id) {
            return cw;
        }
        let m = model_id.to_lowercase();
        if m.contains("qwopus") {
            self.context_windows
                .get("qwopus3.5-9b-v3")
                .copied()
                .unwrap_or(131_072)
        } else if m.contains("opus") {
            self.context_windows
                .get("claude-opus-4-7")
                .copied()
                .unwrap_or(200_000)
        } else if m.contains("sonnet") {
            self.context_windows
                .get("claude-sonnet-4-6")
                .copied()
                .unwrap_or(200_000)
        } else if m.contains("haiku") {
            self.context_windows
                .get("claude-haiku-4-5-20251001")
                .copied()
                .unwrap_or(200_000)
        } else if m.contains("minimax") {
            204_800
        } else {
            150_000
        }
    }

    /// Rates for a model ID: (input_per_million, output_per_million, thinking_multiplier).
    pub fn rates_for(&self, model_id: &str) -> (f64, f64, f64) {
        if let Some(&[inp, out, think]) = self.rates.get(model_id) {
            return (inp, out, think);
        }
        let m = model_id.to_lowercase();
        if m.contains("qwopus") {
            (0.0, 0.0, 1.0)
        } else if m.contains("opus") {
            (15.0, 75.0, 3.5)
        } else if m.contains("haiku") {
            (0.80, 4.0, 1.0)
        } else if m.contains("minimax") && m.contains("highspeed") {
            (0.60, 2.40, 1.0)
        } else if m.contains("minimax") {
            (0.30, 1.20, 1.0)
        } else {
            (3.0, 15.0, 3.5)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn context_window_exact_match() {
        let md = ModelMetadata::get();
        assert_eq!(md.context_window_for("qwopus3.5-9b-v3"), 131_072);
    }

    #[test]
    fn context_window_family_fallback() {
        let md = ModelMetadata::get();
        assert_eq!(md.context_window_for("qwopus-custom"), 131_072);
    }

    #[test]
    fn context_window_unknown_model() {
        let md = ModelMetadata::get();
        assert_eq!(md.context_window_for("unknown-model"), 150_000);
    }

    #[test]
    fn rates_exact_match() {
        let md = ModelMetadata::get();
        let (inp, out, think) = md.rates_for("claude-sonnet-4-6");
        assert!((inp - 3.0).abs() < 0.01);
        assert!((out - 15.0).abs() < 0.01);
        assert!((think - 3.5).abs() < 0.01);
    }

    #[test]
    fn rates_family_fallback() {
        let md = ModelMetadata::get();
        let (inp, _, _) = md.rates_for("sonnet-4.0-custom");
        assert!((inp - 3.0).abs() < 0.01);
    }
}
