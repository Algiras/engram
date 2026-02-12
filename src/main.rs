#![allow(dead_code)]
mod analytics;
mod auth;
mod cli;
mod config;
mod diff;
mod embeddings;
mod error;
mod extractor;
mod graph;
mod health;
mod hive;
mod learning;
mod llm;
mod mcp;
mod parser;
mod renderer;
mod state;
mod sync;
mod tui;

use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{
    AuthCommand, Cli, Commands, GraphCommand, HiveCommand, HooksCommand, LearnCommand, PackCommand,
    RegistryCommand, SyncCommand,
};
use colored::Colorize;
use config::Config;
use error::{MemoryError, Result};

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Auth commands operate on auth.json directly — no Config needed
    if let Commands::Auth { command } = cli.command {
        return match command {
            AuthCommand::Login {
                provider,
                set_default,
            } => cmd_auth_login(provider, set_default),
            AuthCommand::List => cmd_auth_list(),
            AuthCommand::Logout { provider } => cmd_auth_logout(&provider),
            AuthCommand::Status => cmd_auth_status(),
        };
    }

    // TUI operates on memory_dir directly — no Config/LLM auth needed
    if matches!(cli.command, Commands::Tui) {
        return cmd_tui();
    }

    // Inject operates on disk only — no Config/LLM auth needed
    if let Commands::Inject { project } = cli.command {
        return cmd_inject(project);
    }

    // Lookup operates on knowledge files — no Config/LLM auth needed
    if let Commands::Lookup {
        project,
        query,
        all,
    } = cli.command
    {
        return cmd_lookup(&project, &query, all);
    }

    // Add operates on knowledge files — no Config/LLM auth needed
    if let Commands::Add {
        project,
        category,
        content,
        label,
        ttl,
    } = cli.command
    {
        return cmd_add(&project, &category, &content, &label, ttl.as_deref());
    }

    // Review operates on knowledge files — no Config/LLM auth needed
    if let Commands::Review { project, all } = cli.command {
        return cmd_review(&project, all);
    }

    // Promote operates on knowledge files — no Config/LLM auth needed
    if let Commands::Promote {
        project,
        session_id,
        category,
        global,
        label,
        ttl,
    } = cli.command
    {
        return cmd_promote(
            &project,
            &session_id,
            &category,
            global,
            &label,
            ttl.as_deref(),
        );
    }

    // Forget operates on knowledge files — no Config/LLM auth needed
    if let Commands::Forget {
        project,
        session_id,
        topic,
        all,
        purge,
        expired,
    } = cli.command
    {
        return cmd_forget(&project, session_id, topic, all, purge, expired);
    }

    // Hooks operate on settings files — no Config/LLM auth needed
    if let Commands::Hooks { command } = cli.command {
        return match command {
            HooksCommand::Install => cmd_hooks_install(),
            HooksCommand::Uninstall => cmd_hooks_uninstall(),
            HooksCommand::Status => cmd_hooks_status(),
        };
    }

    // Hive operations - distributed knowledge sharing (no Config/LLM auth needed)
    if let Commands::Hive { command } = cli.command {
        return cmd_hive(command);
    }

    // Extract provider override for commands that support it
    let provider_override = match &cli.command {
        Commands::Ingest { provider, .. }
        | Commands::Regen { provider, .. }
        | Commands::Mcp { provider, .. }
        | Commands::Graph {
            command: GraphCommand::Build { provider, .. },
        } => provider.as_deref(),
        _ => None,
    };

    let config = Config::load(provider_override)?;

    // MCP server command
    if let Commands::Mcp { .. } = cli.command {
        return cmd_mcp(&config);
    }

    // Export command
    if let Commands::Export {
        project,
        format,
        output,
        include_conversations,
    } = cli.command
    {
        return cmd_export(
            &config,
            &project,
            &format,
            output.as_deref(),
            include_conversations,
        );
    }

    // Sync command
    if let Commands::Sync { command } = cli.command {
        return match command {
            SyncCommand::Push {
                project,
                gist_id,
                description,
            } => cmd_sync_push(&config, &project, gist_id.as_deref(), &description),
            SyncCommand::Pull {
                project,
                gist_id,
                force,
            } => cmd_sync_pull(&config, &project, &gist_id, force),
            SyncCommand::List { project } => cmd_sync_list(&config, &project),
            SyncCommand::Clone { gist_id, project } => cmd_sync_clone(&config, &gist_id, &project),
            SyncCommand::History { gist_id, version } => {
                cmd_sync_history(&gist_id, version.as_deref())
            }
            SyncCommand::PushRepo {
                project,
                repo,
                message,
                push_remote,
            } => cmd_sync_push_repo(&config, &project, &repo, message.as_deref(), push_remote),
            SyncCommand::PullRepo {
                project,
                repo,
                fetch_remote,
                branch,
            } => cmd_sync_pull_repo(&config, &project, &repo, fetch_remote, &branch),
            SyncCommand::InitRepo { repo } => cmd_sync_init_repo(&repo),
        };
    }

    // Graph command
    if let Commands::Graph { command } = cli.command {
        return match command {
            GraphCommand::Build { project, .. } => cmd_graph_build(&config, &project),
            GraphCommand::Query {
                project,
                concept,
                depth,
            } => cmd_graph_query(&config, &project, &concept, depth),
            GraphCommand::Viz {
                project,
                format,
                output,
                root,
            } => cmd_graph_viz(
                &config,
                &project,
                &format,
                output.as_deref(),
                root.as_deref(),
            ),
            GraphCommand::Path { project, from, to } => {
                cmd_graph_path(&config, &project, &from, &to)
            }
            GraphCommand::Hubs { project, top } => cmd_graph_hubs(&config, &project, top),
        };
    }

    // Embed command
    if let Commands::Embed { project, provider } = &cli.command {
        return cmd_embed(&config, project, provider.as_deref());
    }

    // SearchSemantic command
    if let Commands::SearchSemantic {
        query,
        project,
        top,
        threshold,
    } = &cli.command
    {
        return cmd_search_semantic(&config, query, project.as_deref(), *top, *threshold);
    }

    // Consolidate command
    if let Commands::Consolidate {
        project,
        threshold,
        auto_merge,
        find_contradictions,
    } = &cli.command
    {
        return cmd_consolidate(
            &config,
            project,
            *threshold,
            *auto_merge,
            *find_contradictions,
        );
    }

    // Doctor command (no Config needed for basic checks)
    if let Commands::Doctor {
        project,
        fix,
        verbose,
    } = &cli.command
    {
        return cmd_doctor(&config, project.as_deref(), *fix, *verbose);
    }

    // Analytics command (no Config needed for reading usage data)
    if let Commands::Analytics {
        project,
        days,
        detailed,
        clear_old,
    } = &cli.command
    {
        return cmd_analytics(project.as_deref(), *days, *detailed, *clear_old);
    }

    // Diff command
    if let Commands::Diff {
        project,
        category,
        version,
        history,
    } = &cli.command
    {
        return cmd_diff(&config, project, category, version.as_deref(), *history);
    }

    // Learn command
    if let Commands::Learn { command } = cli.command {
        return match command {
            LearnCommand::Dashboard { project } => cmd_learn_dashboard(&config, project.as_deref()),
            LearnCommand::Optimize {
                project,
                dry_run,
                auto,
            } => cmd_learn_optimize(&config, &project, dry_run, auto),
            LearnCommand::Reset { project } => cmd_learn_reset(&config, &project),
            LearnCommand::Simulate {
                project,
                sessions,
                pattern,
            } => cmd_learn_simulate(&config, &project, sessions, &pattern),
            LearnCommand::Feedback {
                project,
                session,
                helpful,
                unhelpful,
                comment,
            } => cmd_learn_feedback(
                &config,
                &project,
                session.as_deref(),
                helpful,
                unhelpful,
                comment.as_deref(),
            ),
        };
    }

    match cli.command {
        Commands::Ingest {
            force,
            dry_run,
            project,
            since,
            skip_knowledge,
            ttl,
            ..
        } => {
            cmd_ingest(&config, force, dry_run, project, since, skip_knowledge, ttl)?;
        }
        Commands::Search {
            query,
            project,
            knowledge,
            context,
        } => {
            cmd_search(&config, &query, project, knowledge, context)?;
        }
        Commands::Recall { project } => {
            cmd_recall(&config, &project)?;
        }
        Commands::Context { project } => {
            cmd_context(&config, &project)?;
        }
        Commands::Status => {
            cmd_status(&config)?;
        }
        Commands::Projects => {
            cmd_projects(&config)?;
        }
        Commands::Regen { project, .. } => {
            cmd_regen(&config, &project)?;
        }
        Commands::Auth { .. }
        | Commands::Tui
        | Commands::Inject { .. }
        | Commands::Hooks { .. }
        | Commands::Forget { .. }
        | Commands::Lookup { .. }
        | Commands::Add { .. }
        | Commands::Review { .. }
        | Commands::Promote { .. }
        | Commands::Mcp { .. }
        | Commands::Export { .. }
        | Commands::Sync { .. }
        | Commands::Graph { .. }
        | Commands::Embed { .. }
        | Commands::SearchSemantic { .. }
        | Commands::Consolidate { .. }
        | Commands::Doctor { .. }
        | Commands::Analytics { .. }
        | Commands::Diff { .. }
        | Commands::Learn { .. }
        | Commands::Hive { .. } => {
            unreachable!()
        }
    }

    Ok(())
}

// ── TUI command ─────────────────────────────────────────────────────────

fn cmd_tui() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");

    if !memory_dir.exists() {
        eprintln!("No memory directory found. Run 'ingest' first.");
        return Ok(());
    }

    tui::run_tui(memory_dir).map_err(error::MemoryError::Io)
}

// ── Inject command ──────────────────────────────────────────────────────

fn cmd_inject(project: Option<String>) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    // Determine project name
    let project_name = match project {
        Some(name) => name,
        None => std::env::current_dir()
            .ok()
            .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
            .ok_or_else(|| {
                error::MemoryError::Config(
                    "Could not determine project name from current directory".into(),
                )
            })?,
    };

    let memory_dir = home.join("memory");
    let knowledge_dir = memory_dir.join("knowledge");

    // Read project context (with fallback to raw knowledge files)
    let context_path = knowledge_dir.join(&project_name).join("context.md");
    let context_content = if context_path.exists() {
        std::fs::read_to_string(&context_path)?
    } else {
        match build_raw_context(&project_name, &knowledge_dir.join(&project_name)) {
            Some(raw) => raw,
            None => {
                eprintln!(
                    "{} No knowledge found for '{}'. Run 'claude-memory ingest' first.",
                    "Not found:".yellow(),
                    project_name
                );
                return Ok(());
            }
        }
    };

    // Read global preferences (optional), filtering expired entries
    let preferences_path = knowledge_dir.join("_global").join("preferences.md");
    let preferences_content = if preferences_path.exists() {
        use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};
        let raw = std::fs::read_to_string(&preferences_path)?;
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _expired) = partition_by_expiry(blocks);
        Some(reconstruct_blocks(&preamble, &active))
    } else {
        None
    };

    // Read global shared memory (optional), filtering expired entries
    let shared_path = knowledge_dir.join("_global").join("shared.md");
    let shared_content = if shared_path.exists() {
        use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};
        let raw = std::fs::read_to_string(&shared_path)?;
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _expired) = partition_by_expiry(blocks);
        Some(reconstruct_blocks(&preamble, &active))
    } else {
        None
    };

    // Find matching Claude Code project directory
    let claude_projects_dir = home.join(".claude").join("projects");
    let project_dir = find_claude_project_dir(&claude_projects_dir, &project_name)?;

    let Some(project_dir) = project_dir else {
        eprintln!(
            "{} No matching Claude Code project directory found for '{}'.",
            "Not found:".yellow(),
            project_name
        );
        return Ok(());
    };

    // Build combined content
    let mut combined = String::new();
    combined.push_str("# Project Memory (auto-injected by claude-memory)\n\n");
    combined.push_str(
        "<!-- This file is auto-generated. Edit knowledge sources, not this file. -->\n\n",
    );

    if let Some(prefs) = &preferences_content {
        combined.push_str("## Global Preferences\n\n");
        combined.push_str(prefs);
        combined.push_str("\n\n---\n\n");
    }

    if let Some(shared) = &shared_content {
        combined.push_str("## Global Shared Memory\n\n");
        combined.push_str(shared);
        combined.push_str("\n\n---\n\n");
    }

    combined.push_str(&format!("## Project: {}\n\n", project_name));
    combined.push_str(&context_content);

    // Include installed pack knowledge
    let pack_content = get_installed_pack_knowledge(&memory_dir)?;
    if !pack_content.is_empty() {
        combined.push_str("\n\n---\n\n## Installed Pack Knowledge\n\n");
        combined.push_str(&pack_content);
    }

    // Write to MEMORY.md
    let memory_path = project_dir.join("memory");
    std::fs::create_dir_all(&memory_path)?;
    let memory_file = memory_path.join("MEMORY.md");
    std::fs::write(&memory_file, &combined)?;

    println!(
        "{} Injected knowledge for '{}' into {}",
        "Done!".green().bold(),
        project_name,
        memory_file.display()
    );

    Ok(())
}

/// Scan ~/.claude/projects/ and find the directory matching a project name.
fn find_claude_project_dir(
    claude_projects_dir: &Path,
    project_name: &str,
) -> Result<Option<PathBuf>> {
    if !claude_projects_dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(claude_projects_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let dir_name = entry.file_name().to_string_lossy().to_string();
        let decoded = parser::discovery::decode_project_name(&dir_name);
        if decoded == project_name {
            return Ok(Some(entry.path()));
        }
    }

    Ok(None)
}

/// Build a lightweight context string from raw knowledge files (no LLM).
/// Used as fallback when context.md doesn't exist but knowledge files do.
/// Returns None if no knowledge files exist or all are empty/expired.
fn build_raw_context(project: &str, project_knowledge_dir: &Path) -> Option<String> {
    use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};

    let read_and_filter = |path: &Path| -> String {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let decisions = read_and_filter(&project_knowledge_dir.join("decisions.md"));
    let solutions = read_and_filter(&project_knowledge_dir.join("solutions.md"));
    let patterns = read_and_filter(&project_knowledge_dir.join("patterns.md"));

    if decisions.trim().is_empty() && solutions.trim().is_empty() && patterns.trim().is_empty() {
        return None;
    }

    let mut out = format!("# {} - Project Context (raw, not synthesized)\n\n", project);

    if !decisions.trim().is_empty() {
        out.push_str(&decisions);
        out.push_str("\n\n");
    }
    if !solutions.trim().is_empty() {
        out.push_str(&solutions);
        out.push_str("\n\n");
    }
    if !patterns.trim().is_empty() {
        out.push_str(&patterns);
        out.push_str("\n\n");
    }

    Some(out)
}

// ── Hooks commands ──────────────────────────────────────────────────────

const HOOK_SCRIPT: &str = include_str!("../hooks/claude-memory-hook.sh");
const INJECT_SCRIPT: &str = include_str!("../hooks/inject-context.sh");
const SESSION_END_SCRIPT: &str = include_str!("../hooks/session-end-hook.sh");

fn cmd_hooks_install() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let hooks_dir = home.join(".claude").join("hooks");
    std::fs::create_dir_all(&hooks_dir)?;

    // Write hook scripts
    let hook_path = hooks_dir.join("claude-memory-hook.sh");
    std::fs::write(&hook_path, HOOK_SCRIPT)?;
    set_executable(&hook_path)?;

    let inject_path = hooks_dir.join("inject-context.sh");
    std::fs::write(&inject_path, INJECT_SCRIPT)?;
    set_executable(&inject_path)?;

    let session_end_path = hooks_dir.join("session-end-hook.sh");
    std::fs::write(&session_end_path, SESSION_END_SCRIPT)?;
    set_executable(&session_end_path)?;

    // Update settings.json
    let settings_path = home.join(".claude").join("settings.json");
    let mut settings: serde_json::Value = if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        serde_json::from_str(&content)?
    } else {
        serde_json::json!({})
    };

    let hooks = settings
        .as_object_mut()
        .ok_or_else(|| error::MemoryError::Config("settings.json is not an object".into()))?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}));

    // Add SessionStart hook for inject
    add_hook_entry(hooks, "SessionStart", &inject_path.to_string_lossy())?;

    // Add PostToolUse hook for auto-ingest
    add_hook_entry(hooks, "PostToolUse", &hook_path.to_string_lossy())?;

    // Add SessionEnd hook for full knowledge extraction
    add_hook_entry(hooks, "Stop", &session_end_path.to_string_lossy())?;

    std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;

    println!("{} Hooks installed:", "Done!".green().bold());
    println!("  {} -> {}", "SessionStart".cyan(), inject_path.display());
    println!("  {} -> {}", "PostToolUse".cyan(), hook_path.display());
    println!("  {} -> {}", "Stop".cyan(), session_end_path.display());
    println!(
        "\n  Settings updated: {}",
        settings_path.display().to_string().dimmed()
    );

    Ok(())
}

fn cmd_hooks_uninstall() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let hooks_dir = home.join(".claude").join("hooks");

    // Delete hook scripts
    let hook_path = hooks_dir.join("claude-memory-hook.sh");
    let inject_path = hooks_dir.join("inject-context.sh");
    let session_end_path = hooks_dir.join("session-end-hook.sh");

    let mut removed = Vec::new();
    if hook_path.exists() {
        std::fs::remove_file(&hook_path)?;
        removed.push("claude-memory-hook.sh");
    }
    if inject_path.exists() {
        std::fs::remove_file(&inject_path)?;
        removed.push("inject-context.sh");
    }
    if session_end_path.exists() {
        std::fs::remove_file(&session_end_path)?;
        removed.push("session-end-hook.sh");
    }

    // Update settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        let mut settings: serde_json::Value = serde_json::from_str(&content)?;

        if let Some(hooks) = settings.get_mut("hooks").and_then(|h| h.as_object_mut()) {
            for (_key, entries) in hooks.iter_mut() {
                if let Some(arr) = entries.as_array_mut() {
                    arr.retain(|entry| {
                        // Check nested hooks array for claude-memory commands
                        let entry_str = serde_json::to_string(entry).unwrap_or_default();
                        !entry_str.contains("claude-memory")
                    });
                }
            }
        }

        std::fs::write(&settings_path, serde_json::to_string_pretty(&settings)?)?;
    }

    if removed.is_empty() {
        println!("{} No hooks were installed.", "Note:".yellow());
    } else {
        println!("{} Hooks uninstalled:", "Done!".green().bold());
        for name in &removed {
            println!("  Removed {}", name);
        }
        println!("  Settings updated.");
    }

    Ok(())
}

fn cmd_hooks_status() -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;

    let hooks_dir = home.join(".claude").join("hooks");
    let hook_path = hooks_dir.join("claude-memory-hook.sh");
    let inject_path = hooks_dir.join("inject-context.sh");
    let session_end_path = hooks_dir.join("session-end-hook.sh");

    println!("{}", "Claude Memory Hooks Status".green().bold());
    println!("{}", "=".repeat(50));

    let check = |path: &Path, name: &str, event: &str| {
        if path.exists() {
            println!("  {} {} ({})", "installed".green(), name, event.cyan());
        } else {
            println!(
                "  {} {} ({})",
                "not installed".yellow(),
                name,
                event.dimmed()
            );
        }
    };

    check(&inject_path, "inject-context.sh", "SessionStart");
    check(&hook_path, "claude-memory-hook.sh", "PostToolUse");
    check(&session_end_path, "session-end-hook.sh", "Stop");

    // Check settings.json
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path)?;
        let has_hooks = content.contains("claude-memory");
        if has_hooks {
            println!(
                "\n  Settings: {} entries found in {}",
                "claude-memory".cyan(),
                settings_path.display().to_string().dimmed()
            );
        } else {
            println!(
                "\n  Settings: {} in {}",
                "no claude-memory entries".yellow(),
                settings_path.display().to_string().dimmed()
            );
        }
    } else {
        println!(
            "\n  Settings: {}",
            "~/.claude/settings.json not found".yellow()
        );
    }

    Ok(())
}

/// Add a hook entry to a hook event array in settings.json, idempotently.
fn add_hook_entry(hooks: &mut serde_json::Value, event: &str, command: &str) -> Result<()> {
    let event_hooks = hooks
        .as_object_mut()
        .ok_or_else(|| error::MemoryError::Config("hooks is not an object".into()))?
        .entry(event)
        .or_insert_with(|| serde_json::json!([]));

    let arr = event_hooks
        .as_array_mut()
        .ok_or_else(|| error::MemoryError::Config(format!("hooks.{} is not an array", event)))?;

    // Check if already installed (look for "claude-memory" in any entry's command)
    let already_installed = arr.iter().any(|entry| {
        let entry_str = serde_json::to_string(entry).unwrap_or_default();
        entry_str.contains("claude-memory")
    });

    if !already_installed {
        arr.push(serde_json::json!({
            "matcher": "",
            "hooks": [{
                "type": "command",
                "command": command
            }]
        }));
    }

    Ok(())
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<()> {
    Ok(())
}

// ── Auth commands ───────────────────────────────────────────────────────

fn cmd_auth_login(provider_name: Option<String>, set_default: bool) -> Result<()> {
    use auth::providers::Provider;
    use dialoguer::{Password, Select};

    let provider = if let Some(name) = provider_name {
        Provider::from_str_loose(&name).ok_or_else(|| {
            error::MemoryError::Auth(format!(
                "Unknown provider: {}. Use: anthropic, openai, ollama",
                name
            ))
        })?
    } else {
        // Interactive selection
        let items: Vec<&str> = Provider::all().iter().map(|p| p.display_name()).collect();
        let selection = Select::new()
            .with_prompt("Select LLM provider")
            .items(&items)
            .default(0)
            .interact()
            .map_err(|e| error::MemoryError::Auth(format!("Selection cancelled: {}", e)))?;
        Provider::all()[selection]
    };

    if !provider.requires_auth() {
        println!(
            "{} {} does not require authentication.",
            "Note:".cyan(),
            provider.display_name()
        );
        // Still allow setting as default
        if set_default {
            let mut store = auth::AuthStore::load()?;
            store.default_provider = Some(provider.to_string());
            store.save()?;
            println!(
                "{} Set {} as default provider.",
                "Done!".green().bold(),
                provider.display_name()
            );
        }
        return Ok(());
    }

    // Prompt for API key
    let key = Password::new()
        .with_prompt(format!("Enter {} API key", provider.display_name()))
        .interact()
        .map_err(|e| error::MemoryError::Auth(format!("Input cancelled: {}", e)))?;

    if key.trim().is_empty() {
        return Err(error::MemoryError::Auth("API key cannot be empty".into()));
    }

    let mut store = auth::AuthStore::load()?;
    store.set(
        provider,
        auth::ProviderCredential {
            cred_type: "api".to_string(),
            key,
            endpoint: None,
            model: None,
        },
    );

    if set_default {
        store.default_provider = Some(provider.to_string());
    } else if store.default_provider.is_none() {
        // Auto-set as default if no default exists
        store.default_provider = Some(provider.to_string());
    }

    store.save()?;

    println!(
        "{} Logged in to {}.",
        "Done!".green().bold(),
        provider.display_name()
    );
    if store.default_provider.as_deref() == Some(&provider.to_string()) {
        println!("  Set as default provider.");
    }

    Ok(())
}

fn cmd_auth_list() -> Result<()> {
    use auth::providers::Provider;

    let store = auth::AuthStore::load()?;

    println!("{}", "Configured Providers".green().bold());
    println!("{}", "=".repeat(50));

    for &provider in Provider::all() {
        let env_key = if !provider.env_var_name().is_empty() {
            std::env::var(provider.env_var_name()).ok()
        } else {
            None
        };

        let stored = store.get(provider);
        let is_default = store.default_provider.as_deref() == Some(&provider.to_string());

        let status = if env_key.is_some() {
            "env var".green().to_string()
        } else if stored.is_some() {
            "auth.json".cyan().to_string()
        } else if !provider.requires_auth() {
            "no auth needed".dimmed().to_string()
        } else {
            "not configured".dimmed().to_string()
        };

        let default_marker = if is_default { " (default)" } else { "" };

        println!(
            "  {}{}\t{}",
            provider.display_name().cyan().bold(),
            default_marker,
            status
        );
    }

    Ok(())
}

fn cmd_auth_logout(provider_name: &str) -> Result<()> {
    use auth::providers::Provider;

    let provider = Provider::from_str_loose(provider_name).ok_or_else(|| {
        error::MemoryError::Auth(format!(
            "Unknown provider: {}. Use: anthropic, openai, ollama",
            provider_name
        ))
    })?;

    let mut store = auth::AuthStore::load()?;
    store.remove(provider);
    store.save()?;

    println!(
        "{} Removed credentials for {}.",
        "Done!".green().bold(),
        provider.display_name()
    );

    Ok(())
}

fn cmd_auth_status() -> Result<()> {
    // Use the same resolution logic as Config
    let env_endpoint = std::env::var("CLAUDE_MEMORY_LLM_ENDPOINT").ok();
    let env_model = std::env::var("CLAUDE_MEMORY_LLM_MODEL").ok();

    match auth::resolve_provider(None, env_endpoint, env_model) {
        Ok(resolved) => {
            println!("{}", "Active LLM Provider".green().bold());
            println!("{}", "=".repeat(40));
            println!("  Provider:  {}", resolved.provider.display_name().cyan());
            println!("  Model:     {}", resolved.model);
            println!("  Endpoint:  {}", resolved.endpoint);

            if let Some(ref key) = resolved.api_key {
                let masked = if key.len() > 8 {
                    format!("{}...{}", &key[..4], &key[key.len() - 4..])
                } else {
                    "****".to_string()
                };
                println!("  API Key:   {}", masked);

                // Show source
                let env_var = resolved.provider.env_var_name();
                if !env_var.is_empty() && std::env::var(env_var).is_ok() {
                    println!("  Source:    {} (env var)", env_var);
                } else {
                    println!("  Source:    auth.json");
                }
            }
        }
        Err(_) => {
            println!(
                "{} No provider configured. Using {} as fallback.",
                "Note:".yellow(),
                "Ollama (local)".cyan()
            );
            println!("  Run 'claude-memory auth login' to configure a provider.");
        }
    }

    Ok(())
}

// ── Review command ──────────────────────────────────────────────────────

fn cmd_review(project: &str, show_all: bool) -> Result<()> {
    use extractor::knowledge::{parse_session_blocks, partition_by_expiry};

    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;
    let inbox_path = home
        .join("memory")
        .join("knowledge")
        .join(project)
        .join("inbox.md");

    if !inbox_path.exists() {
        println!(
            "{} No inbox entries for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&inbox_path)?;
    let (_preamble, blocks) = parse_session_blocks(&content);

    if blocks.is_empty() {
        println!(
            "{} No inbox entries for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let (mut active, expired) = partition_by_expiry(blocks);
    let expired_ids: std::collections::HashSet<String> =
        expired.iter().map(|b| b.session_id.clone()).collect();
    if show_all {
        active.extend(expired);
    }
    let entries = active;

    if entries.is_empty() {
        println!(
            "{} No active inbox entries for '{}'. Use --all to include expired.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    println!("{} Inbox for '{}':\n", "Review".green().bold(), project);

    for block in &entries {
        let expired_tag = if block.ttl.is_some() && expired_ids.contains(&block.session_id) {
            " [EXPIRED]".red().to_string()
        } else {
            String::new()
        };
        let ttl_text = block
            .ttl
            .as_deref()
            .map(|t| format!(" ttl:{}", t).dimmed().to_string())
            .unwrap_or_default();

        println!(
            "  {} {} ({}){}{}",
            ">".green(),
            block.session_id.cyan(),
            block.timestamp.dimmed(),
            ttl_text,
            expired_tag
        );
        println!("    {}", block.preview);

        if show_all {
            for line in block.content.lines() {
                if !line.trim().is_empty() {
                    println!("    {}", line);
                }
            }
        }
        println!();
    }

    println!(
        "  Promote with: {}",
        format!(
            "claude-memory promote {} <session_id> <category> [--global]",
            project
        )
        .cyan()
    );

    Ok(())
}

// ── Promote command ─────────────────────────────────────────────────────

fn cmd_promote(
    project: &str,
    session_id: &str,
    category: &str,
    global: bool,
    label: &str,
    ttl: Option<&str>,
) -> Result<()> {
    use extractor::knowledge::{parse_session_blocks, parse_ttl, reconstruct_blocks};

    if let Some(ttl_val) = ttl {
        if parse_ttl(ttl_val).is_none() {
            return Err(error::MemoryError::InvalidDuration(format!(
                "Invalid TTL: '{}'. Use format like 30m, 2h, 7d, 2w",
                ttl_val
            )));
        }
    }

    if !global && category == "preferences" {
        return Err(error::MemoryError::Config(
            "preferences can only be promoted with --global".into(),
        ));
    }

    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");
    let project_dir = memory_dir.join("knowledge").join(project);
    let inbox_path = project_dir.join("inbox.md");

    if !inbox_path.exists() {
        println!(
            "{} No inbox found for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let inbox_content = std::fs::read_to_string(&inbox_path)?;
    let (preamble, blocks) = parse_session_blocks(&inbox_content);

    let selected = blocks.iter().find(|b| b.session_id == session_id);
    let Some(selected) = selected else {
        println!(
            "{} Session '{}' not found in inbox for '{}'.",
            "Not found:".yellow(),
            session_id,
            project
        );
        return Ok(());
    };

    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string();
    let promoted_id = format!(
        "{}:{}",
        sanitize_session_id(label),
        sanitize_session_id(session_id)
    );
    let header = session_header(&promoted_id, &now, ttl);

    let (target_dir, target_file, target_title) = if global {
        (
            memory_dir.join("knowledge").join("_global"),
            if category == "preferences" {
                "preferences.md"
            } else {
                "shared.md"
            },
            if category == "preferences" {
                "Preferences"
            } else {
                "Shared"
            },
        )
    } else {
        (
            project_dir.clone(),
            match category {
                "decisions" => "decisions.md",
                "solutions" => "solutions.md",
                "patterns" => "patterns.md",
                _ => unreachable!(),
            },
            match category {
                "decisions" => "Decisions",
                "solutions" => "Solutions",
                "patterns" => "Patterns",
                _ => unreachable!(),
            },
        )
    };

    std::fs::create_dir_all(&target_dir)?;
    let target_path = target_dir.join(target_file);
    init_knowledge_file(&target_path, target_title)?;

    append_session_entry(&target_path, &header, selected.content.trim())?;

    // Remove promoted entry from inbox
    let remaining: Vec<_> = blocks
        .into_iter()
        .filter(|b| b.session_id != session_id)
        .collect();
    let rebuilt_inbox = reconstruct_blocks(&preamble, &remaining);
    std::fs::write(&inbox_path, rebuilt_inbox)?;

    if !global {
        let context_path = project_dir.join("context.md");
        if context_path.exists() {
            std::fs::remove_file(context_path)?;
        }
    }

    println!(
        "{} Promoted '{}' to {}/{}.",
        "Done!".green().bold(),
        session_id,
        if global { "_global" } else { project },
        target_file
    );

    if !global {
        println!(
            "  Run '{}' to regenerate context.",
            format!("claude-memory regen {}", project).cyan()
        );
    }

    Ok(())
}

fn sanitize_session_id(s: &str) -> String {
    let out: String = s
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.') {
                c
            } else {
                '-'
            }
        })
        .collect();
    out.trim_matches('-').to_string()
}

fn session_header(session_id: &str, timestamp: &str, ttl: Option<&str>) -> String {
    if let Some(ttl_val) = ttl {
        format!(
            "\n\n## Session: {} ({}) [ttl:{}]\n\n",
            session_id, timestamp, ttl_val
        )
    } else {
        format!("\n\n## Session: {} ({})\n\n", session_id, timestamp)
    }
}

fn init_knowledge_file(path: &Path, title: &str) -> Result<()> {
    if !path.exists() {
        std::fs::write(path, format!("# {}\n", title))?;
    }
    Ok(())
}

fn append_session_entry(path: &Path, header: &str, content: &str) -> Result<()> {
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
    writeln!(file, "{}{}", header, content)?;
    Ok(())
}

// ── Lookup command ──────────────────────────────────────────────────────

fn cmd_lookup(project: &str, query: &str, include_all: bool) -> Result<()> {
    use extractor::knowledge::{is_expired, parse_session_blocks};

    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");
    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let global_prefs = memory_dir
        .join("knowledge")
        .join("_global")
        .join("preferences.md");
    let global_shared = memory_dir
        .join("knowledge")
        .join("_global")
        .join("shared.md");

    if !knowledge_dir.exists() {
        eprintln!(
            "{} No knowledge found for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let query_lower = query.to_lowercase();
    let mut found = false;

    let files: Vec<(&str, std::path::PathBuf)> = vec![
        ("decisions", knowledge_dir.join("decisions.md")),
        ("solutions", knowledge_dir.join("solutions.md")),
        ("patterns", knowledge_dir.join("patterns.md")),
        ("preferences", global_prefs),
        ("shared", global_shared),
    ];

    for (category, path) in &files {
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(path)?;
        let (_preamble, blocks) = parse_session_blocks(&content);

        for block in &blocks {
            let expired = is_expired(block);

            // Skip expired entries unless --all is passed
            if expired && !include_all {
                continue;
            }

            if block.content.to_lowercase().contains(&query_lower)
                || block.header.to_lowercase().contains(&query_lower)
            {
                if !found {
                    println!(
                        "{} Results for '{}' in '{}':\n",
                        "Lookup".green().bold(),
                        query,
                        project
                    );
                }
                found = true;

                let expired_tag = if expired {
                    " [EXPIRED]".red().to_string()
                } else {
                    String::new()
                };
                println!(
                    "  {} [{}] {} ({}){}",
                    ">".green(),
                    category.cyan(),
                    block.session_id,
                    block.timestamp.dimmed(),
                    expired_tag
                );
                // Print matching lines from content (up to 5)
                let mut match_count = 0;
                for line in block.content.lines() {
                    if line.to_lowercase().contains(&query_lower) && !line.trim().is_empty() {
                        println!("    {}", line.trim());
                        match_count += 1;
                        if match_count >= 5 {
                            println!("    {}", "...".dimmed());
                            break;
                        }
                    }
                }
                println!();
            }
        }
    }

    // Also search installed packs
    let installer = hive::PackInstaller::new(&memory_dir);
    if let Ok(knowledge_dirs) = installer.get_installed_knowledge_dirs() {
        for (pack_name, knowledge_dir) in knowledge_dirs {
            for category in &[
                "patterns.md",
                "solutions.md",
                "decisions.md",
                "preferences.md",
            ] {
                let file_path = knowledge_dir.join(category);
                if !file_path.exists() {
                    continue;
                }
                let content = match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(_) => continue,
                };

                let (_preamble, blocks) = parse_session_blocks(&content);
                for block in &blocks {
                    if block.content.to_lowercase().contains(&query_lower)
                        || block.header.to_lowercase().contains(&query_lower)
                    {
                        if !found {
                            println!(
                                "{} Results for '{}' in '{}':\n",
                                "Lookup".green().bold(),
                                query,
                                project
                            );
                        }
                        found = true;
                        println!(
                            "  {} [{}] {} (pack: {})",
                            ">".green(),
                            category.replace(".md", "").cyan(),
                            block.session_id,
                            pack_name.blue()
                        );
                        let mut match_count = 0;
                        for line in block.content.lines() {
                            if line.to_lowercase().contains(&query_lower) && !line.trim().is_empty()
                            {
                                println!("    {}", line.trim());
                                match_count += 1;
                                if match_count >= 5 {
                                    println!("    {}", "...".dimmed());
                                    break;
                                }
                            }
                        }
                        println!();
                    }
                }
            }
        }
    }

    if !found {
        println!(
            "{} No knowledge matching '{}' in '{}'.",
            "Not found:".yellow(),
            query,
            project
        );
    }

    Ok(())
}

// ── Add command ─────────────────────────────────────────────────────────

fn cmd_add(
    project: &str,
    category: &str,
    content: &str,
    label: &str,
    ttl: Option<&str>,
) -> Result<()> {
    use extractor::knowledge::parse_ttl;

    // Validate TTL format early
    if let Some(ttl_val) = ttl {
        if parse_ttl(ttl_val).is_none() {
            return Err(error::MemoryError::InvalidDuration(format!(
                "Invalid TTL: '{}'. Use format like 30m, 2h, 7d, 2w",
                ttl_val
            )));
        }
    }

    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");

    let (dir, filename) = if category == "preferences" {
        (
            memory_dir.join("knowledge").join("_global"),
            "preferences.md",
        )
    } else {
        (
            memory_dir.join("knowledge").join(project),
            match category {
                "decisions" => "decisions.md",
                "solutions" => "solutions.md",
                "patterns" => "patterns.md",
                _ => unreachable!(),
            },
        )
    };

    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);

    // Initialize file if needed
    if !path.exists() {
        let title = category.chars().next().unwrap().to_uppercase().to_string() + &category[1..];
        std::fs::write(&path, format!("# {}\n", title))?;
    }

    // Build header with timestamp and label
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let header = if let Some(ttl_val) = ttl {
        format!("\n\n## Session: {} ({}) [ttl:{}]\n\n", label, now, ttl_val)
    } else {
        format!("\n\n## Session: {} ({})\n\n", label, now)
    };

    // Append
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().append(true).open(&path)?;
    writeln!(file, "{}{}", header, content)?;

    // Delete stale context.md (manual entries change the knowledge base)
    let context_path = memory_dir
        .join("knowledge")
        .join(project)
        .join("context.md");
    if context_path.exists() {
        std::fs::remove_file(&context_path)?;
    }

    println!(
        "{} Added to {}/{} for '{}'.",
        "Done!".green().bold(),
        category,
        filename,
        project
    );
    println!(
        "  Run '{}' to update context.",
        format!("claude-memory regen {}", project).cyan()
    );

    Ok(())
}

// ── Regen command ───────────────────────────────────────────────────────

fn cmd_regen(config: &Config, project: &str) -> Result<()> {
    use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let summary_dir = config.memory_dir.join("summaries").join(project);

    if !knowledge_dir.exists() {
        eprintln!(
            "{} No knowledge found for '{}'. Run 'ingest' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    // Read existing knowledge files and filter out expired blocks
    let filter_expired = |content: &str| -> String {
        let (preamble, blocks) = parse_session_blocks(content);
        let (active, _expired) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let decisions_raw = read_or_empty(&knowledge_dir.join("decisions.md"));
    let solutions_raw = read_or_empty(&knowledge_dir.join("solutions.md"));
    let patterns_raw = read_or_empty(&knowledge_dir.join("patterns.md"));

    let decisions = filter_expired(&decisions_raw);
    let solutions = filter_expired(&solutions_raw);
    let patterns = filter_expired(&patterns_raw);
    let summaries = collect_summary_dir(&summary_dir)?;

    if decisions.is_empty() && solutions.is_empty() && patterns.is_empty() {
        eprintln!(
            "{} Knowledge files are empty for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    println!(
        "{} Regenerating context for '{}'...",
        "Regen".green().bold(),
        project
    );

    let client = llm::client::LlmClient::new(&config.llm);
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| error::MemoryError::Config(format!("tokio runtime: {}", e)))?;

    let context = rt.block_on(async {
        client
            .chat(
                llm::prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
                &llm::prompts::context_prompt(
                    project, &decisions, &solutions, &patterns, &summaries,
                ),
            )
            .await
    })?;

    let context_with_header = format!("# {} - Project Context\n\n{}\n", project, context);
    std::fs::write(knowledge_dir.join("context.md"), &context_with_header)?;

    println!(
        "{} Context regenerated for '{}'.",
        "Done!".green().bold(),
        project
    );

    Ok(())
}

fn read_or_empty(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}

fn collect_summary_dir(dir: &Path) -> Result<String> {
    let mut summaries = String::new();
    if !dir.exists() {
        return Ok(summaries);
    }

    let mut files: Vec<_> = std::fs::read_dir(dir)?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();

    for path in files {
        let content = std::fs::read_to_string(path)?;
        summaries.push_str(&content);
        summaries.push('\n');
    }
    Ok(summaries)
}

// ── Forget command ──────────────────────────────────────────────────────

fn cmd_forget(
    project: &str,
    session_id: Option<String>,
    topic: Option<String>,
    all: bool,
    purge: bool,
    expired: bool,
) -> Result<()> {
    use extractor::knowledge::{
        find_sessions_by_topic, parse_session_blocks, partition_by_expiry, reconstruct_blocks,
        remove_session_blocks,
    };
    use std::collections::BTreeSet;

    let home = dirs::home_dir()
        .ok_or_else(|| error::MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");
    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let global_prefs = memory_dir
        .join("knowledge")
        .join("_global")
        .join("preferences.md");

    let knowledge_files = ["decisions.md", "solutions.md", "patterns.md"];

    // Helper: collect all project knowledge files that exist
    let existing_files = || -> Vec<std::path::PathBuf> {
        knowledge_files
            .iter()
            .map(|f| knowledge_dir.join(f))
            .filter(|p| p.exists())
            .collect()
    };

    if !knowledge_dir.exists() && !all && !expired {
        eprintln!(
            "{} No knowledge found for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    // ── Expired mode ────────────────────────────────────────────────
    if expired {
        let mut removed_ids = BTreeSet::new();

        let mut process_file = |path: &std::path::Path| -> Result<()> {
            if !path.exists() {
                return Ok(());
            }
            let content = std::fs::read_to_string(path)?;
            let (preamble, blocks) = parse_session_blocks(&content);
            let (active, expired_blocks) = partition_by_expiry(blocks);
            if expired_blocks.is_empty() {
                return Ok(());
            }
            for block in &expired_blocks {
                removed_ids.insert(block.session_id.clone());
            }
            let rebuilt = reconstruct_blocks(&preamble, &active);
            std::fs::write(path, rebuilt)?;
            Ok(())
        };

        for path in existing_files() {
            process_file(&path)?;
        }
        process_file(&global_prefs)?;

        // Delete stale context.md
        if !removed_ids.is_empty() {
            let context_path = knowledge_dir.join("context.md");
            if context_path.exists() {
                std::fs::remove_file(&context_path)?;
            }
        }

        if removed_ids.is_empty() {
            println!(
                "{} No expired entries found for '{}'.",
                "Not found:".yellow(),
                project
            );
        } else {
            println!(
                "{} Removed {} expired session(s) from '{}':",
                "Done!".green().bold(),
                removed_ids.len(),
                project
            );
            for id in &removed_ids {
                println!("  - {}", id);
            }
            println!(
                "  Run '{}' to regenerate context.",
                format!("claude-memory regen {}", project).cyan()
            );
        }
        return Ok(());
    }

    // ── All mode ──────────────────────────────────────────────────
    if all {
        // Collect session IDs before deletion so we can clean global preferences
        let mut session_ids = BTreeSet::new();
        for path in existing_files() {
            let content = std::fs::read_to_string(&path)?;
            let (_preamble, blocks) = parse_session_blocks(&content);
            for block in blocks {
                session_ids.insert(block.session_id);
            }
        }

        // Remove those sessions from global preferences
        if global_prefs.exists() && !session_ids.is_empty() {
            let prefs_content = std::fs::read_to_string(&global_prefs)?;
            let ids_ref: Vec<&str> = session_ids.iter().map(|s| s.as_str()).collect();
            if let Some(cleaned) = remove_session_blocks(&prefs_content, &ids_ref) {
                std::fs::write(&global_prefs, cleaned)?;
            }
        }

        // Delete all knowledge files for project
        if knowledge_dir.exists() {
            std::fs::remove_dir_all(&knowledge_dir)?;
        }

        if purge {
            let conv_dir = memory_dir.join("conversations").join(project);
            if conv_dir.exists() {
                std::fs::remove_dir_all(&conv_dir)?;
            }
            let summ_dir = memory_dir.join("summaries").join(project);
            if summ_dir.exists() {
                std::fs::remove_dir_all(&summ_dir)?;
            }
        }

        println!(
            "{} Removed all knowledge for '{}'{}.",
            "Done!".green().bold(),
            project,
            if purge {
                " (including conversations and summaries)"
            } else {
                ""
            }
        );
        println!(
            "  Run '{}' to regenerate context.",
            format!("claude-memory ingest --project {}", project).cyan()
        );
        return Ok(());
    }

    // ── Topic mode ────────────────────────────────────────────────
    if let Some(ref query) = topic {
        let mut matched_ids = BTreeSet::new();
        for path in existing_files() {
            let content = std::fs::read_to_string(&path)?;
            for id in find_sessions_by_topic(&content, query) {
                matched_ids.insert(id);
            }
        }
        // Also search global preferences
        if global_prefs.exists() {
            let content = std::fs::read_to_string(&global_prefs)?;
            for id in find_sessions_by_topic(&content, query) {
                matched_ids.insert(id);
            }
        }

        if matched_ids.is_empty() {
            println!(
                "{} No sessions matching '{}' in project '{}'.",
                "Not found:".yellow(),
                query,
                project
            );
            return Ok(());
        }

        let ids_ref: Vec<&str> = matched_ids.iter().map(|s| s.as_str()).collect();

        // Remove from all knowledge files
        for path in existing_files() {
            let content = std::fs::read_to_string(&path)?;
            if let Some(cleaned) = remove_session_blocks(&content, &ids_ref) {
                std::fs::write(&path, cleaned)?;
            }
        }
        // Remove from global preferences
        if global_prefs.exists() {
            let content = std::fs::read_to_string(&global_prefs)?;
            if let Some(cleaned) = remove_session_blocks(&content, &ids_ref) {
                std::fs::write(&global_prefs, cleaned)?;
            }
        }

        // Delete stale context.md
        let context_path = knowledge_dir.join("context.md");
        if context_path.exists() {
            std::fs::remove_file(&context_path)?;
        }

        println!(
            "{} Removed {} session(s) matching '{}':",
            "Done!".green().bold(),
            matched_ids.len(),
            query
        );
        for id in &matched_ids {
            println!("  - {}", id);
        }
        println!(
            "  Run '{}' to regenerate context.",
            format!("claude-memory ingest --project {}", project).cyan()
        );
        return Ok(());
    }

    // ── Session mode ──────────────────────────────────────────────
    if let Some(ref sid) = session_id {
        let ids = [sid.as_str()];

        let mut removed_any = false;
        for path in existing_files() {
            let content = std::fs::read_to_string(&path)?;
            if let Some(cleaned) = remove_session_blocks(&content, &ids) {
                std::fs::write(&path, cleaned)?;
                removed_any = true;
            }
        }
        // Remove from global preferences
        if global_prefs.exists() {
            let content = std::fs::read_to_string(&global_prefs)?;
            if let Some(cleaned) = remove_session_blocks(&content, &ids) {
                std::fs::write(&global_prefs, cleaned)?;
                removed_any = true;
            }
        }

        if !removed_any {
            println!(
                "{} Session '{}' not found in knowledge for '{}'.",
                "Not found:".yellow(),
                sid,
                project
            );
            return Ok(());
        }

        // Delete stale context.md
        let context_path = knowledge_dir.join("context.md");
        if context_path.exists() {
            std::fs::remove_file(&context_path)?;
        }

        if purge {
            let conv_session = memory_dir.join("conversations").join(project).join(sid);
            if conv_session.exists() {
                std::fs::remove_dir_all(&conv_session)?;
            }
            let summ_file = memory_dir
                .join("summaries")
                .join(project)
                .join(format!("{}.md", sid));
            if summ_file.exists() {
                std::fs::remove_file(&summ_file)?;
            }
        }

        println!(
            "{} Removed session '{}' from '{}'{}.",
            "Done!".green().bold(),
            sid,
            project,
            if purge {
                " (including conversation and summary)"
            } else {
                ""
            }
        );
        println!(
            "  Run '{}' to regenerate context.",
            format!("claude-memory ingest --project {}", project).cyan()
        );
        return Ok(());
    }

    // ── List mode (default) ───────────────────────────────────────
    use extractor::knowledge::is_expired;
    use std::collections::BTreeMap;
    // session_id -> (timestamp, preview, is_expired)
    let mut all_sessions: BTreeMap<String, (String, String, bool)> = BTreeMap::new();
    let mut collect_blocks = |content: &str| {
        let (_preamble, blocks) = parse_session_blocks(content);
        for block in &blocks {
            let exp = is_expired(block);
            all_sessions
                .entry(block.session_id.clone())
                .or_insert_with(|| (block.timestamp.clone(), block.preview.clone(), exp));
        }
    };
    for path in existing_files() {
        let content = std::fs::read_to_string(&path)?;
        collect_blocks(&content);
    }
    if global_prefs.exists() {
        let content = std::fs::read_to_string(&global_prefs)?;
        collect_blocks(&content);
    }

    if all_sessions.is_empty() {
        println!(
            "{} No sessions found in knowledge for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    println!(
        "{} {} session(s) for '{}':\n",
        "Sessions".green().bold(),
        all_sessions.len(),
        project
    );
    for (sid, (ts, preview, exp)) in &all_sessions {
        let preview_display = if preview.is_empty() {
            String::new()
        } else {
            format!(" - {}", preview.dimmed())
        };
        let expired_tag = if *exp {
            " [EXPIRED]".red().to_string()
        } else {
            String::new()
        };
        println!(
            "  {} ({}){}{}",
            sid.cyan(),
            ts,
            expired_tag,
            preview_display
        );
    }
    println!(
        "\nTo remove a session: {}",
        format!("claude-memory forget {} <session-id>", project).cyan()
    );

    Ok(())
}

// ── Core commands ───────────────────────────────────────────────────────

fn cmd_ingest(
    config: &Config,
    force: bool,
    dry_run: bool,
    project_filter: Option<String>,
    since: Option<String>,
    skip_knowledge: bool,
    ttl: Option<String>,
) -> Result<()> {
    use indicatif::{ProgressBar, ProgressStyle};
    use rayon::prelude::*;

    // Validate TTL format early
    if let Some(ref ttl_val) = ttl {
        if extractor::knowledge::parse_ttl(ttl_val).is_none() {
            return Err(error::MemoryError::InvalidDuration(format!(
                "Invalid TTL: '{}'. Use format like 30m, 2h, 7d, 2w",
                ttl_val
            )));
        }
    }

    let since_duration = since.map(|s| parse_duration(&s)).transpose()?;

    // Discover projects
    let projects = parser::discovery::discover_projects(&config.claude_projects_dir)?;
    let projects: Vec<_> = if let Some(ref filter) = project_filter {
        projects.into_iter().filter(|p| p.name == *filter).collect()
    } else {
        projects
    };

    if projects.is_empty() {
        println!("{}", "No projects found.".yellow());
        return Ok(());
    }

    // Load manifest
    let mut manifest = if force {
        state::Manifest::default()
    } else {
        state::Manifest::load(&config.memory_dir)?
    };

    // Collect all sessions to process
    let mut all_sessions: Vec<(String, parser::discovery::SessionFile)> = Vec::new();
    for project in &projects {
        for session in &project.sessions {
            // Filter by time if --since provided
            if let Some(ref dur) = since_duration {
                let cutoff = chrono::Utc::now() - *dur;
                if session.modified < cutoff {
                    continue;
                }
            }
            // Skip if already processed (unless --force)
            if !force && manifest.is_processed(&session.path) {
                continue;
            }
            all_sessions.push((project.name.clone(), session.clone()));
        }
    }

    if all_sessions.is_empty() {
        println!(
            "{}",
            "Everything up to date. Use --force to re-process.".green()
        );
        return Ok(());
    }

    println!(
        "{} {} sessions across {} projects",
        "Processing".green().bold(),
        all_sessions.len(),
        projects.len()
    );

    if dry_run {
        for (project, session) in &all_sessions {
            println!(
                "  {} {}/{}",
                "Would process:".cyan(),
                project,
                session.session_id
            );
        }
        return Ok(());
    }

    // Ensure output directories exist
    std::fs::create_dir_all(&config.memory_dir)?;

    let pb = ProgressBar::new(all_sessions.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{bar:40.cyan/blue}] {pos}/{len} {msg}")
            .unwrap()
            .progress_chars("=>-"),
    );

    // Process sessions.
    // - Archive-only mode can run in parallel.
    // - Knowledge mode must be sequential to avoid concurrent writes to shared
    //   knowledge files and to keep LLM load predictable.
    let results: Vec<_> = if skip_knowledge {
        all_sessions
            .par_iter()
            .map(|(project_name, session)| {
                let result = process_session(config, project_name, session, true, ttl.as_deref());
                pb.inc(1);
                (session.path.clone(), result)
            })
            .collect()
    } else {
        all_sessions
            .iter()
            .map(|(project_name, session)| {
                let result = process_session(config, project_name, session, false, ttl.as_deref());
                pb.inc(1);
                (session.path.clone(), result)
            })
            .collect()
    };

    pb.finish_with_message("done");

    // Update manifest and collect analytics
    let mut all_analytics = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for (path, result) in results {
        match result {
            Ok(analytics) => {
                manifest.mark_processed(&path)?;
                if let Some(a) = analytics {
                    all_analytics.push(a);
                }
                success_count += 1;
            }
            Err(e) => {
                eprintln!("{} {}: {}", "Error processing".red(), path.display(), e);
                error_count += 1;
            }
        }
    }

    // Write aggregated analytics
    if !all_analytics.is_empty() {
        extractor::analytics::write_aggregated_analytics(config, &all_analytics)?;
    }

    // Save manifest
    manifest.save(&config.memory_dir)?;

    // Trigger learning for each processed project
    let processed_projects: std::collections::HashSet<String> = all_sessions
        .iter()
        .map(|(project_name, _)| project_name.clone())
        .collect();

    for project_name in &processed_projects {
        if let Err(e) = learning::post_ingest_hook(config, project_name) {
            eprintln!("Learning hook failed for {}: {}", project_name, e);
        }
    }

    println!(
        "\n{} {} sessions processed, {} errors",
        "Done!".green().bold(),
        success_count,
        error_count
    );

    Ok(())
}

fn process_session(
    config: &Config,
    project_name: &str,
    session: &parser::discovery::SessionFile,
    skip_knowledge: bool,
    ttl: Option<&str>,
) -> Result<Option<extractor::analytics::SessionAnalytics>> {
    // Parse JSONL
    let entries = parser::jsonl::parse_jsonl(&session.path)?;

    // Build conversation model
    let conversation =
        parser::conversation::build_conversation(&entries, &session.session_id, project_name);

    if conversation.turns.is_empty() {
        return Ok(None);
    }

    // Render markdown
    let conv_dir = config
        .memory_dir
        .join("conversations")
        .join(project_name)
        .join(&session.session_id);
    std::fs::create_dir_all(&conv_dir)?;

    let markdown = renderer::markdown::render_conversation(&conversation);
    std::fs::write(conv_dir.join("conversation.md"), &markdown)?;

    let meta = renderer::markdown::render_meta(&conversation);
    std::fs::write(conv_dir.join("meta.json"), &meta)?;

    // Extract analytics
    let analytics = extractor::analytics::extract_session_analytics(&conversation);

    // Write summary
    let summary_dir = config.memory_dir.join("summaries").join(project_name);
    std::fs::create_dir_all(&summary_dir)?;
    let summary = renderer::markdown::render_summary(&conversation);
    std::fs::write(
        summary_dir.join(format!("{}.md", session.session_id)),
        &summary,
    )?;

    // LLM knowledge extraction (if not skipped)
    if !skip_knowledge {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| crate::error::MemoryError::Config(format!("tokio runtime: {}", e)))?;

        rt.block_on(async {
            if let Err(e) = extractor::knowledge::extract_and_merge_knowledge(
                config,
                project_name,
                &conversation,
                ttl,
            )
            .await
            {
                eprintln!(
                    "  {} knowledge extraction for {}/{}: {}",
                    "Warning:".yellow(),
                    project_name,
                    session.session_id,
                    e
                );
            }
        });
    }

    Ok(Some(analytics))
}

fn cmd_search(
    config: &Config,
    query: &str,
    project: Option<String>,
    knowledge_only: bool,
    context_lines: usize,
) -> Result<()> {
    let search_dir = if knowledge_only {
        config.memory_dir.join("knowledge")
    } else {
        config.memory_dir.clone()
    };

    if !search_dir.exists() {
        println!(
            "{}",
            "No memory directory found. Run 'ingest' first.".yellow()
        );
        return Ok(());
    }

    let pattern = regex::Regex::new(query)
        .map_err(|e| crate::error::MemoryError::Config(format!("Invalid regex: {}", e)))?;

    let mut found = false;
    for entry in walkdir::WalkDir::new(&search_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "md" || ext == "json")
        })
    {
        let path = entry.path();

        // Filter by project if specified
        if let Some(ref proj) = project {
            let path_str = path.to_string_lossy();
            if !path_str.contains(proj) {
                continue;
            }
        }

        let content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let lines: Vec<&str> = content.lines().collect();
        let mut matched_in_file = false;

        for (i, line) in lines.iter().enumerate() {
            if pattern.is_match(line) {
                if !matched_in_file {
                    let rel = path.strip_prefix(&config.memory_dir).unwrap_or(path);
                    println!("\n{}", rel.display().to_string().cyan().bold());
                    matched_in_file = true;
                    found = true;
                }

                let start = i.saturating_sub(context_lines);
                let end = (i + context_lines + 1).min(lines.len());
                for (j, line) in lines.iter().enumerate().take(end).skip(start) {
                    let prefix = if j == i {
                        format!("{:>4} > ", j + 1).green().to_string()
                    } else {
                        format!("{:>4}   ", j + 1).dimmed().to_string()
                    };
                    println!("{}{}", prefix, line);
                }
                if end < lines.len() {
                    println!("{}", "  ---".dimmed());
                }
            }
        }
    }

    if !found {
        println!("{} No matches for '{}'", "Not found:".yellow(), query);
    }

    // Track usage
    let tracker = analytics::EventTracker::new(&config.memory_dir);
    let _ = tracker.track(analytics::UsageEvent {
        timestamp: chrono::Utc::now(),
        event_type: analytics::EventType::Search,
        project: project.clone().unwrap_or_else(|| "all".to_string()),
        query: Some(query.to_string()),
        category: None,
        results_count: if found { Some(1) } else { Some(0) },
        session_id: None,
    });

    Ok(())
}

fn cmd_recall(config: &Config, project: &str) -> Result<()> {
    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let context_path = knowledge_dir.join("context.md");

    // Get local project knowledge
    let local_content = if context_path.exists() {
        Some(std::fs::read_to_string(&context_path)?)
    } else {
        build_raw_context(project, &knowledge_dir)
    };

    // Get knowledge from installed packs
    let pack_content = get_installed_pack_knowledge(&config.memory_dir)?;

    // Combine local and pack knowledge
    let content = if let Some(local) = local_content {
        if pack_content.is_empty() {
            local
        } else {
            format!(
                "{}\n\n---\n\n# Installed Pack Knowledge\n\n{}",
                local, pack_content
            )
        }
    } else if !pack_content.is_empty() {
        format!("# Installed Pack Knowledge\n\n{}", pack_content)
    } else {
        println!(
            "{} No context found for '{}'. Run 'ingest' first or install knowledge packs.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    };

    println!("{}", content);

    // Track usage
    let tracker = analytics::EventTracker::new(&config.memory_dir);
    let _ = tracker.track(analytics::UsageEvent {
        timestamp: chrono::Utc::now(),
        event_type: analytics::EventType::Recall,
        project: project.to_string(),
        query: None,
        category: None,
        results_count: None,
        session_id: None,
    });

    // Track learning signals from recall
    if let Err(e) = learning::post_recall_hook(config, project, &[]) {
        eprintln!("Learning hook failed (non-fatal): {}", e);
    }

    Ok(())
}

/// Get aggregated knowledge from all installed packs
fn get_installed_pack_knowledge(memory_dir: &Path) -> Result<String> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);
    let knowledge_dirs = installer.get_installed_knowledge_dirs()?;

    if knowledge_dirs.is_empty() {
        return Ok(String::new());
    }

    let mut combined = String::new();

    for (pack_name, knowledge_dir) in knowledge_dirs {
        combined.push_str(&format!("## From pack: {}\n\n", pack_name));

        // Read knowledge files from pack
        for category in &[
            "patterns.md",
            "solutions.md",
            "decisions.md",
            "preferences.md",
        ] {
            let file_path = knowledge_dir.join(category);
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    if !content.trim().is_empty() {
                        combined.push_str(&format!("### {}\n\n", category.replace(".md", "")));
                        combined.push_str(&content);
                        combined.push_str("\n\n");
                    }
                }
            }
        }
    }

    Ok(combined)
}

fn cmd_context(config: &Config, project: &str) -> Result<()> {
    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let context_path = knowledge_dir.join("context.md");

    // Raw stdout, no formatting — suitable for piping
    let content = if context_path.exists() {
        std::fs::read_to_string(&context_path)?
    } else {
        match build_raw_context(project, &knowledge_dir) {
            Some(raw) => raw,
            None => {
                eprintln!("No context for project '{}'", project);
                std::process::exit(1);
            }
        }
    };

    print!("{}", content);
    Ok(())
}

fn cmd_status(config: &Config) -> Result<()> {
    if !config.memory_dir.exists() {
        println!(
            "{}",
            "No memory directory found. Run 'ingest' first.".yellow()
        );
        return Ok(());
    }

    let manifest = state::Manifest::load(&config.memory_dir)?;

    // Count files and sizes
    let mut total_size: u64 = 0;
    let mut md_count = 0u64;
    let mut json_count = 0u64;

    for entry in walkdir::WalkDir::new(&config.memory_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        total_size += entry.metadata().map(|m| m.len()).unwrap_or(0);
        match entry.path().extension().and_then(|e| e.to_str()) {
            Some("md") => md_count += 1,
            Some("json") => json_count += 1,
            _ => {}
        }
    }

    // Count projects with conversations
    let conv_dir = config.memory_dir.join("conversations");
    let project_count = if conv_dir.exists() {
        std::fs::read_dir(&conv_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count()
    } else {
        0
    };

    // Count knowledge projects
    let knowledge_dir = config.memory_dir.join("knowledge");
    let knowledge_count = if knowledge_dir.exists() {
        std::fs::read_dir(&knowledge_dir)?
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
            .count()
    } else {
        0
    };

    println!("{}", "Claude Memory Status".green().bold());
    println!("{}", "=".repeat(40));
    println!(
        "  Memory directory:  {}",
        config.memory_dir.display().to_string().cyan()
    );
    println!(
        "  Total size:        {}",
        humansize::format_size(total_size, humansize::BINARY)
    );
    println!("  Markdown files:    {}", md_count);
    println!("  JSON files:        {}", json_count);
    println!("  Projects archived: {}", project_count);
    println!("  Knowledge bases:   {}", knowledge_count);
    println!("  Sessions processed:{}", manifest.processed_count());
    println!(
        "  LLM provider:      {}",
        config.llm.provider.display_name().cyan()
    );

    Ok(())
}

fn cmd_projects(config: &Config) -> Result<()> {
    let projects = parser::discovery::discover_projects(&config.claude_projects_dir)?;

    if projects.is_empty() {
        println!("{}", "No Claude projects found.".yellow());
        return Ok(());
    }

    println!("{}", "Claude Projects".green().bold());
    println!("{}", "=".repeat(60));

    for project in &projects {
        let total_size: u64 = project.sessions.iter().map(|s| s.size).sum();
        let latest = project
            .sessions
            .iter()
            .map(|s| s.modified)
            .max()
            .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "unknown".into());

        // Check if we have knowledge for this project
        let has_knowledge = config
            .memory_dir
            .join("knowledge")
            .join(&project.name)
            .join("context.md")
            .exists();

        let knowledge_indicator = if has_knowledge {
            " *".green().to_string()
        } else {
            String::new()
        };

        println!(
            "  {}{}\t{} sessions, {}, last active: {}",
            project.name.cyan().bold(),
            knowledge_indicator,
            project.sessions.len(),
            humansize::format_size(total_size, humansize::BINARY),
            latest.dimmed()
        );
    }

    Ok(())
}

fn cmd_mcp(config: &Config) -> Result<()> {
    let server = mcp::McpServer::new(config.clone());
    server.run()
}

fn cmd_export(
    config: &Config,
    project: &str,
    format: &str,
    output: Option<&str>,
    include_conversations: bool,
) -> Result<()> {
    use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);

    if !knowledge_dir.exists() {
        eprintln!(
            "{} No knowledge found for '{}'. Run 'ingest' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    // Read and filter knowledge files
    let read_and_filter = |path: &Path| -> String {
        if !path.exists() {
            return String::new();
        }
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let decisions = read_and_filter(&knowledge_dir.join("decisions.md"));
    let solutions = read_and_filter(&knowledge_dir.join("solutions.md"));
    let patterns = read_and_filter(&knowledge_dir.join("patterns.md"));
    let context = read_and_filter(&knowledge_dir.join("context.md"));

    let exported_content = match format {
        "markdown" => export_markdown(
            project,
            &context,
            &decisions,
            &solutions,
            &patterns,
            include_conversations,
            config,
        )?,
        "json" => export_json(
            project,
            &context,
            &decisions,
            &solutions,
            &patterns,
            include_conversations,
            config,
        )?,
        "html" => export_html(
            project,
            &context,
            &decisions,
            &solutions,
            &patterns,
            include_conversations,
            config,
        )?,
        _ => return Err(MemoryError::Config(format!("Unknown format: {}", format))),
    };

    if let Some(output_path) = output {
        std::fs::write(output_path, &exported_content)?;
        println!(
            "{} Exported {} knowledge to {}",
            "Done!".green().bold(),
            project,
            output_path
        );
    } else {
        print!("{}", exported_content);
    }

    Ok(())
}

fn export_markdown(
    project: &str,
    context: &str,
    decisions: &str,
    solutions: &str,
    patterns: &str,
    include_conversations: bool,
    config: &Config,
) -> Result<String> {
    let mut output = String::new();

    output.push_str(&format!("# {} - Knowledge Export\n\n", project));
    output.push_str(&format!(
        "**Exported:** {}\n\n",
        chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
    ));
    output.push_str("**Tool:** [claude-memory](https://github.com/Algiras/claude-memory)\n\n");
    output.push_str("---\n\n");

    if !context.trim().is_empty() {
        output.push_str("## Project Context\n\n");
        output.push_str(context);
        output.push_str("\n\n---\n\n");
    }

    if !decisions.trim().is_empty() {
        output.push_str(decisions);
        output.push_str("\n\n---\n\n");
    }

    if !solutions.trim().is_empty() {
        output.push_str(solutions);
        output.push_str("\n\n---\n\n");
    }

    if !patterns.trim().is_empty() {
        output.push_str(patterns);
        output.push_str("\n\n");
    }

    if include_conversations {
        output.push_str("## Conversations\n\n");
        let conv_dir = config.memory_dir.join("conversations").join(project);
        if conv_dir.exists() {
            for entry in std::fs::read_dir(&conv_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let conv_file = entry.path().join("conversation.md");
                    if conv_file.exists() {
                        let session_id = entry.file_name().to_string_lossy().to_string();
                        output.push_str(&format!("### Session: {}\n\n", session_id));
                        output.push_str(&std::fs::read_to_string(conv_file)?);
                        output.push_str("\n\n---\n\n");
                    }
                }
            }
        }
    }

    Ok(output)
}

fn export_json(
    project: &str,
    context: &str,
    decisions: &str,
    solutions: &str,
    patterns: &str,
    include_conversations: bool,
    config: &Config,
) -> Result<String> {
    use serde_json::json;

    let mut conversations = Vec::new();
    if include_conversations {
        let conv_dir = config.memory_dir.join("conversations").join(project);
        if conv_dir.exists() {
            for entry in std::fs::read_dir(&conv_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let conv_file = entry.path().join("conversation.md");
                    let meta_file = entry.path().join("meta.json");
                    if conv_file.exists() {
                        let session_id = entry.file_name().to_string_lossy().to_string();
                        let content = std::fs::read_to_string(conv_file)?;
                        let meta = if meta_file.exists() {
                            std::fs::read_to_string(meta_file).ok()
                        } else {
                            None
                        };
                        conversations.push(json!({
                            "session_id": session_id,
                            "content": content,
                            "meta": meta.and_then(|m| serde_json::from_str::<serde_json::Value>(&m).ok()),
                        }));
                    }
                }
            }
        }
    }

    let export = json!({
        "project": project,
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "tool": "claude-memory",
        "tool_url": "https://github.com/Algiras/claude-memory",
        "knowledge": {
            "context": context,
            "decisions": decisions,
            "solutions": solutions,
            "patterns": patterns,
        },
        "conversations": conversations,
    });

    Ok(serde_json::to_string_pretty(&export)?)
}

fn export_html(
    project: &str,
    context: &str,
    decisions: &str,
    solutions: &str,
    patterns: &str,
    include_conversations: bool,
    config: &Config,
) -> Result<String> {
    let mut html = String::from(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>"#,
    );
    html.push_str(&format!("{} - Knowledge Export</title>\n", project));
    html.push_str(r#"    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            max-width: 900px;
            margin: 0 auto;
            padding: 2rem;
            line-height: 1.6;
            color: #333;
            background: #f5f5f5;
        }
        .container {
            background: white;
            padding: 2rem;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
        }
        h1 { color: #2c3e50; border-bottom: 3px solid #3498db; padding-bottom: 0.5rem; }
        h2 { color: #34495e; margin-top: 2rem; border-bottom: 1px solid #ddd; padding-bottom: 0.3rem; }
        h3 { color: #555; }
        .meta { color: #777; font-size: 0.9rem; margin-bottom: 2rem; }
        pre { background: #f8f8f8; padding: 1rem; border-radius: 4px; overflow-x: auto; border-left: 3px solid #3498db; }
        code { background: #f0f0f0; padding: 0.2rem 0.4rem; border-radius: 3px; font-size: 0.9em; }
        .search { margin: 2rem 0; }
        .search input { width: 100%; padding: 0.8rem; border: 2px solid #ddd; border-radius: 4px; font-size: 1rem; }
        .search input:focus { outline: none; border-color: #3498db; }
        hr { border: none; border-top: 1px solid #eee; margin: 2rem 0; }
        .footer { margin-top: 3rem; padding-top: 1rem; border-top: 1px solid #eee; color: #999; font-size: 0.9rem; text-align: center; }
    </style>
</head>
<body>
    <div class="container">
        <h1>"#);
    html.push_str(&format!("{} - Knowledge Export</h1>\n", project));
    html.push_str(&format!(r#"        <div class="meta">
            <strong>Exported:</strong> {}<br>
            <strong>Tool:</strong> <a href="https://github.com/Algiras/claude-memory">claude-memory</a>
        </div>
        <div class="search">
            <input type="text" id="searchBox" placeholder="Search knowledge..." onkeyup="filterContent()">
        </div>
        <div id="content">
"#, chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));

    // Convert markdown sections to HTML (simple conversion)
    let sections = vec![
        ("Project Context", context),
        ("Decisions", decisions),
        ("Solutions", solutions),
        ("Patterns", patterns),
    ];

    for (title, content) in sections {
        if !content.trim().is_empty() {
            html.push_str(&format!("<h2>{}</h2>\n", title));
            html.push_str("<div class='section'>\n");
            // Simple markdown to HTML (just preserve formatting)
            html.push_str(
                &content
                    .replace("<", "&lt;")
                    .replace(">", "&gt;")
                    .replace("\n\n", "</p><p>")
                    .replace("\n", "<br>"),
            );
            html.push_str("</div>\n");
        }
    }

    if include_conversations {
        html.push_str("<h2>Conversations</h2>\n");
        let conv_dir = config.memory_dir.join("conversations").join(project);
        if conv_dir.exists() {
            for entry in std::fs::read_dir(&conv_dir)? {
                let entry = entry?;
                if entry.file_type()?.is_dir() {
                    let conv_file = entry.path().join("conversation.md");
                    if conv_file.exists() {
                        let session_id = entry.file_name().to_string_lossy().to_string();
                        let content = std::fs::read_to_string(conv_file)?;
                        html.push_str(&format!("<h3>Session: {}</h3>\n", session_id));
                        html.push_str("<div class='section conversation'>\n");
                        html.push_str(
                            &content
                                .replace("<", "&lt;")
                                .replace(">", "&gt;")
                                .replace("\n\n", "</p><p>")
                                .replace("\n", "<br>"),
                        );
                        html.push_str("</div>\n<hr>\n");
                    }
                }
            }
        }
    }

    html.push_str(
        r#"        </div>
        <div class="footer">
            Generated by <a href="https://github.com/Algiras/claude-memory">claude-memory</a>
        </div>
    </div>
    <script>
        function filterContent() {
            const query = document.getElementById('searchBox').value.toLowerCase();
            const sections = document.querySelectorAll('.section');
            sections.forEach(section => {
                const text = section.textContent.toLowerCase();
                section.style.display = text.includes(query) ? 'block' : 'none';
            });
        }
    </script>
</body>
</html>"#,
    );

    Ok(html)
}

// ── Sync commands ───────────────────────────────────────────────────────

fn cmd_sync_push(
    config: &Config,
    project: &str,
    gist_id: Option<&str>,
    description: &str,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;
        let files = sync::read_knowledge_files(&config.memory_dir, project)?;

        if files.is_empty() {
            eprintln!(
                "{} No knowledge found for '{}'. Run 'ingest' first.",
                "Not found:".yellow(),
                project
            );
            return Ok(());
        }

        let gist = if let Some(id) = gist_id {
            println!("{} Updating gist {}...", "Syncing".green().bold(), id);
            client.update_gist(id, Some(description), files).await?
        } else {
            println!("{} Creating new private gist...", "Syncing".green().bold());
            client.create_gist(description, files).await?
        };

        println!(
            "{} Pushed {} knowledge to gist",
            "Done!".green().bold(),
            project
        );
        println!("  Gist ID:  {}", gist.id.cyan());
        println!("  URL:      {}", gist.html_url.cyan());
        println!("\nTo pull on another machine:");
        println!(
            "  {}",
            format!("claude-memory sync pull {} {}", project, gist.id).cyan()
        );

        Ok(())
    })
}

fn cmd_sync_pull(config: &Config, project: &str, gist_id: &str, force: bool) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;

        println!("{} Fetching gist {}...", "Syncing".green().bold(), gist_id);
        let gist = client.get_gist(gist_id).await?;

        let knowledge_dir = config.memory_dir.join("knowledge").join(project);
        if knowledge_dir.exists() && !force {
            eprintln!(
                "{} Knowledge already exists for '{}'. Use --force to overwrite.",
                "Warning:".yellow(),
                project
            );
            return Ok(());
        }

        sync::write_knowledge_files(&config.memory_dir, project, &gist.files)?;

        println!(
            "{} Pulled {} knowledge from gist",
            "Done!".green().bold(),
            project
        );
        println!("  {} files synced", gist.files.len());
        println!("\nView with:");
        println!("  {}", format!("claude-memory recall {}", project).cyan());

        Ok(())
    })
}

fn cmd_sync_list(_config: &Config, project: &str) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;

        println!(
            "{} Listing gists for '{}'...",
            "Searching".green().bold(),
            project
        );
        let gists = client.list_gists().await?;

        let matching: Vec<_> = gists
            .iter()
            .filter(|g| g.description.contains(project) || g.files.contains_key("metadata.json"))
            .collect();

        if matching.is_empty() {
            println!("{} No gists found for '{}'", "Not found:".yellow(), project);
            return Ok(());
        }

        println!("\n{} gist(s) found:\n", matching.len());
        for gist in matching {
            println!("  {} {}", "ID:".cyan(), gist.id);
            println!("  {} {}", "Description:".cyan(), gist.description);
            println!("  {} {}", "URL:".cyan(), gist.html_url);
            println!("  {} {} file(s)", "Files:".cyan(), gist.files.len());
            println!("  {} {}", "Private:".cyan(), !gist.public);
            println!();
        }

        Ok(())
    })
}

fn cmd_sync_clone(config: &Config, gist_id: &str, project: &str) -> Result<()> {
    cmd_sync_pull(config, project, gist_id, false)
}

fn cmd_sync_history(gist_id: &str, version: Option<&str>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let client = sync::GistClient::from_env()?;

        if let Some(ver) = version {
            // Show specific version
            println!("{} Fetching version {}...", "Loading".green().bold(), ver);
            let gist = client.get_gist_version(gist_id, ver).await?;

            println!("\n{} Version {}", "Gist:".green().bold(), ver);
            println!("{}", "=".repeat(60));
            println!("Description: {}", gist.description);
            println!("Files: {}", gist.files.len());
            println!("\nFiles in this version:");
            for (filename, file) in &gist.files {
                let size = file.content.as_ref().map(|c| c.len()).unwrap_or(0);
                println!("  {} ({} bytes)", filename.cyan(), size);
            }
        } else {
            // Show history
            println!(
                "{} Fetching history for {}...",
                "Loading".green().bold(),
                gist_id
            );
            let history = client.get_gist_history(gist_id).await?;

            if history.is_empty() {
                println!("{} No history found", "Not found:".yellow());
                return Ok(());
            }

            println!("\n{} Version History", "Gist:".green().bold());
            println!("{}", "=".repeat(60));
            println!("{} versions found\n", history.len());

            for (i, entry) in history.iter().enumerate() {
                let user = entry
                    .user
                    .as_ref()
                    .map(|u| u.login.as_str())
                    .unwrap_or("unknown");
                let time = chrono::DateTime::parse_from_rfc3339(&entry.committed_at)
                    .map(|dt| dt.format("%Y-%m-%d %H:%M:%S").to_string())
                    .unwrap_or_else(|_| entry.committed_at.clone());

                println!(
                    "  {} {} ({})",
                    format!("{}.", i + 1).dimmed(),
                    entry.version.cyan(),
                    time.dimmed()
                );
                println!("     By: {}", user);

                if let Some(ref status) = entry.change_status {
                    if let (Some(add), Some(del)) = (status.additions, status.deletions) {
                        println!(
                            "     Changes: {} {} ",
                            format!("+{}", add).green(),
                            format!("-{}", del).red()
                        );
                    }
                }
                println!();
            }

            println!("\nTo view a specific version:");
            println!(
                "  {}",
                format!("claude-memory sync history {} --version <version>", gist_id).cyan()
            );
            println!("\nTo restore a version:");
            println!(
                "  {}",
                format!(
                    "claude-memory sync pull <project> {}",
                    history.first().unwrap().version
                )
                .cyan()
            );
        }

        Ok(())
    })
}

fn cmd_sync_push_repo(
    config: &Config,
    project: &str,
    repo: &str,
    message: Option<&str>,
    push_remote: bool,
) -> Result<()> {
    let expanded = shellexpand::tilde(repo);
    let repo_path = std::path::PathBuf::from(expanded.as_ref());

    println!(
        "{} Syncing {} to git repo {}...",
        "Pushing".green().bold(),
        project,
        repo_path.display()
    );

    sync::push_to_git_repo(
        &config.memory_dir,
        project,
        &repo_path,
        message,
        push_remote,
    )?;

    println!(
        "{} Pushed {} knowledge to {}",
        "Done!".green().bold(),
        project,
        repo_path.display()
    );

    if push_remote {
        println!("  Changes pushed to remote");
    }

    Ok(())
}

fn cmd_sync_pull_repo(
    config: &Config,
    project: &str,
    repo: &str,
    fetch_remote: bool,
    branch: &str,
) -> Result<()> {
    let expanded = shellexpand::tilde(repo);
    let repo_path = std::path::PathBuf::from(expanded.as_ref());

    println!(
        "{} Syncing {} from git repo {}...",
        "Pulling".green().bold(),
        project,
        repo_path.display()
    );

    sync::pull_from_git_repo(
        &config.memory_dir,
        project,
        &repo_path,
        fetch_remote,
        branch,
    )?;

    println!(
        "{} Pulled {} knowledge from {}",
        "Done!".green().bold(),
        project,
        repo_path.display()
    );

    println!("\nView with:");
    println!("  {}", format!("claude-memory recall {}", project).cyan());

    Ok(())
}

fn cmd_sync_init_repo(repo: &str) -> Result<()> {
    let expanded = shellexpand::tilde(repo);
    let repo_path = std::path::PathBuf::from(expanded.as_ref());

    println!(
        "{} Initializing git repository at {}...",
        "Creating".green().bold(),
        repo_path.display()
    );

    sync::init_git_repo(&repo_path)?;

    println!("{} Git repository initialized", "Done!".green().bold());
    println!("  Path: {}", repo_path.display().to_string().cyan());
    println!("\nNext steps:");
    println!("  1. {} (optional)", "git remote add origin <url>".dimmed());
    println!(
        "  2. {}",
        format!("claude-memory sync push-repo <project> {}", repo).cyan()
    );

    Ok(())
}

// ── Graph commands ──────────────────────────────────────────────────────

fn cmd_graph_build(config: &Config, project: &str) -> Result<()> {
    use extractor::knowledge::{parse_session_blocks, partition_by_expiry, reconstruct_blocks};

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);

    if !knowledge_dir.exists() {
        eprintln!(
            "{} No knowledge found for '{}'. Run 'ingest' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    println!(
        "{} Building knowledge graph for '{}'...",
        "Analyzing".green().bold(),
        project
    );

    // Read knowledge files
    let read_and_filter = |path: &Path| -> String {
        if !path.exists() {
            return String::new();
        }
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let mut knowledge_content = String::new();
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("context.md")));
    knowledge_content.push_str("\n\n");
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("decisions.md")));
    knowledge_content.push_str("\n\n");
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("solutions.md")));
    knowledge_content.push_str("\n\n");
    knowledge_content.push_str(&read_and_filter(&knowledge_dir.join("patterns.md")));

    if knowledge_content.trim().is_empty() {
        eprintln!("{} No knowledge content to analyze", "Error:".red());
        return Ok(());
    }

    // Build graph using LLM
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    let graph = rt.block_on(async {
        graph::builder::build_graph_from_knowledge(config, project, &knowledge_content).await
    })?;

    // Save graph
    let graph_path = knowledge_dir.join("graph.json");
    graph
        .save(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to save graph: {}", e)))?;

    println!("{} Knowledge graph created:", "Done!".green().bold());
    println!("  Concepts: {}", graph.concepts.len());
    println!("  Relationships: {}", graph.relationships.len());
    println!("  Saved to: {}", graph_path.display().to_string().cyan());
    println!("\nExplore with:");
    println!(
        "  {}",
        format!("claude-memory graph query {} <concept>", project).cyan()
    );
    println!(
        "  {}",
        format!("claude-memory graph viz {} ascii", project).cyan()
    );

    Ok(())
}

fn cmd_graph_query(config: &Config, project: &str, concept: &str, depth: usize) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    let related = graph::query::find_related(&graph, concept, depth);

    if related.is_empty() {
        println!(
            "{} No concepts found related to '{}'",
            "Not found:".yellow(),
            concept
        );
        return Ok(());
    }

    println!(
        "{} Concepts related to '{}' (depth {}):\n",
        "Graph Query".green().bold(),
        concept,
        depth
    );

    for (concept_id, dist) in related {
        if let Some(c) = graph.concepts.get(&concept_id) {
            let indent = "  ".repeat(dist);
            println!(
                "{}[{}] {} (importance: {:.1})",
                indent,
                dist,
                c.name.cyan(),
                c.importance
            );
        }
    }

    Ok(())
}

fn cmd_graph_viz(
    config: &Config,
    project: &str,
    format: &str,
    output: Option<&str>,
    root: Option<&str>,
) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    let viz_content = match format {
        "dot" => graph::viz::to_dot(&graph),
        "ascii" => graph::viz::to_ascii(&graph, root),
        "svg" => {
            // Generate DOT and convert to SVG using graphviz
            let dot = graph::viz::to_dot(&graph);
            if let Some(out_path) = output {
                // Write DOT to temp file
                let temp_dot = "/tmp/graph.dot";
                std::fs::write(temp_dot, &dot)?;

                // Convert to SVG using dot command
                let status = std::process::Command::new("dot")
                    .args(["-Tsvg", temp_dot, "-o", out_path])
                    .status();

                match status {
                    Ok(s) if s.success() => {
                        println!("{} SVG created: {}", "Done!".green().bold(), out_path);
                        return Ok(());
                    }
                    _ => {
                        eprintln!(
                            "{} graphviz not installed. Install with: brew install graphviz",
                            "Error:".red()
                        );
                        eprintln!("Outputting DOT format instead...");
                        dot
                    }
                }
            } else {
                eprintln!(
                    "{} SVG requires --output. Showing DOT instead.",
                    "Note:".yellow()
                );
                dot
            }
        }
        _ => return Err(MemoryError::Config(format!("Unknown format: {}", format))),
    };

    if let Some(out_path) = output {
        std::fs::write(out_path, &viz_content)?;
        println!(
            "{} Visualization saved to {}",
            "Done!".green().bold(),
            out_path
        );
    } else {
        print!("{}", viz_content);
    }

    Ok(())
}

fn cmd_graph_path(config: &Config, project: &str, from: &str, to: &str) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    match graph::query::shortest_path(&graph, from, to) {
        Some(path) => {
            println!(
                "{} Path from '{}' to '{}':\n",
                "Found".green().bold(),
                from,
                to
            );
            for (i, concept) in path.iter().enumerate() {
                if i > 0 {
                    println!("   ↓");
                }
                println!("  [{}] {}", i, concept.cyan());
            }
        }
        None => {
            println!(
                "{} No path found from '{}' to '{}'",
                "Not found:".yellow(),
                from,
                to
            );
        }
    }

    Ok(())
}

fn cmd_graph_hubs(config: &Config, project: &str, top_n: usize) -> Result<()> {
    let graph_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("graph.json");

    if !graph_path.exists() {
        eprintln!(
            "{} No graph found for '{}'. Run 'graph build' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let graph = graph::KnowledgeGraph::load(&graph_path)
        .map_err(|e| MemoryError::Config(format!("Failed to load graph: {}", e)))?;

    let hubs = graph::query::find_hubs(&graph, top_n);

    println!(
        "{} Top {} most connected concepts:\n",
        "Hubs".green().bold(),
        top_n
    );

    for (i, (concept_id, in_degree, out_degree)) in hubs.iter().enumerate() {
        if let Some(concept) = graph.concepts.get(concept_id) {
            println!(
                "  {}. {} ({} incoming, {} outgoing, importance: {:.1})",
                i + 1,
                concept.name.cyan(),
                in_degree,
                out_degree,
                concept.importance
            );
        }
    }

    Ok(())
}

// ── Embedding commands ──────────────────────────────────────────────────

fn cmd_embed(config: &Config, project: &str, provider_override: Option<&str>) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        println!(
            "{} Building embeddings for '{}'...",
            "Embedding".green().bold(),
            project
        );

        let provider = if let Some(prov) = provider_override {
            match prov {
                "openai" => {
                    let key = std::env::var("OPENAI_API_KEY")
                        .map_err(|_| MemoryError::Config("OPENAI_API_KEY not set".into()))?;
                    embeddings::EmbeddingProvider::OpenAI { api_key: key }
                }
                "gemini" => {
                    let key = std::env::var("GEMINI_API_KEY")
                        .map_err(|_| MemoryError::Config("GEMINI_API_KEY not set".into()))?;
                    embeddings::EmbeddingProvider::Gemini { api_key: key }
                }
                "ollama" => embeddings::EmbeddingProvider::OllamaLocal,
                _ => return Err(MemoryError::Config(format!("Unknown provider: {}", prov))),
            }
        } else {
            embeddings::EmbeddingProvider::from_env()?
        };

        let store =
            embeddings::search::SemanticSearch::build_index(&config.memory_dir, project, &provider)
                .await?;

        let stats = store.stats();

        println!("{} Embeddings created:", "Done!".green().bold());
        println!("  Total chunks: {}", stats.total_chunks);
        println!("  By category:");
        for (cat, count) in stats.by_category {
            println!("    {}: {}", cat, count);
        }
        println!("\nSearch with:");
        println!(
            "  {}",
            format!(
                "claude-memory search-semantic \"your query\" --project {}",
                project
            )
            .cyan()
        );

        Ok(())
    })
}

fn cmd_search_semantic(
    config: &Config,
    query: &str,
    project: Option<&str>,
    top_k: usize,
    threshold: f32,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let provider = embeddings::EmbeddingProvider::from_env()?;

        if let Some(proj) = project {
            // Search specific project
            let results = embeddings::search::SemanticSearch::search(
                &config.memory_dir,
                proj,
                query,
                &provider,
                top_k,
            )
            .await?;

            println!(
                "{} Semantic search results for '{}':\n",
                "Search".green().bold(),
                query
            );

            for (score, text, category) in results {
                if score >= threshold {
                    println!(
                        "  {} [{}] ({:.1}%)",
                        ">".green(),
                        category.cyan(),
                        score * 100.0
                    );
                    println!("    {}\n", truncate_text(&text, 150));
                }
            }
        } else {
            // Search all projects with embeddings
            let knowledge_dir = config.memory_dir.join("knowledge");
            let mut all_results = Vec::new();

            for entry in std::fs::read_dir(&knowledge_dir)? {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }

                let project_name = entry.file_name().to_string_lossy().to_string();
                if project_name == "_global" {
                    continue;
                }

                let index_path = entry.path().join("embeddings.json");
                if !index_path.exists() {
                    continue;
                }

                if let Ok(results) = embeddings::search::SemanticSearch::search(
                    &config.memory_dir,
                    &project_name,
                    query,
                    &provider,
                    top_k,
                )
                .await
                {
                    for (score, text, category) in results {
                        if score >= threshold {
                            all_results.push((score, text, category, project_name.clone()));
                        }
                    }
                }
            }

            // Sort by score
            all_results.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            all_results.truncate(top_k);

            println!(
                "{} Semantic search results for '{}':\n",
                "Search".green().bold(),
                query
            );

            for (score, text, category, proj) in all_results {
                println!(
                    "  {} [{}:{}] ({:.1}%)",
                    ">".green(),
                    proj.dimmed(),
                    category.cyan(),
                    score * 100.0
                );
                println!("    {}\n", truncate_text(&text, 150));
            }
        }

        Ok(())
    })
}

fn truncate_text(text: &str, max_len: usize) -> String {
    let cleaned = text.replace('\n', " ").trim().to_string();
    if cleaned.len() <= max_len {
        cleaned
    } else {
        format!("{}...", &cleaned[..max_len - 3])
    }
}

fn cmd_consolidate(
    config: &Config,
    project: &str,
    threshold: f32,
    auto_merge: bool,
    find_contradictions: bool,
) -> Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

    rt.block_on(async {
        let index_path = config
            .memory_dir
            .join("knowledge")
            .join(project)
            .join("embeddings.json");

        if !index_path.exists() {
            eprintln!(
                "{} No embeddings found. Run 'claude-memory embed {}' first.",
                "Error:".red(),
                project
            );
            return Ok(());
        }

        println!(
            "{} Analyzing knowledge for duplicates...",
            "Consolidating".green().bold()
        );

        let store = embeddings::EmbeddingStore::load(&index_path)?;

        // Find similar chunks
        let mut duplicate_groups: Vec<Vec<(f32, usize)>> = Vec::new();

        for (i, chunk_a) in store.chunks.iter().enumerate() {
            let mut similar = Vec::new();

            for (j, chunk_b) in store.chunks.iter().enumerate() {
                if i >= j {
                    continue; // Skip self and already compared
                }

                let similarity =
                    embeddings::cosine_similarity(&chunk_a.embedding, &chunk_b.embedding);

                if similarity >= threshold {
                    similar.push((similarity, j));
                }
            }

            if !similar.is_empty() {
                similar.insert(0, (1.0, i)); // Add self with perfect score
                duplicate_groups.push(similar);
            }
        }

        if duplicate_groups.is_empty() {
            println!(
                "{} No duplicates found (threshold: {:.0}%)",
                "✓".green(),
                threshold * 100.0
            );
            return Ok(());
        }

        println!("\n{} duplicate group(s) found:\n", duplicate_groups.len());

        for (group_idx, group) in duplicate_groups.iter().enumerate() {
            println!(
                "{}. Duplicate Group (similarity ≥ {:.0}%):",
                group_idx + 1,
                threshold * 100.0
            );

            for (similarity, chunk_idx) in group {
                let chunk = &store.chunks[*chunk_idx];
                println!(
                    "   {} [{:.0}%] [{}]",
                    if *similarity == 1.0 { "▶" } else { " " },
                    similarity * 100.0,
                    chunk.metadata.category.cyan()
                );
                println!("      {}", truncate_text(&chunk.text, 100));
            }
            println!();
        }

        if !auto_merge {
            println!("To merge duplicates, run with: {}", "--auto-merge".cyan());
        }

        // Contradiction detection
        if find_contradictions {
            println!(
                "\n{} Checking for contradictions...",
                "Analyzing".green().bold()
            );
            println!("{} Contradiction detection coming soon!", "Note:".yellow());
        }

        // Track learning signals from consolidation
        if let Err(e) =
            learning::post_consolidate_hook(config, project, duplicate_groups.len(), auto_merge)
        {
            eprintln!("Learning hook failed: {}", e);
        }

        Ok(())
    })
}

// ── Doctor command ──────────────────────────────────────────────────────

fn cmd_doctor(config: &Config, project: Option<&str>, auto_fix: bool, verbose: bool) -> Result<()> {
    let projects_to_check = if let Some(proj) = project {
        vec![proj.to_string()]
    } else {
        // Check all projects
        parser::discovery::discover_projects(&config.claude_projects_dir)?
            .into_iter()
            .map(|p| p.name)
            .collect()
    };

    println!("{}", "🏥 Memory Health Check".green().bold());
    println!("{}", "=".repeat(60));
    println!();

    for proj in &projects_to_check {
        let report = health::check_project_health(&config.memory_dir, proj)?;

        let status_color = report.health_color();
        println!(
            "\u{1f4ca} {} - Health: {}/100 ({})",
            proj.cyan().bold(),
            report.score.to_string().color(status_color),
            report.health_status().color(status_color)
        );

        if report.issues.is_empty() {
            println!("   {} No issues found!\n", "✓".green());
            continue;
        }

        // Group issues by severity
        let critical: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.severity == health::Severity::Critical)
            .collect();
        let warnings: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.severity == health::Severity::Warning)
            .collect();
        let info: Vec<_> = report
            .issues
            .iter()
            .filter(|i| i.severity == health::Severity::Info)
            .collect();

        for (severity_label, color, issues_list) in [
            ("CRITICAL", colored::Color::Red, critical),
            ("WARNING", colored::Color::Yellow, warnings),
            ("INFO", colored::Color::Cyan, info),
        ] {
            if !issues_list.is_empty() {
                println!(
                    "   {} {} issue(s):",
                    severity_label.color(color),
                    issues_list.len()
                );
                for issue in issues_list {
                    println!("     {} {}", "•".color(color), issue.description);
                    if verbose {
                        if let Some(ref cmd) = issue.fix_command {
                            println!("       Fix: {}", cmd.dimmed());
                        }
                    }
                }
            }
        }

        if !report.recommendations.is_empty() && verbose {
            println!("   💡 Recommendations:");
            for rec in &report.recommendations {
                println!("     • {}", rec.dimmed());
            }
        }

        println!();

        // Auto-fix if requested
        if auto_fix {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| MemoryError::Config(format!("tokio runtime: {}", e)))?;

            println!("   \u{1f527} Auto-fixing issues...");
            let fixed = rt.block_on(health::auto_fix_issues(config, proj, &report.issues))?;

            for fix in &fixed {
                println!("     {} {}", "✓".green(), fix);
            }

            if fixed.is_empty() {
                println!("     {} No auto-fixable issues", "ℹ".cyan());
            }

            // Track learning signals from health improvements
            if !fixed.is_empty() {
                let updated_report = health::check_project_health(&config.memory_dir, proj)?;
                if let Err(e) =
                    learning::post_doctor_fix_hook(config, proj, report.score, updated_report.score)
                {
                    eprintln!("Learning hook failed: {}", e);
                }
            }

            println!();
        }
    }

    if !auto_fix {
        println!("💡 Run with {} to automatically fix issues", "--fix".cyan());
    }

    // Check installed packs health
    println!("{}", "📦 Installed Packs Health".green().bold());
    println!("{}", "=".repeat(60));
    println!();

    check_pack_health(&config.memory_dir, auto_fix, verbose)?;

    Ok(())
}

fn check_pack_health(memory_dir: &Path, auto_fix: bool, verbose: bool) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);
    let packs = installer.list()?;

    if packs.is_empty() {
        println!("   {} No packs installed\n", "ℹ".cyan());
        return Ok(());
    }

    let mut total_issues = 0;

    for pack in &packs {
        print!("   {} {}... ", "●".blue(), pack.name.bold());

        let mut pack_issues = Vec::new();

        // Check 1: Manifest exists and is valid
        let manifest_path = pack.path.join(".pack/manifest.json");
        if !manifest_path.exists() {
            pack_issues.push("Missing manifest file");
        } else if hive::KnowledgePack::load(&pack.path).is_err() {
            pack_issues.push("Invalid manifest");
        }

        // Check 2: Knowledge directory exists
        let knowledge_dir = pack.path.join("knowledge");
        if !knowledge_dir.exists() {
            pack_issues.push("Missing knowledge directory");
        } else {
            // Check 3: At least one knowledge file exists
            let has_knowledge = [
                "patterns.md",
                "solutions.md",
                "workflows.md",
                "decisions.md",
                "preferences.md",
            ]
            .iter()
            .any(|f| knowledge_dir.join(f).exists());

            if !has_knowledge {
                pack_issues.push("No knowledge files found");
            }
        }

        // Check 4: Registry still exists
        let registry_path = memory_dir.join("hive/registries").join(&pack.registry);
        if !registry_path.exists() {
            pack_issues.push("Registry removed (orphaned pack)");
        }

        if pack_issues.is_empty() {
            println!("{}", "✓ Healthy".green());
        } else {
            println!("{} {} issue(s)", "⚠".yellow(), pack_issues.len());
            total_issues += pack_issues.len();

            if verbose || !pack_issues.is_empty() {
                for issue in &pack_issues {
                    println!("       {} {}", "•".yellow(), issue);
                }
            }

            if auto_fix {
                // Auto-fix: Re-download corrupted packs
                if pack_issues
                    .iter()
                    .any(|i| i.contains("Missing") || i.contains("Invalid"))
                {
                    println!("       \u{1f527} Attempting to repair...");

                    if let Err(e) = installer.update(&pack.name) {
                        println!("       {} Repair failed: {}", "✗".red(), e);
                    } else {
                        println!("       {} Repaired successfully", "✓".green());
                        total_issues -= pack_issues.len();
                    }
                }

                // Auto-fix: Remove orphaned packs
                if pack_issues.iter().any(|i| i.contains("orphaned")) {
                    println!("       \u{1f527} Removing orphaned pack...");

                    if let Err(e) = installer.uninstall(&pack.name) {
                        println!("       {} Removal failed: {}", "✗".red(), e);
                    } else {
                        println!("       {} Removed successfully", "✓".green());
                        total_issues -= pack_issues.len();
                    }
                }
            }
        }
    }

    println!();

    if total_issues == 0 {
        println!("   {} All packs healthy!", "✓".green().bold());
    } else {
        println!("   {} {} total issue(s) found", "⚠".yellow(), total_issues);

        if !auto_fix {
            println!(
                "   💡 Run with {} to attempt automatic repairs",
                "--fix".cyan()
            );
        }
    }

    println!();

    Ok(())
}

fn cmd_diff(
    config: &Config,
    project: &str,
    category: &str,
    version_id: Option<&str>,
    show_history: bool,
) -> Result<()> {
    let tracker = diff::VersionTracker::new(&config.memory_dir, project);

    if show_history {
        let versions = tracker.get_versions(category)?;

        if versions.is_empty() {
            println!(
                "{} No version history for {}/{}",
                "Not found:".yellow(),
                project,
                category
            );
            println!("\nVersions are created automatically when knowledge is updated.");
            return Ok(());
        }

        println!("{} Version History", "Knowledge:".green().bold());
        println!("{}", "=".repeat(60));
        println!(
            "Project: {} | Category: {}\n",
            project.cyan(),
            category.cyan()
        );

        for (i, v) in versions.iter().enumerate() {
            println!(
                "  {} {} ({})",
                format!("{}.", i + 1).dimmed(),
                v.version_id.cyan(),
                v.timestamp.format("%Y-%m-%d %H:%M:%S").to_string().dimmed()
            );
            println!(
                "     Hash: {} | Size: {} bytes",
                v.content_hash.dimmed(),
                v.size_bytes
            );
        }

        println!("\nTo compare versions:");
        println!(
            "  {}",
            format!(
                "claude-memory diff {} {} --version <version-id>",
                project, category
            )
            .cyan()
        );

        return Ok(());
    }

    // Load current knowledge
    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let current_file = knowledge_dir.join(format!("{}.md", category));

    if !current_file.exists() {
        eprintln!(
            "{} No current knowledge found for {}/{}",
            "Error:".red(),
            project,
            category
        );
        return Ok(());
    }

    let current_content = std::fs::read_to_string(&current_file)?;

    // Load comparison version
    let (old_content, version_label) = if let Some(vid) = version_id {
        (
            tracker.get_version_content(vid)?,
            format!("Version: {}", vid),
        )
    } else {
        match tracker.get_latest_version(category)? {
            Some(v) => (
                tracker.get_version_content(&v.version_id)?,
                format!("Latest version: {}", v.version_id),
            ),
            None => {
                println!("{} No previous versions to compare", "Info:".cyan());
                println!("The current content will be the baseline for future comparisons.");

                // Create first version
                tracker.track_version(category, &current_content)?;
                println!("{} Created baseline version", "Created:".green());

                return Ok(());
            }
        }
    };

    // Compute diff
    let diff_result = diff::compute_diff(&old_content, &current_content, category);

    println!("{}", "Knowledge Diff".green().bold());
    println!("{}", "=".repeat(60));
    println!("Project: {}", project.cyan());
    println!("{}\n", version_label.dimmed());

    if diff_result.is_empty() {
        println!("{}", "No changes detected".yellow());
    } else {
        println!("{}", diff_result);
    }

    Ok(())
}

fn cmd_analytics(project: Option<&str>, days: u32, detailed: bool, clear_old: bool) -> Result<()> {
    let home =
        std::env::var("HOME").map_err(|_| MemoryError::Config("HOME not set".to_string()))?;
    let memory_dir = std::path::PathBuf::from(home).join("memory");
    let tracker = analytics::EventTracker::new(&memory_dir);

    if clear_old {
        let removed = tracker.clear_old_events(days)?;
        println!(
            "\u{1f5d1}\u{fe0f} Removed {} old analytics file(s)",
            removed
        );
        if removed == 0 {
            println!("   (No files older than {} days)", days);
        }
        return Ok(());
    }

    let events = tracker.get_events(project, days)?;

    if events.is_empty() {
        println!("{}", "📊 No usage data found".yellow());
        println!("\nUsage is tracked automatically when you use commands like:");
        println!("  • claude-memory recall <project>");
        println!("  • claude-memory search <query>");
        println!("  • claude-memory add <project> ...");
        println!("\nStart using the system and check back later!");
        return Ok(());
    }

    if detailed {
        println!("{}", "📋 Detailed Event Log".green().bold());
        println!("{}", "=".repeat(60));
        println!();

        for (i, event) in events.iter().take(50).enumerate() {
            let event_icon = match event.event_type {
                analytics::EventType::Recall => "🔍",
                analytics::EventType::Search => "🔎",
                analytics::EventType::Lookup => "📖",
                analytics::EventType::Add => "➕",
                analytics::EventType::Promote => "⬆️",
                analytics::EventType::Forget => "🗑️",
                analytics::EventType::Export => "📤",
                analytics::EventType::GraphQuery => "🕸️",
                analytics::EventType::SemanticSearch => "🧠",
            };

            println!(
                "{:3}. {} {} {} - {}",
                i + 1,
                event_icon,
                format!("{:?}", event.event_type).cyan(),
                event.project.yellow(),
                event
                    .timestamp
                    .format("%Y-%m-%d %H:%M:%S")
                    .to_string()
                    .dimmed()
            );

            if let Some(ref query) = event.query {
                println!("      Query: {}", query.dimmed());
            }
            if let Some(count) = event.results_count {
                println!("      Results: {}", count.to_string().dimmed());
            }
        }

        if events.len() > 50 {
            println!("\n... and {} more events", events.len() - 50);
        }
    } else {
        let insights = analytics::generate_insights(&events);
        print!("{}", analytics::insights::format_insights(&insights));
    }

    println!();
    Ok(())
}

// ── Learning commands ───────────────────────────────────────────────────

fn cmd_learn_dashboard(config: &Config, project: Option<&str>) -> Result<()> {
    use learning::progress;

    if let Some(project) = project {
        // Show dashboard for specific project
        let state = progress::load_state(&config.memory_dir, project)?;
        learning::dashboard::display_dashboard(&state);
        learning::dashboard::suggest_interventions(&state);
    } else {
        // Show dashboard for all projects
        let learning_dir = config.memory_dir.join("learning");
        if !learning_dir.exists() {
            println!("{}", "No learning data found.".yellow());
            println!(
                "Run {} to start learning from usage patterns.",
                "claude-memory ingest".cyan()
            );
            return Ok(());
        }

        for entry in std::fs::read_dir(&learning_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(project_name) = path.file_stem().and_then(|s| s.to_str()) {
                    let state = progress::load_state(&config.memory_dir, project_name)?;
                    learning::dashboard::display_dashboard(&state);
                    println!();
                }
            }
        }
    }

    Ok(())
}

fn cmd_learn_optimize(config: &Config, project: &str, dry_run: bool, auto: bool) -> Result<()> {
    use learning::{adaptation, progress};

    println!(
        "{}",
        format!("Learning Optimization: {}", project).bold().cyan()
    );
    println!("{}", "=".repeat(60).cyan());

    // Load learning state
    let state = progress::load_state(&config.memory_dir, project)?;

    if state.learned_parameters.importance_boosts.is_empty()
        && state.learned_parameters.ttl_adjustments.is_empty()
        && state.learned_parameters.consolidation_strategy.is_none()
    {
        println!("\n{}", "No learned optimizations available yet.".yellow());
        println!("The system needs more usage data to learn patterns.");
        println!(
            "Continue using {} and {} to build learning data.",
            "recall".cyan(),
            "search".cyan()
        );
        return Ok(());
    }

    // Preview changes
    let preview = adaptation::preview_changes(config, project, &state)?;
    learning::dashboard::display_preview(&preview);

    if dry_run {
        println!(
            "{}",
            "\nDry run complete. Use --auto to apply changes.".dimmed()
        );
        return Ok(());
    }

    // Ask for confirmation unless --auto
    if !auto {
        use dialoguer::Confirm;
        let confirmed = Confirm::new()
            .with_prompt("Apply these optimizations?")
            .default(false)
            .interact()
            .map_err(|e| MemoryError::Config(format!("Confirmation cancelled: {}", e)))?;

        if !confirmed {
            println!("{}", "Optimization cancelled.".yellow());
            return Ok(());
        }
    }

    // Apply learned parameters
    let result = adaptation::apply_learned_parameters(config, project, &state)?;

    println!(
        "\n{}",
        "✓ Optimizations applied successfully".green().bold()
    );
    println!("  {} importance adjustments", result.importance_adjustments);
    println!("  {} TTL adjustments", result.ttl_adjustments);
    println!("  {} graph adjustments", result.graph_adjustments);
    if result.consolidation_updated {
        println!("  {} consolidation strategy updated", "✓".green());
    }

    Ok(())
}

fn cmd_learn_reset(config: &Config, project: &str) -> Result<()> {
    use dialoguer::Confirm;
    use learning::progress;

    println!(
        "{}",
        format!("Reset Learning State: {}", project).bold().yellow()
    );
    println!("{}", "=".repeat(60).yellow());

    let confirmed = Confirm::new()
        .with_prompt("This will reset all learned parameters and algorithms. History will be preserved. Continue?")
        .default(false)
        .interact()
        .map_err(|e| MemoryError::Config(format!("Confirmation cancelled: {}", e)))?;

    if !confirmed {
        println!("{}", "Reset cancelled.".yellow());
        return Ok(());
    }

    progress::reset_state(&config.memory_dir, project)?;

    println!("\n{}", "✓ Learning state reset successfully".green().bold());
    println!("  Learned parameters cleared");
    println!("  Algorithms reset to defaults");
    println!("  History preserved for analysis");

    Ok(())
}

fn cmd_learn_simulate(
    config: &Config,
    project: &str,
    sessions: usize,
    pattern: &str,
) -> Result<()> {
    println!(
        "{}",
        format!("🎲 Simulating {} sessions for '{}'...", sessions, project)
            .cyan()
            .bold()
    );
    println!("{}", "=".repeat(60).cyan());

    match pattern {
        "recall" => {
            println!(
                "Pattern: {} (recall events only)",
                "Recall-focused".yellow()
            );
            learning::simulation::simulate_recall_session(config, project, sessions)?;
        }
        "mixed" => {
            println!(
                "Pattern: {} (recall, search, lookup)",
                "Mixed usage".yellow()
            );
            learning::simulation::simulate_mixed_usage(config, project, sessions)?;
        }
        "high-frequency" => {
            println!(
                "Pattern: {} (repeated access to same knowledge)",
                "High-frequency".yellow()
            );
            learning::simulation::simulate_high_frequency_knowledge(
                config,
                project,
                "test-pattern",
                sessions,
            )?;
        }
        _ => return Err(MemoryError::Config(format!("Unknown pattern: {}", pattern))),
    }

    println!("\n{} {} events generated", "✓".green(), sessions);

    // Trigger learning
    println!("{} Extracting learning signals...", "Learning".cyan());
    learning::post_ingest_hook(config, project)?;

    println!("\n{} Simulation complete", "✓".green().bold());
    println!("\nNext steps:");
    println!(
        "  {} {}",
        "1.".dimmed(),
        format!("claude-memory learn dashboard {}", project).cyan()
    );
    println!(
        "  {} {}",
        "2.".dimmed(),
        format!("claude-memory learn optimize {} --dry-run", project).cyan()
    );

    Ok(())
}

fn parse_duration(s: &str) -> Result<chrono::Duration> {
    let s = s.trim();
    let (num_str, unit) = s.split_at(s.len().saturating_sub(1));
    let num: i64 = num_str
        .parse()
        .map_err(|_| crate::error::MemoryError::InvalidDuration(s.to_string()))?;

    match unit {
        "m" => Ok(chrono::Duration::minutes(num)),
        "h" => Ok(chrono::Duration::hours(num)),
        "d" => Ok(chrono::Duration::days(num)),
        "w" => Ok(chrono::Duration::weeks(num)),
        _ => Err(crate::error::MemoryError::InvalidDuration(s.to_string())),
    }
}

fn cmd_learn_feedback(
    config: &Config,
    project: &str,
    session: Option<&str>,
    helpful: bool,
    unhelpful: bool,
    comment: Option<&str>,
) -> Result<()> {
    use learning::outcome_signals::{save_outcome_signal, ExplicitFeedback};
    use learning::{OutcomeSignal, Sentiment};

    if !helpful && !unhelpful {
        return Err(MemoryError::Config(
            "Must specify either --helpful or --unhelpful".to_string(),
        ));
    }

    let sentiment = if helpful {
        Sentiment::Helpful
    } else {
        Sentiment::Unhelpful
    };

    let session_id = session.unwrap_or("current").to_string();

    // Create feedback signal
    let feedback = ExplicitFeedback {
        timestamp: chrono::Utc::now(),
        session_id: session_id.clone(),
        project: project.to_string(),
        knowledge_ids: vec![], // Would be populated from actual usage
        sentiment,
        comment: comment.map(String::from),
    };

    let signal = OutcomeSignal::Explicit(feedback);

    // Save to disk
    save_outcome_signal(&config.memory_dir, &signal)?;

    // Trigger learning update
    learning::post_ingest_hook(config, project)?;

    println!("{}", "✓ Feedback recorded".green());
    println!("  Project: {}", project);
    println!("  Session: {}", session_id);
    println!("  Sentiment: {:?}", sentiment);
    if let Some(comment) = comment {
        println!("  Comment: {}", comment);
    }

    println!("\n💡 This feedback will improve future learning for this project");

    Ok(())
}

// ── Hive commands ───────────────────────────────────────────────────────

fn cmd_hive(command: HiveCommand) -> Result<()> {
    let home = dirs::home_dir()
        .ok_or_else(|| MemoryError::Config("Could not determine home directory".into()))?;
    let memory_dir = home.join("memory");

    match command {
        HiveCommand::Registry { command } => cmd_hive_registry(command, &memory_dir),
        HiveCommand::Pack { command } => cmd_hive_pack(command, &memory_dir),
        HiveCommand::Install {
            pack,
            registry,
            scope,
        } => cmd_hive_install(&pack, registry.as_deref(), &scope, &memory_dir),
        HiveCommand::Uninstall { pack } => cmd_hive_uninstall(&pack, &memory_dir),
        HiveCommand::List => cmd_hive_list(&memory_dir),
        HiveCommand::Update { pack } => cmd_hive_update(pack.as_deref(), &memory_dir),
        HiveCommand::Browse { category, keyword } => {
            cmd_hive_browse(category.as_deref(), keyword.as_deref(), &memory_dir)
        }
        HiveCommand::Search { query } => cmd_hive_search(&query, &memory_dir),
    }
}

fn cmd_hive_registry(command: RegistryCommand, memory_dir: &Path) -> Result<()> {
    use hive::RegistryManager;

    let manager = RegistryManager::new(memory_dir);

    match command {
        RegistryCommand::Add { url } => {
            println!("{} Adding registry: {}", "→".blue(), url);
            let registry = manager.add(&url)?;
            println!(
                "{} Registry '{}' added successfully",
                "✓".green(),
                registry.name
            );
            println!("  URL: {}", registry.url);
        }
        RegistryCommand::Remove { name } => {
            println!("{} Removing registry: {}", "→".blue(), name);
            manager.remove(&name)?;
            println!("{} Registry '{}' removed", "✓".green(), name);
        }
        RegistryCommand::List => {
            let registries = manager.list()?;
            if registries.is_empty() {
                println!("No registries configured.");
                println!("\nAdd a registry with:");
                println!("  claude-memory hive registry add owner/repo");
                return Ok(());
            }

            println!("Knowledge Pack Registries:\n");
            for reg in registries {
                println!("  {} {}", "●".blue(), reg.name.bold());
                println!("    URL: {}", reg.url);
                if let Some(updated) = reg.last_updated {
                    println!("    Last updated: {}", updated.format("%Y-%m-%d %H:%M:%S"));
                }
                println!();
            }
        }
        RegistryCommand::Update { name } => {
            if let Some(name) = name {
                println!("{} Updating registry: {}", "→".blue(), name);
                manager.update(&name)?;
                println!("{} Registry '{}' updated", "✓".green(), name);
            } else {
                println!("{} Updating all registries", "→".blue());
                let registries = manager.list()?;
                for reg in registries {
                    print!("  {} {}... ", "→".blue(), reg.name);
                    manager.update(&reg.name)?;
                    println!("{}", "✓".green());
                }
                println!("\n{} All registries updated", "✓".green());
            }
        }
    }

    Ok(())
}

fn cmd_hive_install(
    pack: &str,
    registry: Option<&str>,
    _scope: &str,
    memory_dir: &Path,
) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);

    println!("{} Installing pack: {}", "→".blue(), pack.bold());
    if let Some(reg) = registry {
        println!("  Registry: {}", reg);
    }

    let installed = installer.install(pack, registry)?;

    println!(
        "{} Pack '{}' installed successfully",
        "✓".green(),
        installed.name
    );
    println!("  Version: {}", installed.version);
    println!("  Registry: {}", installed.registry);
    println!(
        "  Installed at: {}",
        installed.installed_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("  Path: {}", installed.path.display());

    println!("\n💡 Use 'claude-memory recall' to access this pack's knowledge");

    Ok(())
}

fn cmd_hive_uninstall(pack: &str, memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);

    println!("{} Uninstalling pack: {}", "→".blue(), pack.bold());
    installer.uninstall(pack)?;

    println!("{} Pack '{}' uninstalled successfully", "✓".green(), pack);

    Ok(())
}

fn cmd_hive_list(memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);
    let packs = installer.list()?;

    if packs.is_empty() {
        println!("No packs installed.");
        println!("\nBrowse available packs with:");
        println!("  claude-memory hive browse");
        println!("\nInstall a pack with:");
        println!("  claude-memory hive install <pack-name>");
        return Ok(());
    }

    println!("Installed Knowledge Packs:\n");
    for pack in packs {
        println!("  {} {}", "●".green(), pack.name.bold());
        println!("    Version: {}", pack.version);
        println!("    Registry: {}", pack.registry);
        println!(
            "    Installed: {}",
            pack.installed_at.format("%Y-%m-%d %H:%M:%S")
        );
        println!("    Path: {}", pack.path.display());
        println!();
    }

    Ok(())
}

fn cmd_hive_update(pack: Option<&str>, memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);

    if let Some(pack_name) = pack {
        println!("{} Updating pack: {}", "→".blue(), pack_name.bold());
        installer.update(pack_name)?;
        println!("{} Pack '{}' updated successfully", "✓".green(), pack_name);
    } else {
        println!("{} Updating all installed packs", "→".blue());
        let packs = installer.list()?;

        if packs.is_empty() {
            println!("No packs installed.");
            return Ok(());
        }

        for pack in packs {
            print!("  {} {}... ", "→".blue(), pack.name);
            match installer.update(&pack.name) {
                Ok(_) => println!("{}", "✓".green()),
                Err(e) => println!("{} {}", "✗".red(), e),
            }
        }

        println!("\n{} All packs updated", "✓".green());
    }

    Ok(())
}

fn cmd_hive_browse(category: Option<&str>, keyword: Option<&str>, memory_dir: &Path) -> Result<()> {
    use hive::{PackCategory, PackInstaller, RegistryManager};
    use std::str::FromStr;

    let registry_manager = RegistryManager::new(memory_dir);
    let installer = PackInstaller::new(memory_dir);

    let registries = registry_manager.list()?;
    if registries.is_empty() {
        println!("No registries configured.");
        println!("\nAdd a registry with:");
        println!("  claude-memory hive registry add owner/repo");
        return Ok(());
    }

    // Collect all packs from all registries
    let mut all_packs = Vec::new();
    for registry in registries {
        match registry_manager.discover_packs(&registry.name) {
            Ok(packs) => {
                for pack in packs {
                    all_packs.push((registry.name.clone(), pack));
                }
            }
            Err(e) => {
                eprintln!(
                    "Warning: Failed to discover packs in '{}': {}",
                    registry.name, e
                );
            }
        }
    }

    // Filter by category if specified
    if let Some(cat_str) = category {
        let cat = PackCategory::from_str(cat_str)?;
        all_packs.retain(|(_, pack)| pack.has_category(&cat));
    }

    // Filter by keyword if specified
    if let Some(kw) = keyword {
        all_packs.retain(|(_, pack)| pack.matches_keyword(kw));
    }

    if all_packs.is_empty() {
        println!("No packs found matching criteria.");
        return Ok(());
    }

    // Get installed packs for status display
    let installed_packs = installer.list()?;
    let installed_names: std::collections::HashSet<_> =
        installed_packs.iter().map(|p| p.name.as_str()).collect();

    println!("Available Knowledge Packs:\n");
    for (registry_name, pack) in all_packs {
        let status = if installed_names.contains(pack.name.as_str()) {
            format!("[{}]", "INSTALLED".green())
        } else {
            format!("[{}]", "available".dimmed())
        };

        println!("  {} {} {}", "●".blue(), pack.name.bold(), status);
        println!("    Description: {}", pack.description);
        println!(
            "    Categories: {}",
            pack.categories
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("    Registry: {}", registry_name);
        println!("    Version: {}", pack.version);
        if !pack.keywords.is_empty() {
            println!("    Keywords: {}", pack.keywords.join(", "));
        }
        println!();
    }

    println!("\n💡 Install a pack with:");
    println!("  claude-memory hive install <pack-name>");

    Ok(())
}

fn cmd_hive_search(query: &str, memory_dir: &Path) -> Result<()> {
    use hive::{PackInstaller, RegistryManager};

    let registry_manager = RegistryManager::new(memory_dir);
    let installer = PackInstaller::new(memory_dir);

    println!("{} Searching for: {}", "→".blue(), query.bold());

    let results = registry_manager.search_packs(query)?;

    if results.is_empty() {
        println!("\nNo packs found matching '{}'", query);
        return Ok(());
    }

    // Get installed packs for status display
    let installed_packs = installer.list()?;
    let installed_names: std::collections::HashSet<_> =
        installed_packs.iter().map(|p| p.name.as_str()).collect();

    println!("\nSearch Results:\n");
    for (registry_name, packs) in results {
        println!("From registry '{}':", registry_name.bold());
        for pack in packs {
            let status = if installed_names.contains(pack.name.as_str()) {
                format!("[{}]", "INSTALLED".green())
            } else {
                format!("[{}]", "available".dimmed())
            };

            println!("  {} {} {}", "●".blue(), pack.name.bold(), status);
            println!("    {}", pack.description);
            if !pack.keywords.is_empty() {
                println!("    Keywords: {}", pack.keywords.join(", "));
            }
            println!();
        }
    }

    println!("💡 Install a pack with:");
    println!("  claude-memory hive install <pack-name>");

    Ok(())
}

fn cmd_hive_pack(command: PackCommand, memory_dir: &Path) -> Result<()> {
    match command {
        PackCommand::Create {
            name,
            project,
            description,
            author,
            keywords,
            categories,
            output,
        } => cmd_hive_pack_create(
            &name,
            &project,
            description.as_deref(),
            author.as_deref(),
            keywords.as_deref(),
            categories.as_deref(),
            output.as_deref(),
            memory_dir,
        ),
        PackCommand::Stats { name } => cmd_hive_pack_stats(&name, memory_dir),
        PackCommand::Publish {
            path,
            repo,
            push,
            message,
            skip_security,
        } => cmd_hive_pack_publish(
            &path,
            repo.as_deref(),
            push,
            message.as_deref(),
            skip_security,
        ),
        PackCommand::Validate { path } => cmd_hive_pack_validate(&path),
    }
}

#[allow(clippy::too_many_arguments)]
fn cmd_hive_pack_create(
    name: &str,
    project: &str,
    description: Option<&str>,
    author_name: Option<&str>,
    keywords_str: Option<&str>,
    categories_str: Option<&str>,
    output_dir: Option<&str>,
    memory_dir: &Path,
) -> Result<()> {
    use hive::{Author, KnowledgePack, PackCategory, PrivacyPolicy};
    use std::str::FromStr;

    println!("{} Creating knowledge pack: {}", "→".blue(), name.bold());

    // Verify source project exists
    let source_knowledge = memory_dir.join("knowledge").join(project);
    if !source_knowledge.exists() {
        return Err(MemoryError::Config(format!(
            "Project '{}' not found. Run 'ingest' first.",
            project
        )));
    }

    // Determine output directory
    let pack_dir = if let Some(out) = output_dir {
        PathBuf::from(out)
    } else {
        std::env::current_dir()?.join("packs").join(name)
    };

    if pack_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Pack directory already exists: {}",
            pack_dir.display()
        )));
    }

    // Create pack structure
    std::fs::create_dir_all(&pack_dir)?;
    std::fs::create_dir_all(pack_dir.join(".pack"))?;
    std::fs::create_dir_all(pack_dir.join("knowledge"))?;

    // Collect metadata (with prompts if not provided)
    let desc = description
        .map(String::from)
        .unwrap_or_else(|| format!("Knowledge pack from {}", project));

    let author = Author::new(
        author_name
            .map(String::from)
            .unwrap_or_else(|| "Anonymous".to_string()),
    );

    let keywords: Vec<String> = keywords_str
        .map(|s| s.split(',').map(|k| k.trim().to_string()).collect())
        .unwrap_or_default();

    let categories: Vec<PackCategory> = categories_str
        .map(|s| {
            s.split(',')
                .filter_map(|c| PackCategory::from_str(c.trim()).ok())
                .collect()
        })
        .unwrap_or_else(|| vec![PackCategory::Patterns, PackCategory::Solutions]);

    // Create manifest
    let mut pack = KnowledgePack::new(
        name.to_string(),
        desc,
        author,
        format!("https://github.com/user/{}", name),
    );
    pack.keywords = keywords;
    pack.categories = categories.clone();

    // Save manifest
    pack.save(&pack_dir)?;

    // Copy knowledge files based on privacy settings and categories
    let privacy = PrivacyPolicy::default();
    let knowledge_dest = pack_dir.join("knowledge");

    for (category_name, should_include) in [
        ("patterns.md", privacy.share_patterns),
        ("solutions.md", privacy.share_solutions),
        ("decisions.md", privacy.share_decisions),
        ("preferences.md", privacy.share_preferences),
    ] {
        if should_include {
            let source_file = source_knowledge.join(category_name);
            let dest_file = knowledge_dest.join(category_name);

            if source_file.exists() {
                std::fs::copy(&source_file, &dest_file)?;
                println!("  {} Copied {}", "✓".green(), category_name);
            }
        }
    }

    // Scan for secrets
    println!("\n{} Scanning for secrets...", "→".blue());
    let detector = hive::SecretDetector::new()?;
    let secrets = detector.scan_directory(&knowledge_dest)?;

    if !secrets.is_empty() {
        println!("\n{} Secrets detected!", "✗".red().bold());
        println!("\nThe following potential secrets were found:\n");

        for secret in &secrets {
            println!(
                "  {} {}:{}",
                "●".red(),
                secret.file_path,
                secret.line_number
            );
            println!("    Type: {}", secret.pattern_name.yellow());
            println!("    Match: {}", secret.matched_text.dimmed());
            println!();
        }

        println!("{}", "Pack creation blocked for security.".red().bold());
        println!("\nPlease review and remove secrets, then try again.");

        // Clean up
        std::fs::remove_dir_all(&pack_dir)?;

        return Err(MemoryError::Config(format!(
            "{} secret(s) detected",
            secrets.len()
        )));
    }

    println!("  {} No secrets detected", "✓".green());

    // Create README
    let readme_content = format!(
        "# {}\n\n{}\n\n## Installation\n\n```bash\nclaude-memory hive install {}\n```\n\n## Contents\n\n",
        name, pack.description, name
    );
    std::fs::write(pack_dir.join("README.md"), readme_content)?;

    println!("\n{} Pack created successfully!", "✓".green());
    println!("  Location: {}", pack_dir.display());
    println!(
        "  Categories: {}",
        categories
            .iter()
            .map(|c| c.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!("\n💡 Next steps:");
    println!("  1. Review content: cd {}", pack_dir.display());
    println!("  2. Initialize git: git init && git add . && git commit -m 'Initial pack'");
    println!("  3. Push to GitHub: git remote add origin <url> && git push");
    println!("  4. Share: claude-memory hive registry add <owner>/<repo>");

    Ok(())
}

fn cmd_hive_pack_stats(name: &str, memory_dir: &Path) -> Result<()> {
    use hive::PackInstaller;

    let installer = PackInstaller::new(memory_dir);
    let packs = installer.list()?;

    let pack = packs
        .iter()
        .find(|p| p.name == name)
        .ok_or_else(|| MemoryError::Config(format!("Pack '{}' not installed", name)))?;

    println!("{} Pack Statistics: {}", "→".blue(), pack.name.bold());
    println!();

    // Load manifest
    let manifest_path = pack.path.join(".pack/manifest.json");
    if let Ok(content) = std::fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&content) {
            println!("  {} {}", "Name:".bold(), pack.name);
            println!("  {} {}", "Version:".bold(), pack.version);
            println!("  {} {}", "Registry:".bold(), pack.registry);

            if let Some(desc) = manifest.get("description").and_then(|v| v.as_str()) {
                println!("  {} {}", "Description:".bold(), desc);
            }
        }
    }

    println!();

    // Knowledge statistics
    let knowledge_dir = pack.path.join("knowledge");
    if knowledge_dir.exists() {
        println!("  {}", "Knowledge:".bold());

        let mut total_entries = 0;
        let mut total_size = 0;

        for category in &[
            "patterns.md",
            "solutions.md",
            "workflows.md",
            "decisions.md",
            "preferences.md",
        ] {
            let file_path = knowledge_dir.join(category);
            if file_path.exists() {
                if let Ok(content) = std::fs::read_to_string(&file_path) {
                    let entry_count = content.matches("## Session:").count();
                    let size = content.len();

                    total_entries += entry_count;
                    total_size += size;

                    if entry_count > 0 {
                        println!(
                            "    {} {} entries ({} KB)",
                            category.replace(".md", "").cyan(),
                            entry_count,
                            size / 1024
                        );
                    }
                }
            }
        }

        println!();
        println!("  {} {} entries", "Total:".bold(), total_entries);
        println!("  {} {} KB", "Size:".bold(), total_size / 1024);
    }

    println!();
    println!(
        "  {} {}",
        "Installed:".bold(),
        pack.installed_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("  {} {}", "Path:".bold(), pack.path.display());

    Ok(())
}

fn cmd_hive_pack_publish(
    pack_path: &str,
    repo_url: Option<&str>,
    do_push: bool,
    commit_msg: Option<&str>,
    skip_security: bool,
) -> Result<()> {
    use std::path::Path;

    let pack_dir = Path::new(pack_path);

    if !pack_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Pack directory not found: {}",
            pack_path
        )));
    }

    println!("{} Publishing knowledge pack", "→".blue());
    println!("  Path: {}", pack_dir.display());

    // Step 1: Validate pack structure
    println!("\n{} Validating pack structure...", "→".blue());
    validate_pack_structure(pack_dir)?;
    println!("  {} Pack structure valid", "✓".green());

    // Step 2: Load manifest
    let pack = hive::KnowledgePack::load(pack_dir)?;
    println!(
        "  {} Loaded manifest: {} v{}",
        "✓".green(),
        pack.name,
        pack.version
    );

    // Step 3: Security scan (unless skipped)
    if !skip_security {
        println!("\n{} Scanning for secrets...", "→".blue());
        let detector = hive::SecretDetector::new()?;
        let knowledge_dir = pack_dir.join("knowledge");

        if knowledge_dir.exists() {
            let secrets = detector.scan_directory(&knowledge_dir)?;

            if !secrets.is_empty() {
                println!("\n{} Secrets detected!", "✗".red().bold());
                println!("\nThe following potential secrets were found:\n");

                for secret in &secrets {
                    println!(
                        "  {} {}:{}",
                        "●".red(),
                        secret.file_path,
                        secret.line_number
                    );
                    println!("    Type: {}", secret.pattern_name.yellow());
                    println!("    Match: {}", secret.matched_text.dimmed());
                    println!();
                }

                println!("{}", "Publishing blocked for security.".red().bold());
                println!("\nPlease review and remove secrets, then try again.");
                println!(
                    "Use {} to skip this check (NOT RECOMMENDED)",
                    "--skip-security".yellow()
                );

                return Err(MemoryError::Config(format!(
                    "{} secret(s) detected",
                    secrets.len()
                )));
            }

            println!("  {} No secrets detected", "✓".green());
        }
    } else {
        println!("\n{} Skipping security scan", "⚠".yellow().bold());
    }

    // Step 4: Initialize or verify git repo
    println!("\n{} Checking git repository...", "→".blue());

    let is_git_repo = pack_dir.join(".git").exists();

    if !is_git_repo {
        println!("  {} Initializing git repository...", "→".blue());

        let status = std::process::Command::new("git")
            .args(["init"])
            .current_dir(pack_dir)
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(
                "Failed to initialize git repository".into(),
            ));
        }

        println!("  {} Git repository initialized", "✓".green());

        // Create .gitignore
        std::fs::write(pack_dir.join(".gitignore"), "*.tmp\n*.swp\n.DS_Store\n")?;
    } else {
        println!("  {} Git repository exists", "✓".green());
    }

    // Step 5: Commit changes
    println!("\n{} Committing changes...", "→".blue());

    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(pack_dir)
        .status()?;

    let default_msg = format!("Update {} v{}", pack.name, pack.version);
    let message = commit_msg.unwrap_or(&default_msg);

    let commit_status = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(pack_dir)
        .status()?;

    if commit_status.success() {
        println!("  {} Changes committed", "✓".green());
    } else {
        println!("  {} No changes to commit", "ℹ".cyan());
    }

    // Step 6: Set up remote if provided
    if let Some(url) = repo_url {
        println!("\n{} Setting up remote repository...", "→".blue());

        // Check if remote exists
        let has_remote = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(pack_dir)
            .status()?
            .success();

        if !has_remote {
            println!("  {} Adding remote: {}", "→".blue(), url);

            let status = std::process::Command::new("git")
                .args(["remote", "add", "origin", url])
                .current_dir(pack_dir)
                .status()?;

            if !status.success() {
                return Err(MemoryError::Config("Failed to add git remote".into()));
            }

            println!("  {} Remote added", "✓".green());
        } else {
            println!("  {} Remote already configured", "✓".green());
        }
    }

    // Step 7: Push if requested
    if do_push {
        println!("\n{} Pushing to remote...", "→".blue());

        let status = std::process::Command::new("git")
            .args(["push", "-u", "origin", "HEAD"])
            .current_dir(pack_dir)
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(
                "Failed to push to remote. Check git remote configuration.".into(),
            ));
        }

        println!("  {} Pushed successfully", "✓".green());
    }

    // Step 8: Tag version
    println!("\n{} Creating version tag...", "→".blue());

    let tag = format!("v{}", pack.version);
    let tag_status = std::process::Command::new("git")
        .args(["tag", "-a", &tag, "-m", &format!("Release {}", tag)])
        .current_dir(pack_dir)
        .status()?;

    if tag_status.success() {
        println!("  {} Tagged as {}", "✓".green(), tag.cyan());

        if do_push {
            std::process::Command::new("git")
                .args(["push", "origin", &tag])
                .current_dir(pack_dir)
                .status()?;
            println!("  {} Tag pushed", "✓".green());
        }
    }

    println!("\n{} Pack published successfully!", "✓".green().bold());
    println!("\n💡 Share your pack:");
    if let Some(url) = repo_url {
        println!("  Users can install with:");
        println!(
            "  {}",
            format!("claude-memory hive registry add {}", url).cyan()
        );
    } else {
        println!("  1. Push to GitHub: git push -u origin main");
        println!("  2. Share the repository URL");
        println!("  3. Users can add: claude-memory hive registry add <owner>/<repo>");
    }

    Ok(())
}

fn cmd_hive_pack_validate(pack_path: &str) -> Result<()> {
    use std::path::Path;

    let pack_dir = Path::new(pack_path);

    println!("{} Validating pack: {}", "→".blue(), pack_dir.display());
    println!();

    validate_pack_structure(pack_dir)?;

    println!("{} Pack is valid!", "✓".green().bold());

    Ok(())
}

fn validate_pack_structure(pack_dir: &Path) -> Result<()> {
    // Check 1: Directory exists
    if !pack_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Pack directory not found: {}",
            pack_dir.display()
        )));
    }

    // Check 2: Manifest exists and is valid
    let manifest_path = pack_dir.join(".pack/manifest.json");
    if !manifest_path.exists() {
        return Err(MemoryError::Config(
            "Missing .pack/manifest.json file".into(),
        ));
    }

    let _pack = hive::KnowledgePack::load(pack_dir)?;

    // Check 3: Knowledge directory exists
    let knowledge_dir = pack_dir.join("knowledge");
    if !knowledge_dir.exists() {
        return Err(MemoryError::Config("Missing knowledge/ directory".into()));
    }

    // Check 4: At least one knowledge file exists
    let has_knowledge = [
        "patterns.md",
        "solutions.md",
        "workflows.md",
        "decisions.md",
        "preferences.md",
    ]
    .iter()
    .any(|f| knowledge_dir.join(f).exists());

    if !has_knowledge {
        return Err(MemoryError::Config(
            "No knowledge files found in knowledge/ directory".into(),
        ));
    }

    // Check 5: README exists
    if !pack_dir.join("README.md").exists() {
        println!("  {} README.md missing (recommended)", "⚠".yellow());
    }

    // Check 6: Categories match available knowledge
    let mut found_categories = Vec::new();
    for (file, category) in [
        ("patterns.md", "Patterns"),
        ("solutions.md", "Solutions"),
        ("workflows.md", "Workflows"),
        ("decisions.md", "Decisions"),
        ("preferences.md", "Preferences"),
    ] {
        if knowledge_dir.join(file).exists() {
            found_categories.push(category);
        }
    }

    println!("  {} Manifest valid", "✓".green());
    println!("  {} Knowledge directory exists", "✓".green());
    println!(
        "  {} Found categories: {}",
        "✓".green(),
        found_categories.join(", ")
    );

    Ok(())
}
