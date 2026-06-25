//! The agentic orchestrator (Aggregator). Resolves a model name to either a
//! single target or a combo, then executes the combo's strategy:
//! **fallback**, **round-robin**, or **fusion** (fan-out panel + judge synthesis).
//! Ports the semantics of the legacy `open-sse/services/combo.js`.

use crate::provider::Provider;
use crate::types::{
    ChatChunk, ChatMessage, ChatRequest, ChatResponse, ChatStream, Combo, FusionConfig, ModelInfo,
    ProviderError, Strategy, Target,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default)]
struct RotationCursor {
    index: usize,
    used: u32,
}

enum Resolution {
    Single(Target),
    Combo(Combo),
}

/// Holds the provider registry + combo definitions and drives orchestration.
pub struct Orchestrator {
    providers: HashMap<String, Arc<dyn Provider>>,
    combos: HashMap<String, Combo>,
    rotation: Mutex<HashMap<String, RotationCursor>>,
    default_provider: String,
}

impl Orchestrator {
    pub fn new(providers: Vec<Arc<dyn Provider>>, combos: Vec<Combo>) -> Self {
        let default_provider = providers
            .first()
            .map(|p| p.id().to_string())
            .unwrap_or_default();
        let providers = providers
            .into_iter()
            .map(|p| (p.id().to_string(), p))
            .collect();
        let combos = combos.into_iter().map(|c| (c.name.clone(), c)).collect();
        Self {
            providers,
            combos,
            rotation: Mutex::new(HashMap::new()),
            default_provider,
        }
    }

    /// All provider models (prefixed `provider/model`) plus combo names.
    pub fn list_models(&self) -> Vec<ModelInfo> {
        let mut out = Vec::new();
        for p in self.providers.values() {
            for m in p.models() {
                out.push(ModelInfo {
                    id: format!("{}/{}", p.id(), m.id),
                    name: m.name,
                });
            }
        }
        for c in self.combos.values() {
            out.push(ModelInfo {
                id: c.name.clone(),
                name: format!("combo ({})", strategy_label(&c.strategy)),
            });
        }
        out
    }

    pub fn combos(&self) -> Vec<Combo> {
        self.combos.values().cloned().collect()
    }

    fn provider(&self, id: &str) -> Result<&Arc<dyn Provider>, ProviderError> {
        self.providers
            .get(id)
            .ok_or_else(|| ProviderError::NotFound(id.to_string()))
    }

    fn resolve(&self, model: &str) -> Resolution {
        if let Some(c) = self.combos.get(model) {
            return Resolution::Combo(c.clone());
        }
        let (provider, m) = match model.split_once('/') {
            Some((p, m)) => (p.to_string(), m.to_string()),
            None => (self.default_provider.clone(), model.to_string()),
        };
        Resolution::Single(Target { provider, model: m })
    }

    // ---- non-streaming entry point -------------------------------------

    pub async fn run_once(
        &self,
        model: &str,
        req: &ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        match self.resolve(model) {
            Resolution::Single(t) => self.chat_once_target(&t, req).await,
            Resolution::Combo(c) => match &c.strategy {
                Strategy::Fallback => self.fallback_once(&c.targets, req).await,
                Strategy::RoundRobin { sticky } => {
                    let order = self.rotate(&c, *sticky);
                    self.fallback_once(&order, req).await
                }
                Strategy::Fusion(cfg) => self.fusion(&c, cfg, req).await,
            },
        }
    }

    // ---- streaming entry point -----------------------------------------
    // Returns the stream plus the target that produced it.

    pub async fn run_stream(
        &self,
        model: &str,
        req: &ChatRequest,
    ) -> Result<(ChatStream, Option<Target>), ProviderError> {
        match self.resolve(model) {
            Resolution::Single(t) => {
                let s = self.stream_target(&t, req).await?;
                Ok((s, Some(t)))
            }
            Resolution::Combo(c) => match &c.strategy {
                Strategy::Fallback => self.fallback_stream(&c.targets, req).await,
                Strategy::RoundRobin { sticky } => {
                    let order = self.rotate(&c, *sticky);
                    self.fallback_stream(&order, req).await
                }
                Strategy::Fusion(cfg) => {
                    // Fusion is inherently non-streaming (fan-out + judge); emit the
                    // synthesized answer as a single delta + done.
                    let resp = self.fusion(&c, cfg, req).await?;
                    let target = resp.target.clone();
                    let chunks = vec![
                        Ok(ChatChunk::Delta(resp.content)),
                        Ok(ChatChunk::Done(resp.usage)),
                    ];
                    Ok((Box::pin(futures::stream::iter(chunks)), target))
                }
            },
        }
    }

    // ---- per-target helpers --------------------------------------------

    async fn chat_once_target(
        &self,
        t: &Target,
        req: &ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        let p = self.provider(&t.provider)?;
        let mut r = req.clone();
        r.model = t.model.clone();
        let mut resp = p.chat_once(&r).await?;
        resp.target = Some(t.clone());
        Ok(resp)
    }

    async fn stream_target(
        &self,
        t: &Target,
        req: &ChatRequest,
    ) -> Result<ChatStream, ProviderError> {
        let p = self.provider(&t.provider)?;
        let mut r = req.clone();
        r.model = t.model.clone();
        p.chat_stream(&r).await
    }

    // ---- strategy: fallback --------------------------------------------

    async fn fallback_once(
        &self,
        targets: &[Target],
        req: &ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        let mut last: Option<ProviderError> = None;
        for t in targets {
            match self.chat_once_target(t, req).await {
                Ok(r) => return Ok(r),
                Err(e) => {
                    let retry = e.is_retryable();
                    last = Some(e);
                    if !retry {
                        break;
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| ProviderError::Orchestration("no targets".to_string())))
    }

    async fn fallback_stream(
        &self,
        targets: &[Target],
        req: &ChatRequest,
    ) -> Result<(ChatStream, Option<Target>), ProviderError> {
        let mut last: Option<ProviderError> = None;
        for t in targets {
            match self.stream_target(t, req).await {
                Ok(s) => return Ok((s, Some(t.clone()))),
                Err(e) => {
                    let retry = e.is_retryable();
                    last = Some(e);
                    if !retry {
                        break;
                    }
                }
            }
        }
        Err(last.unwrap_or_else(|| ProviderError::Orchestration("no targets".to_string())))
    }

    // ---- strategy: round-robin -----------------------------------------

    fn rotate(&self, combo: &Combo, sticky: u32) -> Vec<Target> {
        let n = combo.targets.len();
        if n == 0 {
            return Vec::new();
        }
        let mut state = self.rotation.lock().unwrap();
        let cur = state.entry(combo.name.clone()).or_default();
        let start = cur.index % n;
        cur.used += 1;
        if cur.used >= sticky.max(1) {
            cur.used = 0;
            cur.index = (cur.index + 1) % n;
        }
        (0..n)
            .map(|i| combo.targets[(start + i) % n].clone())
            .collect()
    }

    // ---- strategy: fusion ----------------------------------------------

    async fn fusion(
        &self,
        combo: &Combo,
        cfg: &FusionConfig,
        req: &ChatRequest,
    ) -> Result<ChatResponse, ProviderError> {
        use futures::stream::{FuturesUnordered, StreamExt};
        use tokio::time::{timeout, Duration, Instant};

        let mut futs = FuturesUnordered::new();
        for t in &combo.targets {
            let t = t.clone();
            futs.push(async move { self.chat_once_target(&t, req).await });
        }

        let hard = Duration::from_millis(cfg.hard_timeout_ms);
        let grace = Duration::from_millis(cfg.grace_ms);
        let start = Instant::now();
        let mut answers: Vec<ChatResponse> = Vec::new();

        loop {
            let remaining = hard.checked_sub(start.elapsed()).unwrap_or(Duration::ZERO);
            if remaining.is_zero() {
                break;
            }
            // Once quorum is reached, only wait the grace window for stragglers.
            let wait = if answers.len() >= cfg.min_panel {
                grace.min(remaining)
            } else {
                remaining
            };
            match timeout(wait, futs.next()).await {
                Ok(Some(Ok(r))) => answers.push(r),
                Ok(Some(Err(_))) => {} // skip a failed panelist
                Ok(None) => break,     // all panelists resolved
                Err(_) => break,       // grace / hard window elapsed
            }
        }

        if answers.is_empty() {
            return Err(ProviderError::Orchestration(
                "fusion: no panel answers".to_string(),
            ));
        }
        if answers.len() == 1 {
            return Ok(answers.remove(0));
        }

        let judge_target = cfg
            .judge
            .clone()
            .unwrap_or_else(|| combo.targets[0].clone());
        let judge_req = build_judge_request(req, &answers);
        let mut resp = self.chat_once_target(&judge_target, &judge_req).await?;
        resp.fused = true;
        Ok(resp)
    }
}

fn strategy_label(s: &Strategy) -> &'static str {
    match s {
        Strategy::Fallback => "fallback",
        Strategy::RoundRobin { .. } => "round-robin",
        Strategy::Fusion(_) => "fusion",
    }
}

/// Build the anonymized judge prompt from collected panel answers.
fn build_judge_request(req: &ChatRequest, answers: &[ChatResponse]) -> ChatRequest {
    let mut prompt = String::from(
        "You are an expert judge. Below are candidate answers from multiple AI models \
         to the same user query. Synthesize the single best, correct and complete answer. \
         Do not mention the sources.\n\n",
    );
    for (i, a) in answers.iter().enumerate() {
        prompt.push_str(&format!("Source {}:\n{}\n\n", i + 1, a.content));
    }
    prompt.push_str("Original user message:\n");
    prompt.push_str(req.last_user_message());

    ChatRequest {
        model: String::new(), // overwritten by chat_once_target
        messages: vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
        }],
        stream: false,
        max_tokens: req.max_tokens,
        temperature: req.temperature,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;

    fn req() -> ChatRequest {
        ChatRequest {
            model: "x".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: "hi".to_string(),
            }],
            ..Default::default()
        }
    }

    fn target(p: &str) -> Target {
        Target {
            provider: p.to_string(),
            model: "m".to_string(),
        }
    }

    #[tokio::test]
    async fn fallback_advances_past_retryable_failure() {
        let orch = Orchestrator::new(
            vec![
                Arc::new(MockProvider::fail("a", 503)),
                Arc::new(MockProvider::ok("b", "B-ok")),
            ],
            vec![Combo {
                name: "c".to_string(),
                targets: vec![target("a"), target("b")],
                strategy: Strategy::Fallback,
            }],
        );
        let r = orch.run_once("c", &req()).await.unwrap();
        assert_eq!(r.content, "B-ok");
        assert_eq!(r.target.unwrap().provider, "b");
    }

    #[tokio::test]
    async fn fallback_stops_on_client_error() {
        let orch = Orchestrator::new(
            vec![
                Arc::new(MockProvider::fail("a", 400)),
                Arc::new(MockProvider::ok("b", "B-ok")),
            ],
            vec![Combo {
                name: "c".to_string(),
                targets: vec![target("a"), target("b")],
                strategy: Strategy::Fallback,
            }],
        );
        let e = orch.run_once("c", &req()).await.unwrap_err();
        assert_eq!(e.status(), 400); // 400 is not retryable → did not fall through
    }

    #[tokio::test]
    async fn round_robin_rotates_each_call() {
        let orch = Orchestrator::new(
            vec![
                Arc::new(MockProvider::ok("a", "A")),
                Arc::new(MockProvider::ok("b", "B")),
            ],
            vec![Combo {
                name: "rr".to_string(),
                targets: vec![target("a"), target("b")],
                strategy: Strategy::RoundRobin { sticky: 1 },
            }],
        );
        let r1 = orch.run_once("rr", &req()).await.unwrap();
        let r2 = orch.run_once("rr", &req()).await.unwrap();
        let r3 = orch.run_once("rr", &req()).await.unwrap();
        assert_eq!(r1.content, "A");
        assert_eq!(r2.content, "B");
        assert_eq!(r3.content, "A"); // wrapped around
    }

    #[tokio::test]
    async fn round_robin_respects_sticky() {
        let orch = Orchestrator::new(
            vec![
                Arc::new(MockProvider::ok("a", "A")),
                Arc::new(MockProvider::ok("b", "B")),
            ],
            vec![Combo {
                name: "rr".to_string(),
                targets: vec![target("a"), target("b")],
                strategy: Strategy::RoundRobin { sticky: 2 },
            }],
        );
        let mut seq = Vec::new();
        for _ in 0..4 {
            seq.push(orch.run_once("rr", &req()).await.unwrap().content);
        }
        // sticky=2 → each target is used twice before advancing.
        assert_eq!(seq, vec!["A", "A", "B", "B"]);
    }

    #[tokio::test]
    async fn fusion_synthesizes_with_judge() {
        let orch = Orchestrator::new(
            vec![
                Arc::new(MockProvider::ok("a", "ans-a")),
                Arc::new(MockProvider::ok("b", "ans-b")),
                Arc::new(MockProvider::ok("judge", "SYNTHESIZED")),
            ],
            vec![Combo {
                name: "fz".to_string(),
                targets: vec![target("a"), target("b")],
                strategy: Strategy::Fusion(FusionConfig {
                    min_panel: 2,
                    grace_ms: 10,
                    hard_timeout_ms: 1000,
                    judge: Some(target("judge")),
                }),
            }],
        );
        let r = orch.run_once("fz", &req()).await.unwrap();
        assert_eq!(r.content, "SYNTHESIZED");
        assert!(r.fused);
    }

    #[tokio::test]
    async fn fusion_single_answer_skips_judge() {
        let orch = Orchestrator::new(
            vec![
                Arc::new(MockProvider::ok("a", "only-answer")),
                Arc::new(MockProvider::fail("b", 503)),
            ],
            vec![Combo {
                name: "fz".to_string(),
                targets: vec![target("a"), target("b")],
                strategy: Strategy::Fusion(FusionConfig {
                    min_panel: 2,
                    grace_ms: 10,
                    hard_timeout_ms: 500,
                    judge: None,
                }),
            }],
        );
        let r = orch.run_once("fz", &req()).await.unwrap();
        assert_eq!(r.content, "only-answer");
        assert!(!r.fused);
    }
}
