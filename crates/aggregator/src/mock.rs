//! A deterministic in-memory provider for unit tests.

use crate::provider::Provider;
use crate::types::{
    ChatChunk, ChatRequest, ChatResponse, ChatStream, ModelInfo, ProviderError, Usage,
};
use async_trait::async_trait;

#[derive(Clone)]
enum Behavior {
    Ok(String),
    Fail(u16),
}

/// Mock provider that always returns the same answer (or the same error).
pub struct MockProvider {
    id: String,
    behavior: Behavior,
}

impl MockProvider {
    pub fn ok(id: &str, content: &str) -> Self {
        Self {
            id: id.to_string(),
            behavior: Behavior::Ok(content.to_string()),
        }
    }

    pub fn fail(id: &str, status: u16) -> Self {
        Self {
            id: id.to_string(),
            behavior: Behavior::Fail(status),
        }
    }
}

#[async_trait]
impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "m".to_string(),
            name: format!("{} model", self.id),
        }]
    }

    async fn chat_once(&self, _req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        match &self.behavior {
            Behavior::Ok(c) => Ok(ChatResponse {
                content: c.clone(),
                finish_reason: Some("stop".to_string()),
                ..Default::default()
            }),
            Behavior::Fail(s) => Err(ProviderError::Http {
                status: *s,
                message: "mock failure".to_string(),
            }),
        }
    }

    async fn chat_stream(&self, _req: &ChatRequest) -> Result<ChatStream, ProviderError> {
        match &self.behavior {
            Behavior::Ok(c) => {
                let chunks = vec![
                    Ok(ChatChunk::Delta(c.clone())),
                    Ok(ChatChunk::Done(Usage::default())),
                ];
                Ok(Box::pin(futures::stream::iter(chunks)))
            }
            Behavior::Fail(s) => Err(ProviderError::Http {
                status: *s,
                message: "mock failure".to_string(),
            }),
        }
    }
}
