//! 9Router backend — Axum HTTP server hosting the agentic orchestrator.

mod config;
mod provider_openai;

use aggregator::{ChatChunk, ChatRequest, Orchestrator, ProviderError};
use axum::{
    extract::State,
    http::StatusCode,
    response::{sse::Event, IntoResponse, Response, Sse},
    routing::{get, post},
    Json, Router,
};
use futures::StreamExt;
use std::sync::Arc;
use tokio::net::TcpListener;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

type SharedOrch = Arc<Orchestrator>;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into()),
        )
        .init();

    let cfg_path = std::env::var("CONFIG_PATH").unwrap_or_else(|_| "config.toml".to_string());
    let cfg = match config::load(&cfg_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("{e}; starting with empty config");
            config::Config {
                server: Default::default(),
                providers: Vec::new(),
                combos: Vec::new(),
            }
        }
    };
    let port = cfg.server.port;
    let orch: SharedOrch = Arc::new(config::build_orchestrator(cfg));

    let app = Router::new()
        .route("/health", get(health))
        .route("/version", get(version))
        .route("/v1/models", get(list_models))
        .route("/v1/chat/completions", post(chat))
        .with_state(orch)
        .layer(CorsLayer::permissive())
        .fallback_service(ServeDir::new("crates/frontend/dist"));

    let addr = format!("0.0.0.0:{port}");
    tracing::info!(
        "9Router backend v{} listening on http://{addr}",
        env!("CARGO_PKG_VERSION")
    );
    let listener = TcpListener::bind(&addr).await.expect("bind listener");
    axum::serve(listener, app).await.expect("serve");
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "ok": true }))
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_models(State(orch): State<SharedOrch>) -> Json<serde_json::Value> {
    let data: Vec<_> = orch
        .list_models()
        .into_iter()
        .map(|m| serde_json::json!({ "id": m.id, "object": "model", "name": m.name }))
        .collect();
    Json(serde_json::json!({ "object": "list", "data": data }))
}

async fn chat(State(orch): State<SharedOrch>, Json(req): Json<ChatRequest>) -> Response {
    if req.stream {
        match orch.run_stream(&req.model, &req).await {
            Ok((stream, target)) => {
                let label = target.map(|t| t.label()).unwrap_or_else(|| req.model.clone());
                let event_stream = async_stream::stream! {
                    futures::pin_mut!(stream);
                    while let Some(item) = stream.next().await {
                        match item {
                            Ok(ChatChunk::Delta(text)) => {
                                let chunk = serde_json::json!({
                                    "object": "chat.completion.chunk",
                                    "model": label,
                                    "choices": [{ "index": 0, "delta": { "content": text } }],
                                    "x_9router_target": label,
                                });
                                yield Ok::<Event, std::convert::Infallible>(Event::default().data(chunk.to_string()));
                            }
                            Ok(ChatChunk::Done(usage)) => {
                                let chunk = serde_json::json!({
                                    "object": "chat.completion.chunk",
                                    "model": label,
                                    "choices": [{ "index": 0, "delta": {}, "finish_reason": "stop" }],
                                    "usage": usage,
                                });
                                yield Ok(Event::default().data(chunk.to_string()));
                            }
                            Err(e) => {
                                let chunk = serde_json::json!({
                                    "error": { "message": e.to_string(), "status": e.status() }
                                });
                                yield Ok(Event::default().data(chunk.to_string()));
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
                Json(serde_json::json!({
                    "object": "chat.completion",
                    "model": label,
                    "choices": [{
                        "index": 0,
                        "message": { "role": "assistant", "content": resp.content },
                        "finish_reason": resp.finish_reason.unwrap_or_else(|| "stop".to_string()),
                    }],
                    "usage": resp.usage,
                    "x_9router_target": label,
                    "x_9router_fused": resp.fused,
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
        Json(serde_json::json!({ "error": { "message": e.to_string(), "status": e.status() } })),
    )
        .into_response()
}
