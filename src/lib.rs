// Library interface for claude-memory
#![allow(dead_code)]

pub mod analytics;
pub mod auth;
pub mod cli;
pub mod config;
pub mod diff;
pub mod embeddings;
pub mod error;
pub mod extractor;
pub mod graph;
pub mod health;
pub mod hive;
pub mod learning;
pub mod llm;
pub mod mcp;
pub mod parser;
pub mod renderer;
pub mod state;
pub mod sync;
pub mod tui;

// Re-export commonly used types
pub use config::Config;
pub use error::{MemoryError, Result};
