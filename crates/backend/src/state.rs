//! Mutable application state, persisted to a JSON file. Holds providers, combos,
//! API keys, the require-key flag, and usage counters. Rebuilds the orchestrator
//! whenever providers/combos change.

use crate::provider_openai::OpenAiCompatibleProvider;
use aggregator::{Combo, ModelInfo, Orchestrator, Provider};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
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
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn build_orch(stored: &StoredState) -> Arc<Orchestrator> {
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
        let orch = build_orch(&stored);
        Self {
            path,
            inner: RwLock::new(Inner { stored, orch }),
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
            inner.orch = build_orch(&inner.stored);
        }
        self.save_locked(&inner);
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
        self.mutate(false, |s| {
            let row = s.usage.entry(key.to_string()).or_default();
            row.requests += 1;
            row.prompt_tokens += prompt;
            row.completion_tokens += completion;
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
