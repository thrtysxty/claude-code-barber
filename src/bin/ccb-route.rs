//! ccb-route — Model Router
//!
//! Routes Claude Code API calls by model tier to a configurable backend.
//! Backends: aibox (Anthropic-compat, local GPU), ollama (OpenAI-compat, local),
//!           anthropic (real Anthropic API)
//!
//! Tier routing (env var → default):
//!   CCB_HAIKU_BACKEND  → aibox      (haiku-tier requests)
//!   CCB_SONNET_BACKEND → ollama     (sonnet-tier requests)
//!   CCB_OPUS_BACKEND   → anthropic  (opus-tier + unrecognized)
//!
//! Build:  cargo build --release --features route
//! Run:    ccb-route
//! Use:    ANTHROPIC_BASE_URL=http://localhost:9001 ANTHROPIC_API_KEY=router claude
//!
//! Examples:
//!   CCB_SONNET_BACKEND=aibox ccb-route   # route sonnet to qwopus too
//!   CCB_HAIKU_BACKEND=anthropic ccb-route # route haiku to real Anthropic

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Router,
};
use bytes::Bytes;
use futures_util::StreamExt;
use reqwest::Client;
use serde_json::{json, Value};
use std::{env, fs, str, sync::Arc};
use tokio::net::TcpListener;
use uuid::Uuid;

// ── Config ────────────────────────────────────────────────────────────────────

#[derive(Clone)]
struct Cfg {
    port: u16,
    aibox_url: String,
    aibox_model: String,
    ollama_url: String,
    ollama_model: String,
    anthropic_url: String,
    real_key: String,
    // which backend handles each Claude model tier
    haiku_backend: String, // "aibox" | "ollama" | "anthropic"
    sonnet_backend: String,
    opus_backend: String,
}

impl Cfg {
    fn load() -> Self {
        Cfg {
            port: evar_u16("CCB_ROUTE_PORT", 9001),
            aibox_url: evar("AIBOX_URL", "http://aibox:8080"),
            aibox_model: evar("AIBOX_MODEL", "qwopus3.5-9b-v3"),
            ollama_url: evar("OLLAMA_URL", "http://localhost:11434"),
            ollama_model: evar("OLLAMA_MODEL", "glm-5.1:cloud"),
            anthropic_url: "https://api.anthropic.com".into(),
            real_key: load_key(),
            haiku_backend: evar("CCB_HAIKU_BACKEND", "aibox"),
            sonnet_backend: evar("CCB_SONNET_BACKEND", "ollama"),
            opus_backend: evar("CCB_OPUS_BACKEND", "anthropic"),
        }
    }

    fn tier(&self, model: &str) -> &str {
        let m = model.to_lowercase();
        if m.contains("haiku") {
            &self.haiku_backend
        } else if m.contains("sonnet") {
            &self.sonnet_backend
        } else {
            &self.opus_backend
        }
    }
}

fn evar(k: &str, default: &str) -> String {
    env::var(k).unwrap_or_else(|_| default.into())
}

fn evar_u16(k: &str, default: u16) -> u16 {
    env::var(k)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn load_key() -> String {
    if let Ok(k) = env::var("ANTHROPIC_API_KEY_REAL") {
        return k;
    }
    let path = dirs::home_dir().unwrap_or_default().join(".secrets");
    if let Ok(src) = fs::read_to_string(path) {
        for line in src.lines() {
            let l = line.trim();
            if l.starts_with("ANTHROPIC_API_KEY=") && !l.to_lowercase().contains("router") {
                #[allow(clippy::manual_split_once)]
                return l
                    .splitn(2, '=')
                    .nth(1)
                    .unwrap_or("")
                    .trim_matches('"')
                    .trim_matches('\'')
                    .into();
            }
        }
    }
    env::var("ANTHROPIC_API_KEY").unwrap_or_default()
}

// ── Backend selection ─────────────────────────────────────────────────────────

enum Backend {
    Anthropic,
    Ollama,
}

struct Route {
    kind: Backend,
    url: String,
    model: String,
    api_key: String,
}

fn pick(model: &str, cfg: &Cfg) -> Route {
    let tier = cfg.tier(model);
    match tier {
        "aibox" => {
            eprintln!("  → aibox [{}] ({})", cfg.aibox_model, model);
            Route {
                kind: Backend::Anthropic,
                url: cfg.aibox_url.clone(),
                model: cfg.aibox_model.clone(),
                api_key: "ollama".into(),
            }
        }
        "ollama" => {
            eprintln!("  → ollama [{}] ({})", cfg.ollama_model, model);
            Route {
                kind: Backend::Ollama,
                url: cfg.ollama_url.clone(),
                model: cfg.ollama_model.clone(),
                api_key: "ollama".into(),
            }
        }
        _ => {
            eprintln!("  → anthropic [{}]", model);
            Route {
                kind: Backend::Anthropic,
                url: cfg.anthropic_url.clone(),
                model: model.into(),
                api_key: cfg.real_key.clone(),
            }
        }
    }
}

// ── Format conversion (Anthropic ↔ OpenAI) ───────────────────────────────────

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

fn fmt_event(event: &str, data: &str) -> Bytes {
    Bytes::from(format!("event: {event}\ndata: {data}\n\n"))
}

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

// ── Handlers ─────────────────────────────────────────────────────────────────

async fn health(State(cfg): State<Arc<Cfg>>) -> impl IntoResponse {
    axum::Json(json!({
        "status":  "ok",
        "routing": {
            "haiku":  cfg.haiku_backend,
            "sonnet": cfg.sonnet_backend,
            "opus":   cfg.opus_backend,
        },
        "backends": {
            "aibox":     format!("{} [{}]", cfg.aibox_url,     cfg.aibox_model),
            "ollama":    format!("{} [{}]", cfg.ollama_url,    cfg.ollama_model),
            "anthropic": cfg.anthropic_url,
        },
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
    let route = pick(&model, &cfg);

    eprintln!("← {model}  stream={streaming}");

    let client = Client::new();

    match route.kind {
        // ── Anthropic-compatible backend (aibox or real Anthropic) ──────────
        Backend::Anthropic => {
            body_val["model"] = json!(route.model);
            let ant_ver = headers
                .get("anthropic-version")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("2023-06-01")
                .to_string();

            let url = format!("{}/v1/messages", route.url.trim_end_matches('/'));
            let mut req = client
                .post(&url)
                .header("x-api-key", &route.api_key)
                .header("anthropic-auth-token", &route.api_key)
                .header("anthropic-version", ant_ver)
                .header("content-type", "application/json")
                .json(&body_val);

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
                    let stream = resp
                        .bytes_stream()
                        .map(|r| {
                            #[allow(clippy::redundant_closure)]
                            r.map_err(|e| std::io::Error::other(e))
                        });
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

        // ── Ollama / OpenAI-compat backend ──────────────────────────────────
        Backend::Ollama => {
            let oai = to_openai(&body_val, &route.model);
            let url = format!("{}/v1/chat/completions", route.url.trim_end_matches('/'));
            let msg_id = format!("msg_{}", Uuid::new_v4().simple());
            let model_name = route.model.clone();

            if streaming {
                let mut oai_stream = oai.clone();
                oai_stream["stream"] = json!(true);

                match client
                    .post(&url)
                    .header("authorization", format!("Bearer {}", route.api_key))
                    .header("content-type", "application/json")
                    .json(&oai_stream)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let pre = preamble(&model_name, &msg_id);
                        let pre_stream = futures_util::stream::iter(
                            pre.into_iter().map(Ok::<_, std::io::Error>),
                        );
                        let mut buf = String::new();
                        let byte_stream = resp.bytes_stream().flat_map(move |chunk| {
                            let chunk_str = match &chunk {
                                Ok(b) => String::from_utf8_lossy(b).into_owned(),
                                Err(_) => return futures_util::stream::iter(vec![]).left_stream(),
                            };
                            buf.push_str(&chunk_str);
                            let mut events: Vec<Result<Bytes, std::io::Error>> = vec![];
                            while let Some(pos) = buf.find("\n\n") {
                                let line = buf[..pos].to_string();
                                buf = buf[pos + 2..].to_string();
                                if let Some(b) = oai_chunk_to_ant(&line) {
                                    events.push(Ok(b));
                                }
                            }
                            futures_util::stream::iter(events).right_stream()
                        });

                        Response::builder()
                            .status(200)
                            .header("content-type", "text/event-stream")
                            .header("cache-control", "no-cache")
                            .body(Body::from_stream(pre_stream.chain(byte_stream)))
                            .unwrap()
                    }
                    Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
                }
            } else {
                let mut oai_req = oai;
                oai_req["stream"] = json!(false);
                match client
                    .post(&url)
                    .header("authorization", format!("Bearer {}", route.api_key))
                    .header("content-type", "application/json")
                    .json(&oai_req)
                    .send()
                    .await
                {
                    Ok(resp) => {
                        let oai_body: Value = resp
                            .json()
                            .await
                            .unwrap_or_else(|_| json!({"choices":[{"message":{"content":""}}]}));
                        let text = oai_body["choices"][0]["message"]["content"]
                            .as_str()
                            .unwrap_or("")
                            .to_string();
                        let ant = json!({
                            "id": msg_id, "type":"message","role":"assistant",
                            "model": model_name, "stop_reason":"end_turn",
                            "stop_sequence": null,
                            "usage": {
                                "input_tokens":  oai_body["usage"]["prompt_tokens"].as_u64().unwrap_or(0),
                                "output_tokens": oai_body["usage"]["completion_tokens"].as_u64().unwrap_or(0),
                            },
                            "content": [{"type":"text","text": text}],
                        });
                        (StatusCode::OK, axum::Json(ant)).into_response()
                    }
                    Err(e) => (StatusCode::BAD_GATEWAY, e.to_string()).into_response(),
                }
            }
        }
    }
}

fn backend_label(backend: &str, cfg: &Cfg) -> String {
    match backend {
        "aibox" => format!("{} [{}]", cfg.aibox_url, cfg.aibox_model),
        "ollama" => format!("{} [{}]", cfg.ollama_url, cfg.ollama_model),
        _ => cfg.anthropic_url.clone(),
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cfg = Arc::new(Cfg::load());

    eprintln!("ccb-route  :{}", cfg.port);
    eprintln!(
        "  haiku  → {} ({})",
        cfg.haiku_backend,
        backend_label(&cfg.haiku_backend, &cfg)
    );
    eprintln!(
        "  sonnet → {} ({})",
        cfg.sonnet_backend,
        backend_label(&cfg.sonnet_backend, &cfg)
    );
    eprintln!(
        "  opus   → {} ({})",
        cfg.opus_backend,
        backend_label(&cfg.opus_backend, &cfg)
    );
    eprintln!(
        "  anthropic key: {}",
        if cfg.real_key.is_empty() {
            "MISSING"
        } else {
            "found"
        }
    );
    eprintln!();
    eprintln!(
        "  ANTHROPIC_BASE_URL=http://localhost:{} ANTHROPIC_API_KEY=router claude",
        cfg.port
    );

    let app = Router::new()
        .route("/v1/messages", post(messages))
        .route("/health", get(health))
        .with_state(cfg.clone());

    let addr = format!("0.0.0.0:{}", cfg.port);
    let listener = TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| panic!("bind {addr}: {e}"));

    axum::serve(listener, app).await.unwrap();
}
