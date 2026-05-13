use std::time::Duration;

use async_trait::async_trait;
use fast2flow_llm::{LlmError, LlmProvider, LlmResponse};
use greentic_secrets_lib::{EnvSecretsManager, SecretsManager};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct OllamaProvider {
    client: Client,
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            client: Client::new(),
            endpoint,
            model,
        }
    }

    pub fn localhost_default(model: String) -> Self {
        Self::new("http://127.0.0.1:11434/api/generate".to_string(), model)
    }

    pub async fn from_secrets(
        endpoint_secret_path: Option<&str>,
        model_secret_path: &str,
    ) -> Result<Self, LlmError> {
        let manager = EnvSecretsManager;
        let endpoint = if let Some(path) = endpoint_secret_path {
            read_utf8_secret(&manager, path).await?
        } else {
            "http://127.0.0.1:11434/api/generate".to_string()
        };
        let model = read_utf8_secret(&manager, model_secret_path).await?;
        Ok(Self::new(endpoint, model))
    }
}

#[derive(Debug, Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    format: String,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    response: String,
}

#[derive(Debug, Deserialize)]
struct LlmOutputSchema {
    target: String,
    confidence: f32,
    reason: String,
}

#[async_trait]
impl LlmProvider for OllamaProvider {
    async fn complete(&self, prompt: &str, timeout: Duration) -> Result<LlmResponse, LlmError> {
        let request = OllamaRequest {
            model: self.model.clone(),
            prompt: format!(
                "Return strict JSON with target, confidence, reason. Input: {}",
                prompt
            ),
            format: "json".to_string(),
            stream: false,
        };

        debug!(
            model = %self.model,
            endpoint = %self.endpoint,
            prompt_len = prompt.len(),
            timeout_ms = timeout.as_millis() as u64,
            "ollama: requesting completion"
        );

        let send_future = async {
            self.client
                .post(&self.endpoint)
                .json(&request)
                .send()
                .await
                .map_err(|err| LlmError::Unavailable(err.to_string()))?
                .error_for_status()
                .map_err(|err| LlmError::Provider(err.to_string()))?
                .json::<OllamaResponse>()
                .await
                .map_err(|err| LlmError::Provider(err.to_string()))
        };

        let payload = match tokio::time::timeout(timeout, send_future).await {
            Ok(Ok(payload)) => payload,
            Ok(Err(err)) => {
                warn!(error = %err, "ollama: request failed");
                return Err(err);
            }
            Err(_) => {
                warn!(
                    timeout_ms = timeout.as_millis() as u64,
                    "ollama: request timed out"
                );
                return Err(LlmError::Timeout);
            }
        };

        let output = serde_json::from_str::<LlmOutputSchema>(&payload.response).map_err(|err| {
            warn!(error = %err, "ollama: response was not valid routing JSON");
            LlmError::InvalidJson(err.to_string())
        })?;

        debug!(
            target = %output.target,
            confidence = output.confidence,
            "ollama: completion parsed"
        );
        Ok(LlmResponse {
            target: output.target,
            confidence: output.confidence,
            reason: output.reason,
        })
    }
}

async fn read_utf8_secret(manager: &EnvSecretsManager, path: &str) -> Result<String, LlmError> {
    let bytes = manager
        .read(path)
        .await
        .map_err(|err| LlmError::Unavailable(err.to_string()))?;
    let value = String::from_utf8(bytes).map_err(|err| LlmError::InvalidJson(err.to_string()))?;
    Ok(value.trim().to_string())
}
