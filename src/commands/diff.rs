use colored::Colorize;

use crate::analytics;
use crate::config::Config;
use crate::diff;
use crate::error::*;

pub fn cmd_diff(
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
                "engram diff {} {} --version <version-id>",
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

pub fn cmd_analytics(
    project: Option<&str>,
    days: u32,
    detailed: bool,
    clear_old: bool,
) -> Result<()> {
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
        println!("  • engram recall <project>");
        println!("  • engram search <query>");
        println!("  • engram add <project> ...");
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
                analytics::EventType::Context => "📄",
                analytics::EventType::Inject => "💉",
                analytics::EventType::Ingest => "📥",
                analytics::EventType::Ask => "❓",
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
