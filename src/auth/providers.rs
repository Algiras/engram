use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Anthropic,
    OpenAI,
    Ollama,
    Gemini,
    VSCode,
    OpenRouter,
}

impl Provider {
    pub fn all() -> &'static [Provider] {
        &[
            Provider::Anthropic,
            Provider::OpenAI,
            Provider::Ollama,
            Provider::Gemini,
            Provider::VSCode,
            Provider::OpenRouter,
        ]
    }

    pub fn default_endpoint(&self) -> &'static str {
        match self {
            Provider::Anthropic => "https://api.anthropic.com",
            Provider::OpenAI => "https://api.openai.com/v1",
            Provider::Ollama => "http://localhost:11434/v1",
            Provider::Gemini => "https://generativelanguage.googleapis.com/v1",
            Provider::VSCode => "http://localhost:11435/v1",
            Provider::OpenRouter => "https://openrouter.ai/api/v1",
        }
    }

    pub fn default_model(&self) -> &'static str {
        match self {
            Provider::Anthropic => "claude-sonnet-4-5-20250929",
            Provider::OpenAI => "gpt-4o",
            Provider::Ollama => "gemma3:4b",
            Provider::Gemini => "gemini-2.0-flash",
            Provider::VSCode => "gpt-4o",
            // A free model available without credits — use `engram auth models openrouter`
            // to discover all available models including other free ones.
            Provider::OpenRouter => "meta-llama/llama-3.1-8b-instruct:free",
        }
    }

    pub fn requires_auth(&self) -> bool {
        match self {
            Provider::Anthropic | Provider::OpenAI | Provider::Gemini | Provider::OpenRouter => {
                true
            }
            Provider::Ollama | Provider::VSCode => false,
        }
    }

    pub fn env_var_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "ANTHROPIC_API_KEY",
            Provider::OpenAI => "OPENAI_API_KEY",
            Provider::Ollama => "",
            Provider::Gemini => "GEMINI_API_KEY",
            Provider::VSCode => "",
            Provider::OpenRouter => "OPENROUTER_API_KEY",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Provider::Anthropic => "Anthropic (Claude)",
            Provider::OpenAI => "OpenAI",
            Provider::Ollama => "Ollama (local)",
            Provider::Gemini => "Google Gemini",
            Provider::VSCode => "VS Code LM API",
            Provider::OpenRouter => "OpenRouter",
        }
    }

    /// Whether this provider supports model discovery (live query).
    /// Anthropic has no public /models endpoint.
    pub fn supports_model_list(&self) -> bool {
        !matches!(self, Provider::Anthropic)
    }

    pub fn from_str_loose(s: &str) -> Option<Provider> {
        match s.to_lowercase().as_str() {
            "anthropic" | "claude" => Some(Provider::Anthropic),
            "openai" | "gpt" => Some(Provider::OpenAI),
            "ollama" | "local" => Some(Provider::Ollama),
            "gemini" | "google" => Some(Provider::Gemini),
            "vscode" | "vs-code" | "copilot" => Some(Provider::VSCode),
            "openrouter" | "or" => Some(Provider::OpenRouter),
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
            Provider::Gemini => write!(f, "gemini"),
            Provider::VSCode => write!(f, "vscode"),
            Provider::OpenRouter => write!(f, "openrouter"),
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
