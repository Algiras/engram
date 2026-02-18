use crate::error::{MemoryError, Result};
use serde::Deserialize;

pub enum EmbeddingProvider {
    OpenAI { api_key: String, model: String },
    Gemini { api_key: String, model: String },
    OllamaLocal { model: String },
}

impl EmbeddingProvider {
    /// Create provider from environment.
    ///
    /// Prefer [`from_config`] which also reads auth.json credentials.
    #[deprecated(note = "Use from_config instead — it respects auth.json in addition to env vars")]
    pub fn from_env() -> Result<Self> {
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            return Ok(Self::OpenAI {
                api_key: key,
                model: "text-embedding-3-small".to_string(),
            });
        }
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            return Ok(Self::Gemini {
                api_key: key,
                model: "gemini-embedding-001".to_string(),
            });
        }
        Ok(Self::OllamaLocal {
            model: "nomic-embed-text".to_string(),
        })
    }

    /// Create provider from resolved config, with env vars taking highest priority.
    pub fn from_config(config: &crate::config::Config) -> Self {
        use crate::auth::providers::Provider;

        // Env vars have highest priority (same as from_env)
        if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            return Self::OpenAI {
                api_key: key,
                model: "text-embedding-3-small".to_string(),
            };
        }
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            return Self::Gemini {
                api_key: key,
                model: "gemini-embedding-001".to_string(),
            };
        }

        // Check stored embedding preference in auth.json
        if let Ok(store) = crate::auth::AuthStore::load() {
            if let Some(ref pref) = store.embed_provider {
                // embed_model overrides the default for the chosen provider
                let stored_model = store.embed_model.clone();
                match pref.as_str() {
                    "openai" => {
                        let key = config
                            .llm
                            .api_key
                            .clone()
                            .or_else(|| store.get(Provider::OpenAI).map(|c| c.key.clone()))
                            .filter(|k| !k.is_empty());
                        if let Some(k) = key {
                            return Self::OpenAI {
                                api_key: k,
                                model: stored_model
                                    .unwrap_or_else(|| "text-embedding-3-small".to_string()),
                            };
                        }
                    }
                    "gemini" => {
                        let key = config
                            .llm
                            .api_key
                            .clone()
                            .or_else(|| store.get(Provider::Gemini).map(|c| c.key.clone()))
                            .filter(|k| !k.is_empty());
                        if let Some(k) = key {
                            return Self::Gemini {
                                api_key: k,
                                model: stored_model
                                    .unwrap_or_else(|| "gemini-embedding-001".to_string()),
                            };
                        }
                    }
                    "ollama" => {
                        return Self::OllamaLocal {
                            model: stored_model.unwrap_or_else(|| "nomic-embed-text".to_string()),
                        }
                    }
                    _ => {}
                }
            }
        }

        // Derive from resolved LLM provider in config
        match config.llm.provider {
            Provider::OpenAI => {
                if let Some(ref key) = config.llm.api_key {
                    return Self::OpenAI {
                        api_key: key.clone(),
                        model: "text-embedding-3-small".to_string(),
                    };
                }
            }
            Provider::Gemini => {
                if let Some(ref key) = config.llm.api_key {
                    return Self::Gemini {
                        api_key: key.clone(),
                        model: "gemini-embedding-001".to_string(),
                    };
                }
            }
            // These providers have no native embedding API — fall through to OllamaLocal
            Provider::Anthropic | Provider::Ollama | Provider::VSCode | Provider::OpenRouter => {}
        }

        Self::OllamaLocal {
            model: "nomic-embed-text".to_string(),
        }
    }

    /// Generate embeddings for a batch of texts
    pub async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        match self {
            Self::OpenAI { api_key, model } => self.embed_openai(texts, api_key, model).await,
            Self::Gemini { api_key, model } => self.embed_gemini(texts, api_key, model).await,
            Self::OllamaLocal { model } => self.embed_ollama(texts, model).await,
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

    async fn embed_openai(
        &self,
        texts: &[String],
        api_key: &str,
        model: &str,
    ) -> Result<Vec<Vec<f32>>> {
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
                "model": model,
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

    async fn embed_gemini(
        &self,
        texts: &[String],
        api_key: &str,
        model: &str,
    ) -> Result<Vec<Vec<f32>>> {
        let client = reqwest::Client::new();
        let mut embeddings = Vec::new();

        for text in texts {
            let response = client
                .post(format!(
                    "https://generativelanguage.googleapis.com/v1beta/models/{}:embedContent?key={}",
                    model, api_key
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

    async fn embed_ollama(&self, texts: &[String], model: &str) -> Result<Vec<Vec<f32>>> {
        let client = reqwest::Client::new();
        let mut embeddings = Vec::new();

        for text in texts {
            let response = client
                .post("http://localhost:11434/api/embeddings")
                .json(&serde_json::json!({
                    "model": model,
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
