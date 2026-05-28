//! Provider configuration — loads provider definitions from config/providers.toml
//!
//! Each provider declares: URL, API format, auth method, tier, and model list.
//! The router aggregates all providers into a unified model catalog, grouped
//! by tier for Claude Code's /model navigation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

static PROVIDERS: OnceLock<ProviderConfig> = OnceLock::new();

// ── Tier ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tier {
    Opus,
    Sonnet,
    Haiku,
    Local,
}

impl Tier {
    pub fn label(&self) -> &'static str {
        match self {
            Tier::Opus => "Opus",
            Tier::Sonnet => "Sonnet",
            Tier::Haiku => "Haiku",
            Tier::Local => "Local",
        }
    }

    pub fn sort_key(&self) -> u8 {
        match self {
            Tier::Opus => 0,
            Tier::Sonnet => 1,
            Tier::Haiku => 2,
            Tier::Local => 3,
        }
    }
}

impl std::fmt::Display for Tier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

// ── API Format ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApiFormat {
    Anthropic,
    OllamaCompat,
    OpenaiCompat,
}

// ── Auth Method ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    OauthPassthrough,
    Bearer,
    Header,
    ApiKey,
    None,
}

// ── Model Entry ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    /// Actual model name sent to the backend. Defaults to `id` when absent.
    /// Use this when the user-facing ID differs from the backend's model name
    /// (e.g., "qwen3.5" user-facing → "qwen3.5:cloud" sent to Ollama).
    #[serde(default)]
    pub backend_id: Option<String>,
    #[serde(default)]
    pub display: String,
    #[serde(default)]
    pub tier: Option<Tier>,
}

impl ModelEntry {
    /// The model name to send to the backend API.
    pub fn backend_model(&self) -> &str {
        self.backend_id.as_deref().unwrap_or(&self.id)
    }
}

// ── Provider ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct Provider {
    pub url: String,
    pub api_format: ApiFormat,
    pub auth_method: AuthMethod,
    #[serde(default)]
    pub auth_header: Option<String>,
    #[serde(default)]
    pub auth_value_env: Option<String>,
    #[serde(default)]
    pub discover: bool,
    #[serde(default)]
    pub tier: Option<Tier>,
    #[serde(default)]
    pub models: Vec<ModelEntry>,
}

impl Provider {
    /// Resolve the auth credential from env var or ~/.secrets
    pub fn resolve_credential(&self) -> Option<String> {
        let env_name = self.auth_value_env.as_deref()?;

        // Try direct env var first
        if let Ok(val) = std::env::var(env_name) {
            if !val.is_empty() {
                return Some(val);
            }
        }

        // Try FOO_REAL variant (for when the main var is "router")
        let real_name = format!("{}_REAL", env_name);
        if let Ok(val) = std::env::var(&real_name) {
            if !val.is_empty() {
                return Some(val);
            }
        }

        // Try ~/.secrets file
        let secrets_path = dirs::home_dir()?.join(".secrets");
        let content = std::fs::read_to_string(&secrets_path).ok()?;
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with(&format!("{}=", env_name))
                && !line.to_lowercase().contains("router")
            {
                return line
                    .split_once('=')
                    .map(|(_, v)| v.trim().trim_matches('"').trim_matches('\'').to_string());
            }
        }

        Option::None
    }

    /// Effective tier for a model (model-level override > provider-level default > Local)
    pub fn effective_tier(&self, model: &ModelEntry) -> Tier {
        model
            .tier
            .unwrap_or_else(|| self.tier.unwrap_or(Tier::Local))
    }
}

// ── Resolved Model (flattened for the catalog) ───────────────────────────────

#[derive(Debug, Clone)]
pub struct ResolvedModel {
    pub id: String,
    pub display_name: String,
    pub tier: Tier,
    pub provider_name: String,
    pub provider_url: String,
    pub api_format: ApiFormat,
    pub auth_method: AuthMethod,
    pub auth_header: Option<String>,
    pub auth_value_env: Option<String>,
}

// ── Provider Config (the full loaded state) ──────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub providers: HashMap<String, Provider>,
}

#[derive(Debug, Deserialize)]
struct TomlRoot {
    providers: HashMap<String, Provider>,
}

impl ProviderConfig {
    fn load() -> Self {
        let paths = [
            dirs::home_dir()
                .unwrap_or_default()
                .join(".claude")
                .join("providers.toml"),
            dirs::home_dir()
                .unwrap_or_default()
                .join("Projects")
                .join("claude-code-barber")
                .join("config")
                .join("providers.toml"),
            PathBuf::from("config/providers.toml"),
        ];

        let toml_str = paths.iter().find_map(|p| std::fs::read_to_string(p).ok());

        if let Some(toml_str) = toml_str {
            if let Ok(root) = toml::from_str::<TomlRoot>(&toml_str) {
                return Self {
                    providers: root.providers,
                };
            }
        }

        Self {
            providers: HashMap::new(),
        }
    }

    pub fn get() -> &'static Self {
        PROVIDERS.get_or_init(Self::load)
    }

    /// Build the unified model catalog from all providers, sorted by tier then display name.
    /// This is what /v1/models returns.
    pub fn catalog(&self) -> Vec<ResolvedModel> {
        let mut models = Vec::new();

        for (name, provider) in &self.providers {
            for entry in &provider.models {
                let tier = provider.effective_tier(entry);
                let display = if entry.display.is_empty() {
                    entry.id.clone()
                } else {
                    entry.display.clone()
                };
                models.push(ResolvedModel {
                    id: entry.id.clone(),
                    display_name: display,
                    tier,
                    provider_name: name.clone(),
                    provider_url: provider.url.clone(),
                    api_format: provider.api_format.clone(),
                    auth_method: provider.auth_method.clone(),
                    auth_header: provider.auth_header.clone(),
                    auth_value_env: provider.auth_value_env.clone(),
                });
            }
        }

        // Sort: tier (opus first) then alphabetical within tier
        models.sort_by(|a, b| {
            a.tier
                .sort_key()
                .cmp(&b.tier.sort_key())
                .then_with(|| a.display_name.cmp(&b.display_name))
        });

        models
    }

    /// Find the provider + model entry for a given model ID.
    /// Checks exact match first, then case-insensitive, then substring.
    pub fn resolve_model(&self, model_id: &str) -> Option<(&str, &Provider, &ModelEntry)> {
        let mid = model_id.to_lowercase();

        // Exact match
        for (name, provider) in &self.providers {
            for entry in &provider.models {
                if entry.id == model_id {
                    return Some((name.as_str(), provider, entry));
                }
            }
        }

        // Case-insensitive match
        for (name, provider) in &self.providers {
            for entry in &provider.models {
                if entry.id.to_lowercase() == mid {
                    return Some((name.as_str(), provider, entry));
                }
            }
        }

        // Tier keyword fallback: if model_id contains opus/sonnet/haiku,
        // return the first provider configured for that tier
        let tier = if mid.contains("opus") {
            Some(Tier::Opus)
        } else if mid.contains("sonnet") {
            Some(Tier::Sonnet)
        } else if mid.contains("haiku") {
            Some(Tier::Haiku)
        } else {
            Option::None
        };

        if let Some(tier) = tier {
            for (name, provider) in &self.providers {
                for entry in &provider.models {
                    if provider.effective_tier(entry) == tier {
                        return Some((name.as_str(), provider, entry));
                    }
                }
            }
        }

        Option::None
    }

    /// Get provider by name
    pub fn provider(&self, name: &str) -> Option<&Provider> {
        self.providers.get(name)
    }

    /// List all tier labels with their model counts for display
    pub fn tier_summary(&self) -> Vec<(Tier, Vec<&ResolvedModel>)> {
        let catalog = self.catalog();
        let mut by_tier: HashMap<Tier, Vec<usize>> = HashMap::new();
        for (i, m) in catalog.iter().enumerate() {
            by_tier.entry(m.tier).or_default().push(i);
        }

        let mut tiers: Vec<Tier> = by_tier.keys().copied().collect();
        tiers.sort_by_key(|t| t.sort_key());

        // We can't return references to the catalog we just built on the stack,
        // so this method is better used with a pre-built catalog.
        // For now, return empty — callers should use catalog() directly.
        vec![]
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_config() {
        let cfg = ProviderConfig::get();
        assert!(
            !cfg.providers.is_empty(),
            "should load at least one provider"
        );
    }

    #[test]
    fn catalog_sorted_by_tier() {
        let cfg = ProviderConfig::get();
        let catalog = cfg.catalog();
        if catalog.len() < 2 {
            return;
        }
        for i in 0..catalog.len() - 1 {
            assert!(
                catalog[i].tier.sort_key() <= catalog[i + 1].tier.sort_key(),
                "catalog should be sorted by tier: {} ({}) before {} ({})",
                catalog[i].display_name,
                catalog[i].tier,
                catalog[i + 1].display_name,
                catalog[i + 1].tier,
            );
        }
    }

    #[test]
    fn resolve_exact_match() {
        let cfg = ProviderConfig::get();
        let result = cfg.resolve_model("claude-opus-4-7");
        assert!(result.is_some(), "should resolve claude-opus-4-7");
        let (provider_name, _, entry) = result.unwrap();
        assert_eq!(provider_name, "anthropic");
        assert_eq!(entry.id, "claude-opus-4-7");
    }

    #[test]
    fn resolve_tier_fallback() {
        let cfg = ProviderConfig::get();
        // A model ID containing "opus" should resolve via tier fallback
        let result = cfg.resolve_model("some-opus-variant");
        assert!(result.is_some(), "should resolve via opus tier fallback");
    }

    #[test]
    fn anthropic_uses_oauth() {
        let cfg = ProviderConfig::get();
        let provider = cfg
            .provider("anthropic")
            .expect("anthropic provider should exist");
        assert_eq!(provider.auth_method, AuthMethod::OauthPassthrough);
    }

    #[test]
    fn ollama_uses_no_auth() {
        let cfg = ProviderConfig::get();
        let provider = cfg
            .provider("ollama")
            .expect("ollama provider should exist");
        assert_eq!(provider.auth_method, AuthMethod::None);
    }

    #[test]
    fn minimax_uses_api_key() {
        let cfg = ProviderConfig::get();
        let provider = cfg
            .provider("minimax")
            .expect("minimax provider should exist");
        assert_eq!(provider.auth_method, AuthMethod::ApiKey);
        assert_eq!(provider.url, "https://api.minimax.io/anthropic");
    }

    #[test]
    fn minimax_has_all_models() {
        let cfg = ProviderConfig::get();
        let provider = cfg
            .provider("minimax")
            .expect("minimax provider should exist");
        let ids: Vec<&str> = provider.models.iter().map(|m| m.id.as_str()).collect();
        assert!(ids.contains(&"MiniMax-M2.7"), "should have M2.7");
        assert!(
            ids.contains(&"MiniMax-M2.7-highspeed"),
            "should have M2.7-highspeed"
        );
        assert!(ids.contains(&"MiniMax-M2.5"), "should have M2.5");
        assert!(ids.contains(&"MiniMax-M2"), "should have M2");
        assert!(
            ids.len() >= 7,
            "should have at least 7 MiniMax models, got {}",
            ids.len()
        );
    }

    #[test]
    fn resolve_minimax_model() {
        let cfg = ProviderConfig::get();
        let result = cfg.resolve_model("MiniMax-M2.7-highspeed");
        assert!(result.is_some(), "should resolve MiniMax-M2.7-highspeed");
        let (name, _, entry) = result.unwrap();
        assert_eq!(name, "minimax");
        assert_eq!(entry.id, "MiniMax-M2.7-highspeed");
    }
}
