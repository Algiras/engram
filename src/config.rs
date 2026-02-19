use std::path::PathBuf;

use crate::auth;
use crate::auth::providers::ResolvedProvider;
use crate::error::{MemoryError, Result};

/// Reserved pseudo-project name for cross-project global knowledge
pub const GLOBAL_PROJECT: &str = "global";
/// Directory under knowledge/ that holds global knowledge files
pub const GLOBAL_DIR: &str = "_global";

/// Minimum active block count before daemon triggers distillation
pub const DISTILL_THRESHOLD: usize = 30;
/// Age cutoff for daemon distillation (days)
pub const DISTILL_STALE_DAYS: u64 = 90;
/// Per-ingest-cycle decay factor for importance boosts
pub const IMPORTANCE_DECAY_FACTOR: f32 = 0.98;

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
        let home = dirs::home_dir()
            .ok_or_else(|| MemoryError::Config("Could not determine home directory".into()))?;

        let claude_projects_dir = home.join(".claude").join("projects");
        let memory_dir = home.join("memory");

        // Legacy env var overrides still work
        let env_endpoint = std::env::var("ENGRAM_LLM_ENDPOINT").ok();
        let env_model = std::env::var("ENGRAM_LLM_MODEL").ok();

        let llm = auth::resolve_provider(provider_override, env_endpoint, env_model)?;

        Ok(Config {
            claude_projects_dir,
            memory_dir,
            llm,
        })
    }
}
