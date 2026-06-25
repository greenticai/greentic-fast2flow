use std::time::Duration;

use async_trait::async_trait;
use fast2flow_contracts::FlowExecutionType;
use fast2flow_llm::{LlmError, LlmProvider, LlmResponse};
use greentic_secrets_lib::{EnvSecretsManager, SecretsManager};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct OpenAiProvider {
    client: Client,
    endpoint: String,
    model: String,
    api_key: String,
}

impl OpenAiProvider {
    pub fn new(api_key: String, model: String) -> Self {
        Self {
            client: Client::new(),
            endpoint: "https://api.openai.com/v1/chat/completions".to_string(),
            model,
            api_key,
        }
    }

    pub fn from_api_key(api_key: String) -> Self {
        Self::new(api_key, "gpt-4o-mini".to_string())
    }

    pub async fn from_secrets(
        api_key_secret_path: &str,
        model_secret_path: Option<&str>,
    ) -> Result<Self, LlmError> {
        let manager = EnvSecretsManager;
        let api_key = read_utf8_secret(&manager, api_key_secret_path).await?;
        let model = if let Some(path) = model_secret_path {
            read_utf8_secret(&manager, path).await?
        } else {
            "gpt-4o-mini".to_string()
        };
        Ok(Self::new(api_key, model))
    }
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    response_format: OpenAiResponseFormat,
    messages: Vec<OpenAiMessage>,
}

#[derive(Debug, Serialize)]
struct OpenAiResponseFormat {
    r#type: String,
}

#[derive(Debug, Serialize)]
struct OpenAiMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct OpenAiApiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiChoiceMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoiceMessage {
    content: String,
}

#[derive(Debug, Deserialize)]
struct LlmOutputSchema {
    target: String,
    confidence: f32,
    reason: String,
    #[serde(default)]
    flow_type: FlowExecutionType,
}

#[async_trait]
impl LlmProvider for OpenAiProvider {
    async fn complete(&self, prompt: &str, timeout: Duration) -> Result<LlmResponse, LlmError> {
        let request = OpenAiRequest {
            model: self.model.clone(),
            response_format: OpenAiResponseFormat {
                r#type: "json_object".to_string(),
            },
            messages: vec![
                OpenAiMessage {
                    role: "system".to_string(),
                    content: "Return strict JSON object with keys: target, confidence, reason"
                        .to_string(),
                },
                OpenAiMessage {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
        };

        debug!(
            model = %self.model,
            prompt_len = prompt.len(),
            timeout_ms = timeout.as_millis() as u64,
            "openai: requesting completion"
        );

        let send_future = async {
            self.client
                .post(&self.endpoint)
                .bearer_auth(&self.api_key)
                .json(&request)
                .send()
                .await
                .map_err(|err| LlmError::Unavailable(err.to_string()))?
                .error_for_status()
                .map_err(|err| LlmError::Provider(err.to_string()))?
                .json::<OpenAiApiResponse>()
                .await
                .map_err(|err| LlmError::Provider(err.to_string()))
        };

        let payload = match tokio::time::timeout(timeout, send_future).await {
            Ok(Ok(payload)) => payload,
            Ok(Err(err)) => {
                warn!(error = %err, "openai: request failed");
                return Err(err);
            }
            Err(_) => {
                warn!(
                    timeout_ms = timeout.as_millis() as u64,
                    "openai: request timed out"
                );
                return Err(LlmError::Timeout);
            }
        };

        let content = payload
            .choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .ok_or_else(|| {
                LlmError::InvalidJson("missing choices[0].message.content".to_string())
            })?;

        let output = serde_json::from_str::<LlmOutputSchema>(content).map_err(|err| {
            warn!(error = %err, "openai: response was not valid routing JSON");
            LlmError::InvalidJson(err.to_string())
        })?;

        debug!(
            target = %output.target,
            confidence = output.confidence,
            "openai: completion parsed"
        );
        Ok(LlmResponse {
            target: output.target,
            confidence: output.confidence,
            reason: output.reason,
            flow_type: output.flow_type,
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
