use std::path::Path;

use colored::Colorize;

use crate::analytics;
use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::extractor;
use crate::hive;
use crate::learning;
use crate::mcp;
use crate::parser;
use crate::renderer;
use crate::state;

// ── Core commands ───────────────────────────────────────────────────────

pub fn cmd_ingest(
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
            return Err(MemoryError::InvalidDuration(format!(
                "Invalid TTL: '{}'. Use format like 30m, 2h, 7d, 2w",
                ttl_val
            )));
        }
    }

    let since_duration = since.map(|s| crate::parse_duration(&s)).transpose()?;

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

pub fn cmd_search(
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

pub fn cmd_recall(config: &Config, project: &str) -> Result<()> {
    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let context_path = knowledge_dir.join("context.md");

    // Get local project knowledge
    let local_content = if context_path.exists() {
        Some(std::fs::read_to_string(&context_path)?)
    } else {
        crate::build_raw_context(project, &knowledge_dir)
    };

    // Get knowledge from installed packs
    let pack_content = hive::get_installed_pack_knowledge(&config.memory_dir)?;

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

pub fn cmd_context(config: &Config, project: &str) -> Result<()> {
    let knowledge_dir = config.memory_dir.join("knowledge").join(project);
    let context_path = knowledge_dir.join("context.md");

    // Raw stdout, no formatting — suitable for piping
    let content = if context_path.exists() {
        std::fs::read_to_string(&context_path)?
    } else {
        match crate::build_raw_context(project, &knowledge_dir) {
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

pub fn cmd_status(config: &Config) -> Result<()> {
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

    println!("{}", "Engram Status".green().bold());
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

pub fn cmd_projects(config: &Config) -> Result<()> {
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

pub fn cmd_mcp(config: &Config) -> Result<()> {
    let server = mcp::McpServer::new(config.clone());
    server.run()
}

pub fn cmd_export(
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
    output.push_str("**Tool:** [engram](https://github.com/Algiras/engram)\n\n");
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
        "tool": "engram",
        "tool_url": "https://github.com/Algiras/engram",
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
            <strong>Tool:</strong> <a href="https://github.com/Algiras/engram">engram</a>
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
            Generated by <a href="https://github.com/Algiras/engram">engram</a>
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
