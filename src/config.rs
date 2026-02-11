use std::path::PathBuf;

use crate::auth;
use crate::auth::providers::ResolvedProvider;
use crate::error::{MemoryError, Result};

#[derive(Debug, Clone)]
pub struct Config {
    /// Where Claude stores project data
    pub claude_projects_dir: PathBuf,
    /// Where we write memory output
    pub memory_dir: PathBuf,
    /// Resolved LLM provider configuration
    pub llm: ResolvedProvider,
}

impl Config {
    pub fn load(provider_override: Option<&str>) -> Result<Self> {
        let home = dirs::home_dir().ok_or_else(|| {
            MemoryError::Config("Could not determine home directory".into())
        })?;

        let claude_projects_dir = home.join(".claude").join("projects");
        let memory_dir = home.join("memory");

        // Legacy env var overrides still work
        let env_endpoint = std::env::var("CLAUDE_MEMORY_LLM_ENDPOINT").ok();
        let env_model = std::env::var("CLAUDE_MEMORY_LLM_MODEL").ok();

        let llm = auth::resolve_provider(provider_override, env_endpoint, env_model)?;

        Ok(Config {
            claude_projects_dir,
            memory_dir,
            llm,
        })
    }
}
