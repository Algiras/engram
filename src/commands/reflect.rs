use chrono::{DateTime, Utc};
use colored::Colorize;

use crate::error::Result;
use crate::extractor::knowledge::{parse_session_blocks, parse_ttl, partition_by_expiry};

struct CategoryStats {
    total: usize,
    high_confidence: usize,
    medium_confidence: usize,
    low_confidence: usize,
    unknown_confidence: usize,
    with_ttl: usize,
    expiring_soon: usize,
    stale: usize,
    recent: usize,
}

impl Default for CategoryStats {
    fn default() -> Self {
        Self {
            total: 0,
            high_confidence: 0,
            medium_confidence: 0,
            low_confidence: 0,
            unknown_confidence: 0,
            with_ttl: 0,
            expiring_soon: 0,
            stale: 0,
            recent: 0,
        }
    }
}

/// Reflect on memory quality for a project: confidence, staleness, coverage, recommendations.
pub fn cmd_reflect(project: &str) -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::error::MemoryError::Config("Could not determine home directory".into())
    })?;
    let memory_dir = home.join("memory");
    let knowledge_dir = memory_dir.join("knowledge").join(project);

    if !knowledge_dir.exists() {
        eprintln!(
            "{} No knowledge found for '{}'.",
            "Not found:".yellow(),
            project
        );
        return Ok(());
    }

    let now = Utc::now();
    let categories = [
        "decisions",
        "solutions",
        "patterns",
        "bugs",
        "insights",
        "questions",
    ];

    let mut total_expired = 0usize;
    let mut category_stats: Vec<(String, CategoryStats)> = Vec::new();

    for cat in &categories {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if !path.exists() {
            continue;
        }

        let content = std::fs::read_to_string(&path)?;
        let (_, blocks) = parse_session_blocks(&content);
        let (active, expired) = partition_by_expiry(blocks);

        total_expired += expired.len();

        let mut stats = CategoryStats::default();
        stats.total = active.len();

        for block in &active {
            // Confidence distribution
            match block.confidence.as_deref() {
                Some("high") => stats.high_confidence += 1,
                Some("medium") => stats.medium_confidence += 1,
                Some("low") => stats.low_confidence += 1,
                _ => stats.unknown_confidence += 1,
            }

            // TTL tracking
            if let Some(ref ttl_str) = block.ttl {
                if ttl_str != "never" {
                    stats.with_ttl += 1;
                    // Expiring within 7 days
                    if let (Ok(ts), Some(dur)) = (
                        DateTime::parse_from_rfc3339(&block.timestamp),
                        parse_ttl(ttl_str),
                    ) {
                        let expiry = ts.with_timezone(&Utc) + dur;
                        let days_left = (expiry - now).num_days();
                        if days_left >= 0 && days_left <= 7 {
                            stats.expiring_soon += 1;
                        }
                    }
                }
            }

            // Recency tracking
            if let Ok(ts) = DateTime::parse_from_rfc3339(&block.timestamp) {
                let age_days = (now - ts.with_timezone(&Utc)).num_days();
                if age_days > 30 {
                    stats.stale += 1;
                }
                if age_days <= 7 {
                    stats.recent += 1;
                }
            }
        }

        if stats.total > 0 {
            category_stats.push((cat.to_string(), stats));
        }
    }

    let total_entries: usize = category_stats.iter().map(|(_, s)| s.total).sum();

    if total_entries == 0 {
        println!(
            "{} No active knowledge entries for '{}'.",
            "Empty:".yellow(),
            project
        );
        return Ok(());
    }

    // Aggregate totals
    let stale_total: usize = category_stats.iter().map(|(_, s)| s.stale).sum();
    let low_conf_total: usize = category_stats.iter().map(|(_, s)| s.low_confidence).sum();
    let unknown_conf_total: usize = category_stats
        .iter()
        .map(|(_, s)| s.unknown_confidence)
        .sum();
    let high_total: usize = category_stats.iter().map(|(_, s)| s.high_confidence).sum();
    let med_total: usize = category_stats
        .iter()
        .map(|(_, s)| s.medium_confidence)
        .sum();
    let recent_total: usize = category_stats.iter().map(|(_, s)| s.recent).sum();
    let expiring_soon_total: usize = category_stats.iter().map(|(_, s)| s.expiring_soon).sum();

    let stale_pct = stale_total * 100 / total_entries.max(1);
    let low_conf_pct = low_conf_total * 100 / total_entries.max(1);
    let unknown_conf_pct = unknown_conf_total * 100 / total_entries.max(1);

    // Compute quality score
    let mut quality_score: i32 = 100;
    let mut recommendations: Vec<String> = Vec::new();

    if stale_pct > 50 {
        quality_score -= 20;
        recommendations.push(format!(
            "{}% of entries are stale (>30 days) — run `engram forget {} --stale 60d`",
            stale_pct, project
        ));
    } else if stale_pct > 25 {
        quality_score -= 10;
        recommendations.push(format!(
            "{}% of entries are stale — review and remove outdated knowledge",
            stale_pct
        ));
    }

    if low_conf_pct > 20 {
        quality_score -= 10;
        recommendations.push(format!(
            "{}% of entries have low confidence — validate or re-ingest",
            low_conf_pct
        ));
    }

    if unknown_conf_pct > 50 {
        quality_score -= 5;
        recommendations.push(
            "Most entries lack confidence scores — re-run `engram ingest` with latest version"
                .into(),
        );
    }

    if total_expired > 5 {
        quality_score -= 5;
        recommendations.push(format!(
            "{} expired entries accumulating — run `engram forget {} --expired`",
            total_expired, project
        ));
    }

    if category_stats.len() < 3 {
        quality_score -= 10;
        recommendations.push(
            "Low category diversity — only a few knowledge categories are populated".into(),
        );
    }

    if expiring_soon_total > 0 {
        recommendations.push(format!(
            "{} entries expiring within 7 days — consider refreshing or extending TTL",
            expiring_soon_total
        ));
    }

    if recent_total == 0 {
        recommendations
            .push("No entries added in the last 7 days — memory may not be current".into());
    }

    let quality_score = quality_score.clamp(0, 100) as u8;

    let score_color = match quality_score {
        90..=100 => colored::Color::Green,
        75..=89 => colored::Color::Cyan,
        50..=74 => colored::Color::Yellow,
        _ => colored::Color::Red,
    };

    let score_label = match quality_score {
        90..=100 => "Excellent",
        75..=89 => "Good",
        50..=74 => "Fair",
        _ => "Poor",
    };

    let pct = |n: usize| format!("{:.0}%", n as f32 / total_entries as f32 * 100.0);

    println!();
    println!("{}", format!("Memory Reflection: {}", project).bold());
    println!("{}", "═".repeat(52));
    println!();
    println!(
        "  Quality Score   {} / 100  ({})",
        quality_score.to_string().color(score_color).bold(),
        score_label.color(score_color)
    );
    println!("  Total entries   {}", total_entries);
    if total_expired > 0 {
        println!("  Expired         {}", total_expired.to_string().yellow());
    }
    println!();

    // Category table
    println!("{}", "  Category Breakdown".bold());
    println!(
        "  {:<14} {:>6} {:>7} {:>8} {:>8} {:>8}",
        "Category", "Total", "Recent", "Stale", "Lo-Conf", "w/TTL"
    );
    println!("  {}", "─".repeat(56));

    for (cat, stats) in &category_stats {
        let stale_str = if stats.stale > 0 {
            stats.stale.to_string().yellow().to_string()
        } else {
            "0".normal().to_string()
        };
        let lo_str = if stats.low_confidence > 0 {
            stats.low_confidence.to_string().yellow().to_string()
        } else {
            "0".normal().to_string()
        };

        println!(
            "  {:<14} {:>6} {:>7} {:>8} {:>8} {:>8}",
            cat, stats.total, stats.recent, stale_str, lo_str, stats.with_ttl,
        );
    }

    // Confidence distribution
    println!();
    println!("{}", "  Confidence Distribution".bold());
    println!("  High    {:>4}  ({})", high_total, pct(high_total));
    println!("  Medium  {:>4}  ({})", med_total, pct(med_total));
    let low_line = format!("  Low     {:>4}  ({})", low_conf_total, pct(low_conf_total));
    if low_conf_total > 0 {
        println!("{}", low_line.yellow());
    } else {
        println!("{}", low_line);
    }
    let unk_line = format!(
        "  Unknown {:>4}  ({})",
        unknown_conf_total,
        pct(unknown_conf_total)
    );
    if unknown_conf_pct > 30 {
        println!("{}", unk_line.yellow());
    } else {
        println!("{}", unk_line);
    }

    // Activity
    println!();
    println!("{}", "  Activity".bold());
    let recent_line = format!("  New last 7d    {}", recent_total);
    if recent_total > 0 {
        println!("{}", recent_line.green());
    } else {
        println!("{}", recent_line.yellow());
    }
    let stale_line = format!("  Stale >30d     {}  ({})", stale_total, pct(stale_total));
    if stale_pct > 25 {
        println!("{}", stale_line.yellow());
    } else {
        println!("{}", stale_line);
    }
    if expiring_soon_total > 0 {
        println!(
            "{}",
            format!("  Expiring ≤7d   {}", expiring_soon_total).yellow()
        );
    }

    // Recommendations
    if !recommendations.is_empty() {
        println!();
        println!("{}", "  Recommendations".bold());
        for rec in &recommendations {
            println!("  {} {}", "→".cyan(), rec);
        }
    } else {
        println!();
        println!("  {} Memory is in great shape!", "✓".green());
    }

    println!();
    Ok(())
}
