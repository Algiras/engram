mod auth;
mod cli;
mod config;
mod error;
mod extractor;
mod llm;
mod parser;
mod renderer;
mod state;

use clap::Parser;
use cli::{AuthCommand, Cli, Commands};
use colored::Colorize;
use config::Config;
use error::Result;

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

    // Extract provider override for commands that support it
    let provider_override = match &cli.command {
        Commands::Ingest { provider, .. } => provider.as_deref(),
        _ => None,
    };

    let config = Config::load(provider_override)?;

    match cli.command {
        Commands::Ingest {
            force,
            dry_run,
            project,
            since,
            skip_knowledge,
            ..
        } => {
            cmd_ingest(&config, force, dry_run, project, since, skip_knowledge)?;
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
        Commands::Auth { .. } => unreachable!(),
    }

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

// ── Core commands ───────────────────────────────────────────────────────

fn cmd_ingest(
    config: &Config,
    force: bool,
    dry_run: bool,
    project_filter: Option<String>,
    since: Option<String>,
    skip_knowledge: bool,
) -> Result<()> {
    use indicatif::{ProgressBar, ProgressStyle};
    use rayon::prelude::*;

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

    // Process sessions (parallel for parsing, sequential for LLM)
    let results: Vec<_> = all_sessions
        .par_iter()
        .map(|(project_name, session)| {
            let result = process_session(config, project_name, session, skip_knowledge);
            pb.inc(1);
            (session.path.clone(), result)
        })
        .collect();

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

    Ok(())
}

fn cmd_recall(config: &Config, project: &str) -> Result<()> {
    let context_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("context.md");

    if !context_path.exists() {
        println!(
            "{} No context found for '{}'. Run 'ingest' first.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let content = std::fs::read_to_string(&context_path)?;
    println!("{}", content);
    Ok(())
}

fn cmd_context(config: &Config, project: &str) -> Result<()> {
    let context_path = config
        .memory_dir
        .join("knowledge")
        .join(project)
        .join("context.md");

    if !context_path.exists() {
        eprintln!("No context for project '{}'", project);
        std::process::exit(1);
    }

    // Raw stdout, no formatting — suitable for piping
    let content = std::fs::read_to_string(&context_path)?;
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
