//! Core data types shared across the orchestrator. The slice keeps everything in
//! OpenAI chat shape; cross-format translation is a later module.

use serde::{Deserialize, Serialize};
use std::pin::Pin;

/// A single chat message (OpenAI shape).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

/// An incoming chat request.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    #[serde(default)]
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
}

impl ChatRequest {
    /// Text of the latest user message (used to build the fusion judge prompt).
    pub fn last_user_message(&self) -> &str {
        self.messages
            .iter()
            .rev()
            .find(|m| m.role == "user")
            .map(|m| m.content.as_str())
            .unwrap_or("")
    }
}

/// Token accounting.
#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct Usage {
    #[serde(default)]
    pub prompt_tokens: u32,
    #[serde(default)]
    pub completion_tokens: u32,
    #[serde(default)]
    pub total_tokens: u32,
}

/// A non-streaming chat result.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct ChatResponse {
    pub content: String,
    #[serde(default)]
    pub usage: Usage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    /// Which target produced this answer (filled in by the orchestrator).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<Target>,
    /// True when produced by the fusion judge (aggregated from a panel).
    #[serde(default)]
    pub fused: bool,
}

/// A streamed chunk.
#[derive(Clone, Debug)]
pub enum ChatChunk {
    /// Incremental text delta.
    Delta(String),
    /// Terminal chunk carrying final usage.
    Done(Usage),
}

/// A boxed stream of chunks.
pub type ChatStream = Pin<Box<dyn futures::Stream<Item = Result<ChatChunk, ProviderError>> + Send>>;

/// Model metadata for listings.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

/// A concrete provider+model destination.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct Target {
    pub provider: String,
    pub model: String,
}

impl Target {
    pub fn label(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

fn default_sticky() -> u32 {
    1
}
fn default_min_panel() -> usize {
    2
}
fn default_grace_ms() -> u64 {
    8_000
}
fn default_hard_ms() -> u64 {
    90_000
}

/// Fusion tuning (mirrors the legacy combo.js quorum-grace logic).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FusionConfig {
    #[serde(default = "default_min_panel")]
    pub min_panel: usize,
    #[serde(default = "default_grace_ms")]
    pub grace_ms: u64,
    #[serde(default = "default_hard_ms")]
    pub hard_timeout_ms: u64,
    #[serde(default)]
    pub judge: Option<Target>,
}

impl Default for FusionConfig {
    fn default() -> Self {
        Self {
            min_panel: default_min_panel(),
            grace_ms: default_grace_ms(),
            hard_timeout_ms: default_hard_ms(),
            judge: None,
        }
    }
}

/// Orchestration strategy for a combo.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Strategy {
    /// Try targets in order; advance on a retryable error.
    Fallback,
    /// Rotate the starting target across calls, repeating each `sticky` times.
    RoundRobin {
        #[serde(default = "default_sticky")]
        sticky: u32,
    },
    /// Fan out to the panel, collect a quorum, then synthesize with a judge.
    Fusion(FusionConfig),
}

/// A named multi-target combo.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Combo {
    pub name: String,
    pub targets: Vec<Target>,
    pub strategy: Strategy,
}

/// Errors a provider may return.
#[derive(Debug, Clone, thiserror::Error)]
pub enum ProviderError {
    #[error("provider '{0}' not configured")]
    NotFound(String),
    #[error("upstream HTTP {status}: {message}")]
    Http { status: u16, message: String },
    #[error("network error: {0}")]
    Network(String),
    #[error("request timed out")]
    Timeout,
    #[error("orchestrator: {0}")]
    Orchestration(String),
}

impl ProviderError {
    /// Whether the orchestrator should advance to the next target on this error.
    /// Client errors (400/404/422) won't be fixed by trying another provider; auth,
    /// rate-limit, overload, network and timeout errors should fall through.
    pub fn is_retryable(&self) -> bool {
        match self {
            ProviderError::Http { status, .. } => {
                !matches!(status, 400 | 404 | 422)
            }
            ProviderError::Network(_) | ProviderError::Timeout | ProviderError::NotFound(_) => true,
            ProviderError::Orchestration(_) => false,
        }
    }

    pub fn status(&self) -> u16 {
        match self {
            ProviderError::Http { status, .. } => *status,
            ProviderError::NotFound(_) => 404,
            ProviderError::Timeout => 504,
            ProviderError::Network(_) => 502,
            ProviderError::Orchestration(_) => 500,
        }
    }
}
