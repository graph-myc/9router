//! Mutable application state, persisted to a JSON file. Holds providers, combos,
//! API keys, the require-key flag, and usage counters. Rebuilds the orchestrator
//! whenever providers/combos change.

use crate::provider_openai::OpenAiCompatibleProvider;
use myc_core::{Combo, ModelInfo, Orchestrator, Provider};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Serialize, Deserialize)]
pub struct ModelStored {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ProviderStored {
    pub id: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub models: Vec<ModelStored>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: String,
    pub key: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub created_at: u64,
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct UsageRow {
    pub requests: u64,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

/// One recorded request, kept in a capped ring for time-windowed analytics.
#[derive(Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub ts: u64,
    pub target: String,
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    #[serde(default)]
    pub status: u16,
    #[serde(default)]
    pub stream: bool,
}

const MAX_LOG: usize = 5000;

/// Latest rate-limit signals captured from a provider's HTTP responses. Runtime
/// only (not persisted).
#[derive(Clone, Default, Serialize)]
pub struct QuotaSnapshot {
    pub limit_requests: Option<u64>,
    pub remaining_requests: Option<u64>,
    pub limit_tokens: Option<u64>,
    pub remaining_tokens: Option<u64>,
    pub reset: Option<String>,
    pub retry_after: Option<String>,
    pub last_status: u16,
    pub updated: u64,
}

pub type QuotaMap = Arc<Mutex<HashMap<String, QuotaSnapshot>>>;

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct StoredState {
    #[serde(default)]
    pub providers: Vec<ProviderStored>,
    #[serde(default)]
    pub combos: Vec<Combo>,
    #[serde(default)]
    pub api_keys: Vec<ApiKey>,
    #[serde(default)]
    pub require_api_key: bool,
    #[serde(default)]
    pub usage: HashMap<String, UsageRow>,
    #[serde(default)]
    pub request_log: Vec<LogEntry>,
}

impl StoredState {
    /// Seed from the TOML config (first run only).
    pub fn from_config(cfg: crate::config::Config) -> Self {
        let providers = cfg
            .providers
            .into_iter()
            .map(|p| {
                let api_key = p
                    .api_key
                    .clone()
                    .or_else(|| p.api_key_env.as_ref().and_then(|e| std::env::var(e).ok()))
                    .unwrap_or_default();
                ProviderStored {
                    id: p.id,
                    base_url: p.base_url,
                    api_key,
                    models: p
                        .models
                        .into_iter()
                        .map(|m| ModelStored { id: m.id, name: m.name })
                        .collect(),
                }
            })
            .collect();
        let combos = cfg.combos.into_iter().map(|c| c.into_combo()).collect();
        StoredState {
            providers,
            combos,
            api_keys: Vec::new(),
            require_api_key: false,
            usage: HashMap::new(),
            request_log: Vec::new(),
        }
    }
}

struct Inner {
    stored: StoredState,
    orch: Arc<Orchestrator>,
}

pub struct AppState {
    path: PathBuf,
    inner: RwLock<Inner>,
    quota: QuotaMap,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Public epoch-seconds helper (used by the provider client for quota stamps).
pub fn now_ts() -> u64 {
    now()
}

fn build_orch(stored: &StoredState, quota: &QuotaMap) -> Arc<Orchestrator> {
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();
    for p in &stored.providers {
        let models = p
            .models
            .iter()
            .map(|m| ModelInfo {
                id: m.id.clone(),
                name: m.name.clone().unwrap_or_else(|| m.id.clone()),
            })
            .collect();
        providers.push(Arc::new(OpenAiCompatibleProvider::new(
            p.id.clone(),
            p.base_url.clone(),
            p.api_key.clone(),
            models,
            quota.clone(),
        )));
    }
    Arc::new(Orchestrator::new(providers, stored.combos.clone()))
}

impl AppState {
    /// Load state.json if present, else seed.
    pub fn load(path: PathBuf, seed: StoredState) -> Self {
        let stored = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str::<StoredState>(&s).ok())
            .unwrap_or(seed);
        let quota: QuotaMap = Arc::new(Mutex::new(HashMap::new()));
        let orch = build_orch(&stored, &quota);
        Self {
            path,
            inner: RwLock::new(Inner { stored, orch }),
            quota,
        }
    }

    fn save_locked(&self, inner: &Inner) {
        if let Ok(s) = serde_json::to_string_pretty(&inner.stored) {
            let _ = std::fs::write(&self.path, s);
        }
    }

    pub fn orch(&self) -> Arc<Orchestrator> {
        self.inner.read().unwrap().orch.clone()
    }

    pub fn snapshot(&self) -> StoredState {
        self.inner.read().unwrap().stored.clone()
    }

    pub fn require_api_key(&self) -> bool {
        self.inner.read().unwrap().stored.require_api_key
    }

    pub fn key_valid(&self, key: &str) -> bool {
        self.inner
            .read()
            .unwrap()
            .stored
            .api_keys
            .iter()
            .any(|k| k.key == key)
    }

    fn mutate<F: FnOnce(&mut StoredState)>(&self, rebuild: bool, f: F) {
        let mut inner = self.inner.write().unwrap();
        f(&mut inner.stored);
        if rebuild {
            inner.orch = build_orch(&inner.stored, &self.quota);
        }
        self.save_locked(&inner);
    }

    /// Clone of the latest captured rate-limit snapshots, keyed by provider id.
    pub fn quota_snapshot(&self) -> HashMap<String, QuotaSnapshot> {
        self.quota.lock().unwrap().clone()
    }

    pub fn upsert_provider(&self, mut p: ProviderStored) {
        self.mutate(true, |s| {
            if let Some(e) = s.providers.iter_mut().find(|x| x.id == p.id) {
                // Preserve the stored key when the edit form leaves it blank
                // (the list endpoint never returns the secret).
                if p.api_key.is_empty() {
                    p.api_key = e.api_key.clone();
                }
                *e = p;
            } else {
                s.providers.push(p);
            }
        });
    }

    pub fn delete_provider(&self, id: &str) {
        self.mutate(true, |s| s.providers.retain(|x| x.id != id));
    }

    pub fn upsert_combo(&self, c: Combo) {
        self.mutate(true, |s| {
            if let Some(e) = s.combos.iter_mut().find(|x| x.name == c.name) {
                *e = c;
            } else {
                s.combos.push(c);
            }
        });
    }

    pub fn delete_combo(&self, name: &str) {
        self.mutate(true, |s| s.combos.retain(|x| x.name != name));
    }

    pub fn create_key(&self, name: String) -> ApiKey {
        let key = gen_key();
        let ak = ApiKey {
            id: short_id(&key),
            key,
            name,
            created_at: now(),
        };
        self.mutate(false, |s| s.api_keys.push(ak.clone()));
        ak
    }

    pub fn delete_key(&self, id: &str) {
        self.mutate(false, |s| s.api_keys.retain(|k| k.id != id));
    }

    pub fn set_require_key(&self, v: bool) {
        self.mutate(false, |s| s.require_api_key = v);
    }

    pub fn record_usage(&self, key: &str, prompt: u64, completion: u64) {
        self.record_request(key, prompt, completion, 200, false);
    }

    /// Record a request into both the aggregate counters and the time-series log.
    pub fn record_request(
        &self,
        target: &str,
        prompt: u64,
        completion: u64,
        status: u16,
        stream: bool,
    ) {
        self.mutate(false, |s| {
            let row = s.usage.entry(target.to_string()).or_default();
            row.requests += 1;
            row.prompt_tokens += prompt;
            row.completion_tokens += completion;
            s.request_log.push(LogEntry {
                ts: now(),
                target: target.to_string(),
                prompt_tokens: prompt,
                completion_tokens: completion,
                status,
                stream,
            });
            let len = s.request_log.len();
            if len > MAX_LOG {
                s.request_log.drain(0..len - MAX_LOG);
            }
        });
    }
}

fn short_id(key: &str) -> String {
    key.chars().skip(key.len().saturating_sub(8)).collect()
}

pub fn mask(key: &str) -> String {
    if key.len() <= 12 {
        return "•".repeat(key.len());
    }
    format!("{}…{}", &key[..8], &key[key.len() - 4..])
}

/// Non-cryptographic key generator — fine for a local slice.
fn gen_key() -> String {
    use std::hash::{Hash, Hasher};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let mut h = std::collections::hash_map::DefaultHasher::new();
    nanos.hash(&mut h);
    std::process::id().hash(&mut h);
    let a = h.finish();
    a.hash(&mut h);
    nanos.hash(&mut h);
    let b = h.finish();
    format!("sk-9r-{a:016x}{b:016x}")
}
