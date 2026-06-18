//! Provider configuration — loads provider definitions from config/providers.toml
//!
//! Each provider declares: URL, API format, auth method, tier, and model list.
//! The router aggregates all providers into a unified model catalog, grouped
//! by tier for Claude Code's /model navigation.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::UNIX_EPOCH;

use serde::Deserialize;

/// Provider configuration — lazily initialized and hot-reloadable.
/// `RwLock<Option<Arc<ProviderConfig>>>` enables:
/// - Lazy init: first call populates the cache
/// - Hot-reload: when the config file mtime changes, `get()` atomically swaps to a fresh config
/// - Cheap reads: callers clone the Arc (no lock held during request handling)
static PROVIDERS: RwLock<Option<Arc<ProviderConfig>>> = RwLock::new(None);
/// Tracks the mtime of the config file when it was last loaded.
static CONFIG_MTIME: AtomicU64 = AtomicU64::new(0);

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

    /// Extract tier from a model ID string.
    /// Handles Claude model names like "claude-sonnet-4-20250514" and clean names like "sonnet-4".
    pub fn extract_from_model_id(model_id: &str) -> Option<Tier> {
        let mid = model_id.to_lowercase();
        // Strip claude- prefix and version suffixes for cleaner matching
        let clean = mid.strip_prefix("claude-").unwrap_or(&mid);

        if clean.starts_with("opus") || clean.contains("-opus-") {
            Some(Tier::Opus)
        } else if clean.starts_with("sonnet") || clean.contains("-sonnet-") {
            Some(Tier::Sonnet)
        } else if clean.starts_with("haiku") || clean.contains("-haiku-") {
            Some(Tier::Haiku)
        } else {
            None
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
    /// Whether the model supports tool/function calling.
    #[serde(default)]
    pub tools: Option<bool>,
    /// Whether the model supports image/vision input.
    #[serde(default)]
    pub vision: Option<bool>,
    /// Whether the model supports extended thinking/reasoning.
    #[serde(default)]
    pub thinking: Option<bool>,
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
    pub tools: bool,
    pub vision: bool,
    pub thinking: bool,
}

// ── Provider Config (the full loaded state) ──────────────────────────────────

/// Tier routing configuration — maps each tier to an ordered list of model IDs.
/// The router walks this list in order and routes to the first resolvable model.
#[derive(Debug, Clone, Default)]
pub struct TierRouting {
    /// Ordered model IDs for each tier. First resolvable model wins.
    pub opus: Vec<String>,
    pub sonnet: Vec<String>,
    pub haiku: Vec<String>,
    /// Override: route ALL tier requests to this single model (for cost optimization).
    /// When set, tier_routing arrays are ignored.
    pub override_all: Option<String>,
}

impl TierRouting {
    /// Get the ordered model list for a given tier.
    pub fn models_for_tier(&self, tier: &Tier) -> &[String] {
        match tier {
            Tier::Opus => &self.opus,
            Tier::Sonnet => &self.sonnet,
            Tier::Haiku => &self.haiku,
            Tier::Local => &[],
        }
    }

    /// Check if override_all is set.
    pub fn is_overridden(&self) -> bool {
        self.override_all.is_some()
    }
}

#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub providers: HashMap<String, Provider>,
    pub tier_routing: TierRouting,
}

#[derive(Debug, Deserialize)]
struct TomlRoot {
    providers: HashMap<String, Provider>,
}

#[derive(Debug, Default, Deserialize)]
struct TomlTierRouting {
    #[serde(default)]
    opus: Vec<String>,
    #[serde(default)]
    sonnet: Vec<String>,
    #[serde(default)]
    haiku: Vec<String>,
    #[serde(default)]
    override_all: Option<String>,
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

        if let Some(ref toml_str) = toml_str {
            // Parse full TOML including [tier_routing] section
            #[derive(Deserialize)]
            struct FullRoot {
                providers: HashMap<String, Provider>,
                #[serde(default)]
                tier_routing: TomlTierRouting,
            }

            if let Ok(root) = toml::from_str::<FullRoot>(toml_str) {
                let tier_routing = TierRouting {
                    opus: root.tier_routing.opus,
                    sonnet: root.tier_routing.sonnet,
                    haiku: root.tier_routing.haiku,
                    override_all: root.tier_routing.override_all,
                };
                return Self {
                    providers: root.providers,
                    tier_routing,
                };
            }

            // Fallback: try parsing without tier_routing (for backwards compat)
            if let Ok(root) = toml::from_str::<TomlRoot>(toml_str) {
                return Self {
                    providers: root.providers,
                    tier_routing: TierRouting::default(),
                };
            }
        }

        Self {
            providers: HashMap::new(),
            tier_routing: TierRouting::default(),
        }
    }

    /// Get the provider config, reloading from disk if the file has been modified.
    /// This implements hot-reload: changing providers.toml takes effect on the next call.
    /// Returns an `Arc` so callers can cheaply clone it for async handlers.
    pub fn get() -> Arc<ProviderConfig> {
        // Fast path: check if we have a cached config that's still fresh
        {
            let guard = PROVIDERS.read().unwrap();
            if guard.is_some() {
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
                let best_mtime = paths
                    .iter()
                    .filter_map(|p| p.metadata().ok().and_then(|m| m.modified().ok()))
                    .max()
                    .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
                let old_mtime = CONFIG_MTIME.load(Ordering::SeqCst);
                // If file hasn't been modified since last load, use cached
                if best_mtime.map(|m| m <= old_mtime).unwrap_or(false) {
                    return guard.as_ref().unwrap().clone();
                }
            }
        }

        // Slow path: write lock + reload
        let mut guard = PROVIDERS.write().unwrap();

        // Re-check after acquiring write lock (another thread may have loaded)
        let needs_load = guard.is_none() || {
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
            let best_mtime = paths
                .iter()
                .filter_map(|p| p.metadata().ok().and_then(|m| m.modified().ok()))
                .max()
                .map(|t| t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs());
            let old_mtime = CONFIG_MTIME.load(Ordering::SeqCst);
            best_mtime.map(|m| m > old_mtime).unwrap_or(true)
        };

        if needs_load {
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
            if let Some(t) = paths
                .iter()
                .filter_map(|p| p.metadata().ok().and_then(|m| m.modified().ok()))
                .max()
            {
                CONFIG_MTIME.store(
                    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs(),
                    Ordering::SeqCst,
                );
            }
            *guard = Some(Arc::new(Self::load()));
        }

        guard.as_ref().unwrap().clone()
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
                    tools: entry.tools.unwrap_or(false),
                    vision: entry.vision.unwrap_or(false),
                    thinking: entry.thinking.unwrap_or(false),
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

        // Tier keyword fallback: if model_id looks like a tier request,
        // use the improved extraction method
        if let Some(tier) = Tier::extract_from_model_id(model_id) {
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

    /// Resolve a tier request by walking the tier_routing preference list.
    /// Returns (provider_name, provider, model_entry, preference_position, total_preferences).
    /// Falls through to (None) if no model in the list resolves.
    ///
    /// Logs warnings for invalid model IDs that don't exist in any provider.
    pub fn resolve_tier_route(
        &self,
        tier: Tier,
    ) -> Option<(&str, &Provider, &ModelEntry, usize, usize)> {
        let models = self.tier_routing.models_for_tier(&tier);
        let total = models.len();

        if total == 0 {
            // No explicit tier_routing config: fall back to first matching provider
            // (same as old HashMap-based behavior)
            for (name, provider) in &self.providers {
                for entry in &provider.models {
                    if provider.effective_tier(entry) == tier {
                        return Some((name.as_str(), provider, entry, 1, 1));
                    }
                }
            }
            return None;
        }

        let mut seen_invalid: Vec<&str> = Vec::new();

        for (i, model_id) in models.iter().enumerate() {
            let pos = i + 1;

            // Check override_all first
            if let Some(ref override_model) = self.tier_routing.override_all {
                if let Some(result) = self.resolve_model(override_model) {
                    return Some((result.0, result.1, result.2, 1, 1));
                }
            }

            // Try to resolve this model
            if let Some((name, provider, entry)) = self.resolve_model(model_id) {
                return Some((name, provider, entry, pos, total));
            }

            // Model not found — record for warning
            seen_invalid.push(model_id.as_str());
        }

        // All models failed — log warnings for invalid ones
        if !seen_invalid.is_empty() {
            eprintln!(
                "  tier_route warning: no models resolved for {:?} tier: invalid IDs: {:?}",
                tier, seen_invalid
            );
        }

        None
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
        assert!(ids.contains(&"MiniMax-M3"), "should have M3");
        assert!(ids.contains(&"MiniMax-M2.7"), "should have M2.7");
        assert!(
            ids.contains(&"MiniMax-M2.7-highspeed"),
            "should have M2.7-highspeed"
        );
        assert!(ids.contains(&"MiniMax-M2.5"), "should have M2.5");
        assert!(ids.contains(&"MiniMax-M2"), "should have M2");
        assert!(
            ids.len() >= 8,
            "should have at least 8 MiniMax models, got {}",
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

    // ── Tier extraction (AC24) ───────────────────────────────────────────────

    #[test]
    fn tier_extract_anthropic_sonnet() {
        assert_eq!(
            Tier::extract_from_model_id("claude-sonnet-4-20250514"),
            Some(Tier::Sonnet)
        );
    }

    #[test]
    fn tier_extract_anthropic_opus() {
        assert_eq!(
            Tier::extract_from_model_id("claude-opus-4-7"),
            Some(Tier::Opus)
        );
    }

    #[test]
    fn tier_extract_anthropic_haiku() {
        assert_eq!(
            Tier::extract_from_model_id("claude-haiku-4-5-20251001"),
            Some(Tier::Haiku)
        );
    }

    #[test]
    fn tier_extract_strips_claude_prefix() {
        // "claude-sonnet-4-6" → "sonnet-4-6" → Sonnet
        assert_eq!(
            Tier::extract_from_model_id("claude-sonnet-4-6"),
            Some(Tier::Sonnet)
        );
    }

    #[test]
    fn tier_extract_unknown_returns_none() {
        assert_eq!(Tier::extract_from_model_id("MiniMax-M2.7"), None);
        assert_eq!(Tier::extract_from_model_id("gemma4:31b"), None);
        assert_eq!(Tier::extract_from_model_id("qwopus3.5-9b-v3"), None);
    }

    // ── Tier routing resolution (AC25) ────────────────────────────────────────

    #[test]
    fn tier_routing_fallback_when_no_tier_routing_config() {
        // When tier_routing is configured (providers.toml has it), resolve_tier_route
        // returns the preference list. Sonnet has 3 models configured: MiniMax-M2.5, glm-5.1, claude-sonnet-4-6.
        let cfg = ProviderConfig::get();
        let result = cfg.resolve_tier_route(Tier::Sonnet);
        assert!(
            result.is_some(),
            "should resolve sonnet tier with tier_routing config"
        );
        let (_, _, entry, pos, total) = result.unwrap();
        // First choice in sonnet list: MiniMax-M2.5
        assert_eq!(pos, 1);
        assert_eq!(total, 4, "sonnet tier has 4 models in tier_routing config");
        assert_eq!(
            entry.id, "MiniMax-M2.5",
            "first sonnet choice should be MiniMax-M2.5"
        );
    }

    // ── Direct model match bypasses tier routing (AC27) ─────────────────────
    // Direct model requests (e.g. "MiniMax-M2.7") are handled by resolve_model
    // before tier routing is consulted. The router checks exact match first.
    #[test]
    fn direct_model_bypasses_tier_routing() {
        let cfg = ProviderConfig::get();
        // Direct model ID should resolve via exact match, NOT tier routing
        let result = cfg.resolve_model("MiniMax-M2.7");
        assert!(result.is_some(), "MiniMax-M2.7 should resolve directly");
        let (name, _, entry) = result.unwrap();
        assert_eq!(name, "minimax");
        assert_eq!(entry.id, "MiniMax-M2.7");
    }

    #[test]
    fn tier_routing_returns_correct_provider() {
        let cfg = ProviderConfig::get();
        // Sonnet tier should resolve to MiniMax-M2.5 as first preference
        let result = cfg.resolve_tier_route(Tier::Sonnet);
        assert!(result.is_some());
        let (_, _, entry, pos, _) = result.unwrap();
        // Should resolve to the first sonnet-tier model available
        assert_eq!(pos, 1);
        assert_eq!(entry.id, "MiniMax-M2.5");
    }
}
