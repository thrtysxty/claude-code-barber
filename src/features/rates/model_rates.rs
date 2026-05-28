//! model_rates — Anthropic API cost model
//!
//! All token costs are expressed at Anthropic's published rates regardless of which
//! backend (minimax/ollama/direct) handled the request.

/// Anthropic per-model rate constants (input $/1M, output $/1M, thinking multiplier)
pub struct ModelRate {
    /// Cost per million input tokens
    pub input_per_million: f64,
    /// Cost per million output tokens (before thinking multiplier)
    pub output_per_million: f64,
    /// Output token cost multiplier when extended thinking is active
    pub thinking_multiplier: f64,
}

impl ModelRate {
    /// Look up rates for a model ID string from model_metadata.toml.
    /// Falls back to sonnet rates for unrecognized models.
    pub fn for_model(model_id: &str) -> Self {
        let (inp, out, think) =
            crate::features::model_metadata::ModelMetadata::get().rates_for(model_id);
        Self {
            input_per_million: inp,
            output_per_million: out,
            thinking_multiplier: think,
        }
    }

    /// Compute cost in USD for a given token count.
    pub fn compute_cost(&self, input_tokens: u64, output_tokens: u64, thinking: bool) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.input_per_million;
        let output_cost = (output_tokens as f64 / 1_000_000.0)
            * self.output_per_million
            * if thinking {
                self.thinking_multiplier
            } else {
                1.0
            };
        input_cost + output_cost
    }
}

/// Compute total session cost from token usage log and thinking state.
pub fn session_cost(model: &str, thinking: bool, usage_file: &std::path::Path) -> f64 {
    let content = match std::fs::read_to_string(usage_file) {
        Ok(c) => c,
        Err(_) => return 0.0,
    };

    let rate = ModelRate::for_model(model);
    let mut total = 0.0;

    for line in content.lines() {
        if let Ok(entry) = serde_json::from_str::<TokenEntry>(line) {
            total += rate.compute_cost(entry.in_tokens, entry.out_tokens, thinking);
        }
    }

    total
}

#[derive(Debug, serde::Deserialize)]
#[allow(dead_code)]
struct TokenEntry {
    #[serde(rename = "t")]
    timestamp: String,
    #[serde(rename = "mdl")]
    model: String,
    #[serde(rename = "in")]
    in_tokens: u64,
    #[serde(rename = "out")]
    out_tokens: u64,
    #[serde(rename = "be")]
    backend: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn haiku_rates() {
        let r = ModelRate::for_model("haiku-4.5");
        assert_eq!(r.input_per_million, 0.80);
        assert_eq!(r.output_per_million, 4.00);
        assert_eq!(r.thinking_multiplier, 1.0);
    }

    #[test]
    fn sonnet_rates() {
        let r = ModelRate::for_model("sonnet-4.6");
        assert_eq!(r.input_per_million, 3.00);
        assert_eq!(r.output_per_million, 15.00);
        assert_eq!(r.thinking_multiplier, 3.5);
    }

    #[test]
    fn opus_rates() {
        let r = ModelRate::for_model("opus-4.1");
        assert_eq!(r.input_per_million, 15.00);
        assert_eq!(r.output_per_million, 75.00);
        assert_eq!(r.thinking_multiplier, 3.5);
    }

    #[test]
    fn compute_cost_basic() {
        let r = ModelRate::for_model("sonnet-4.6");
        // 1M input at $3/M = $3, 100K output at $15/M = $1.50, no thinking
        let cost = r.compute_cost(1_000_000, 100_000, false);
        assert!((cost - 4.50).abs() < 0.01);
    }

    #[test]
    fn compute_cost_with_thinking() {
        let r = ModelRate::for_model("sonnet-4.6");
        // same tokens but with thinking 3.5✕ output
        let cost = r.compute_cost(1_000_000, 100_000, true);
        assert!((cost - 8.25).abs() < 0.01); // $3 + ($1.50 × 3.5)
    }
}
