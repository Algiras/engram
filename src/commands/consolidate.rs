use std::path::Path;

use colored::Colorize;

use crate::config::Config;
use crate::embeddings;
use crate::error::{MemoryError, Result};
use crate::health;
use crate::hive;
use crate::learning;
use crate::parser;

fn truncate_text(text: &str, max_len: usize) -> String {
    let cleaned = text.replace('\n', " ").trim().to_string();
    if cleaned.len() <= max_len {
        cleaned
    } else {
        format!("{}...", &cleaned[..max_len - 3])
    }
}

pub fn cmd_consolidate(
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
                "{} No embeddings found. Run 'engram embed {}' first.",
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

        // Contradiction detection via LLM on semantically related but non-identical chunks
        if find_contradictions {
            println!(
                "\n{} Checking for contradictions...",
                "Analyzing".green().bold()
            );

            use crate::embeddings::cosine_similarity;
            use crate::llm::client::LlmClient;
            use crate::llm::prompts;

            let llm_client = LlmClient::new(&config.llm);
            let mut contradiction_count = 0;

            // Check pairs with similarity in the "related but different" range
            let low = 0.5f32;
            let high = 0.92f32;

            for i in 0..store.chunks.len() {
                for j in (i + 1)..store.chunks.len() {
                    let sim =
                        cosine_similarity(&store.chunks[i].embedding, &store.chunks[j].embedding);
                    if sim < low || sim > high {
                        continue;
                    }

                    let a = &store.chunks[i];
                    let b = &store.chunks[j];
                    let snippet_a = format!(
                        "[{}:{}]\n{}",
                        a.metadata.category,
                        a.metadata.session_id.as_deref().unwrap_or(&a.id),
                        truncate_text(&a.text, 300)
                    );
                    let snippet_b = format!(
                        "[{}:{}]\n{}",
                        b.metadata.category,
                        b.metadata.session_id.as_deref().unwrap_or(&b.id),
                        truncate_text(&b.text, 300)
                    );

                    if let Ok(response) = llm_client
                        .chat(
                            prompts::SYSTEM_CONTRADICTION_CHECKER,
                            &prompts::contradiction_check_prompt(&snippet_a, &snippet_b),
                        )
                        .await
                    {
                        let resp = response.trim();
                        if resp != "No contradictions detected."
                            && !resp.is_empty()
                            && resp.contains("CONTRADICTS")
                        {
                            println!(
                                "  {} [{:.0}% similarity]\n    {}\n    vs\n    {}\n    {}",
                                "CONTRADICTION:".red().bold(),
                                sim * 100.0,
                                snippet_a.lines().next().unwrap_or("").cyan(),
                                snippet_b.lines().next().unwrap_or("").cyan(),
                                resp.dimmed()
                            );
                            contradiction_count += 1;
                        }
                    }
                }
            }

            if contradiction_count == 0 {
                println!("{} No contradictions detected.", "✓".green());
            } else {
                println!(
                    "\n{} {} potential contradiction(s) found. Review and update knowledge files manually.",
                    "Summary:".yellow(),
                    contradiction_count
                );
            }
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

pub fn cmd_doctor(
    config: &Config,
    project: Option<&str>,
    auto_fix: bool,
    verbose: bool,
) -> Result<()> {
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

    // System-level hook integrity check
    println!("{}", "🔗 System Health".green().bold());
    println!("{}", "=".repeat(60));
    println!();
    if let Some(home) = dirs::home_dir() {
        let hook_issues = health::check_hooks_health(&home);
        if hook_issues.is_empty() {
            println!("   {} All hooks registered\n", "✓".green());
        } else {
            for issue in &hook_issues {
                println!(
                    "   {} {} [{}]",
                    "✗".red(),
                    issue.description,
                    "CRITICAL".red()
                );
                if verbose {
                    if let Some(ref cmd) = issue.fix_command {
                        println!("       Fix: {}", cmd.dimmed());
                    }
                }
            }
            if auto_fix {
                print!("   {} Reinstalling hooks... ", "🔧".yellow());
                match crate::commands::hooks::cmd_hooks_install() {
                    Ok(()) => println!("{}", "ok".green()),
                    Err(e) => println!("{}: {}", "error".red(), e),
                }
            } else {
                println!("   💡 Run {} to fix", "engram hooks install".cyan());
            }
            println!();
        }
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
