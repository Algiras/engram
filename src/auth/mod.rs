pub mod providers;

use std::collections::HashMap;
use std::path::PathBuf;

use crate::error::{MemoryError, Result};
use providers::{Provider, ResolvedProvider};

/// On-disk representation of auth.json
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct AuthStore {
    #[serde(default)]
    pub default_provider: Option<String>,
    #[serde(default)]
    pub providers: HashMap<String, ProviderCredential>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderCredential {
    #[serde(rename = "type")]
    pub cred_type: String,
    pub key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl AuthStore {
    /// Path to auth.json
    pub fn path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| MemoryError::Auth("Could not determine config directory".into()))?;
        Ok(config_dir.join("engram").join("auth.json"))
    }

    /// Load from disk, returning default if file doesn't exist
    pub fn load() -> Result<Self> {
        let path = Self::path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let data = std::fs::read_to_string(&path)?;
        let store: AuthStore = serde_json::from_str(&data)?;
        Ok(store)
    }

    /// Save to disk with 0600 permissions
    pub fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(self)?;
        std::fs::write(&path, &data)?;

        // Set permissions to owner-only access
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        }

        // Windows: full per-user ACL requires a platform crate (e.g. windows-acl).
        // The standard library only exposes a read-only flag which is not equivalent
        // to Unix 0600, so no meaningful restriction is applied here.

        Ok(())
    }

    /// Get credential for a provider
    pub fn get(&self, provider: Provider) -> Option<&ProviderCredential> {
        self.providers.get(&provider.to_string())
    }

    /// Set credential for a provider
    pub fn set(&mut self, provider: Provider, cred: ProviderCredential) {
        self.providers.insert(provider.to_string(), cred);
    }

    /// Remove credential for a provider
    pub fn remove(&mut self, provider: Provider) {
        self.providers.remove(&provider.to_string());
        // Clear default if it was this provider
        if self.default_provider.as_deref() == Some(&provider.to_string()) {
            self.default_provider = None;
        }
    }
}

/// Resolve the active provider using the full precedence chain:
/// explicit CLI arg > env vars > auth.json default > any stored cred > Ollama fallback
pub fn resolve_provider(
    explicit: Option<&str>,
    env_endpoint: Option<String>,
    env_model: Option<String>,
) -> Result<ResolvedProvider> {
    let store = AuthStore::load()?;

    // 1. If explicit provider specified on CLI
    if let Some(name) = explicit {
        let provider = Provider::from_str_loose(name)
            .ok_or_else(|| MemoryError::Auth(format!("Unknown provider: {}", name)))?;
        return resolve_for_provider(provider, &store, env_endpoint, env_model);
    }

    // 2. Detect from env vars
    if let Some(provider) = detect_from_env() {
        return resolve_for_provider(provider, &store, env_endpoint, env_model);
    }

    // 3. auth.json default
    if let Some(ref default_name) = store.default_provider {
        if let Some(provider) = Provider::from_str_loose(default_name) {
            return resolve_for_provider(provider, &store, env_endpoint, env_model);
        }
    }

    // 4. Any stored credential (prefer anthropic > openai > gemini)
    for &provider in &[Provider::Anthropic, Provider::OpenAI, Provider::Gemini] {
        if store.get(provider).is_some() {
            return resolve_for_provider(provider, &store, env_endpoint, env_model);
        }
    }

    // 5. Fallback to Ollama
    resolve_for_provider(Provider::Ollama, &store, env_endpoint, env_model)
}

/// Resolve a specific provider with env/auth.json credentials
fn resolve_for_provider(
    provider: Provider,
    store: &AuthStore,
    env_endpoint: Option<String>,
    env_model: Option<String>,
) -> Result<ResolvedProvider> {
    // API key: env var > auth.json
    let api_key = if !provider.env_var_name().is_empty() {
        std::env::var(provider.env_var_name()).ok()
    } else {
        None
    }
    .or_else(|| store.get(provider).map(|c| c.key.clone()));

    // Endpoint: env override > auth.json > provider default
    let endpoint = env_endpoint
        .or_else(|| store.get(provider).and_then(|c| c.endpoint.clone()))
        .unwrap_or_else(|| provider.default_endpoint().to_string());

    // Model: env override > auth.json > provider default
    let model = env_model
        .or_else(|| store.get(provider).and_then(|c| c.model.clone()))
        .unwrap_or_else(|| provider.default_model().to_string());

    // Validate: cloud providers need an API key
    if provider.requires_auth() && api_key.is_none() {
        return Err(MemoryError::Auth(format!(
            "No API key found for {}. Set {} or run: engram auth login --provider {}",
            provider.display_name(),
            provider.env_var_name(),
            provider
        )));
    }

    Ok(ResolvedProvider {
        provider,
        endpoint,
        model,
        api_key,
    })
}

/// Check if any provider env var is set
fn detect_from_env() -> Option<Provider> {
    if std::env::var("ANTHROPIC_API_KEY").is_ok() {
        return Some(Provider::Anthropic);
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return Some(Provider::OpenAI);
    }
    if std::env::var("GEMINI_API_KEY").is_ok() {
        return Some(Provider::Gemini);
    }
    None
}
