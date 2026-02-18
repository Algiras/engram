use crate::config::Config;
use crate::error;
use crate::error::Result;
use crate::extractor;
use crate::llm;
use colored::Colorize;
use std::path::Path;

// ── Regen command ───────────────────────────────────────────────────────

pub fn cmd_regen(config: &Config, project: &str, persist_cleanup: bool) -> Result<()> {
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

    let (decisions, solutions, patterns) = if persist_cleanup {
        // Persist cleanup to disk before regenerating
        use crate::extractor::knowledge::auto_cleanup_expired;
        let cleanup = auto_cleanup_expired(&config.memory_dir, project, true)?;
        if cleanup.removed_count > 0 {
            println!(
                "{} Cleaned {} expired entries from files",
                "✓".green(),
                cleanup.removed_count
            );
        }

        // Re-read files after cleanup (already filtered)
        let decisions_raw = read_or_empty(&knowledge_dir.join("decisions.md"));
        let solutions_raw = read_or_empty(&knowledge_dir.join("solutions.md"));
        let patterns_raw = read_or_empty(&knowledge_dir.join("patterns.md"));

        (decisions_raw, solutions_raw, patterns_raw)
    } else {
        // Filter in-memory only (don't persist)
        let filter_expired = |content: &str| -> String {
            let (preamble, blocks) = parse_session_blocks(content);
            let (active, _expired) = partition_by_expiry(blocks);
            reconstruct_blocks(&preamble, &active)
        };

        let decisions_raw = read_or_empty(&knowledge_dir.join("decisions.md"));
        let solutions_raw = read_or_empty(&knowledge_dir.join("solutions.md"));
        let patterns_raw = read_or_empty(&knowledge_dir.join("patterns.md"));

        (
            filter_expired(&decisions_raw),
            filter_expired(&solutions_raw),
            filter_expired(&patterns_raw),
        )
    };
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

pub fn cmd_forget(
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
                format!("engram regen {}", project).cyan()
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
            format!("engram ingest --project {}", project).cyan()
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
            format!("engram ingest --project {}", project).cyan()
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
            format!("engram ingest --project {}", project).cyan()
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
        format!("engram forget {} <session-id>", project).cyan()
    );

    Ok(())
}
