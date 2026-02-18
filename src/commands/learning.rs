use crate::config::Config;
use crate::error::{MemoryError, Result};
use crate::learning;
use colored::Colorize;

pub fn cmd_learn_dashboard(config: &Config, project: Option<&str>) -> Result<()> {
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
                "engram ingest".cyan()
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

pub fn cmd_learn_optimize(config: &Config, project: &str, dry_run: bool, auto: bool) -> Result<()> {
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

pub fn cmd_learn_reset(config: &Config, project: &str) -> Result<()> {
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

pub fn cmd_learn_simulate(
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
        format!("engram learn dashboard {}", project).cyan()
    );
    println!(
        "  {} {}",
        "2.".dimmed(),
        format!("engram learn optimize {} --dry-run", project).cyan()
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

pub fn cmd_learn_feedback(
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
