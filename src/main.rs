#![allow(dead_code)]
mod analytics;
mod auth;
mod cli;
mod commands;
mod config;
mod daemon;
mod diff;
mod embeddings;
mod error;
mod extractor;
mod graph;
mod health;
mod hive;
mod inject;
mod learning;
mod llm;
mod mcp;
mod parser;
mod renderer;
mod state;
mod sync;
mod tui;
mod vcs;

use std::path::{Path, PathBuf};

use clap::Parser;
use cli::{
    AuthCommand, Cli, Commands, DaemonCommand, GraphCommand, HooksCommand, LearnCommand,
    MemCommand, SyncCommand,
};
use colored::Colorize;
use config::Config;
use error::Result;

use commands::ask::cmd_ask;
use commands::auth::{
    cmd_auth_embed, cmd_auth_embed_model, cmd_auth_list, cmd_auth_login, cmd_auth_logout,
    cmd_auth_model, cmd_auth_models, cmd_auth_status, cmd_auth_test,
};
use commands::consolidate::{cmd_consolidate, cmd_doctor};
use commands::core::{
    cmd_context, cmd_entities, cmd_export, cmd_ingest, cmd_mcp, cmd_projects, cmd_recall,
    cmd_search, cmd_status,
};
use commands::diff::{cmd_analytics, cmd_diff};
use commands::embeddings::{cmd_embed, cmd_search_semantic};
use commands::graph::{
    cmd_graph_build, cmd_graph_hubs, cmd_graph_path, cmd_graph_query, cmd_graph_viz,
};
use commands::heal::cmd_heal;
use commands::hive::cmd_hive;
use commands::hooks::{cmd_hooks_install, cmd_hooks_status, cmd_hooks_uninstall};
use commands::knowledge::{cmd_forget, cmd_regen};
use commands::learning::{
    cmd_learn_dashboard, cmd_learn_feedback, cmd_learn_optimize, cmd_learn_reset,
    cmd_learn_simulate,
};
use commands::manual::{cmd_add, cmd_lookup, cmd_promote, cmd_review};
use commands::observe::cmd_observe;
use commands::sync::{
    cmd_sync_clone, cmd_sync_history, cmd_sync_init_repo, cmd_sync_list, cmd_sync_pull,
    cmd_sync_pull_repo, cmd_sync_push, cmd_sync_push_repo,
};
use commands::vcs::{
    cmd_mem_branch, cmd_mem_checkout, cmd_mem_commit, cmd_mem_diff, cmd_mem_init, cmd_mem_log,
    cmd_mem_show, cmd_mem_stage, cmd_mem_status,
};

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
            AuthCommand::Test { provider } => cmd_auth_test(provider),
            AuthCommand::Model { provider, model } => cmd_auth_model(&provider, &model),
            AuthCommand::Embed { provider } => cmd_auth_embed(&provider),
            AuthCommand::Models { provider, embed } => cmd_auth_models(provider, embed),
            AuthCommand::EmbedModel { provider, model } => cmd_auth_embed_model(&provider, &model),
        };
    }

    // TUI operates on memory_dir directly — no Config/LLM auth needed
    if matches!(cli.command, Commands::Tui) {
        return cmd_tui();
    }

    // Inject operates on disk only (smart mode needs embed index but not LLM)
    if let Commands::Inject {
        project,
        full,
        no_auto_clean,
        smart,
        budget,
    } = cli.command
    {
        return cmd_inject(project, full, no_auto_clean, smart, budget);
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
        stale,
        auto: auto_approve,
        summarize,
    } = cli.command
    {
        return cmd_forget(
            &project,
            session_id,
            topic,
            all,
            purge,
            expired,
            stale,
            auto_approve,
            summarize,
        );
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

    // Daemon - status/stop/logs don't need LLM auth; start/run load Config themselves
    if let Commands::Daemon { command } = cli.command {
        return cmd_daemon(command);
    }

    // Observe - reads stdin, no Config/LLM needed
    if let Commands::Observe { project } = cli.command {
        return cmd_observe(project.as_deref());
    }

    // Mem (VCS) - filesystem only, no Config/LLM needed
    if let Commands::Mem { command } = cli.command {
        return match command {
            MemCommand::Init { project } => cmd_mem_init(project.as_deref()),
            MemCommand::Status { project } => cmd_mem_status(project.as_deref()),
            MemCommand::Stage {
                project,
                sessions,
                all,
            } => cmd_mem_stage(&project, &sessions, all),
            MemCommand::Commit {
                project,
                message,
                all,
                session,
            } => cmd_mem_commit(&project, &message, all, session.as_deref()),
            MemCommand::Log {
                project,
                limit,
                verbose,
                grep,
            } => cmd_mem_log(&project, limit, verbose, grep.as_deref()),
            MemCommand::Show {
                project,
                target,
                category,
            } => cmd_mem_show(&project, target.as_deref(), category.as_deref()),
            MemCommand::Branch {
                project,
                create,
                delete,
            } => cmd_mem_branch(&project, create.as_deref(), delete.as_deref()),
            MemCommand::Checkout {
                project,
                target,
                dry_run,
                force,
            } => cmd_mem_checkout(&project, &target, dry_run, force),
            MemCommand::Diff {
                project,
                from,
                to,
                category,
            } => cmd_mem_diff(
                &project,
                from.as_deref(),
                to.as_deref(),
                category.as_deref(),
            ),
        };
    }

    // Extract provider override for commands that support it
    let provider_override = match &cli.command {
        Commands::Ingest { provider, .. }
        | Commands::Regen { provider, .. }
        | Commands::Mcp { provider, .. }
        | Commands::Ask { provider, .. }
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
        return cmd_embed(&config, project, provider.as_deref(), cli.verbose);
    }

    // SearchSemantic command
    if let Commands::SearchSemantic {
        query,
        project,
        top,
        threshold,
        since,
        category,
        file,
    } = &cli.command
    {
        return cmd_search_semantic(
            &config,
            query,
            project.as_deref(),
            *top,
            *threshold,
            cli.verbose,
            since.as_deref(),
            category.as_deref(),
            file.as_deref(),
        );
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

    // Heal command
    if let Commands::Heal { check } = &cli.command {
        return cmd_heal(&config, *check);
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

    // Entities command — filesystem only, no LLM needed
    if let Commands::Entities { project } = &cli.command {
        return cmd_entities(&config, project);
    }

    // Ask command
    if let Commands::Ask {
        query,
        project,
        top_k,
        threshold,
        ..
    } = &cli.command
    {
        let project_name = project.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .ok()
                .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()))
                .unwrap_or_else(|| "default".to_string())
        });
        return cmd_ask(
            &config,
            query,
            &project_name,
            *top_k,
            *threshold,
            cli.verbose,
        );
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
            cmd_ingest(
                &config,
                force,
                dry_run,
                project,
                since,
                skip_knowledge,
                ttl,
                cli.verbose,
            )?;
        }
        Commands::Search {
            query,
            project,
            knowledge,
            context,
            global,
        } => {
            let effective_project = if global {
                Some(crate::config::GLOBAL_DIR.to_string())
            } else {
                project
            };
            cmd_search(&config, &query, effective_project, knowledge, context)?;
        }
        Commands::Recall { project } => {
            cmd_recall(&config, &project, cli.verbose)?;
        }
        Commands::Context { project } => {
            cmd_context(&config, &project, cli.verbose)?;
        }
        Commands::Status => {
            cmd_status(&config)?;
        }
        Commands::Projects => {
            cmd_projects(&config)?;
        }
        Commands::Regen {
            project,
            persist_cleanup,
            ..
        } => {
            cmd_regen(&config, &project, persist_cleanup, cli.verbose)?;
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
        | Commands::Hive { .. }
        | Commands::Daemon { .. }
        | Commands::Observe { .. }
        | Commands::Mem { .. }
        | Commands::Ask { .. }
        | Commands::Entities { .. }
        | Commands::Heal { .. } => {
            unreachable!()
        }
    }

    Ok(())
}

// ── Daemon command ───────────────────────────────────────────────────────

fn cmd_daemon(command: DaemonCommand) -> Result<()> {
    // Daemon status/stop/logs work without LLM auth — load Config directly
    let config = Config::load(None)?;
    match command {
        DaemonCommand::Start { interval, provider } => {
            daemon::cmd_daemon_start(&config, interval, provider.as_deref())
        }
        DaemonCommand::Stop => daemon::cmd_daemon_stop(&config),
        DaemonCommand::Status => daemon::cmd_daemon_status(&config),
        DaemonCommand::Logs { lines, follow } => daemon::cmd_daemon_logs(&config, lines, follow),
        DaemonCommand::Run { interval, provider } => {
            daemon::cmd_daemon_run(&config, interval, provider.as_deref())
        }
    }
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

fn cmd_inject(
    project: Option<String>,
    full: bool,
    no_auto_clean: bool,
    smart: bool,
    budget: usize,
) -> Result<()> {
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

    // Auto-cleanup expired entries before building MEMORY.md
    if !no_auto_clean {
        use crate::extractor::knowledge::auto_cleanup_expired;
        let cleanup = auto_cleanup_expired(&memory_dir, &project_name, false)?;
        if cleanup.removed_count > 0 {
            println!(
                "{} Auto-cleaned {} expired entries",
                "✓".green(),
                cleanup.removed_count
            );
        }
    }

    // Read project context (with fallback to raw knowledge files)
    let context_path = knowledge_dir.join(&project_name).join("context.md");
    let context_content = if context_path.exists() {
        std::fs::read_to_string(&context_path)?
    } else {
        match inject::build_raw_context(&project_name, &knowledge_dir.join(&project_name)) {
            Some(raw) => raw,
            None => {
                eprintln!(
                    "{} No knowledge found for '{}'. Run 'engram ingest' first.",
                    "Not found:".yellow(),
                    project_name
                );
                return Ok(());
            }
        }
    };

    // Read raw global preferences
    let preferences_path = knowledge_dir.join("_global").join("preferences.md");
    let raw_preferences = if preferences_path.exists() {
        Some(std::fs::read_to_string(&preferences_path)?)
    } else {
        None
    };

    // Read raw global shared memory
    let shared_path = knowledge_dir.join("_global").join("shared.md");
    let raw_shared = if shared_path.exists() {
        Some(std::fs::read_to_string(&shared_path)?)
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

    let (combined, mode) = if smart {
        let signal = inject::detect_work_context(&project_name);
        println!(
            "{} Context signal: {}",
            "Smart:".cyan(),
            &signal[..signal.len().min(100)]
        );
        let entries = inject::smart_search_sync(&project_name, &memory_dir, &signal, 20, 0.45)?;
        if entries.is_empty() {
            println!(
                "{} No embedding index found for '{}' — falling back to compact mode.",
                "Note:".yellow(),
                project_name
            );
            (
                inject::build_compact_memory(
                    &project_name,
                    &context_content,
                    &raw_preferences,
                    &raw_shared,
                    &memory_dir,
                )?,
                "compact (fallback)",
            )
        } else {
            let selected = entries.iter().filter(|e| e.selected).count();
            let tokens: usize = entries
                .iter()
                .filter(|e| e.selected)
                .map(|e| e.estimated_tokens())
                .sum();
            println!(
                "{} Selected {}/{} entries (~{}/{} tokens)",
                "Smart:".cyan(),
                selected,
                entries.len(),
                tokens,
                budget
            );
            (
                inject::format_smart_memory(&project_name, &signal, &entries, budget, &memory_dir)?,
                "smart",
            )
        }
    } else if full {
        (
            inject::build_full_memory(
                &project_name,
                &context_content,
                &raw_preferences,
                &raw_shared,
                &memory_dir,
            )?,
            "full",
        )
    } else {
        (
            inject::build_compact_memory(
                &project_name,
                &context_content,
                &raw_preferences,
                &raw_shared,
                &memory_dir,
            )?,
            "compact",
        )
    };

    // Write to MEMORY.md
    let memory_path = project_dir.join("memory");
    std::fs::create_dir_all(&memory_path)?;
    let memory_file = memory_path.join("MEMORY.md");
    let line_count = combined.lines().count();
    std::fs::write(&memory_file, &combined)?;

    println!(
        "{} Injected {} knowledge for '{}' ({} lines) into {}",
        "Done!".green().bold(),
        mode,
        project_name,
        line_count,
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
