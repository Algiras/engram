use crate::learning::adaptation::ChangePreview;
use crate::learning::progress::LearningState;
use colored::Colorize;

/// Format and display learning dashboard
pub fn display_dashboard(state: &LearningState) {
    println!("\n{}", "Learning Progress Dashboard".bold().cyan());
    println!("{}", "=".repeat(60).cyan());

    // Project info
    println!("\n{}: {}", "Project".bold(), state.project);
    println!(
        "{}: {}",
        "Created".bold(),
        state.created_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!(
        "{}: {}",
        "Updated".bold(),
        state.updated_at.format("%Y-%m-%d %H:%M:%S")
    );
    println!("{}: {}", "Sessions".bold(), state.session_count());

    // Convergence status
    let converged = state.has_converged();
    let status = if converged {
        "Converged ✓".green()
    } else {
        "Learning...".yellow()
    };
    println!("{}: {}", "Status".bold(), status);

    // Metrics trends
    if let Some(latest) = state.metrics_history.last() {
        println!("\n{}", "Current Metrics".bold().cyan());
        println!("{}", "-".repeat(60).cyan());

        display_metric("Health Score", latest.health_score, 90);
        display_metric_u32("Avg Query Time", latest.avg_query_time_ms, 100, "ms");
        display_metric_f32("Stale Knowledge", latest.stale_knowledge_pct, 10.0, "%");
        display_metric_f32("Storage Size", latest.storage_size_mb, 15.0, "MB");
    }

    // Metrics improvement over time
    if state.metrics_history.len() >= 2 {
        let first = &state.metrics_history[0];
        let latest = state.metrics_history.last().unwrap();

        println!("\n{}", "Improvements Since Start".bold().cyan());
        println!("{}", "-".repeat(60).cyan());

        let health_delta = latest.health_score as i16 - first.health_score as i16;
        display_delta("Health Score", first.health_score, latest.health_score, health_delta);

        let query_delta = latest.avg_query_time_ms as i32 - first.avg_query_time_ms as i32;
        display_delta_i32(
            "Avg Query Time",
            first.avg_query_time_ms,
            latest.avg_query_time_ms,
            query_delta,
            "ms",
            true, // Lower is better
        );

        let stale_delta = latest.stale_knowledge_pct - first.stale_knowledge_pct;
        display_delta_f32(
            "Stale Knowledge",
            first.stale_knowledge_pct,
            latest.stale_knowledge_pct,
            stale_delta,
            "%",
            true, // Lower is better
        );
    }

    // Adaptation success
    println!("\n{}", "Adaptation Performance".bold().cyan());
    println!("{}", "-".repeat(60).cyan());

    let success_rate = state.adaptation_success_rate();
    let total = state.adaptation_history.len();
    let successful = (total as f32 * success_rate) as usize;

    println!(
        "{}: {}/{} ({:.1}%)",
        "Success Rate".bold(),
        successful,
        total,
        success_rate * 100.0
    );

    if let Some(latest) = state.adaptation_history.last() {
        println!(
            "{}: {} adjustments",
            "Last Adaptation".bold(),
            latest.importance_adjustments + latest.ttl_adjustments + latest.graph_adjustments
        );
        println!(
            "{}: {} → {} ({:+})",
            "  Health Impact".bold(),
            latest.health_before,
            latest.health_after,
            latest.health_improvement
        );
    }

    // Hyperparameters
    println!("\n{}", "Hyperparameters".bold().cyan());
    println!("{}", "-".repeat(60).cyan());
    println!(
        "{}: {:.2}",
        "Importance Learning Rate".bold(),
        state.hyperparameters.importance_learning_rate
    );
    println!(
        "{}: {:.2}",
        "TTL Learning Rate".bold(),
        state.hyperparameters.ttl_learning_rate
    );
    println!(
        "{}: {:.2}",
        "Exploration Rate (ε)".bold(),
        state.hyperparameters.exploration_rate
    );

    // Top improvements
    if !state.learned_parameters.importance_boosts.is_empty() {
        println!("\n{}", "Top Importance Boosts".bold().cyan());
        println!("{}", "-".repeat(60).cyan());

        let mut boosts: Vec<_> = state.learned_parameters.importance_boosts.iter().collect();
        boosts.sort_by(|a, b| b.1.partial_cmp(a.1).unwrap_or(std::cmp::Ordering::Equal));

        for (id, boost) in boosts.iter().take(5) {
            println!("  {} {:+.2}", id, boost);
        }
    }

    println!();
}

/// Display a single metric with target comparison
fn display_metric<T: std::fmt::Display + PartialOrd>(label: &str, value: T, target: T) {
    let status = if value >= target {
        "✓".green()
    } else {
        "⚠".yellow()
    };
    println!("{}: {} {}", label.bold(), value, status);
}

fn display_metric_u32(label: &str, value: u32, target: u32, unit: &str) {
    let status = if value <= target {
        "✓".green()
    } else {
        "⚠".yellow()
    };
    println!("{}: {}{} {}", label.bold(), value, unit, status);
}

fn display_metric_f32(label: &str, value: f32, target: f32, unit: &str) {
    let status = if value <= target {
        "✓".green()
    } else {
        "⚠".yellow()
    };
    println!("{}: {:.1}{} {}", label.bold(), value, unit, status);
}

/// Display a delta between two values
fn display_delta<T: std::fmt::Display>(label: &str, before: T, after: T, delta: i16) {
    let formatted_delta = if delta > 0 {
        format!("(+{})", delta).green()
    } else if delta < 0 {
        format!("({})", delta).red()
    } else {
        "(±0)".dimmed()
    };

    println!("{}: {} → {} {}", label.bold(), before, after, formatted_delta);
}

fn display_delta_i32(label: &str, before: u32, after: u32, delta: i32, unit: &str, lower_is_better: bool) {
    let formatted_delta = if delta > 0 {
        if lower_is_better {
            format!("(+{}{})", delta, unit).red()
        } else {
            format!("(+{}{})", delta, unit).green()
        }
    } else if delta < 0 {
        if lower_is_better {
            format!("({}{})", delta, unit).green()
        } else {
            format!("({}{})", delta, unit).red()
        }
    } else {
        format!("(±0{})", unit).dimmed()
    };

    println!("{}: {}{} → {}{} {}", label.bold(), before, unit, after, unit, formatted_delta);
}

fn display_delta_f32(label: &str, before: f32, after: f32, delta: f32, unit: &str, lower_is_better: bool) {
    let formatted_delta = if delta > 0.01 {
        if lower_is_better {
            format!("(+{:.1}{})", delta, unit).red()
        } else {
            format!("(+{:.1}{})", delta, unit).green()
        }
    } else if delta < -0.01 {
        if lower_is_better {
            format!("({:.1}{})", delta, unit).green()
        } else {
            format!("({:.1}{})", delta, unit).red()
        }
    } else {
        format!("(±0{})", unit).dimmed()
    };

    println!("{}: {:.1}{} → {:.1}{} {}", label.bold(), before, unit, after, unit, formatted_delta);
}

/// Display a preview of proposed changes
pub fn display_preview(preview: &ChangePreview) {
    println!("\n{}", "Proposed Changes (Dry Run)".bold().cyan());
    println!("{}", "=".repeat(60).cyan());

    if !preview.importance_changes.is_empty() {
        println!("\n{}", "Importance Adjustments".bold().yellow());
        println!("{}", "-".repeat(60).yellow());

        for change in &preview.importance_changes {
            let delta_formatted = if change.boost > 0.0 {
                format!("(+{:.2})", change.boost).green()
            } else {
                format!("({:.2})", change.boost).red()
            };

            println!(
                "  {}: {:.2} → {:.2} {}",
                change.knowledge_id, change.current, change.proposed, delta_formatted
            );
        }
    }

    if !preview.ttl_changes.is_empty() {
        println!("\n{}", "TTL Adjustments".bold().yellow());
        println!("{}", "-".repeat(60).yellow());

        for change in &preview.ttl_changes {
            let current_str = change
                .current
                .map(|d| format!("{}d", d))
                .unwrap_or_else(|| "permanent".to_string());
            let proposed_str = change
                .proposed
                .map(|d| format!("{}d", d))
                .unwrap_or_else(|| "permanent".to_string());

            println!(
                "  {}: {} → {}",
                change.knowledge_id, current_str, proposed_str
            );
        }
    }

    if let Some(strategy) = preview.consolidation_change {
        println!("\n{}", "Consolidation Strategy".bold().yellow());
        println!("{}", "-".repeat(60).yellow());
        println!(
            "  Similarity Threshold: {:.2}",
            strategy.similarity_threshold
        );
        println!(
            "  Trigger Frequency: {}d",
            strategy.trigger_frequency_days
        );
        println!("  Size Trigger: {:.1}MB", strategy.size_trigger_mb);
    }

    if preview.importance_changes.is_empty()
        && preview.ttl_changes.is_empty()
        && preview.consolidation_change.is_none()
    {
        println!("\n{}", "No changes proposed.".dimmed());
    }

    println!();
}

/// Suggest manual interventions based on learning state
pub fn suggest_interventions(state: &LearningState) {
    let mut suggestions = Vec::new();

    // Check if not converging
    if state.session_count() > 100 && !state.has_converged() {
        suggestions.push(
            "Learning has not converged after 100 sessions. Consider adjusting hyperparameters or reviewing data quality."
                .to_string(),
        );
    }

    // Check if adaptation success rate is low
    if state.adaptation_success_rate() < 0.5 && state.adaptation_history.len() > 5 {
        suggestions.push(
            "Low adaptation success rate (<50%). Review learned parameters or reduce learning rates."
                .to_string(),
        );
    }

    // Check if exploration rate should be reduced
    if state.has_converged() && state.hyperparameters.exploration_rate > 0.1 {
        suggestions.push(
            "Learning has converged. Consider reducing exploration rate (epsilon) to exploit learned policy."
                .to_string(),
        );
    }

    if !suggestions.is_empty() {
        println!("\n{}", "Suggestions".bold().cyan());
        println!("{}", "=".repeat(60).cyan());

        for (i, suggestion) in suggestions.iter().enumerate() {
            println!("{}. {}", i + 1, suggestion);
        }
        println!();
    }
}
