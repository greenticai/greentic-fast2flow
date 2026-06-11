use std::str::FromStr;
use std::sync::Arc;

use anyhow::{Context, Result};
use fast2flow_core::{CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

use crate::{ENV_CANDIDATE_LIMIT, ENV_LLM_MIN_CONFIDENCE, ENV_MIN_CONFIDENCE};

#[derive(Debug, Clone)]
pub struct RouterBootstrapConfig {
    pub min_confidence: f32,
    pub llm_min_confidence: f32,
    pub candidate_limit: usize,
}

impl Default for RouterBootstrapConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            llm_min_confidence: 0.5,
            candidate_limit: 20,
        }
    }
}

impl RouterBootstrapConfig {
    pub fn from_env() -> Result<Self> {
        let mut config = Self::default();

        if let Some(value) = env_var(ENV_MIN_CONFIDENCE) {
            config.min_confidence =
                f32::from_str(&value).with_context(|| format!("invalid {}", ENV_MIN_CONFIDENCE))?;
        }
        if let Some(value) = env_var(ENV_LLM_MIN_CONFIDENCE) {
            config.llm_min_confidence = f32::from_str(&value)
                .with_context(|| format!("invalid {}", ENV_LLM_MIN_CONFIDENCE))?;
        }
        if let Some(value) = env_var(ENV_CANDIDATE_LIMIT) {
            config.candidate_limit = usize::from_str(&value)
                .with_context(|| format!("invalid {}", ENV_CANDIDATE_LIMIT))?;
        }

        Ok(config)
    }
}

pub async fn build_router_from_config(config: RouterBootstrapConfig) -> Result<CoreRouter> {
    let strategy = Arc::new(Phase1DeterministicStrategy);
    let filter = Arc::new(DefaultHookFilter::default());
    // The host does deterministic routing only — the LLM fallback now lives in
    // greentic-start. The core keeps a generic (unused) LlmProvider seam.
    let llm = None;
    Ok(CoreRouter::new(
        strategy,
        vec![filter],
        llm,
        RouterConfig {
            min_confidence: config.min_confidence,
            llm_min_confidence: config.llm_min_confidence,
            candidate_limit: config.candidate_limit,
        },
    ))
}

pub async fn build_router_from_env() -> Result<CoreRouter> {
    let config = RouterBootstrapConfig::from_env()?;
    build_router_from_config(config).await
}

pub(crate) fn env_var(key: &str) -> Option<String> {
    std::env::var(key).ok().and_then(|v| {
        let trimmed = v.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}
