use std::collections::VecDeque;
use std::time::Duration;

use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Serialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct LlmRequest {
    pub messages: Vec<LlmMessage>,
    pub timeout: Duration,
}

#[derive(Debug, Clone)]
pub struct LlmResponse {
    pub content: String,
}

#[derive(Debug, Error)]
pub enum LlmError {
    #[error("missing OPENAI_API_KEY")]
    MissingApiKey,
    #[error("http error: {0}")]
    Http(String),
    #[error("openai error: {0}")]
    OpenAi(String),
    #[error("empty response")]
    EmptyResponse,
    #[error("mock responses exhausted")]
    MockExhausted,
}

pub struct OpenAiClient {
    api_key: String,
    model: String,
    client: Client,
}

impl OpenAiClient {
    pub fn new(api_key: String, model: String) -> Result<Self, LlmError> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| LlmError::Http(e.to_string()))?;
        Ok(Self {
            api_key,
            model,
            client,
        })
    }

    pub async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let body = OpenAiRequest {
            model: self.model.clone(),
            messages: req.messages,
            temperature: 0.0,
        };
        let resp = self
            .client
            .post("https://api.openai.com/v1/chat/completions")
            .bearer_auth(&self.api_key)
            .json(&body)
            .timeout(req.timeout)
            .send()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(LlmError::OpenAi(format!("{status} {text}")));
        }
        let parsed: OpenAiResponse = resp
            .json()
            .await
            .map_err(|e| LlmError::Http(e.to_string()))?;
        let content = parsed
            .choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or(LlmError::EmptyResponse)?;
        Ok(LlmResponse { content })
    }
}

pub struct MockLlm {
    responses: Mutex<VecDeque<String>>,
}

impl MockLlm {
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Mutex::new(responses.into()),
        }
    }

    pub async fn complete(&self, _req: LlmRequest) -> Result<LlmResponse, LlmError> {
        let mut guard = self.responses.lock().await;
        let content = guard.pop_front().ok_or(LlmError::MockExhausted)?;
        Ok(LlmResponse { content })
    }
}

pub enum LlmClient {
    OpenAi(OpenAiClient),
    Mock(MockLlm),
}

impl LlmClient {
    pub async fn complete(&self, req: LlmRequest) -> Result<LlmResponse, LlmError> {
        match self {
            LlmClient::OpenAi(client) => client.complete(req).await,
            LlmClient::Mock(client) => client.complete(req).await,
        }
    }
}

#[derive(Debug, Serialize)]
struct OpenAiRequest {
    model: String,
    messages: Vec<LlmMessage>,
    temperature: f32,
}

#[derive(Debug, Deserialize)]
struct OpenAiResponse {
    choices: Vec<OpenAiChoice>,
}

#[derive(Debug, Deserialize)]
struct OpenAiChoice {
    message: OpenAiMessage,
}

#[derive(Debug, Deserialize)]
struct OpenAiMessage {
    content: Option<String>,
}
