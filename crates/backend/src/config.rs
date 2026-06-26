//! Seed configuration loaded from `config.toml` on first run. After that the
//! mutable state lives in `state.json` (see state.rs).

use aggregator::{Combo, FusionConfig, Strategy, Target};
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub server: ServerCfg,
    #[serde(default)]
    pub providers: Vec<ProviderCfg>,
    #[serde(default)]
    pub combos: Vec<ComboCfg>,
}

#[derive(Debug, Deserialize)]
pub struct ServerCfg {
    #[serde(default = "default_port")]
    pub port: u16,
}

impl Default for ServerCfg {
    fn default() -> Self {
        Self { port: default_port() }
    }
}

fn default_port() -> u16 {
    20127
}

#[derive(Debug, Deserialize)]
pub struct ProviderCfg {
    pub id: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
    #[serde(default)]
    pub models: Vec<ModelCfg>,
}

#[derive(Debug, Deserialize)]
pub struct ModelCfg {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ComboCfg {
    pub name: String,
    pub targets: Vec<Target>,
    pub strategy: String,
    #[serde(default = "one")]
    pub sticky: u32,
    #[serde(default)]
    pub min_panel: Option<usize>,
    #[serde(default)]
    pub grace_ms: Option<u64>,
    #[serde(default)]
    pub hard_timeout_ms: Option<u64>,
    #[serde(default)]
    pub judge: Option<Target>,
}

fn one() -> u32 {
    1
}

impl ComboCfg {
    pub fn into_combo(self) -> Combo {
        let strategy = match self.strategy.as_str() {
            "round_robin" | "round-robin" => Strategy::RoundRobin { sticky: self.sticky },
            "fusion" => {
                let d = FusionConfig::default();
                Strategy::Fusion(FusionConfig {
                    min_panel: self.min_panel.unwrap_or(d.min_panel),
                    grace_ms: self.grace_ms.unwrap_or(d.grace_ms),
                    hard_timeout_ms: self.hard_timeout_ms.unwrap_or(d.hard_timeout_ms),
                    judge: self.judge,
                })
            }
            _ => Strategy::Fallback,
        };
        Combo {
            name: self.name,
            targets: self.targets,
            strategy,
        }
    }
}

pub fn load(path: &str) -> Result<Config, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("reading {path}: {e}"))?;
    toml::from_str(&text).map_err(|e| format!("parsing {path}: {e}"))
}
