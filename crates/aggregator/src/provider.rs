//! The provider abstraction — models the legacy `BaseExecutor` interface.

use crate::types::{ChatRequest, ChatResponse, ChatStream, ModelInfo, ProviderError};
use async_trait::async_trait;

/// A backend that can answer chat requests. Each instance is pre-configured with
/// its endpoint + credentials, so callers only pass the request.
#[async_trait]
pub trait Provider: Send + Sync {
    /// Stable provider id (matches `Target.provider`).
    fn id(&self) -> &str;

    /// Models this provider exposes (for `/v1/models`).
    fn models(&self) -> Vec<ModelInfo>;

    /// Non-streaming completion (used by fallback collection + fusion fan-out).
    async fn chat_once(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError>;

    /// Streaming completion (used to stream the winning target to the client).
    async fn chat_stream(&self, req: &ChatRequest) -> Result<ChatStream, ProviderError>;
}
