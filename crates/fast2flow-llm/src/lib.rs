use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LlmResponse {
    pub target: String,
    pub confidence: f32,
    pub reason: String,
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("llm provider timed out")]
    Timeout,
    #[error("llm provider unavailable: {0}")]
    Unavailable(String),
    #[error("llm response was not valid JSON: {0}")]
    InvalidJson(String),
    #[error("llm provider error: {0}")]
    Provider(String),
}

#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn complete(&self, prompt: &str, timeout: Duration) -> Result<LlmResponse, LlmError>;
}
