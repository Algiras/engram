use crate::auth::providers::{Provider, ResolvedProvider};
use crate::error::{MemoryError, Result};

/// Multi-provider LLM client
pub struct LlmClient {
    provider: Provider,
    endpoint: String,
    model: String,
    api_key: Option<String>,
    client: reqwest::Client,
}

impl LlmClient {
    pub fn new(resolved: &ResolvedProvider) -> Self {
        Self {
            provider: resolved.provider,
            endpoint: resolved.endpoint.clone(),
            model: resolved.model.clone(),
            api_key: resolved.api_key.clone(),
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(120))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Send a chat completion request and return the response text
    pub async fn chat(&self, system: &str, user: &str) -> Result<String> {
        match self.provider {
            Provider::Anthropic => self.chat_anthropic(system, user).await,
            Provider::OpenAI | Provider::Ollama => self.chat_openai_compat(system, user).await,
        }
    }

    /// Anthropic Messages API
    async fn chat_anthropic(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/v1/messages", self.endpoint);

        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 2048,
            "system": system,
            "messages": [
                { "role": "user", "content": user },
            ],
            "temperature": 0.3,
        });

        let mut req = self.client.post(&url).json(&body);

        if let Some(ref key) = self.api_key {
            req = req
                .header("x-api-key", key)
                .header("anthropic-version", "2023-06-01");
        }

        let response = req.send().await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "LLM returned {}: {}",
                status, text
            )));
        }

        let json: serde_json::Value = response.json().await?;

        json.get("content")
            .and_then(|c| c.get(0))
            .and_then(|b| b.get("text"))
            .and_then(|t| t.as_str())
            .map(|s| s.to_string())
            .ok_or(MemoryError::LlmEmptyResponse)
    }

    /// OpenAI-compatible API (OpenAI, Ollama, etc.)
    async fn chat_openai_compat(&self, system: &str, user: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.endpoint);

        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user },
            ],
            "temperature": 0.3,
            "max_tokens": 2048,
        });

        let mut req = self.client.post(&url).json(&body);

        if let Some(ref key) = self.api_key {
            req = req.header("Authorization", format!("Bearer {}", key));
        }

        let response = req.send().await?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "LLM returned {}: {}",
                status, text
            )));
        }

        let json: serde_json::Value = response.json().await?;

        json.get("choices")
            .and_then(|c| c.get(0))
            .and_then(|c| c.get("message"))
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            .ok_or(MemoryError::LlmEmptyResponse)
    }
}
