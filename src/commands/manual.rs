use std::path::Path;

use chrono;
use colored::Colorize;

use crate::error::{self, Result};
use crate::extractor;
use crate::hive;

// ── Review command ──────────────────────────────────────────────────────

pub fn cmd_review(project: &str, show_all: bool) -> Result<()> {
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
            "engram promote {} <session_id> <category> [--global]",
            project
        )
        .cyan()
    );

    Ok(())
}

// ── Promote command ─────────────────────────────────────────────────────

pub fn cmd_promote(
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
            format!("engram regen {}", project).cyan()
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

pub fn cmd_lookup(project: &str, query: &str, include_all: bool) -> Result<()> {
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

pub fn cmd_add(
    project: &str,
    category: &str,
    content: &str,
    label: &str,
    ttl: Option<&str>,
) -> Result<()> {
    use extractor::knowledge::parse_ttl;

    if category.is_empty() {
        return Err(error::MemoryError::Config(
            "Category cannot be empty".into(),
        ));
    }

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
                "bugs" => "bugs.md",
                "insights" => "insights.md",
                "questions" => "questions.md",
                _ => return Err(error::MemoryError::Config(format!(
                    "Unknown category: '{}'. Use: decisions, solutions, patterns, bugs, insights, questions, preferences",
                    category
                ))),
            },
        )
    };

    std::fs::create_dir_all(&dir)?;
    let path = dir.join(filename);

    // Initialize file if needed
    if !path.exists() {
        let title = if category.is_empty() {
            String::new()
        } else {
            let mut t = category.chars().next().unwrap().to_uppercase().to_string();
            t.push_str(&category[1..]);
            t
        };
        std::fs::write(&path, format!("# {}\n", title))?;
    }

    // Build header with timestamp and label
    let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let header = if let Some(ttl_val) = ttl {
        format!("\n\n## Session: {} ({}) [ttl:{}]\n\n", label, now, ttl_val)
    } else {
        format!("\n\n## Session: {} ({})\n\n", label, now)
    };

    // Dedup: replace existing session if same label already present
    use crate::extractor::knowledge::replace_session_block;
    use std::io::Write;
    let existing = std::fs::read_to_string(&path).unwrap_or_default();
    if let Some(replaced) = replace_session_block(&existing, label, &header, content) {
        std::fs::write(&path, replaced)?;
    } else {
        let mut file = std::fs::OpenOptions::new().append(true).open(&path)?;
        writeln!(file, "{}{}", header, content)?;
    }

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
        format!("engram regen {}", project).cyan()
    );

    Ok(())
}
