use thiserror::Error;

#[derive(Error, Debug)]
pub enum MemoryError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Walk error: {0}")]
    Walk(#[from] walkdir::Error),

    #[error("No Claude projects directory found at {0}")]
    NoProjectsDir(String),

    #[error("LLM request failed: {0}")]
    LlmRequest(#[from] reqwest::Error),

    #[error("LLM returned empty response")]
    LlmEmptyResponse,

    #[error("Config error: {0}")]
    Config(String),

    #[error("Invalid duration format: {0}")]
    InvalidDuration(String),

    #[error("Auth error: {0}")]
    Auth(String),
}

pub type Result<T> = std::result::Result<T, MemoryError>;
