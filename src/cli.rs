use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "engram",
    about = "Conversation memory system for Claude Code",
    version
)]
pub struct Cli {
    /// Enable verbose output
    #[arg(global = true, long, short)]
    pub verbose: bool,

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

        /// Search global memory (_global knowledge store)
        #[arg(long)]
        global: bool,
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
        /// Inject full (uncompacted) knowledge dump instead of compact summary
        #[arg(long)]
        full: bool,
        /// Skip automatic cleanup of expired entries (not recommended)
        #[arg(long)]
        no_auto_clean: bool,
        /// Use semantic search to inject only what is relevant to current git context
        #[arg(long)]
        smart: bool,
        /// Token budget for smart inject (default: 1500)
        #[arg(long, default_value = "1500")]
        budget: usize,
        /// Line budget for compact inject (default: 180). Scales all sections proportionally.
        /// Useful for long-context models: e.g. --lines 500 for ~3x more context.
        #[arg(long)]
        lines: Option<usize>,
        /// Measure and report token efficiency vs. full-context baseline
        #[arg(long)]
        measure_tokens: bool,
    },

    /// Manage Claude Code hooks for automatic memory integration
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
    },

    /// Run a background daemon that continuously ingests new sessions
    Daemon {
        #[command(subcommand)]
        command: DaemonCommand,
    },

    /// Regenerate context.md from existing knowledge files (no re-ingestion)
    Regen {
        /// Project name
        project: String,

        /// LLM provider override (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,

        /// Persist expired entry cleanup to disk (default: filter in-memory only)
        #[arg(long)]
        persist_cleanup: bool,
    },

    /// Add a manual knowledge entry to a project
    Add {
        /// Project name
        project: String,

        /// Knowledge category: decisions, solutions, patterns, bugs, insights, questions, procedures, or preferences
        #[arg(value_parser = ["decisions", "solutions", "patterns", "bugs", "insights", "questions", "procedures", "preferences"])]
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

    /// Bulk-promote all inbox entries to their respective knowledge category files
    Drain {
        /// Project name (defaults to basename of current directory)
        project: Option<String>,

        /// Preview what would be promoted without writing anything
        #[arg(long)]
        dry_run: bool,

        /// Only drain entries of this category (e.g. decisions, bugs)
        #[arg(long)]
        category: Option<String>,
    },

    /// Promote an inbox entry into project/global long-term memory
    Promote {
        /// Project name
        project: String,

        /// Inbox session ID to promote (for example: abc123:decisions)
        session_id: String,

        /// Target category: decisions, solutions, patterns, bugs, insights, questions, procedures, or preferences
        #[arg(value_parser = ["decisions", "solutions", "patterns", "bugs", "insights", "questions", "procedures", "preferences"])]
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

        /// Remove entries older than a duration (e.g. "30d", "6w") that have no TTL
        #[arg(long)]
        stale: Option<String>,

        /// Skip confirmation prompt when used with --stale
        #[arg(long)]
        auto: bool,

        /// Summarize stale entries with LLM instead of deleting (requires --stale)
        #[arg(long)]
        summarize: bool,
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

        /// Only include chunks from this time window (e.g. "7d", "2h", "30m")
        #[arg(long)]
        since: Option<String>,

        /// Only include chunks from this knowledge category (e.g. "decisions", "bugs")
        #[arg(long)]
        category: Option<String>,

        /// Only include chunks whose session_id or text contains this string (e.g. "src/auth")
        #[arg(long)]
        file: Option<String>,
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
        #[arg(value_parser = ["decisions", "solutions", "patterns", "bugs", "insights", "questions", "procedures", "preferences"])]
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

    /// Record a PostToolUse observation from stdin (called by hook script)
    #[command(hide = true)]
    Observe {
        /// Project name (defaults to CWD basename)
        #[arg(long)]
        project: Option<String>,
    },

    /// Git-like versioning for project knowledge (VCS)
    Mem {
        #[command(subcommand)]
        command: MemCommand,
    },

    /// Answer a question using RAG over project knowledge
    Ask {
        /// The question to answer
        query: String,

        /// Project name (defaults to basename of current directory)
        #[arg(long)]
        project: Option<String>,

        /// Maximum knowledge entries to retrieve (default: 12)
        #[arg(long, default_value = "12")]
        top_k: usize,

        /// Minimum similarity threshold for semantic search 0.0–1.0 (default: 0.15)
        #[arg(long, default_value = "0.15")]
        threshold: f32,

        /// LLM provider override (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,

        /// Use graph-augmented retrieval (traverses 2-hop entity neighbors for multi-hop QA)
        #[arg(long)]
        use_graph: bool,

        /// Return a short answer (1-10 words) — better for benchmark evaluation
        #[arg(long)]
        concise: bool,
    },

    /// Show named entity cards for a project (libraries, tools, APIs extracted from sessions)
    Entities {
        /// Project name
        project: String,
    },

    /// Detect and repair issues: hook drift, stale context, missing embeddings
    Heal {
        /// Check only — report issues without fixing
        #[arg(long)]
        check: bool,
    },

    /// Reflect on memory quality: confidence, staleness, coverage, recommendations
    Reflect {
        /// Project name (omit with --all to scan every project)
        project: Option<String>,

        /// Show quality summary for all projects
        #[arg(long)]
        all: bool,
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
        #[arg(long, default_value = "engram knowledge")]
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

    /// Test connectivity to a provider (real API call, ~10 tokens)
    Test {
        /// Provider to test. Tests all configured providers if omitted.
        provider: Option<String>,
    },

    /// Set model override for a provider (saved to auth.json)
    Model {
        /// Provider name (anthropic, openai, ollama, gemini)
        provider: String,
        /// Model name to use for this provider
        model: String,
    },

    /// Set preferred embedding provider (saved to auth.json)
    Embed {
        /// Embedding provider to use
        #[arg(value_parser = ["openai", "gemini", "ollama"])]
        provider: String,
    },

    /// List available models from a provider's /v1/models endpoint
    Models {
        /// Provider to query (openai, ollama, vscode, openrouter).
        /// Defaults to the active provider.
        provider: Option<String>,

        /// List embedding models instead of LLM models
        #[arg(long)]
        embed: bool,
    },

    /// Set embedding model override (saved to auth.json)
    EmbedModel {
        /// Embedding provider (openai, gemini, ollama)
        #[arg(value_parser = ["openai", "gemini", "ollama"])]
        provider: String,
        /// Model name to use for embeddings
        model: String,
    },
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

    /// Publish a pack to Git repository
    Publish {
        /// Pack directory path
        path: String,

        /// Git repository URL (creates if doesn't exist)
        #[arg(long)]
        repo: Option<String>,

        /// Push to remote after commit
        #[arg(long)]
        push: bool,

        /// Commit message
        #[arg(short, long)]
        message: Option<String>,

        /// Skip secret detection (WARNING: use with caution)
        #[arg(long)]
        skip_security: bool,
    },

    /// Validate a pack structure
    Validate {
        /// Pack directory path
        path: String,
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

#[derive(Subcommand)]
pub enum MemCommand {
    /// Initialize VCS for a project
    Init {
        /// Project name (defaults to basename of current directory)
        #[arg(long)]
        project: Option<String>,
    },

    /// Show staged / unstaged sessions
    Status {
        /// Project name (defaults to basename of current directory)
        #[arg(long)]
        project: Option<String>,
    },

    /// Stage specific session IDs for the next commit
    Stage {
        /// Project name
        project: String,

        /// Session IDs to stage
        #[arg(value_name = "SESSION_ID")]
        sessions: Vec<String>,

        /// Stage all new (not-yet-committed) sessions
        #[arg(short = 'a', long)]
        all: bool,
    },

    /// Create a commit from staged (or specified) sessions
    Commit {
        /// Project name
        project: String,

        /// Commit message
        #[arg(short, long)]
        message: String,

        /// Stage and commit all new sessions
        #[arg(short = 'a', long)]
        all: bool,

        /// Commit a specific session ID directly (skips staging)
        #[arg(long)]
        session: Option<String>,
    },

    /// Show commit log
    Log {
        /// Project name
        project: String,

        /// Maximum number of commits to show
        #[arg(long, default_value = "10")]
        limit: usize,

        /// Show full session lists and category hashes
        #[arg(long)]
        verbose: bool,

        /// Filter commits whose message or session IDs match this pattern
        #[arg(long)]
        grep: Option<String>,
    },

    /// Inspect snapshot content for a commit or branch (no checkout)
    Show {
        /// Project name
        project: String,

        /// Branch name or commit hash (default: HEAD)
        target: Option<String>,

        /// Limit to a single category
        #[arg(long)]
        category: Option<String>,
    },

    /// List or manage branches
    Branch {
        /// Project name
        project: String,

        /// Create a new branch from HEAD
        #[arg(short = 'c', long, value_name = "NAME")]
        create: Option<String>,

        /// Delete a branch
        #[arg(short = 'd', long, value_name = "NAME")]
        delete: Option<String>,
    },

    /// Checkout a branch or commit (restores knowledge files)
    Checkout {
        /// Project name
        project: String,

        /// Branch name or commit hash to check out
        target: String,

        /// Preview changes without applying them
        #[arg(long)]
        dry_run: bool,

        /// Force checkout even with uncommitted changes (preserves uncommitted sessions)
        #[arg(long)]
        force: bool,
    },

    /// Show diff between two commits or between HEAD and working state
    Diff {
        /// Project name
        project: String,

        /// From ref (default: HEAD)
        from: Option<String>,

        /// To ref (default: working state)
        to: Option<String>,

        /// Limit diff to a single category
        #[arg(long)]
        category: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum DaemonCommand {
    /// Start the background ingest daemon
    Start {
        /// How often to run ingest, in minutes (1-1440, default: 15)
        #[arg(long, default_value = "15", value_parser = clap::value_parser!(u64).range(1..=1440))]
        interval: u64,

        /// LLM provider override (anthropic, openai, ollama)
        #[arg(long)]
        provider: Option<String>,
    },

    /// Stop the running daemon
    Stop,

    /// Show daemon status
    Status,

    /// Show daemon logs
    Logs {
        /// Number of lines to show (default: 50)
        #[arg(short, long, default_value = "50")]
        lines: usize,

        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,
    },

    /// Run the daemon loop in the foreground (internal — used by start)
    #[command(hide = true)]
    Run {
        /// Polling interval in minutes
        #[arg(long, default_value = "15")]
        interval: u64,

        /// LLM provider override
        #[arg(long)]
        provider: Option<String>,
    },
}
