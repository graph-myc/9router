//! M Y C backend — Axum HTTP server hosting the agentic orchestrator,
//! with provider/combo CRUD, API keys, and usage analytics (JSON-persisted).

mod config;
mod provider_openai;
mod state;

use aggregator::{ChatChunk, ChatRequest, Combo, ProviderError};
use axum::{
    extract::{Path, Query, Request, State},
    http::StatusCode,
    middleware::{self, Next},
    response::{sse::Event, IntoResponse, Redirect, Response, Sse},
    routing::{delete, get, post, put},
    Json, Router,
};
use futures::StreamExt;
use serde_json::{json, Value};
use state::{AppState, ProviderStored};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::{ServeDir, ServeFile};

type St = State<Arc<AppState>>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let cfg = config::load(&cfg_path).unwrap_or_else(|e| {
        tracing::warn!("{e}; seeding empty config");
        config::Config::default()
    });
    let port = cfg.server.port;
    let seed = state::StoredState::from_config(cfg);
    let state_path =
        PathBuf::from(std::env::var("STATE_PATH").unwrap_or_else(|_| "state.json".to_string()));
    let app_state = Arc::new(AppState::load(state_path, seed));

    let v1 = Router::new()
        .route("/models", get(list_models))
        .route("/chat/completions", post(chat))
        .layer(middleware::from_fn_with_state(app_state.clone(), require_key_mw));

    let api = Router::new()
        .route("/combos", get(list_combos).post(create_combo))
        .route("/combos/{name}", delete(delete_combo))
        .route("/providers", get(list_providers).post(upsert_provider))
        .route("/providers/{id}", delete(delete_provider))
        .route("/providers/{id}/test", post(test_provider))
        .route("/providers/{id}/models/fetch", post(fetch_provider_models))
        .route("/keys", get(list_keys).post(create_key))
        .route("/keys/{id}", delete(delete_key))
        .route("/usage", get(get_usage))
        .route("/usage/summary", get(get_usage_summary))
        .route("/usage/logs", get(get_usage_logs))
        .route("/settings/require-key", put(set_require_key));

    // The Leptos dashboard (SPA) is served under /dashboard; unknown sub-paths
    // fall back to index.html so client-side routing works.
    let dist = "crates/frontend/dist";
    let dashboard = ServeDir::new(dist)
        .not_found_service(ServeFile::new(format!("{dist}/index.html")));

    let app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/", get(|| async { Redirect::permanent("/dashboard") }))
        .nest("/v1", v1)
        .nest("/api", api)
        .with_state(app_state)
        .nest_service("/dashboard", dashboard)
        .layer(CorsLayer::permissive());

    let addr = format!("0.0.0.0:{port}");
    tracing::info!(
        "M Y C backend v{} on http://{addr} — dashboard at http://localhost:{port}/dashboard",
        env!("CARGO_PKG_VERSION")
    );
    let listener = TcpListener::bind(&addr).await.expect("bind listener");
    axum::serve(listener, app).await.expect("serve");
}

// ---- auth middleware (only enforced when require_api_key is on) ----

async fn require_key_mw(State(st): St, req: Request, next: Next) -> Response {
    if st.require_api_key() {
        let ok = req
            .headers()
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .map(|k| st.key_valid(k.trim()))
            .unwrap_or(false);
        if !ok {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({ "error": { "message": "missing or invalid API key", "status": 401 } })),
            )
                .into_response();
        }
    }
    next.run(req).await
}

// ---- basic ----

async fn health() -> Json<Value> {
    Json(json!({ "ok": true }))
}
async fn version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_models(State(st): St) -> Json<Value> {
    let data: Vec<_> = st
        .orch()
        .list_models()
        .into_iter()
        .map(|m| json!({ "id": m.id, "object": "model", "name": m.name }))
        .collect();
    Json(json!({ "object": "list", "data": data }))
}

// ---- combos ----

async fn list_combos(State(st): St) -> Json<Value> {
    Json(json!({ "combos": st.snapshot().combos }))
}
async fn create_combo(State(st): St, Json(c): Json<Combo>) -> Json<Value> {
    st.upsert_combo(c);
    Json(json!({ "ok": true }))
}
async fn delete_combo(State(st): St, Path(name): Path<String>) -> Json<Value> {
    st.delete_combo(&name);
    Json(json!({ "ok": true }))
}

// ---- providers ----

async fn list_providers(State(st): St) -> Json<Value> {
    let provs: Vec<_> = st
        .snapshot()
        .providers
        .iter()
        .map(|p| {
            json!({
                "id": p.id,
                "base_url": p.base_url,
                "api_key_set": !p.api_key.is_empty(),
                "models": p.models.iter().map(|m| json!({
                    "id": m.id, "name": m.name.clone().unwrap_or_else(|| m.id.clone())
                })).collect::<Vec<_>>(),
            })
        })
        .collect();
    Json(json!({ "providers": provs }))
}
async fn upsert_provider(State(st): St, Json(p): Json<ProviderStored>) -> Json<Value> {
    st.upsert_provider(p);
    Json(json!({ "ok": true }))
}
async fn delete_provider(State(st): St, Path(id): Path<String>) -> Json<Value> {
    st.delete_provider(&id);
    Json(json!({ "ok": true }))
}
async fn test_provider(
    State(st): St,
    Path(id): Path<String>,
    body: Option<Json<Value>>,
) -> Json<Value> {
    let snap = st.snapshot();
    let Some(p) = snap.providers.iter().find(|x| x.id == id) else {
        return Json(json!({ "ok": false, "error": "provider not found" }));
    };
    let model = body
        .as_ref()
        .and_then(|b| b.0["model"].as_str().map(|s| s.to_string()))
        .or_else(|| p.models.first().map(|m| m.id.clone()));
    let Some(model) = model else {
        return Json(json!({ "ok": false, "error": "provider has no models" }));
    };
    let req = ChatRequest {
        model: format!("{}/{}", p.id, model),
        messages: vec![aggregator::ChatMessage { role: "user".into(), content: "ping".into() }],
        max_tokens: Some(1),
        ..Default::default()
    };
    let t0 = std::time::Instant::now();
    let result = st.orch().run_once(&req.model, &req).await;
    let latency_ms = t0.elapsed().as_millis() as u64;
    match result {
        Ok(r) => Json(json!({
            "ok": true, "target": r.target.map(|t| t.label()),
            "latency_ms": latency_ms, "model": model
        })),
        Err(e) => Json(json!({
            "ok": false, "error": e.to_string(), "status": e.status(),
            "latency_ms": latency_ms, "model": model
        })),
    }
}

/// Fetch the upstream model catalog (`GET {base_url}/models`) for a provider.
async fn fetch_provider_models(State(st): St, Path(id): Path<String>) -> Json<Value> {
    let snap = st.snapshot();
    let Some(p) = snap.providers.iter().find(|x| x.id == id) else {
        return Json(json!({ "ok": false, "error": "provider not found" }));
    };
    let url = format!("{}/models", p.base_url.trim_end_matches('/'));
    let client = reqwest::Client::new();
    let mut rb = client.get(&url);
    if !p.api_key.is_empty() {
        rb = rb.bearer_auth(&p.api_key);
    }
    match rb.send().await {
        Ok(resp) => {
            let status = resp.status();
            if !status.is_success() {
                let msg: String = resp.text().await.unwrap_or_default().chars().take(240).collect();
                return Json(json!({ "ok": false, "error": msg, "status": status.as_u16() }));
            }
            match resp.json::<Value>().await {
                Ok(v) => {
                    let models: Vec<String> = v["data"]
                        .as_array()
                        .cloned()
                        .unwrap_or_default()
                        .iter()
                        .filter_map(|m| m["id"].as_str().map(|s| s.to_string()))
                        .collect();
                    Json(json!({ "ok": true, "models": models }))
                }
                Err(e) => Json(json!({ "ok": false, "error": e.to_string() })),
            }
        }
        Err(e) => Json(json!({ "ok": false, "error": e.to_string() })),
    }
}

// ---- keys ----

async fn list_keys(State(st): St) -> Json<Value> {
    let s = st.snapshot();
    let keys: Vec<_> = s
        .api_keys
        .iter()
        .map(|k| json!({ "id": k.id, "name": k.name, "masked": state::mask(&k.key), "created_at": k.created_at }))
        .collect();
    Json(json!({ "keys": keys, "require_api_key": s.require_api_key }))
}
async fn create_key(State(st): St, Json(body): Json<Value>) -> Json<Value> {
    let name = body["name"].as_str().unwrap_or("default").to_string();
    let k = st.create_key(name);
    // Return the full key once (clients must store it now).
    Json(json!({ "id": k.id, "key": k.key, "name": k.name }))
}
async fn delete_key(State(st): St, Path(id): Path<String>) -> Json<Value> {
    st.delete_key(&id);
    Json(json!({ "ok": true }))
}
async fn set_require_key(State(st): St, Json(body): Json<Value>) -> Json<Value> {
    let v = body["require"].as_bool().unwrap_or(false);
    st.set_require_key(v);
    Json(json!({ "ok": true, "require_api_key": v }))
}

// ---- usage ----

fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn period_since(period: &str) -> u64 {
    let now = now_secs();
    match period {
        "24h" => now.saturating_sub(86_400),
        "7d" => now.saturating_sub(7 * 86_400),
        "30d" => now.saturating_sub(30 * 86_400),
        "60d" => now.saturating_sub(60 * 86_400),
        _ => now - (now % 86_400), // today (UTC midnight)
    }
}

async fn get_usage(State(st): St) -> Json<Value> {
    let s = st.snapshot();
    let mut rows: Vec<_> = s
        .usage
        .iter()
        .map(|(k, u)| {
            json!({
                "target": k, "requests": u.requests,
                "prompt_tokens": u.prompt_tokens, "completion_tokens": u.completion_tokens,
                "total_tokens": u.prompt_tokens + u.completion_tokens
            })
        })
        .collect();
    rows.sort_by(|a, b| b["requests"].as_u64().cmp(&a["requests"].as_u64()));
    Json(json!({ "usage": rows }))
}

async fn get_usage_summary(
    State(st): St,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let period = q.get("period").cloned().unwrap_or_else(|| "today".to_string());
    let since = period_since(&period);
    let s = st.snapshot();
    let mut by: std::collections::HashMap<String, (u64, u64, u64)> = std::collections::HashMap::new();
    let (mut reqs, mut pin, mut pout) = (0u64, 0u64, 0u64);
    for e in s.request_log.iter().filter(|e| e.ts >= since) {
        let row = by.entry(e.target.clone()).or_default();
        row.0 += 1;
        row.1 += e.prompt_tokens;
        row.2 += e.completion_tokens;
        reqs += 1;
        pin += e.prompt_tokens;
        pout += e.completion_tokens;
    }
    let mut by_target: Vec<Value> = by
        .into_iter()
        .map(|(t, (r, p, c))| {
            json!({ "target": t, "requests": r, "prompt_tokens": p, "completion_tokens": c, "total_tokens": p + c })
        })
        .collect();
    by_target.sort_by(|a, b| b["requests"].as_u64().cmp(&a["requests"].as_u64()));
    Json(json!({
        "period": period, "since": since,
        "totals": { "requests": reqs, "prompt_tokens": pin, "completion_tokens": pout, "total_tokens": pin + pout },
        "by_target": by_target
    }))
}

async fn get_usage_logs(
    State(st): St,
    Query(q): Query<std::collections::HashMap<String, String>>,
) -> Json<Value> {
    let limit: usize = q.get("limit").and_then(|s| s.parse().ok()).unwrap_or(200);
    let s = st.snapshot();
    let logs: Vec<Value> = s
        .request_log
        .iter()
        .rev()
        .take(limit)
        .map(|e| {
            json!({
                "ts": e.ts, "target": e.target,
                "prompt_tokens": e.prompt_tokens, "completion_tokens": e.completion_tokens,
                "total_tokens": e.prompt_tokens + e.completion_tokens,
                "status": e.status, "stream": e.stream
            })
        })
        .collect();
    Json(json!({ "logs": logs }))
}

// ---- chat ----

async fn chat(State(st): St, Json(req): Json<ChatRequest>) -> Response {
    let orch = st.orch();
    if req.stream {
        match orch.run_stream(&req.model, &req).await {
            Ok((stream, target)) => {
                let label = target.map(|t| t.label()).unwrap_or_else(|| req.model.clone());
                st.record_request(&label, 0, 0, 200, true); // count the request; tokens tracked on non-stream
                let event_stream = async_stream::stream! {
                    futures::pin_mut!(stream);
                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(ChatChunk::Delta(text)) => {
                                let chunk = json!({
                                    "object": "chat.completion.chunk", "model": label,
                                    "choices": [{ "index": 0, "delta": { "content": text } }],
                                    "x_myc_target": label,
                                });
                                yield Ok::<Event, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                            }
                            Ok(ChatChunk::Done(usage)) => {
                                let chunk = json!({
                                    "object": "chat.completion.chunk", "model": label,
                                    "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
                                    "usage": usage,
                                });
                                yield Ok(Event::default().data(chunk.to_string()));
                            }
                            Err(e) => {
                                yield Ok(Event::default().data(json!({ "error": { "message": e.to_string(), "status": e.status() } }).to_string()));
                            }
                        }
                    }
                    yield Ok(Event::default().data("[DONE]"));
                };
                Sse::new(event_stream).into_response()
            }
            Err(e) => error_response(e),
        }
    } else {
        match orch.run_once(&req.model, &req).await {
            Ok(resp) => {
                let label = resp
                    .target
                    .as_ref()
                    .map(|t| t.label())
                    .unwrap_or_else(|| req.model.clone());
                st.record_usage(
                    &label,
                    resp.usage.prompt_tokens as u64,
                    resp.usage.completion_tokens as u64,
                );
                Json(json!({
                    "object": "chat.completion", "model": label,
                    "choices": [{
                        "index": 0,
                        "message": { "role": "assistant", "content": resp.content },
                        "finish_reason": resp.finish_reason.unwrap_or_else(|| "stop".to_string()),
                    }],
                    "usage": resp.usage,
                    "x_myc_target": label, "x_myc_fused": resp.fused,
                }))
                .into_response()
            }
            Err(e) => error_response(e),
        }
    }
}

fn error_response(e: ProviderError) -> Response {
    let status = StatusCode::from_u16(e.status()).unwrap_or(StatusCode::BAD_GATEWAY);
    (
        status,
        Json(json!({ "error": { "message": e.to_string(), "status": e.status() } })),
    )
        .into_response()
}
