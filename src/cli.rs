use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "claude-memory",
    about = "Conversation memory system for Claude Code",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Parse JSONL conversations -> archive + extract knowledge
    Ingest {
        /// Re-process everything (ignore manifest)
        #[arg(long)]
        force: bool,

        /// Preview without writing
        #[arg(long)]
        dry_run: bool,

        /// Process only a specific project
        #[arg(long)]
        project: Option<String>,

        /// Only process recent sessions (e.g., "1d", "2h", "30m")
        #[arg(long)]
        since: Option<String>,

        /// Archive only, skip LLM knowledge extraction
        #[arg(long)]
        skip_knowledge: bool,

        /// LLM provider override (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Full-text search across all memory
    Search {
        /// Search query (regex supported)
        query: String,

        /// Limit search to a specific project
        #[arg(long)]
        project: Option<String>,

        /// Search only knowledge/ files
        #[arg(long)]
        knowledge: bool,

        /// Lines of context around matches
        #[arg(short, long, default_value = "2")]
        context: usize,
    },

    /// Show project context (knowledge summary)
    Recall {
        /// Project name
        project: String,
    },

    /// Output context.md to stdout (for piping into prompts)
    Context {
        /// Project name
        project: String,
    },

    /// Show memory statistics
    Status,

    /// List all projects with activity
    Projects,

    /// Manage LLM provider authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Log in to an LLM provider
    Login {
        /// Provider name (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,

        /// Set as default provider
        #[arg(long)]
        set_default: bool,
    },

    /// List configured providers
    List,

    /// Remove provider credentials
    Logout {
        /// Provider name to remove
        provider: String,
    },

    /// Show active provider and configuration
    Status,
}
