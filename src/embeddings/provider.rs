use crate::error::{MemoryError, Result};
use serde::Deserialize;

pub enum EmbeddingProvider {
    OpenAI { api_key: String },
    Gemini { api_key: String },
    OllamaLocal,
}

impl EmbeddingProvider {
    /// Create provider from environment
    pub fn from_env() -> Result<Self> {
        // Try OpenAI first
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            return Ok(Self::OpenAI { api_key: key });
        }

        // Try Gemini
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            return Ok(Self::Gemini { api_key: key });
        }

        // Fall back to Ollama
        Ok(Self::OllamaLocal)
    }

    /// Generate embeddings for a batch of texts
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::OpenAI { api_key } => self.embed_openai(texts, api_key).await,
            Self::Gemini { api_key } => self.embed_gemini(texts, api_key).await,
            Self::OllamaLocal => self.embed_ollama(texts).await,
        }
    }

    /// Generate embedding for single text
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let batch = self.embed_batch(&[text.to_string()]).await?;
        batch
            .into_iter()
            .next()
            .ok_or_else(|| MemoryError::Config("No embedding returned".into()))
    }

    async fn embed_openai(&self, texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>> {
        #[derive(Deserialize)]
        struct EmbeddingResponse {
            data: Vec<EmbeddingData>,
        }

        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        let client = reqwest::Client::new();
        let response = client
            .post("https://api.openai.com/v1/embeddings")
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": "text-embedding-3-small",
                "input": texts,
            }))
            .send()
            .await?;

        if !response.status().is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!("OpenAI API error: {}", text)));
        }

        let result: EmbeddingResponse = response.json().await?;
        Ok(result.data.into_iter().map(|d| d.embedding).collect())
    }

    async fn embed_gemini(&self, texts: &[String], api_key: &str) -> Result<Vec<Vec<f32>>> {
        // Gemini embedding API
        let client = reqwest::Client::new();
        let mut embeddings = Vec::new();

        for text in texts {
            let response = client
                .post(format!(
                    "https://generativelanguage.googleapis.com/v1/models/text-embedding-004:embedContent?key={}",
                    api_key
                ))
                .json(&serde_json::json!({
                    "content": {
                        "parts": [{
                            "text": text
                        }]
                    }
                }))
                .send()
                .await?;

            if !response.status().is_success() {
                let text = response.text().await.unwrap_or_default();
                return Err(MemoryError::Config(format!("Gemini API error: {}", text)));
            }

            let result: serde_json::Value = response.json().await?;
            let embedding: Vec<f32> = result
                .get("embedding")
                .and_then(|e| e.get("values"))
                .and_then(|v| v.as_array())
                .ok_or_else(|| MemoryError::Config("Invalid Gemini embedding response".into()))?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            embeddings.push(embedding);
        }

        Ok(embeddings)
    }

    async fn embed_ollama(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let client = reqwest::Client::new();
        let mut embeddings = Vec::new();

        for text in texts {
            let response = client
                .post("http://localhost:11434/api/embeddings")
                .json(&serde_json::json!({
                    "model": "nomic-embed-text",
                    "prompt": text,
                }))
                .send()
                .await?;

            if !response.status().is_success() {
                let text = response.text().await.unwrap_or_default();
                return Err(MemoryError::Config(format!("Ollama API error: {}", text)));
            }

            let result: serde_json::Value = response.json().await?;
            let embedding: Vec<f32> = result
                .get("embedding")
                .and_then(|e| e.as_array())
                .ok_or_else(|| MemoryError::Config("Invalid Ollama embedding response".into()))?
                .iter()
                .filter_map(|v| v.as_f64().map(|f| f as f32))
                .collect();

            embeddings.push(embedding);
        }

        Ok(embeddings)
    }
}
