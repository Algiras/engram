use std::time::Instant;

use crate::auth::{providers::Provider, resolve_provider};
use crate::error::{MemoryError, Result};
use crate::llm::client::LlmClient;

pub struct TestResult {
    pub provider: Provider,
    pub model: String,
    pub latency_ms: u64,
    pub response_snippet: String, // first 60 chars of reply
    pub success: bool,
    pub error: Option<String>,
}

impl Default for TestResult {
    fn default() -> Self {
        Self {
            provider: Provider::Ollama,
            model: String::new(),
            latency_ms: 0,
            response_snippet: String::new(),
            success: false,
            error: None,
        }
    }
}

pub async fn test_provider_async(provider: Provider) -> TestResult {
    let resolved = match resolve_provider(Some(&provider.to_string()), None, None) {
        Ok(r) => r,
        Err(e) => {
            return TestResult {
                provider,
                success: false,
                error: Some(e.to_string()),
                ..Default::default()
            }
        }
    };
    let model = resolved.model.clone();
    let client = LlmClient::new(&resolved);
    let start = Instant::now();
    match client.chat_minimal("hi").await {
        Ok(resp) => TestResult {
            provider,
            model,
            success: true,
            latency_ms: start.elapsed().as_millis() as u64,
            response_snippet: resp.chars().take(60).collect(),
            error: None,
        },
        Err(e) => TestResult {
            provider,
            model,
            success: false,
            latency_ms: start.elapsed().as_millis() as u64,
            error: Some(e.to_string()),
            response_snippet: String::new(),
        },
    }
}

/// Sync wrapper — creates its own tokio runtime. For CLI and TUI (blocking).
pub fn test_provider_sync(provider: Provider) -> TestResult {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(test_provider_async(provider))
}

/// Query the provider's models endpoint and return model IDs.
/// Uses the native API for each provider — Gemini uses v1beta/models,
/// all others use the OpenAI-compatible GET /models endpoint.
pub async fn fetch_models_async(provider: Provider) -> Result<Vec<String>> {
    // Gemini uses its own listing API
    if provider == Provider::Gemini {
        let store = crate::auth::AuthStore::load().unwrap_or_default();
        let api_key = std::env::var("GEMINI_API_KEY")
            .ok()
            .or_else(|| store.get(Provider::Gemini).map(|c| c.key.clone()))
            .ok_or_else(|| MemoryError::Config("GEMINI_API_KEY not set".into()))?;

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| MemoryError::Config(e.to_string()))?;

        let resp = client
            .get(format!(
                "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                api_key
            ))
            .send()
            .await?;

        let json: serde_json::Value = resp.json().await?;
        let mut models: Vec<String> = json
            .get("models")
            .and_then(|m| m.as_array())
            .map(|arr| {
                arr.iter()
                    .filter(|m| {
                        m.get("supportedGenerationMethods")
                            .and_then(|s| s.as_array())
                            .map(|methods| {
                                methods
                                    .iter()
                                    .any(|v| v.as_str() == Some("generateContent"))
                            })
                            .unwrap_or(false)
                    })
                    .filter_map(|m| {
                        m.get("name")
                            .and_then(|n| n.as_str())
                            .map(|n| n.trim_start_matches("models/").to_string())
                    })
                    .collect()
            })
            .unwrap_or_default();
        models.sort();
        return Ok(models);
    }

    // OpenAI-compatible /models endpoint
    let resolved = resolve_provider(Some(&provider.to_string()), None, None)?;
    let url = format!("{}/models", resolved.endpoint);

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| MemoryError::Config(e.to_string()))?;

    let mut req = client.get(&url);
    if let Some(ref key) = resolved.api_key {
        req = req.header("Authorization", format!("Bearer {}", key));
    }
    if provider == Provider::OpenRouter {
        req = req
            .header("HTTP-Referer", "https://github.com/user/engram")
            .header("X-Title", "engram");
    }

    let response = req.send().await?;
    if !response.status().is_success() {
        return Err(MemoryError::Config(format!(
            "{} /models returned {}",
            provider.display_name(),
            response.status()
        )));
    }

    let json: serde_json::Value = response.json().await?;
    let mut models: Vec<String> = json
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(str::to_string))
                .collect()
        })
        .unwrap_or_default();

    models.sort();
    Ok(models)
}

/// Sync wrapper for `fetch_models_async`.
pub fn fetch_models_sync(provider: Provider) -> Result<Vec<String>> {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(fetch_models_async(provider))
}

/// Fetch embedding-capable models for a given provider.
/// For OpenAI: filters /v1/models for ids containing "embedding".
/// For Ollama: filters /v1/models for ids containing "embed".
/// For Gemini: returns the known embedding models (no filterable API).
pub async fn fetch_embed_models_async(provider_name: &str) -> Result<Vec<String>> {
    match provider_name {
        "openai" => {
            // Reuse LLM model list, filter for embedding models
            let all = fetch_models_async(crate::auth::providers::Provider::OpenAI).await?;
            let embed: Vec<String> = all
                .into_iter()
                .filter(|m| m.contains("embed") || m.contains("embedding"))
                .collect();
            Ok(embed)
        }
        "ollama" => {
            // Ollama: models with "embed" in their name are embedding models
            let all = fetch_models_async(crate::auth::providers::Provider::Ollama).await?;
            let embed: Vec<String> = all.into_iter().filter(|m| m.contains("embed")).collect();
            Ok(embed)
        }
        "gemini" => {
            // Gemini: query v1beta/models and filter by embedContent support
            let store = crate::auth::AuthStore::load().unwrap_or_default();
            let api_key = std::env::var("GEMINI_API_KEY")
                .ok()
                .or_else(|| {
                    store
                        .get(crate::auth::providers::Provider::Gemini)
                        .map(|c| c.key.clone())
                })
                .ok_or_else(|| MemoryError::Config("GEMINI_API_KEY not set".into()))?;

            let client = reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .map_err(|e| MemoryError::Config(e.to_string()))?;

            let resp = client
                .get(format!(
                    "https://generativelanguage.googleapis.com/v1beta/models?key={}",
                    api_key
                ))
                .send()
                .await?;

            let json: serde_json::Value = resp.json().await?;
            let models: Vec<String> = json
                .get("models")
                .and_then(|m| m.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter(|m| {
                            m.get("supportedGenerationMethods")
                                .and_then(|s| s.as_array())
                                .map(|methods| {
                                    methods.iter().any(|v| v.as_str() == Some("embedContent"))
                                })
                                .unwrap_or(false)
                        })
                        .filter_map(|m| {
                            m.get("name")
                                .and_then(|n| n.as_str())
                                .map(|n| n.trim_start_matches("models/").to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
            Ok(models)
        }
        other => Err(MemoryError::Config(format!(
            "Embedding model listing not supported for '{}'. Try: openai, gemini, ollama",
            other
        ))),
    }
}

pub fn fetch_embed_models_sync(provider_name: &str) -> Result<Vec<String>> {
    tokio::runtime::Runtime::new()
        .expect("tokio runtime")
        .block_on(fetch_embed_models_async(provider_name))
}

/// Test all providers that have credentials or don't require auth.
pub fn test_all_providers_sync() -> Vec<TestResult> {
    use crate::auth::AuthStore;
    let store = AuthStore::load().unwrap_or_default();
    Provider::all()
        .iter()
        .filter(|&&p| store.get(p).is_some() || !p.requires_auth())
        .map(|&p| test_provider_sync(p))
        .collect()
}
