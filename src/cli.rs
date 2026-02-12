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

        /// Time-to-live for extracted entries (e.g., "7d", "2w")
        #[arg(long)]
        ttl: Option<String>,
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

    /// Interactive TUI for browsing and managing memories
    Tui,

    /// Inject knowledge into Claude Code's project memory
    Inject {
        /// Project name (defaults to basename of current directory)
        project: Option<String>,
    },

    /// Manage Claude Code hooks for automatic memory integration
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },

    /// Regenerate context.md from existing knowledge files (no re-ingestion)
    Regen {
        /// Project name
        project: String,

        /// LLM provider override (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Add a manual knowledge entry to a project
    Add {
        /// Project name
        project: String,

        /// Knowledge category: decisions, solutions, patterns, or preferences
        #[arg(value_parser = ["decisions", "solutions", "patterns", "preferences"])]
        category: String,

        /// The knowledge content to add
        content: String,

        /// Label for this entry (defaults to "manual")
        #[arg(long, default_value = "manual")]
        label: String,

        /// Time-to-live for this entry (e.g., "30m", "2h", "7d", "2w")
        #[arg(long)]
        ttl: Option<String>,
    },

    /// Review extracted memory candidates before promotion
    Review {
        /// Project name
        project: String,

        /// Show full content and include expired entries
        #[arg(long)]
        all: bool,
    },

    /// Promote an inbox entry into project/global long-term memory
    Promote {
        /// Project name
        project: String,

        /// Inbox session ID to promote (for example: abc123:decisions)
        session_id: String,

        /// Target category: decisions, solutions, patterns, or preferences
        #[arg(value_parser = ["decisions", "solutions", "patterns", "preferences"])]
        category: String,

        /// Promote to global memory instead of project memory
        #[arg(long)]
        global: bool,

        /// Label for promoted entry (defaults to "promoted")
        #[arg(long, default_value = "promoted")]
        label: String,

        /// Time-to-live for promoted entry (e.g., "30m", "2h", "7d", "2w")
        #[arg(long)]
        ttl: Option<String>,
    },

    /// Look up knowledge by topic across all files for a project
    Lookup {
        /// Project name
        project: String,

        /// Topic to search for (case-insensitive substring match)
        query: String,

        /// Include expired entries in results (marked with [EXPIRED])
        #[arg(long)]
        all: bool,
    },

    /// Remove knowledge for a project
    Forget {
        /// Project name
        project: String,

        /// Specific session ID to remove
        session_id: Option<String>,

        /// Remove sessions matching a topic (case-insensitive)
        #[arg(long)]
        topic: Option<String>,

        /// Wipe all knowledge for the project
        #[arg(long)]
        all: bool,

        /// Also delete conversation archives and summaries
        #[arg(long)]
        purge: bool,

        /// Remove only expired TTL entries
        #[arg(long)]
        expired: bool,
    },

    /// Manage LLM provider authentication
    Auth {
        #[command(subcommand)]
        command: AuthCommand,
    },

    /// Run as MCP server (Model Context Protocol)
    Mcp {
        /// LLM provider override (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Export project knowledge to various formats
    Export {
        /// Project name
        project: String,

        /// Output format
        #[arg(value_parser = ["markdown", "json", "html"])]
        format: String,

        /// Output file path (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Include conversation archives (not just knowledge)
        #[arg(long)]
        include_conversations: bool,
    },

    /// Sync knowledge with GitHub Gists
    Sync {
        #[command(subcommand)]
        command: SyncCommand,
    },

    /// Knowledge graph operations
    Graph {
        #[command(subcommand)]
        command: GraphCommand,
    },

    /// Generate embeddings for semantic search
    Embed {
        /// Project name
        project: String,

        /// Embedding provider (openai, gemini, ollama)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Semantic search using embeddings
    SearchSemantic {
        /// Search query
        query: String,

        /// Project name (optional)
        #[arg(long)]
        project: Option<String>,

        /// Number of results
        #[arg(long, default_value = "10")]
        top: usize,

        /// Minimum similarity threshold (0.0 - 1.0)
        #[arg(long, default_value = "0.5")]
        threshold: f32,
    },

    /// Detect and consolidate duplicate/similar knowledge
    Consolidate {
        /// Project name
        project: String,

        /// Similarity threshold for duplicates (0.85-0.95 recommended)
        #[arg(long, default_value = "0.9")]
        threshold: f32,

        /// Automatically merge duplicates without confirmation
        #[arg(long)]
        auto_merge: bool,

        /// Detect contradictions using LLM
        #[arg(long)]
        find_contradictions: bool,
    },

    /// Self-diagnose and fix issues (health check)
    Doctor {
        /// Project name (optional - checks all if not specified)
        project: Option<String>,

        /// Automatically fix issues without confirmation
        #[arg(long)]
        fix: bool,

        /// Show detailed diagnostic information
        #[arg(long)]
        verbose: bool,
    },

    /// Show usage analytics and insights
    Analytics {
        /// Project name (optional - shows all if not specified)
        project: Option<String>,

        /// Days to include (default: 30)
        #[arg(long, default_value = "30")]
        days: u32,

        /// Show detailed event log
        #[arg(long)]
        detailed: bool,

        /// Clear old analytics data (beyond --days)
        #[arg(long)]
        clear_old: bool,
    },

    /// Show knowledge changes over time
    Diff {
        /// Project name
        project: String,

        /// Knowledge category
        #[arg(value_parser = ["decisions", "solutions", "patterns", "preferences"])]
        category: String,

        /// Compare with specific version ID
        #[arg(long)]
        version: Option<String>,

        /// Show version history
        #[arg(long)]
        history: bool,
    },

    /// View and manage reinforcement learning progress
    Learn {
        #[command(subcommand)]
        command: LearnCommand,
    },

    /// Manage distributed knowledge sharing (Hive Mind)
    Hive {
        #[command(subcommand)]
        command: HiveCommand,
    },
}

#[derive(Subcommand)]
pub enum GraphCommand {
    /// Build knowledge graph from project knowledge
    Build {
        /// Project name
        project: String,

        /// LLM provider override
        #[arg(long)]
        provider: Option<String>,
    },

    /// Query the knowledge graph
    Query {
        /// Project name
        project: String,

        /// Concept to explore
        concept: String,

        /// Maximum traversal depth
        #[arg(long, default_value = "2")]
        depth: usize,
    },

    /// Visualize the knowledge graph
    Viz {
        /// Project name
        project: String,

        /// Output format
        #[arg(value_parser = ["dot", "svg", "ascii"])]
        format: String,

        /// Output file (default: stdout)
        #[arg(short, long)]
        output: Option<String>,

        /// Root concept for ASCII tree view
        #[arg(long)]
        root: Option<String>,
    },

    /// Find shortest path between concepts
    Path {
        /// Project name
        project: String,

        /// Source concept
        from: String,

        /// Target concept
        to: String,
    },

    /// Find most connected concepts (hubs)
    Hubs {
        /// Project name
        project: String,

        /// Number of hubs to show
        #[arg(long, default_value = "10")]
        top: usize,
    },
}

#[derive(Subcommand)]
pub enum SyncCommand {
    /// Push knowledge to a private gist
    Push {
        /// Project name
        project: String,

        /// Gist ID (optional, will create new if not provided)
        #[arg(long)]
        gist_id: Option<String>,

        /// Gist description
        #[arg(long, default_value = "claude-memory knowledge")]
        description: String,
    },

    /// Pull knowledge from a gist
    Pull {
        /// Project name
        project: String,

        /// Gist ID
        gist_id: String,

        /// Overwrite local knowledge if conflicts
        #[arg(long)]
        force: bool,
    },

    /// List gists for current project
    List {
        /// Project name
        project: String,
    },

    /// Clone knowledge from a gist to a new project
    Clone {
        /// Gist ID
        gist_id: String,

        /// Target project name
        project: String,
    },

    /// Show version history for a gist
    History {
        /// Gist ID
        gist_id: String,

        /// Show detailed diff for a specific version
        #[arg(long)]
        version: Option<String>,
    },

    /// Push knowledge to a git repository
    PushRepo {
        /// Project name
        project: String,

        /// Git repository path
        repo: String,

        /// Commit message
        #[arg(short, long)]
        message: Option<String>,

        /// Push to remote after commit
        #[arg(long)]
        push_remote: bool,
    },

    /// Pull knowledge from a git repository
    PullRepo {
        /// Project name
        project: String,

        /// Git repository path
        repo: String,

        /// Pull from remote before reading
        #[arg(long)]
        fetch_remote: bool,

        /// Branch to use
        #[arg(long, default_value = "main")]
        branch: String,
    },

    /// Initialize a git repository for knowledge sharing
    InitRepo {
        /// Repository path
        repo: String,
    },
}

#[derive(Subcommand)]
pub enum HooksCommand {
    /// Install hooks into Claude Code settings
    Install,

    /// Remove hooks from Claude Code settings
    Uninstall,

    /// Show hook installation status
    Status,
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

#[derive(Subcommand)]
pub enum LearnCommand {
    /// Show learning progress dashboard
    Dashboard {
        /// Project name (optional - shows all if not specified)
        project: Option<String>,
    },

    /// Apply learned optimizations to a project
    Optimize {
        /// Project name
        project: String,

        /// Preview changes without applying (dry run)
        #[arg(long)]
        dry_run: bool,

        /// Apply automatically without confirmation
        #[arg(long)]
        auto: bool,
    },

    /// Reset learning state to defaults
    Reset {
        /// Project name
        project: String,
    },

    /// Run learning simulation
    Simulate {
        /// Project name
        project: String,

        /// Number of simulated sessions
        #[arg(long, default_value = "50")]
        sessions: usize,

        /// Pattern: recall, mixed, high-frequency
        #[arg(long, default_value = "mixed")]
        pattern: String,
    },

    /// Provide explicit feedback about knowledge quality
    Feedback {
        /// Project name
        project: String,

        /// Session ID (from conversation history)
        #[arg(long)]
        session: Option<String>,

        /// Knowledge was helpful
        #[arg(long, conflicts_with = "unhelpful")]
        helpful: bool,

        /// Knowledge was unhelpful or incorrect
        #[arg(long, conflicts_with = "helpful")]
        unhelpful: bool,

        /// Optional comment about the feedback
        #[arg(long)]
        comment: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum HiveCommand {
    /// Manage knowledge pack registries
    Registry {
        #[command(subcommand)]
        command: RegistryCommand,
    },

    /// Manage knowledge packs
    Pack {
        #[command(subcommand)]
        command: PackCommand,
    },

    /// Install a knowledge pack
    Install {
        /// Pack name
        pack: String,

        /// Registry name (optional - searches all if not specified)
        #[arg(long)]
        registry: Option<String>,

        /// Installation scope (user/project)
        #[arg(long, default_value = "user")]
        scope: String,
    },

    /// Uninstall a knowledge pack
    Uninstall {
        /// Pack name
        pack: String,
    },

    /// List installed knowledge packs
    List,

    /// Update an installed pack
    Update {
        /// Pack name (updates all if not specified)
        pack: Option<String>,
    },

    /// Browse available knowledge packs
    Browse {
        /// Filter by category
        #[arg(long)]
        category: Option<String>,

        /// Filter by keyword
        #[arg(long)]
        keyword: Option<String>,
    },

    /// Search for knowledge packs
    Search {
        /// Search query
        query: String,
    },
}

#[derive(Subcommand)]
pub enum PackCommand {
    /// Create a new knowledge pack from local knowledge
    Create {
        /// Pack name
        name: String,

        /// Source project
        #[arg(long)]
        project: String,

        /// Pack description
        #[arg(long)]
        description: Option<String>,

        /// Author name
        #[arg(long)]
        author: Option<String>,

        /// Keywords (comma-separated)
        #[arg(long)]
        keywords: Option<String>,

        /// Categories (comma-separated: patterns,solutions,decisions,workflows,preferences)
        #[arg(long)]
        categories: Option<String>,

        /// Output directory (default: ./packs/<name>)
        #[arg(long)]
        output: Option<String>,
    },

    /// Show pack statistics
    Stats {
        /// Pack name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum RegistryCommand {
    /// Add a knowledge pack registry
    Add {
        /// Registry URL (supports GitHub shorthand: owner/repo)
        url: String,
    },

    /// Remove a registry
    Remove {
        /// Registry name
        name: String,
    },

    /// List all registries
    List,

    /// Update a registry (git pull)
    Update {
        /// Registry name (updates all if not specified)
        name: Option<String>,
    },
}
