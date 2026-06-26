//! Concrete provider: any OpenAI-compatible `/chat/completions` endpoint.
//! One client type covers OpenAI, GLM, OpenRouter, Groq, etc.

use crate::state::{now_ts, QuotaMap, QuotaSnapshot};
use myc_core::{
    ChatChunk, ChatRequest, ChatResponse, ChatStream, ModelInfo, Provider, ProviderError, Usage,
};
use async_trait::async_trait;
use futures::StreamExt;

pub struct OpenAiCompatibleProvider {
    id: String,
    base_url: String,
    api_key: String,
    models: Vec<ModelInfo>,
    quota: QuotaMap,
    client: reqwest::Client,
}

impl OpenAiCompatibleProvider {
    pub fn new(
        id: String,
        base_url: String,
        api_key: String,
        models: Vec<ModelInfo>,
        quota: QuotaMap,
    ) -> Self {
        Self {
            id,
            base_url,
            api_key,
            models,
            quota,
            client: reqwest::Client::new(),
        }
    }

    /// Capture rate-limit headers (OpenAI-style) into the shared quota map.
    fn capture_quota(&self, headers: &reqwest::header::HeaderMap, status: u16) {
        let g = |k: &str| {
            headers
                .get(k)
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string())
        };
        let gu = |k: &str| g(k).and_then(|s| s.parse::<u64>().ok());
        let snap = QuotaSnapshot {
            limit_requests: gu("x-ratelimit-limit-requests"),
            remaining_requests: gu("x-ratelimit-remaining-requests"),
            limit_tokens: gu("x-ratelimit-limit-tokens"),
            remaining_tokens: gu("x-ratelimit-remaining-tokens"),
            reset: g("x-ratelimit-reset-requests").or_else(|| g("x-ratelimit-reset-tokens")),
            retry_after: g("retry-after"),
            last_status: status,
            updated: now_ts(),
        };
        if let Ok(mut m) = self.quota.lock() {
            m.insert(self.id.clone(), snap);
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/chat/completions", self.base_url.trim_end_matches('/'))
    }

    fn body(&self, req: &ChatRequest, stream: bool) -> serde_json::Value {
        let mut b = serde_json::json!({
            "model": req.model,
            "messages": req.messages,
            "stream": stream,
        });
        if let Some(m) = req.max_tokens {
            b["max_tokens"] = m.into();
        }
        if let Some(t) = req.temperature {
            b["temperature"] = t.into();
        }
        b
    }
}

fn truncate(mut s: String) -> String {
    s.truncate(240);
    s
}

#[async_trait]
impl Provider for OpenAiCompatibleProvider {
    fn id(&self) -> &str {
        &self.id
    }

    fn models(&self) -> Vec<ModelInfo> {
        self.models.clone()
    }

    async fn chat_once(&self, req: &ChatRequest) -> Result<ChatResponse, ProviderError> {
        let resp = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&self.body(req, false))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        self.capture_quota(resp.headers(), status.as_u16());
        if !status.is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http {
                status: status.as_u16(),
                message: truncate(msg),
            });
        }

        let v: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let content = v["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        let finish_reason = v["choices"][0]["finish_reason"]
            .as_str()
            .map(|s| s.to_string());
        let usage = Usage {
            prompt_tokens: v["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: v["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: v["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
        };

        Ok(ChatResponse {
            content,
            usage,
            finish_reason,
            target: None,
            fused: false,
        })
    }

    async fn chat_stream(&self, req: &ChatRequest) -> Result<ChatStream, ProviderError> {
        let resp = self
            .client
            .post(self.endpoint())
            .bearer_auth(&self.api_key)
            .json(&self.body(req, true))
            .send()
            .await
            .map_err(|e| ProviderError::Network(e.to_string()))?;

        let status = resp.status();
        self.capture_quota(resp.headers(), status.as_u16());
        if !status.is_success() {
            let msg = resp.text().await.unwrap_or_default();
            return Err(ProviderError::Http {
                status: status.as_u16(),
                message: truncate(msg),
            });
        }

        let byte_stream = resp.bytes_stream();
        let s = async_stream::stream! {
            futures::pin_mut!(byte_stream);
            let mut buf = String::new();
            let mut usage = Usage::default();
            while let Some(chunk) = byte_stream.next().await {
                let bytes = match chunk {
                    Ok(b) => b,
                    Err(e) => { yield Err(ProviderError::Network(e.to_string())); return; }
                };
                buf.push_str(&String::from_utf8_lossy(&bytes));
                while let Some(pos) = buf.find('\n') {
                    let line: String = buf.drain(..=pos).collect();
                    let line = line.trim();
                    let data = match line.strip_prefix("data:") {
                        Some(d) => d.trim(),
                        None => continue,
                    };
                    if data == "[DONE]" {
                        yield Ok(ChatChunk::Done(usage.clone()));
                        return;
                    }
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                        if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                            if !delta.is_empty() {
                                yield Ok(ChatChunk::Delta(delta.to_string()));
                            }
                        }
                        if let Some(u) = v.get("usage").filter(|u| !u.is_null()) {
                            usage = Usage {
                                prompt_tokens: u["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                                completion_tokens: u["completion_tokens"].as_u64().unwrap_or(0) as u32,
                                total_tokens: u["total_tokens"].as_u64().unwrap_or(0) as u32,
                            };
                        }
                    }
                }
            }
            yield Ok(ChatChunk::Done(usage));
        };

        Ok(Box::pin(s))
    }
}
