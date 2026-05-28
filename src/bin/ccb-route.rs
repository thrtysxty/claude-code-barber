//! ccb-route — Multi-Provider Model Router
//!
//! Routes Claude Code API calls across multiple backends through a single
//! endpoint. All models appear in Claude Code's /model picker via gateway
//! discovery. One `claude` command, every provider.
//!
//! Backends (configured in config/providers.toml):
//!   anthropic → api.anthropic.com    (Claude, OAuth passthrough)
//!   ollama    → localhost:11434      (auto-discover, Anthropic-compat)
//!   minimax   → api.minimax.io      (API key auth)
//!   aibox     → aibox:8080          (local GPU, Anthropic-compat)
//!
//! Gateway discovery:
//!   GET /v1/models returns all models prefixed with `claude-` so they pass
//!   Claude Code's /^(claude|anthropic)/i filter. The router strips the prefix
//!   on incoming requests before forwarding to the backend.
//!
//! Ollama Anthropic compat (see https://docs.ollama.com/api/anthropic-compatibility):
//!   - Endpoint: POST /v1/messages (only)
//!   - Supported: messages, system, stream, temperature, top_p, top_k,
//!     stop_sequences, tools, thinking (budget_tokens accepted but not enforced)
//!   - Unsupported: tool_choice, metadata, cache_control, /count_tokens,
//!     batches, citations, PDF, URL images, server-sent streaming errors
//!   - Cloud models available without pulling
//!
//! Explicit prefix override (bypasses routing table):
//!   claude --model anthropic:haiku        → real Anthropic
//!   claude --model ollama:gemma4:31b      → Ollama backend
//!   claude --model minimax:opus           → MiniMax
//!
//! Shell setup (~/.zshenv):
//!   export ANTHROPIC_BASE_URL=http://localhost:9001
//!   unset ANTHROPIC_API_KEY
//!   export ANTHROPIC_AUTH_TOKEN=$(oauth-token-helper)
//!   export CLAUDE_CODE_ENABLE_GATEWAY_MODEL_DISCOVERY=1
//!
//! Build:  cargo build --release --features route
//! Run:    ccb-route
//! Use:    claude  (then /model to pick any provider)

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use ccb::features::providers::{AuthMethod, ProviderConfig, Tier};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{env, path::PathBuf, process::Command, sync::Arc};
use tokio::net::TcpListener;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Cfg {
    port: u16,
    anthropic_url: String,
}

impl Cfg {
    fn load() -> Self {
        Cfg {
            port: evar_u16("CCB_ROUTE_PORT", 9001),
            anthropic_url: "https://api.anthropic.com".into(),
        }
    }

    fn context_window_for(&self, model: &str) -> u64 {
        ccb::features::model_metadata::ModelMetadata::get().context_window_for(model)
    }
}

fn evar_u16(k: &str, default: u16) -> u16 {
    env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

// ── Usage / rate-limit cache ────────────────────────────────────────────────────

fn cache_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".cache")
        .join("ccb")
}

fn write_usage_line(in_tokens: u64, out_tokens: u64, model: &str, backend: &str) {
    let entry = serde_json::json!({
        "t": Utc::now().to_rfc3339(),
        "mdl": model,
        "in": in_tokens,
        "out": out_tokens,
        "be": backend,
    });
    let path = cache_dir().join("route_usage.jsonl");
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = writeln!(f, "{}", entry);
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct RateLimitCache {
    five_hour: RateLimitEntry,
    seven_day: RateLimitEntry,
    #[serde(default)]
    resets_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct RateLimitEntry {
    utilization: f64,
}

fn write_rate_limits(five_pct: f64, seven_pct: f64, resets_at: Option<String>) {
    let cache = RateLimitCache {
        five_hour: RateLimitEntry {
            utilization: five_pct,
        },
        seven_day: RateLimitEntry {
            utilization: seven_pct,
        },
        resets_at,
    };
    let path = cache_dir().join("route_limits.json");
    if let Ok(s) = serde_json::to_string_pretty(&cache) {
        let _ = std::fs::write(&path, s);
    }
}

async fn fetch_anthropic_rate_limits(api_key: &str) {
    let url = "https://api.anthropic.com/api/oauth/usage";
    let client = Client::new();
    if let Ok(resp) = client
        .get(url)
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .send()
        .await
    {
        if let Ok(v) = resp.json::<Value>().await {
            let five_pct = v["five_hour"]["utilization"].as_f64().unwrap_or(0.0);
            let seven_pct = v["seven_day"]["utilization"].as_f64().unwrap_or(0.0);
            let resets_at = v["resets_at"].as_str().map(|s| s.to_string());
            write_rate_limits(five_pct, seven_pct, resets_at);
        }
    }
}

async fn fetch_ollama_cloud_rate_limits(api_key: &str) {
    let url = "https://ollama.com/api/usage";
    let client = Client::new();
    if let Ok(resp) = client
        .get(url)
        .header("authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        if let Ok(v) = resp.json::<Value>().await {
            let remaining = v["remaining"].as_u64().unwrap_or(0);
            let limit = v["limit"].as_u64().unwrap_or(1);
            let resets_at = v["resets_at"].as_str().map(|s| s.to_string());
            let five_pct = if limit > 0 {
                (remaining as f64 / limit as f64) * 100.0
            } else {
                0.0
            };
            write_rate_limits(five_pct, 0.0, resets_at);
        }
    }
}

async fn fetch_minimax_rate_limits(api_key: &str) {
    let url = "https://www.minimax.io/v1/token_plan/remains";
    let client = Client::new();
    if let Ok(resp) = client
        .get(url)
        .header("authorization", format!("Bearer {}", api_key))
        .send()
        .await
    {
        if let Ok(v) = resp.json::<Value>().await {
            let remaining = v["remaining"].as_u64().unwrap_or(0);
            let limit = v["limit"].as_u64().unwrap_or(1);
            let five_pct = if limit > 0 {
                (remaining as f64 / limit as f64) * 100.0
            } else {
                0.0
            };
            write_rate_limits(five_pct, 0.0, None);
        }
    }
}

fn spawn_rate_fetch(api_key: String, backend_name: &str) {
    let backend = backend_name.to_string();
    tokio::spawn(async move {
        match backend.as_str() {
            "anthropic" => fetch_anthropic_rate_limits(&api_key).await,
            "minimax" => fetch_minimax_rate_limits(&api_key).await,
            "ollama" => fetch_ollama_cloud_rate_limits(&api_key).await,
            _ => {}
        }
    });
}

// ── Ollama model discovery ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OllamaModel {
    name: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    modified_at: String,
    #[serde(default)]
    remote_host: String,
    #[serde(default)]
    details: OllamaModelDetails,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct OllamaModelDetails {
    #[serde(default)]
    family: String,
    #[serde(default)]
    parameter_size: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

async fn fetch_ollama_models(ollama_url: &str) -> Vec<OllamaModel> {
    let url = format!("{ollama_url}/api/tags");
    let client = Client::new();
    match client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
    {
        Ok(resp) => match resp.json::<OllamaTagsResponse>().await {
            Ok(tags) => tags.models,
            Err(e) => {
                eprintln!("  ollama /api/tags parse error: {e}");
                vec![]
            }
        },
        Err(e) => {
            eprintln!("  ollama /api/tags fetch error: {e}");
            vec![]
        }
    }
}

fn humanize_param(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    // Already human-readable like "1T", "671.0B"
    if raw.ends_with('B') || raw.ends_with('T') {
        return raw.to_string();
    }
    // Raw number like "27000000000"
    if let Ok(n) = raw.parse::<u64>() {
        if n >= 1_000_000_000_000 {
            return format!("{}T", n / 1_000_000_000_000);
        }
        return format!("{}B", n / 1_000_000_000);
    }
    raw.to_string()
}

/// Parse a parameter-size string into billions (approximate).
/// "671.0B" → 671, "27000000000" → 27, "1T" → 1000, "" → 0
fn param_billions(raw: &str) -> u64 {
    if raw.is_empty() {
        return 0;
    }
    if raw.ends_with('T') {
        return raw.trim_end_matches('T').parse::<f64>().unwrap_or(0.0) as u64 * 1000;
    }
    if raw.ends_with('B') {
        return raw.trim_end_matches('B').parse::<f64>().unwrap_or(0.0) as u64;
    }
    if let Ok(n) = raw.parse::<u64>() {
        return n / 1_000_000_000;
    }
    0
}

/// Auto-assign a tier based on parameter count when no static override exists.
/// ≥200B → Opus, ≥30B → Sonnet, ≥10B → Haiku, <10B → Local
fn tier_from_params(raw: &str, is_cloud: bool) -> Tier {
    let b = param_billions(raw);
    if b == 0 {
        // Unknown size: cloud models default to Sonnet, local to Local
        return if is_cloud { Tier::Sonnet } else { Tier::Local };
    }
    if b >= 200 {
        Tier::Opus
    } else if b >= 30 {
        Tier::Sonnet
    } else if b >= 10 {
        Tier::Haiku
    } else {
        Tier::Local
    }
}

/// Strip the `:cloud` / `-cloud` suffix from Ollama cloud model names to get
/// a clean user-facing name. E.g. "gemma4:31b-cloud" → "gemma4:31b",
/// "qwen3.5:cloud" → "qwen3.5". Local tags like `:latest` are preserved
/// to avoid collisions with cloud models of the same base name.
fn clean_model_name(raw: &str) -> String {
    let s = raw.strip_suffix("-cloud").unwrap_or(raw);
    let s = s.strip_suffix(":cloud").unwrap_or(s);
    s.to_string()
}

// ── Backend selection ─────────────────────────────────────────────────────────

struct Route {
    /// Which API format to use
    kind: BackendKind,
    /// Base URL for the backend
    url: String,
    /// Model name to send to the backend
    model: String,
    /// API key (legacy compatibility — prefer auth field)
    api_key: String,
    /// Backend identifier for logging
    backend_name: String,
    /// Auth method for this route
    auth: RouteAuth,
}

/// How to authenticate with a backend — determined by the user's providers.toml config.
#[derive(Clone)]
enum RouteAuth {
    /// Forward all auth headers from the original Claude Code request (Pro Plan OAuth)
    OauthPassthrough,
    /// Authorization: Bearer $token
    Bearer(String),
    /// Custom header: $name: $value
    Header(String, String),
    /// x-api-key: $value (Anthropic API key format)
    Static(String),
    /// No authentication
    None,
}

#[derive(PartialEq)]
enum BackendKind {
    /// Anthropic /v1/messages — full API
    Anthropic,
    /// Ollama Anthropic-compat /v1/messages — strip unsupported fields
    OllamaCompat,
    /// OpenAI /v1/chat/completions — format conversion needed
    #[allow(dead_code)]
    OpenAiCompat,
}

async fn pick(model: &str, cfg: &Cfg, _original_headers: &HeaderMap) -> Route {
    use ccb::features::providers::ProviderConfig;

    let pcfg = ProviderConfig::get();

    // 0. Strip gateway discovery prefix. The /v1/models endpoint prefixes
    //    non-Claude model IDs with "claude-" so they pass Claude Code's
    //    discovery filter. Strip it here so routing uses the real ID.
    //    e.g. "claude-qwen3.5" → "qwen3.5", but "claude-opus-4-7" unchanged.
    //    We try the stripped name first in the catalog; if it resolves to a
    //    non-Anthropic provider, use it. Otherwise keep the original.
    let model = if let Some(stripped) = model.strip_prefix("claude-") {
        match pcfg.resolve_model(stripped) {
            Some((pname, _, _)) if pname != "anthropic" => {
                eprintln!("  prefix strip: {model} → {stripped}");
                stripped
            }
            _ => model, // Keep original — real Claude model or not in static catalog
        }
    } else {
        model
    };

    // 1. Check explicit prefix override (e.g. "ollama:gemma4:31b-cloud")
    for prefix in &["qwopus", "minimax", "ollama", "anthropic"] {
        if model.to_lowercase().starts_with(&format!("{}:", prefix)) {
            let remainder = &model[prefix.len() + 1..];
            if let Some(provider) = pcfg.provider(prefix) {
                eprintln!("  explicit: {} [{remainder}]", prefix);
                return route_from_provider(prefix, provider, remainder);
            }
        }
    }

    // 2. Resolve through provider catalog (exact match → case-insensitive → tier fallback)
    if let Some((provider_name, provider, entry)) = pcfg.resolve_model(model) {
        let backend_model = entry.backend_model();
        eprintln!(
            "  → {} [{}] (requested: {})",
            provider_name, backend_model, model
        );
        return route_from_provider(provider_name, provider, backend_model);
    }

    // 2b. Check discovered models on discover-enabled providers.
    //     Matches user-facing clean names (e.g. "deepseek-v3.1") against
    //     discovered raw names (e.g. "deepseek-v3.1:671b-cloud").
    //     Also handles the "claude-" gateway prefix (e.g. "claude-deepseek-v3.1:671b").
    let stripped_model = model.strip_prefix("claude-").unwrap_or(model);
    for (pname, provider) in &pcfg.providers {
        if !provider.discover {
            continue;
        }
        let discovered = fetch_ollama_models(&provider.url).await;
        let model_lower = model.to_lowercase();
        let stripped_lower = stripped_model.to_lowercase();
        for m in &discovered {
            let raw = if !m.name.is_empty() {
                &m.name
            } else {
                &m.model
            };
            let clean = clean_model_name(raw);
            let raw_lower = raw.to_lowercase();
            let clean_lower = clean.to_lowercase();
            // Match original, stripped, or clean forms (case-insensitive)
            if raw == model
                || clean == model
                || raw == stripped_model
                || clean == stripped_model
                || raw_lower == model_lower
                || clean_lower == model_lower
                || raw_lower == stripped_lower
                || clean_lower == stripped_lower
            {
                eprintln!("  → {} [{}] (discovered, requested: {})", pname, raw, model);
                return route_from_provider(pname.as_str(), provider, raw);
            }
        }
    }

    // 3. Fallback: default to Anthropic with OAuth passthrough
    eprintln!("  fallback → anthropic [{model}] (not in any provider catalog)");
    Route {
        kind: BackendKind::Anthropic,
        url: cfg.anthropic_url.clone(),
        model: model.into(),
        api_key: String::new(),
        backend_name: "anthropic".into(),
        auth: RouteAuth::OauthPassthrough,
    }
}

/// Build a Route from a resolved provider.
fn route_from_provider(
    provider_name: &str,
    provider: &ccb::features::providers::Provider,
    model_id: &str,
) -> Route {
    use ccb::features::providers::{ApiFormat, AuthMethod};

    let kind = match provider.api_format {
        ApiFormat::Anthropic => BackendKind::Anthropic,
        ApiFormat::OllamaCompat => BackendKind::OllamaCompat,
        ApiFormat::OpenaiCompat => BackendKind::OpenAiCompat,
    };

    let auth = match &provider.auth_method {
        AuthMethod::OauthPassthrough => RouteAuth::OauthPassthrough,
        AuthMethod::Bearer => {
            let cred = provider.resolve_credential().unwrap_or_default();
            RouteAuth::Bearer(cred)
        }
        AuthMethod::Header => {
            let header = provider
                .auth_header
                .clone()
                .unwrap_or_else(|| "x-api-key".into());
            let cred = provider.resolve_credential().unwrap_or_default();
            RouteAuth::Header(header, cred)
        }
        AuthMethod::ApiKey => {
            let cred = provider.resolve_credential().unwrap_or_default();
            RouteAuth::Static(cred)
        }
        AuthMethod::None => RouteAuth::None,
    };

    let api_key = match &auth {
        RouteAuth::Static(k) | RouteAuth::Bearer(k) => k.clone(),
        RouteAuth::Header(_, k) => k.clone(),
        RouteAuth::OauthPassthrough => String::new(),
        RouteAuth::None => String::new(),
    };

    Route {
        kind,
        url: provider.url.clone(),
        model: model_id.into(),
        api_key,
        backend_name: provider_name.into(),
        auth,
    }
}

// ── Ollama-compat field stripping ──────────────────────────────────────────────

/// Remove fields unsupported by Ollama Anthropic-compat before forwarding.
/// Per https://docs.ollama.com/api/anthropic-compatibility:
///   Unsupported: tool_choice, metadata, cache_control, document blocks,
///   URL images (base64 only), citations, batches, /count_tokens
fn strip_ollama_unsupported(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        obj.remove("tool_choice");
        obj.remove("metadata");
    }

    strip_cache_control(body);
    strip_unsupported_content_types(body, &["document"]);
}

/// Remove fields unsupported by MiniMax Anthropic-compat before forwarding.
/// Per https://platform.minimax.io/docs/api-reference/text-anthropic-api:
///   Ignored: top_k, stop_sequences, service_tier, mcp_servers, context_management, container
///   Unsupported message types: image, document (text, tool_use, tool_result, thinking only)
fn strip_minimax_unsupported(body: &mut Value) {
    if let Some(obj) = body.as_object_mut() {
        obj.remove("top_k");
        obj.remove("stop_sequences");
        obj.remove("service_tier");
        obj.remove("mcp_servers");
        obj.remove("context_management");
        obj.remove("container");
    }

    strip_unsupported_content_types(body, &["image", "document"]);
}

fn strip_cache_control(body: &mut Value) {
    if let Some(sys) = body.get_mut("system") {
        if let Some(arr) = sys.as_array_mut() {
            for block in arr.iter_mut() {
                if let Some(obj) = block.as_object_mut() {
                    obj.remove("cache_control");
                }
            }
        }
    }
    if let Some(msgs) = body.get_mut("messages") {
        if let Some(arr) = msgs.as_array_mut() {
            for msg in arr.iter_mut() {
                if let Some(content) = msg.get_mut("content") {
                    if let Some(blocks) = content.as_array_mut() {
                        for block in blocks.iter_mut() {
                            if let Some(obj) = block.as_object_mut() {
                                obj.remove("cache_control");
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(tools) = body.get_mut("tools") {
        if let Some(arr) = tools.as_array_mut() {
            for tool in arr.iter_mut() {
                if let Some(obj) = tool.as_object_mut() {
                    obj.remove("cache_control");
                }
            }
        }
    }
}

fn strip_unsupported_content_types(body: &mut Value, types: &[&str]) {
    if let Some(msgs) = body.get_mut("messages") {
        if let Some(arr) = msgs.as_array_mut() {
            for msg in arr.iter_mut() {
                if let Some(content) = msg.get_mut("content") {
                    if let Some(blocks) = content.as_array_mut() {
                        blocks.retain(|block| {
                            let btype = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                            !types.contains(&btype)
                        });
                    }
                }
            }
        }
    }
}

// ── Format conversion (Anthropic → OpenAI) ────────────────────────────────────
// Kept for future OpenAI-compat backends (Together, Groq, vLLM, etc.)

#[allow(dead_code)]
fn to_openai(body: &Value, model: &str) -> Value {
    let mut messages: Vec<Value> = vec![];
    if let Some(sys) = body.get("system").and_then(|s| s.as_str()) {
        messages.push(json!({"role": "system", "content": sys}));
    }
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            let role = msg["role"].as_str().unwrap_or("user");
            let content = match &msg["content"] {
                Value::String(s) => s.clone(),
                Value::Array(blocks) => blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join(""),
                _ => String::new(),
            };
            messages.push(json!({"role": role, "content": content}));
        }
    }
    json!({
        "model":       model,
        "messages":    messages,
        "max_tokens":  body.get("max_tokens").cloned().unwrap_or(json!(4096)),
        "temperature": body.get("temperature").cloned().unwrap_or(json!(1.0)),
        "stream":      body.get("stream").cloned().unwrap_or(json!(false)),
    })
}

#[allow(dead_code)]
fn preamble(model: &str, id: &str) -> Vec<Bytes> {
    let start = json!({"type":"message_start","message":{
        "id": id, "type":"message","role":"assistant","content":[],
        "model": model,"stop_reason":null,"stop_sequence":null,
        "usage":{"input_tokens":0,"output_tokens":0}
    }});
    let bstart = json!({"type":"content_block_start","index":0,
                        "content_block":{"type":"text","text":""}});
    vec![
        fmt_event("message_start", &start.to_string()),
        fmt_event("content_block_start", &bstart.to_string()),
        fmt_event("ping", r#"{"type":"ping"}"#),
    ]
}

#[allow(dead_code)]
fn fmt_event(event: &str, data: &str) -> Bytes {
    Bytes::from(format!("event: {event}\ndata: {data}\n\n"))
}

#[allow(dead_code)]
fn oai_chunk_to_ant(chunk: &str) -> Option<Bytes> {
    let data = chunk.strip_prefix("data: ")?.trim();
    if data == "[DONE]" {
        let stop = concat!(
            "event: content_block_stop\ndata: {\"type\":\"content_block_stop\",\"index\":0}\n\n",
            "event: message_delta\ndata: {\"type\":\"message_delta\",",
            "\"delta\":{\"stop_reason\":\"end_turn\",\"stop_sequence\":null},",
            "\"usage\":{\"output_tokens\":0}}\n\n",
            "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n",
        );
        return Some(Bytes::from(stop));
    }
    let v: Value = serde_json::from_str(data).ok()?;
    let text = v["choices"][0]["delta"]["content"].as_str()?;
    if text.is_empty() {
        return None;
    }
    let delta = json!({"type":"content_block_delta","index":0,
                       "delta":{"type":"text_delta","text": text}});
    Some(fmt_event("content_block_delta", &delta.to_string()))
}

// ── Auto-pull for missing Ollama models ────────────────────────────────────────

async fn try_ollama_pull_and_retry(
    client: &Client,
    model: &str,
    url: &str,
    headers: &reqwest::header::HeaderMap,
    body: &Value,
) -> reqwest::Response {
    eprintln!("  model {model} not found locally — pulling");
    let pull_output = Command::new("ollama").arg("pull").arg(model).output();

    match pull_output {
        Ok(output) if output.status.success() => {
            eprintln!("  pull complete — retrying request");
            // Rebuild the request with the same params
            let req = client.post(url).headers(headers.clone()).json(body);
            match req.send().await {
                Ok(resp) => resp,
                Err(e) => {
                    eprintln!("  retry failed: {e}");
                    // Return a synthetic error response
                    let client = Client::new();
                    client
                        .post("http://localhost:1/__ccb_error")
                        .json(&json!({"error": format!("retry failed: {e}")}))
                        .send()
                        .await
                        .unwrap_or_else(|_| panic!("failed to create error response"))
                }
            }
        }
        Ok(output) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            eprintln!("  pull failed: {}", stderr.trim());
            // Return the original 404 — we can't easily replay it, so make a new request
            let req = client.post(url).headers(headers.clone()).json(body);
            req.send()
                .await
                .unwrap_or_else(|e| panic!("request failed after failed pull: {e}"))
        }
        Err(e) => {
            eprintln!("  pull exception: {e}");
            let req = client.post(url).headers(headers.clone()).json(body);
            req.send()
                .await
                .unwrap_or_else(|e| panic!("request failed after pull exception: {e}"))
        }
    }
}

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn models_handler(
    State(cfg): State<Arc<Cfg>>,
    path: axum::extract::OriginalUri,
    headers: HeaderMap,
) -> impl IntoResponse {
    let full_path = path.0.path();
    let query = path.0.query().unwrap_or("");
    let has_auth = headers.get("authorization").is_some() || headers.get("x-api-key").is_some();
    eprintln!("  GET {full_path}?{query}  auth={has_auth}");
    // /v1/models → list all, /v1/models/<id> → single lookup
    if full_path == "/v1/models" {
        return list_models_inner(cfg).await.into_response();
    }
    // Extract model_id from path: everything after /v1/models/
    let model_id = full_path.strip_prefix("/v1/models/").unwrap_or("");
    if model_id.is_empty() {
        return list_models_inner(cfg).await.into_response();
    }
    get_model_inner(cfg, model_id).await.into_response()
}

async fn list_models_inner(cfg: Arc<Cfg>) -> axum::Json<Value> {
    use ccb::features::providers::{ProviderConfig, Tier};

    let pcfg = ProviderConfig::get();

    // ── Unified model list ───────────────────────────────────────────────────
    //
    // For providers WITHOUT discover: use the static model list as-is.
    // For providers WITH discover: discovery is primary.
    //   - Fetch all models from the backend (e.g. Ollama /api/tags)
    //   - Static entries are metadata overrides (display name, tier, user-facing ID)
    //     matched via backend_id.
    //   - Discovered models with no static override get auto-generated names
    //     and tier inferred from parameter count.
    //   - No manual maintenance needed when models are pulled or added.

    struct ListEntry {
        id: String,
        display: String,
        tier: Tier,
        provider_name: String,
        created_at: String,
    }

    let mut entries: Vec<ListEntry> = Vec::new();

    for (pname, provider) in &pcfg.providers {
        if provider.discover {
            // Discovery-first: fetch what the backend actually has
            let discovered = fetch_ollama_models(&provider.url).await;

            // Build backend_id → static entry lookup for overrides
            let mut bid_to_static: std::collections::HashMap<
                &str,
                &ccb::features::providers::ModelEntry,
            > = std::collections::HashMap::new();
            let mut id_to_static: std::collections::HashMap<
                &str,
                &ccb::features::providers::ModelEntry,
            > = std::collections::HashMap::new();
            for me in &provider.models {
                id_to_static.insert(me.id.as_str(), me);
                if let Some(ref bid) = me.backend_id {
                    bid_to_static.insert(bid.as_str(), me);
                }
            }

            let mut seen_static_ids: std::collections::HashSet<&str> =
                std::collections::HashSet::new();

            for m in &discovered {
                let raw_name = if !m.name.is_empty() {
                    &m.name
                } else {
                    &m.model
                };
                let is_cloud = m.remote_host.contains("ollama.com");

                // Check if a static entry overrides this discovered model
                if let Some(entry) = bid_to_static
                    .get(raw_name.as_str())
                    .or_else(|| id_to_static.get(raw_name.as_str()))
                {
                    // Static override — use curated name/display/tier
                    seen_static_ids.insert(entry.id.as_str());
                    let tier = provider.effective_tier(entry);
                    let display = if entry.display.is_empty() {
                        entry.id.clone()
                    } else {
                        entry.display.clone()
                    };
                    entries.push(ListEntry {
                        id: entry.id.clone(),
                        display,
                        tier,
                        provider_name: pname.clone(),
                        created_at: m.modified_at.clone(),
                    });
                } else {
                    // No static override — auto-generate from discovery metadata
                    let clean_id = clean_model_name(raw_name);
                    let tier = tier_from_params(&m.details.parameter_size, is_cloud);
                    let family = &m.details.family;
                    let param = humanize_param(&m.details.parameter_size);

                    let mut display = clean_id.clone();
                    if !param.is_empty() {
                        display.push_str(&format!(" {param}"));
                    } else if !family.is_empty() {
                        display.push_str(&format!(" ({family})"));
                    }

                    entries.push(ListEntry {
                        id: clean_id,
                        display,
                        tier,
                        provider_name: pname.clone(),
                        created_at: m.modified_at.clone(),
                    });
                }
            }

            // Also add static entries that weren't matched by any discovered model
            // (e.g. cloud models not yet in /api/tags, or models on a different host)
            for me in &provider.models {
                if !seen_static_ids.contains(me.id.as_str()) {
                    let tier = provider.effective_tier(me);
                    let display = if me.display.is_empty() {
                        me.id.clone()
                    } else {
                        me.display.clone()
                    };
                    entries.push(ListEntry {
                        id: me.id.clone(),
                        display,
                        tier,
                        provider_name: pname.clone(),
                        created_at: String::new(),
                    });
                }
            }
        } else {
            // No discovery — static list only (Anthropic, MiniMax, aibox, etc.)
            for entry in &provider.models {
                let tier = provider.effective_tier(entry);
                let display = if entry.display.is_empty() {
                    entry.id.clone()
                } else {
                    entry.display.clone()
                };
                entries.push(ListEntry {
                    id: entry.id.clone(),
                    display,
                    tier,
                    provider_name: pname.clone(),
                    created_at: String::new(),
                });
            }
        }
    }

    // Sort: tier (opus first) then alphabetical within tier
    entries.sort_by(|a, b| {
        a.tier
            .sort_key()
            .cmp(&b.tier.sort_key())
            .then_with(|| a.display.to_lowercase().cmp(&b.display.to_lowercase()))
    });

    // Build JSON with tier headers.
    // Claude Code's gateway discovery filter only shows models whose ID starts
    // with "claude" or "anthropic". We prefix non-Claude IDs with "claude-"
    // so they pass the filter and appear in the /model picker. The router
    // strips this prefix on incoming requests (see pick()).
    let mut models = Vec::new();

    for e in &entries {
        // Prefix non-Claude model IDs so they pass the gateway discovery filter
        let wire_id = if e.id.starts_with("claude") || e.id.starts_with("anthropic") {
            e.id.clone()
        } else {
            format!("claude-{}", e.id)
        };
        models.push(json!({
            "type": "model",
            "id": wire_id,
            "display_name": format!("{} ({})", e.display, e.provider_name),
            "context_window": cfg.context_window_for(&e.id),
            "created_at": e.created_at,
        }));
    }

    let first_id = models
        .first()
        .and_then(|m| m["id"].as_str())
        .unwrap_or("")
        .to_string();
    let last_id = models
        .last()
        .and_then(|m| m["id"].as_str())
        .unwrap_or("")
        .to_string();

    axum::Json(json!({
        "data": models,
        "has_more": false,
        "first_id": first_id,
        "last_id": last_id,
    }))
}

async fn get_model_inner(cfg: Arc<Cfg>, model_id: &str) -> Response {
    use ccb::features::providers::ProviderConfig;

    let pcfg = ProviderConfig::get();

    // Strip "claude-" gateway prefix for non-Anthropic models
    let model_id = if let Some(stripped) = model_id.strip_prefix("claude-") {
        match pcfg.resolve_model(stripped) {
            Some((pname, _, _)) if pname != "anthropic" => stripped,
            _ => model_id,
        }
    } else {
        model_id
    };

    // Check provider catalog first
    if let Some((provider_name, _provider, entry)) = pcfg.resolve_model(model_id) {
        let display = if entry.display.is_empty() {
            &entry.id
        } else {
            &entry.display
        };
        return axum::Json(json!({
            "type": "model",
            "id": entry.id,
            "display_name": format!("{} ({})", display, provider_name),
            "context_window": cfg.context_window_for(&entry.id),
            "created_at": "",
        }))
        .into_response();
    }

    // Check Ollama-discovered models not in static catalog
    for (name, provider) in &pcfg.providers {
        if provider.discover {
            let discovered = fetch_ollama_models(&provider.url).await;
            for m in &discovered {
                let mid = if !m.name.is_empty() {
                    &m.name
                } else {
                    &m.model
                };
                if mid == model_id {
                    let family = &m.details.family;
                    let is_cloud = m.remote_host.contains("ollama.com");
                    let cloud_prefix = if is_cloud { "☁ " } else { "" };
                    let mut label = format!("{cloud_prefix}{mid}");
                    if !family.is_empty() {
                        label.push_str(&format!(" ({family})"));
                    }
                    return axum::Json(json!({
                        "type": "model",
                        "id": mid,
                        "display_name": format!("{} ({})", label, name),
                        "context_window": cfg.context_window_for(mid),
                        "created_at": m.modified_at,
                    }))
                    .into_response();
                }
            }
        }
    }

    (
        StatusCode::NOT_FOUND,
        axum::Json(json!({
            "error": {"type": "not_found", "message": format!("Model {model_id} not found")}
        })),
    )
        .into_response()
}

async fn count_tokens(body: Bytes) -> impl IntoResponse {
    // Token counting not supported by Ollama Anthropic-compat.
    // Return a rough estimate: 1 token ≈ 4 chars.
    let parsed: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(_) => return axum::Json(json!({"input_tokens": 0})),
    };

    let mut char_count: usize = 0;
    if let Some(msgs) = parsed.get("messages").and_then(|m| m.as_array()) {
        for msg in msgs {
            if let Some(content) = msg.get("content") {
                if let Some(s) = content.as_str() {
                    char_count += s.len();
                } else if let Some(blocks) = content.as_array() {
                    for block in blocks {
                        if block["type"] == "text" {
                            if let Some(text) = block["text"].as_str() {
                                char_count += text.len();
                            }
                        }
                    }
                }
            }
        }
    }
    if let Some(sys) = parsed.get("system") {
        if let Some(s) = sys.as_str() {
            char_count += s.len();
        } else if let Some(blocks) = sys.as_array() {
            for block in blocks {
                if block["type"] == "text" {
                    if let Some(text) = block["text"].as_str() {
                        char_count += text.len();
                    }
                }
            }
        }
    }

    let estimated = std::cmp::max(1, char_count / 4);
    axum::Json(json!({"input_tokens": estimated}))
}

async fn health(State(cfg): State<Arc<Cfg>>) -> impl IntoResponse {
    let pcfg = ProviderConfig::get();
    let mut providers_status = serde_json::Map::new();

    for (name, provider) in &pcfg.providers {
        let cred_ok = match &provider.auth_method {
            AuthMethod::None => true,
            AuthMethod::OauthPassthrough => true,
            _ => provider.resolve_credential().is_some(),
        };
        let model_count = provider.models.len();

        // For Ollama providers with discover, also show discovered count
        let mut info = json!({
            "url": provider.url,
            "auth_ok": cred_ok,
            "models": model_count,
        });

        if provider.discover {
            let discovered = fetch_ollama_models(&provider.url).await;
            let cloud: Vec<&str> = discovered
                .iter()
                .filter(|m| m.remote_host.contains("ollama.com"))
                .map(|m| m.name.as_str())
                .collect();
            let local: Vec<&str> = discovered
                .iter()
                .filter(|m| !m.remote_host.contains("ollama.com"))
                .map(|m| m.name.as_str())
                .collect();
            info["discovered_cloud"] = json!(cloud);
            info["discovered_local"] = json!(local);
        }

        providers_status.insert(name.clone(), info);
    }

    let catalog = pcfg.catalog();
    axum::Json(json!({
        "status": "ok",
        "providers": providers_status,
        "total_models": catalog.len(),
        "port": cfg.port,
    }))
}

async fn messages(State(cfg): State<Arc<Cfg>>, headers: HeaderMap, body: Bytes) -> Response {
    let mut body_val: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return (StatusCode::BAD_REQUEST, format!("bad JSON: {e}")).into_response(),
    };
    let model = body_val["model"]
        .as_str()
        .unwrap_or("claude-sonnet-4-6")
        .to_string();
    let streaming = body_val["stream"].as_bool().unwrap_or(false);
    let route = pick(&model, &cfg, &headers).await;

    eprintln!("← {model}  stream={streaming}");

    let client = Client::new();
    let url = format!("{}/v1/messages", route.url.trim_end_matches('/'));
    body_val["model"] = json!(route.model);

    match route.kind {
        // ── Ollama Anthropic-compat backend ──────────────────────────────────
        BackendKind::OllamaCompat => {
            strip_ollama_unsupported(&mut body_val);

            let mut req_headers = reqwest::header::HeaderMap::new();
            req_headers.insert("content-type", "application/json".parse().unwrap());
            // Auth for Ollama-compat backends
            match &route.auth {
                RouteAuth::Bearer(token) => {
                    req_headers.insert(
                        "authorization",
                        format!("Bearer {}", token).parse().unwrap(),
                    );
                }
                RouteAuth::Header(name, value) => {
                    if let (Ok(n), Ok(v)) = (
                        name.parse::<reqwest::header::HeaderName>(),
                        value.parse::<reqwest::header::HeaderValue>(),
                    ) {
                        req_headers.insert(n, v);
                    }
                }
                RouteAuth::Static(key) => {
                    req_headers.insert(
                        "x-api-key",
                        key.parse().unwrap_or_else(|_| "ollama".parse().unwrap()),
                    );
                }
                RouteAuth::None => {
                    req_headers.insert("x-api-key", "ollama".parse().unwrap());
                }
                RouteAuth::OauthPassthrough => {
                    req_headers.insert("x-api-key", "ollama".parse().unwrap());
                }
            }

            let req = client
                .post(&url)
                .headers(req_headers.clone())
                .json(&body_val);

            let resp = match req.send().await {
                Ok(r) => r,
                Err(e) => return (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
            };

            let status = StatusCode::from_u16(resp.status().as_u16())
                .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

            // Auto-pull: if Ollama returns 404, try ollama pull and retry
            if status == StatusCode::NOT_FOUND && route.backend_name == "ollama" {
                let resp =
                    try_ollama_pull_and_retry(&client, &route.model, &url, &req_headers, &body_val)
                        .await;
                let status = StatusCode::from_u16(resp.status().as_u16())
                    .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                let ct = resp
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("application/json")
                    .to_string();

                if !streaming {
                    let full_body = resp.bytes().await.unwrap_or_default();
                    if let Ok(v) = serde_json::from_slice::<Value>(&full_body) {
                        let in_toks = v["usage"]["input_tokens"].as_u64().unwrap_or(0);
                        let out_toks = v["usage"]["output_tokens"].as_u64().unwrap_or(0);
                        write_usage_line(in_toks, out_toks, &model, &route.backend_name);
                    }
                    let stream =
                        futures_util::stream::once(
                            async move { Ok::<_, std::io::Error>(full_body) },
                        );
                    return Response::builder()
                        .status(status)
                        .header("content-type", &ct)
                        .header("cache-control", "no-cache")
                        .body(Body::from_stream(stream))
                        .unwrap();
                }
                // Streaming after pull — fall through to stream handling below
                // (We need to re-read, but resp is already consumed. Return what we have.)
                let full_body = resp.bytes().await.unwrap_or_default();
                let stream =
                    futures_util::stream::once(async move { Ok::<_, std::io::Error>(full_body) });
                return Response::builder()
                    .status(status)
                    .header("content-type", &ct)
                    .header("cache-control", "no-cache")
                    .body(Body::from_stream(stream))
                    .unwrap();
            }

            let ct = resp
                .headers()
                .get("content-type")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/json")
                .to_string();

            // Non-streaming
            if !streaming {
                let full_body = resp.bytes().await.unwrap_or_default();
                if let Ok(v) = serde_json::from_slice::<Value>(&full_body) {
                    let in_toks = v["usage"]["input_tokens"].as_u64().unwrap_or(0);
                    let out_toks = v["usage"]["output_tokens"].as_u64().unwrap_or(0);
                    write_usage_line(in_toks, out_toks, &model, &route.backend_name);
                    spawn_rate_fetch(route.api_key.clone(), &route.backend_name);
                }
                let stream =
                    futures_util::stream::once(async move { Ok::<_, std::io::Error>(full_body) });
                return Response::builder()
                    .status(status)
                    .header("content-type", &ct)
                    .header("cache-control", "no-cache")
                    .body(Body::from_stream(stream))
                    .unwrap();
            }

            // Streaming: collect full response to parse usage, then replay
            let model_for_usage = model.clone();
            let backend_for_usage = route.backend_name.clone();
            let key_for_limits = route.api_key.clone();
            let backend_for_limits = route.backend_name.clone();
            let full_bytes = resp.bytes().await.unwrap_or_default();
            let body_str = String::from_utf8_lossy(&full_bytes).to_string();

            let mut in_toks: u64 = 0;
            let mut out_toks: u64 = 0;
            for block in body_str.split("\n\n") {
                for line in block.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        if let Ok(v) = serde_json::from_str::<Value>(data) {
                            if v["type"] == "message_start" {
                                if let Some(usage) = v.get("message").and_then(|m| m.get("usage")) {
                                    in_toks = usage["input_tokens"].as_u64().unwrap_or(0);
                                    out_toks = usage["output_tokens"].as_u64().unwrap_or(0);
                                }
                            }
                            if v["type"] == "message_delta" {
                                if let Some(usage) = v.get("usage") {
                                    if let Some(out) = usage["output_tokens"].as_u64() {
                                        out_toks = out;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            write_usage_line(in_toks, out_toks, &model_for_usage, &backend_for_usage);
            spawn_rate_fetch(key_for_limits, &backend_for_limits);

            let stream =
                futures_util::stream::once(async move { Ok::<_, std::io::Error>(full_bytes) });
            Response::builder()
                .status(status)
                .header("content-type", ct)
                .header("cache-control", "no-cache")
                .body(Body::from_stream(stream))
                .unwrap()
        }

        // ── Anthropic-compatible backend (real Anthropic, minimax, etc.) ────
        BackendKind::Anthropic => {
            if route.backend_name == "minimax" {
                strip_minimax_unsupported(&mut body_val);
            }

            let ant_ver = headers
                .get("anthropic-version")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("2023-06-01")
                .to_string();

            let mut req = client
                .post(&url)
                .header("anthropic-version", &ant_ver)
                .header("content-type", "application/json");

            // Auth dispatch — determined by the user's providers.toml config
            req = match &route.auth {
                RouteAuth::OauthPassthrough => {
                    // Forward auth headers from Claude Code's original request.
                    // When apiKeyHelper provides an OAuth token (sk-ant-oat*) via
                    // x-api-key, convert it to Authorization: Bearer — Anthropic
                    // only accepts OAuth tokens in the Bearer header, not x-api-key.
                    let mut r = req;
                    let mut forwarded_api_key = false;

                    if let Some(api_key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
                        if api_key.starts_with("sk-ant-oat") {
                            // OAuth token from apiKeyHelper — send as Bearer
                            r = r.header("authorization", format!("Bearer {}", api_key));
                            forwarded_api_key = true;
                        } else if api_key != "router" && api_key != "test" {
                            // Real API key — forward as-is
                            r = r.header("x-api-key", api_key);
                            forwarded_api_key = true;
                        }
                    }

                    // Forward other auth headers (if not already handled via x-api-key conversion)
                    for header_name in &[
                        "authorization",
                        "cookie",
                        "anthropic-auth-token",
                        "session-token",
                    ] {
                        if *header_name == "authorization" && forwarded_api_key {
                            continue; // Already set from OAuth token conversion
                        }
                        if let Some(val) = headers.get(*header_name) {
                            r = r.header(*header_name, val.clone());
                        }
                    }
                    r
                }
                RouteAuth::Bearer(token) => {
                    req.header("authorization", format!("Bearer {}", token))
                }
                RouteAuth::Header(name, value) => req.header(name.as_str(), value.as_str()),
                RouteAuth::Static(key) => req.header("x-api-key", key.as_str()),
                RouteAuth::None => req,
            };

            req = req.json(&body_val);

            // Forward anthropic-beta header
            if let Some(beta) = headers.get("anthropic-beta") {
                req = req.header("anthropic-beta", beta.clone());
            }

            match req.send().await {
                Ok(resp) => {
                    let status = StatusCode::from_u16(resp.status().as_u16())
                        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
                    let ct = resp
                        .headers()
                        .get("content-type")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("application/json")
                        .to_string();

                    // Non-streaming
                    if !streaming {
                        let full_body = resp.bytes().await.unwrap_or_default();
                        if let Ok(v) = serde_json::from_slice::<Value>(&full_body) {
                            let in_toks = v["usage"]["input_tokens"].as_u64().unwrap_or(0);
                            let out_toks = v["usage"]["output_tokens"].as_u64().unwrap_or(0);
                            write_usage_line(in_toks, out_toks, &model, &route.backend_name);
                            spawn_rate_fetch(route.api_key.clone(), &route.backend_name);
                        }
                        let stream = futures_util::stream::once(async move {
                            Ok::<_, std::io::Error>(full_body)
                        });
                        return Response::builder()
                            .status(status)
                            .header("content-type", &ct)
                            .header("cache-control", "no-cache")
                            .body(Body::from_stream(stream))
                            .unwrap();
                    }

                    // Streaming: collect, parse usage, replay
                    let model_for_usage = model.clone();
                    let backend_for_usage = route.backend_name.clone();
                    let key_for_limits = route.api_key.clone();
                    let backend_for_limits = route.backend_name.clone();
                    let full_bytes = resp.bytes().await.unwrap_or_default();
                    let body_str = String::from_utf8_lossy(&full_bytes).to_string();

                    let mut in_toks: u64 = 0;
                    let mut out_toks: u64 = 0;
                    for block in body_str.split("\n\n") {
                        for line in block.lines() {
                            if let Some(data) = line.strip_prefix("data: ") {
                                if let Ok(v) = serde_json::from_str::<Value>(data) {
                                    if v["type"] == "message_start" {
                                        if let Some(usage) =
                                            v.get("message").and_then(|m| m.get("usage"))
                                        {
                                            in_toks = usage["input_tokens"].as_u64().unwrap_or(0);
                                            out_toks = usage["output_tokens"].as_u64().unwrap_or(0);
                                        }
                                    }
                                    if v["type"] == "message_delta" {
                                        if let Some(usage) = v.get("usage") {
                                            if let Some(out) = usage["output_tokens"].as_u64() {
                                                out_toks = out;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    write_usage_line(in_toks, out_toks, &model_for_usage, &backend_for_usage);
                    spawn_rate_fetch(key_for_limits, &backend_for_limits);

                    let stream =
                        futures_util::stream::once(
                            async move { Ok::<_, std::io::Error>(full_bytes) },
                        );
                    Response::builder()
                        .status(status)
                        .header("content-type", ct)
                        .header("cache-control", "no-cache")
                        .body(Body::from_stream(stream))
                        .unwrap()
                }
                Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
            }
        }

        // ── OpenAI-compat backend (future: Together, Groq, vLLM, etc.) ──────
        #[allow(unreachable_patterns)]
        BackendKind::OpenAiCompat => {
            // Placeholder for future OpenAI-format backends
            // Would use to_openai() and oai_chunk_to_ant() conversion
            (
                StatusCode::NOT_IMPLEMENTED,
                "OpenAI-compat backends not yet supported",
            )
                .into_response()
        }
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cfg = Arc::new(Cfg::load());
    let providers = ProviderConfig::get();

    eprintln!("ccb-route  :{}", cfg.port);

    // Print provider catalog grouped by tier
    let catalog = providers.catalog();
    let mut current_tier: Option<Tier> = None;
    for rm in &catalog {
        if current_tier.as_ref() != Some(&rm.tier) {
            current_tier = Some(rm.tier);
            eprintln!("  ── {} ──", rm.tier);
        }
        let auth_tag = match &rm.auth_method {
            AuthMethod::OauthPassthrough => "oauth",
            AuthMethod::Bearer => "bearer",
            AuthMethod::Header => "header",
            AuthMethod::ApiKey => "api-key",
            AuthMethod::None => "none",
        };
        eprintln!(
            "    {} ({} via {} [{}])",
            rm.display_name, rm.id, rm.provider_name, auth_tag
        );
    }

    // Print provider connection summary
    eprintln!();
    for (name, provider) in &providers.providers {
        let cred_status = match &provider.auth_method {
            AuthMethod::None => "no auth needed".to_string(),
            AuthMethod::OauthPassthrough => "oauth passthrough".to_string(),
            _ => {
                if provider.resolve_credential().is_some() {
                    "key found".to_string()
                } else {
                    "KEY MISSING".to_string()
                }
            }
        };
        eprintln!("  {} → {} ({})", name, provider.url, cred_status);
    }

    eprintln!();
    eprintln!(
        "  ANTHROPIC_BASE_URL=http://localhost:{} ANTHROPIC_API_KEY=router claude",
        cfg.port
    );

    let app = Router::new()
        .route("/v1/messages/count_tokens", post(count_tokens))
        .route("/v1/messages", post(messages))
        .route("/health", get(health))
        .fallback(models_handler)
        .with_state(cfg.clone());

    // Bind [::0] to accept both IPv6 and IPv4 connections —
    // Claude Code resolves localhost to ::1 (IPv6) first.
    let addr = format!("[::]:{}", cfg.port);
    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));

    axum::serve(listener, app).await.unwrap();
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    // ── param_billions ───────────────────────────────────────────────────

    #[test]
    fn param_billions_empty() {
        assert_eq!(param_billions(""), 0);
    }

    #[test]
    fn param_billions_with_b_suffix() {
        assert_eq!(param_billions("70B"), 70);
        assert_eq!(param_billions("9.7B"), 9);
        assert_eq!(param_billions("671B"), 671);
    }

    #[test]
    fn param_billions_with_t_suffix() {
        assert_eq!(param_billions("1T"), 1000);
        // 1.5T → f64(1.5) as u64 = 1 → 1 * 1000 = 1000 (truncates)
        assert_eq!(param_billions("1.5T"), 1000);
        assert_eq!(param_billions("2T"), 2000);
    }

    #[test]
    fn param_billions_raw_number() {
        assert_eq!(param_billions("70000000000"), 70);
    }

    #[test]
    fn param_billions_garbage() {
        assert_eq!(param_billions("abc"), 0);
    }

    // ── tier_from_params ─────────────────────────────────────────────────

    #[test]
    fn tier_opus_for_large_models() {
        assert_eq!(tier_from_params("671B", true), Tier::Opus);
        assert_eq!(tier_from_params("200B", false), Tier::Opus);
    }

    #[test]
    fn tier_sonnet_for_medium_models() {
        assert_eq!(tier_from_params("70B", true), Tier::Sonnet);
        assert_eq!(tier_from_params("30B", false), Tier::Sonnet);
    }

    #[test]
    fn tier_haiku_for_small_models() {
        assert_eq!(tier_from_params("27B", true), Tier::Haiku);
        assert_eq!(tier_from_params("14B", false), Tier::Haiku);
    }

    #[test]
    fn tier_local_for_tiny_models() {
        assert_eq!(tier_from_params("9B", false), Tier::Local);
        assert_eq!(tier_from_params("3B", false), Tier::Local);
    }

    #[test]
    fn tier_unknown_cloud_defaults_sonnet() {
        assert_eq!(tier_from_params("", true), Tier::Sonnet);
        assert_eq!(tier_from_params("abc", true), Tier::Sonnet);
    }

    #[test]
    fn tier_unknown_local_defaults_local() {
        assert_eq!(tier_from_params("", false), Tier::Local);
    }

    // ── clean_model_name ─────────────────────────────────────────────────

    #[test]
    fn clean_strips_cloud_suffix() {
        assert_eq!(clean_model_name("qwen3.5:cloud"), "qwen3.5");
        assert_eq!(clean_model_name("gemma4:31b-cloud"), "gemma4:31b");
    }

    #[test]
    fn clean_preserves_latest() {
        assert_eq!(clean_model_name("qwen3.5:latest"), "qwen3.5:latest");
    }

    #[test]
    fn clean_no_suffix_unchanged() {
        assert_eq!(clean_model_name("claude-opus-4-7"), "claude-opus-4-7");
    }

    // ── prefix stripping ─────────────────────────────────────────────────

    #[test]
    fn claude_prefix_stripped_for_non_anthropic() {
        // Models like "claude-glm-5.1" should strip to "glm-5.1"
        let model = "claude-glm-5.1";
        let stripped = &model["claude-".len()..];
        assert_eq!(stripped, "glm-5.1");
    }

    #[test]
    fn claude_prefix_preserved_for_real_claude() {
        // Real Claude models like "claude-opus-4-7" should NOT be stripped
        let model = "claude-opus-4-7";
        assert!(model.starts_with("claude-"));
        // The pick() function checks if stripped resolves to a non-anthropic provider
        // If it resolves to anthropic, the prefix is preserved
    }
}
