use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Ollama,
}

impl Provider {
    pub fn all() -> &'static [Provider] {
        &[Provider::Anthropic, Provider::OpenAI, Provider::Ollama]
    }

    pub fn default_endpoint(&self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com",
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::Ollama => "http://localhost:11434/v1",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-sonnet-4-5-20250929",
            Provider::OpenAI => "gpt-4o",
            Provider::Ollama => "gemma3:4b",
        }
    }

    pub fn requires_auth(&self) -> bool {
        match self {
            Provider::Anthropic | Provider::OpenAI => true,
            Provider::Ollama => false,
        }
    }

    pub fn env_var_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Ollama => "",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic (Claude)",
            Provider::OpenAI => "OpenAI",
            Provider::Ollama => "Ollama (local)",
        }
    }

    pub fn from_str_loose(s: &str) -> Option<Provider> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "openai" | "gpt" => Some(Provider::OpenAI),
            "ollama" | "local" => Some(Provider::Ollama),
            _ => None,
        }
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Provider::Anthropic => write!(f, "anthropic"),
            Provider::OpenAI => write!(f, "openai"),
            Provider::Ollama => write!(f, "ollama"),
        }
    }
}

/// A fully resolved provider configuration — no further lookups needed.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    pub provider: Provider,
    pub endpoint: String,
    pub model: String,
    pub api_key: Option<String>,
}
