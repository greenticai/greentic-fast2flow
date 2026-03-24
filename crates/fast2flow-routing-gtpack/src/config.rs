use std::str::FromStr;
use std::sync::Arc;

use anyhow::{anyhow, Context, Result};
use fast2flow_core::{CoreRouter, RouterConfig};
use fast2flow_hooks::DefaultHookFilter;
use fast2flow_llm::LlmProvider;
#[cfg(not(target_arch = "wasm32"))]
use fast2flow_llm_ollama::OllamaProvider;
#[cfg(not(target_arch = "wasm32"))]
use fast2flow_llm_openai::OpenAiProvider;
use fast2flow_strategy_phase1::Phase1DeterministicStrategy;

use crate::{
    ENV_CANDIDATE_LIMIT, ENV_LLM_MIN_CONFIDENCE, ENV_LLM_PROVIDER, ENV_MIN_CONFIDENCE,
    ENV_OLLAMA_ENDPOINT_PATH, ENV_OLLAMA_MODEL_PATH, ENV_OPENAI_API_KEY_PATH,
    ENV_OPENAI_MODEL_PATH,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmRuntimeConfig {
    Disabled,
    OpenAi {
        api_key_secret_path: String,
        model_secret_path: Option<String>,
    },
    Ollama {
        endpoint_secret_path: Option<String>,
        model_secret_path: String,
    },
}

#[derive(Debug, Clone)]
pub struct RouterBootstrapConfig {
    pub min_confidence: f32,
    pub llm_min_confidence: f32,
    pub candidate_limit: usize,
    pub llm: LlmRuntimeConfig,
}

impl Default for RouterBootstrapConfig {
    fn default() -> Self {
        Self {
            min_confidence: 0.5,
            llm_min_confidence: 0.5,
            candidate_limit: 20,
            llm: LlmRuntimeConfig::Disabled,
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

        config.llm = parse_llm_from_env()?;
        Ok(config)
    }
}

pub async fn build_router_from_config(config: RouterBootstrapConfig) -> Result<CoreRouter> {
    let strategy = Arc::new(Phase1DeterministicStrategy);
    let filter = Arc::new(DefaultHookFilter::default());
    let llm = build_llm(&config.llm).await?;
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

#[cfg(not(target_arch = "wasm32"))]
pub(crate) async fn build_llm(config: &LlmRuntimeConfig) -> Result<Option<Arc<dyn LlmProvider>>> {
    let provider: Option<Arc<dyn LlmProvider>> = match config {
        LlmRuntimeConfig::Disabled => None,
        LlmRuntimeConfig::OpenAi {
            api_key_secret_path,
            model_secret_path,
        } => {
            let provider =
                OpenAiProvider::from_secrets(api_key_secret_path, model_secret_path.as_deref())
                    .await
                    .map_err(|err| anyhow!("openai secrets bootstrap failed: {err}"))?;
            Some(Arc::new(provider))
        }
        LlmRuntimeConfig::Ollama {
            endpoint_secret_path,
            model_secret_path,
        } => {
            let provider =
                OllamaProvider::from_secrets(endpoint_secret_path.as_deref(), model_secret_path)
                    .await
                    .map_err(|err| anyhow!("ollama secrets bootstrap failed: {err}"))?;
            Some(Arc::new(provider))
        }
    };
    Ok(provider)
}

#[cfg(target_arch = "wasm32")]
pub(crate) async fn build_llm(config: &LlmRuntimeConfig) -> Result<Option<Arc<dyn LlmProvider>>> {
    if !matches!(config, LlmRuntimeConfig::Disabled) {
        return Err(anyhow!(
            "LLM providers are not supported in wasm32 component runtime; set {}=disabled",
            ENV_LLM_PROVIDER
        ));
    }
    Ok(None)
}

fn parse_llm_from_env() -> Result<LlmRuntimeConfig> {
    let provider = env_var(ENV_LLM_PROVIDER)
        .unwrap_or_else(|| "disabled".to_string())
        .to_ascii_lowercase();

    match provider.as_str() {
        "" | "disabled" | "none" => Ok(LlmRuntimeConfig::Disabled),
        "openai" => {
            let api_key_secret_path =
                env_var(ENV_OPENAI_API_KEY_PATH).unwrap_or_else(|| "OPENAI_API_KEY".to_string());
            let model_secret_path = env_var(ENV_OPENAI_MODEL_PATH);
            Ok(LlmRuntimeConfig::OpenAi {
                api_key_secret_path,
                model_secret_path,
            })
        }
        "ollama" => {
            let model_secret_path = env_var(ENV_OLLAMA_MODEL_PATH).ok_or_else(|| {
                anyhow!(
                    "{} is required when FAST2FLOW_LLM_PROVIDER=ollama",
                    ENV_OLLAMA_MODEL_PATH
                )
            })?;
            let endpoint_secret_path = env_var(ENV_OLLAMA_ENDPOINT_PATH);
            Ok(LlmRuntimeConfig::Ollama {
                endpoint_secret_path,
                model_secret_path,
            })
        }
        _ => Err(anyhow!(
            "unsupported {} value: {}",
            ENV_LLM_PROVIDER,
            provider
        )),
    }
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
