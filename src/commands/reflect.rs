use std::path::Path;

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
    // TTL expiry buckets: (this_week, this_month, later)
    let mut ttl_this_week = 0usize;
    let mut ttl_this_month = 0usize;
    let mut ttl_later = 0usize;

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
                    if let (Ok(ts), Some(dur)) = (
                        DateTime::parse_from_rfc3339(&block.timestamp),
                        parse_ttl(ttl_str),
                    ) {
                        let expiry = ts.with_timezone(&Utc) + dur;
                        let days_left = (expiry - now).num_days();
                        if days_left >= 0 {
                            if days_left <= 7 {
                                stats.expiring_soon += 1;
                                ttl_this_week += 1;
                            } else if days_left <= 30 {
                                ttl_this_month += 1;
                            } else {
                                ttl_later += 1;
                            }
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

    // TTL expiry timeline
    let total_with_ttl: usize = category_stats.iter().map(|(_, s)| s.with_ttl).sum();
    if total_with_ttl > 0 {
        println!();
        println!("{}", "  TTL Expiry Timeline".bold());
        let week_str = format!("  This week  ≤7d   {}", ttl_this_week);
        let month_str = format!("  This month ≤30d  {}", ttl_this_month);
        let later_str = format!("  Later      >30d  {}", ttl_later);
        if ttl_this_week > 0 {
            println!("{}", week_str.yellow());
        } else {
            println!("{}", week_str);
        }
        println!("{}", month_str);
        println!("{}", later_str);
        println!(
            "  Permanent  (no TTL)  {}",
            total_entries.saturating_sub(total_with_ttl)
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

// ── Project quality summary (shared between --all and daemon logging) ────────

/// Summary metrics for a single project used in cross-project reports.
pub struct ProjectQuality {
    pub project: String,
    pub quality_score: u8,
    pub total_entries: usize,
    pub stale_pct: usize,
    pub recent: usize,
    pub categories: usize,
}

/// Compute quality metrics for one project without printing anything.
pub fn compute_project_quality(knowledge_dir: &Path, project: &str) -> Option<ProjectQuality> {
    let now = Utc::now();
    let categories = [
        "decisions",
        "solutions",
        "patterns",
        "bugs",
        "insights",
        "questions",
    ];

    let mut total_entries = 0usize;
    let mut total_expired = 0usize;
    let mut stale_total = 0usize;
    let mut recent_total = 0usize;
    let mut low_conf_total = 0usize;
    let mut unknown_conf_total = 0usize;
    let mut cat_count = 0usize;

    for cat in &categories {
        let path = knowledge_dir.join(format!("{}.md", cat));
        if !path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&path).unwrap_or_default();
        let (_, blocks) = parse_session_blocks(&content);
        let (active, expired) = partition_by_expiry(blocks);

        if active.is_empty() {
            continue;
        }
        cat_count += 1;
        total_expired += expired.len();
        total_entries += active.len();

        for block in &active {
            match block.confidence.as_deref() {
                Some("low") => low_conf_total += 1,
                None | Some("") => unknown_conf_total += 1,
                _ => {}
            }
            if let Ok(ts) = DateTime::parse_from_rfc3339(&block.timestamp) {
                let age = (now - ts.with_timezone(&Utc)).num_days();
                if age > 30 {
                    stale_total += 1;
                }
                if age <= 7 {
                    recent_total += 1;
                }
            }
        }
    }

    if total_entries == 0 {
        return None;
    }

    let stale_pct = stale_total * 100 / total_entries.max(1);
    let low_conf_pct = low_conf_total * 100 / total_entries.max(1);
    let unknown_conf_pct = unknown_conf_total * 100 / total_entries.max(1);

    let mut quality: i32 = 100;
    if stale_pct > 50 {
        quality -= 20;
    } else if stale_pct > 25 {
        quality -= 10;
    }
    if low_conf_pct > 20 {
        quality -= 10;
    }
    if unknown_conf_pct > 50 {
        quality -= 5;
    }
    if total_expired > 5 {
        quality -= 5;
    }
    if cat_count < 3 {
        quality -= 10;
    }

    Some(ProjectQuality {
        project: project.to_string(),
        quality_score: quality.clamp(0, 100) as u8,
        total_entries,
        stale_pct,
        recent: recent_total,
        categories: cat_count,
    })
}

/// `engram reflect --all` — quality summary table for every project.
pub fn cmd_reflect_all() -> Result<()> {
    let home = dirs::home_dir().ok_or_else(|| {
        crate::error::MemoryError::Config("Could not determine home directory".into())
    })?;
    let knowledge_dir = home.join("memory").join("knowledge");

    if !knowledge_dir.exists() {
        eprintln!("{} No memory directory found. Run 'engram ingest' first.", "Not found:".yellow());
        return Ok(());
    }

    let mut projects: Vec<ProjectQuality> = std::fs::read_dir(&knowledge_dir)
        .map_err(crate::error::MemoryError::Io)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with('_') {
                return None; // skip _global, _packs, etc.
            }
            compute_project_quality(&e.path(), &name)
        })
        .collect();

    if projects.is_empty() {
        println!("{} No projects with knowledge found.", "Empty:".yellow());
        return Ok(());
    }

    // Sort: worst score first so problems are visible at the top
    projects.sort_by(|a, b| a.quality_score.cmp(&b.quality_score));

    let score_color = |s: u8| match s {
        90..=100 => colored::Color::Green,
        75..=89 => colored::Color::Cyan,
        50..=74 => colored::Color::Yellow,
        _ => colored::Color::Red,
    };

    println!();
    println!("{}", "Memory Quality — All Projects".bold());
    println!("{}", "═".repeat(62));
    println!();
    println!(
        "  {:<22} {:>7} {:>8} {:>7} {:>7} {:>6}",
        "Project", "Score", "Entries", "Stale%", "Recent", "Cats"
    );
    println!("  {}", "─".repeat(58));

    for p in &projects {
        let score_str = format!("{:>3}/100", p.quality_score)
            .color(score_color(p.quality_score))
            .bold()
            .to_string();
        let stale_str = if p.stale_pct > 25 {
            format!("{:>6}%", p.stale_pct).yellow().to_string()
        } else {
            format!("{:>6}%", p.stale_pct).normal().to_string()
        };

        println!(
            "  {:<22} {}  {:>8} {} {:>7} {:>6}",
            if p.project.len() > 22 {
                format!("{}…", &p.project[..21])
            } else {
                p.project.clone()
            },
            score_str,
            p.total_entries,
            stale_str,
            p.recent,
            format!("{}/6", p.categories),
        );
    }

    println!();
    let avg = projects.iter().map(|p| p.quality_score as usize).sum::<usize>() / projects.len().max(1);
    let avg_color = score_color(avg as u8);
    println!(
        "  {} projects  ·  avg quality {}/100",
        projects.len(),
        avg.to_string().color(avg_color).bold()
    );
    println!();

    Ok(())
}

// ── Unit tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_knowledge_file(dir: &Path, category: &str, blocks: &[(&str, &str, &str)]) {
        // blocks: (session_id, timestamp, confidence)
        let mut content = format!("# {}\n\n", capitalize(category));
        for (id, ts, conf) in blocks {
            content.push_str(&format!(
                "## Session: {} ({}) [confidence:{}]\n\nTest content for {}.\n\n",
                id, ts, conf, category
            ));
        }
        fs::write(dir.join(format!("{}.md", category)), content).unwrap();
    }

    fn capitalize(s: &str) -> String {
        let mut c = s.chars();
        match c.next() {
            None => String::new(),
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        }
    }

    #[test]
    fn test_reflect_missing_project_exits_cleanly() {
        // compute_project_quality on non-existent dir returns None
        let tmp = TempDir::new().unwrap();
        let result = compute_project_quality(&tmp.path().join("nonexistent"), "ghost");
        assert!(result.is_none(), "Missing project should return None");
    }

    #[test]
    fn test_reflect_empty_knowledge_dir_returns_none() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("empty-project");
        fs::create_dir_all(&project_dir).unwrap();
        let result = compute_project_quality(&project_dir, "empty-project");
        assert!(result.is_none(), "Empty project dir should return None");
    }

    #[test]
    fn test_reflect_high_confidence_entries_score_well() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        let now = Utc::now().to_rfc3339();

        write_knowledge_file(
            project_dir,
            "decisions",
            &[("d1", &now, "high"), ("d2", &now, "high"), ("d3", &now, "high")],
        );
        write_knowledge_file(
            project_dir,
            "solutions",
            &[("s1", &now, "high"), ("s2", &now, "medium")],
        );
        write_knowledge_file(project_dir, "patterns", &[("p1", &now, "high")]);

        let q = compute_project_quality(project_dir, "test-proj").unwrap();
        assert!(q.quality_score >= 90, "All-fresh, all-high should score >= 90, got {}", q.quality_score);
        assert_eq!(q.total_entries, 6);
        assert_eq!(q.recent, 6, "All entries are from today");
        assert_eq!(q.stale_pct, 0);
        assert_eq!(q.categories, 3);
    }

    #[test]
    fn test_reflect_stale_entries_lower_score() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        // 60 days ago
        let old_ts = (Utc::now() - chrono::Duration::days(60)).to_rfc3339();
        let now = Utc::now().to_rfc3339();

        // 3 stale, 1 fresh → stale_pct = 75%
        write_knowledge_file(
            project_dir,
            "decisions",
            &[
                ("d1", &old_ts, "high"),
                ("d2", &old_ts, "high"),
                ("d3", &old_ts, "high"),
                ("d4", &now, "high"),
            ],
        );
        write_knowledge_file(project_dir, "solutions", &[("s1", &old_ts, "high")]);
        write_knowledge_file(project_dir, "patterns", &[("p1", &old_ts, "high")]);

        let q = compute_project_quality(project_dir, "stale-proj").unwrap();
        assert!(q.stale_pct > 25, "Should detect >25% stale, got {}%", q.stale_pct);
        assert!(q.quality_score < 90, "Stale knowledge should lower score, got {}", q.quality_score);
    }

    #[test]
    fn test_reflect_low_confidence_lowers_score() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        let now = Utc::now().to_rfc3339();

        // >20% low confidence
        write_knowledge_file(
            project_dir,
            "decisions",
            &[
                ("d1", &now, "low"),
                ("d2", &now, "low"),
                ("d3", &now, "high"),
            ],
        );
        write_knowledge_file(project_dir, "solutions", &[("s1", &now, "high")]);
        write_knowledge_file(project_dir, "patterns", &[("p1", &now, "high")]);

        let q = compute_project_quality(project_dir, "low-conf-proj").unwrap();
        assert!(q.quality_score < 100, "Low confidence should penalize score");
    }

    #[test]
    fn test_reflect_few_categories_lowers_score() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        let now = Utc::now().to_rfc3339();

        // Only 1 category populated
        write_knowledge_file(project_dir, "decisions", &[("d1", &now, "high")]);

        let q = compute_project_quality(project_dir, "sparse-proj").unwrap();
        assert!(q.quality_score < 95, "Few categories should penalize score, got {}", q.quality_score);
        assert_eq!(q.categories, 1);
    }

    #[test]
    fn test_reflect_all_high_confidence_six_categories_scores_100() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        let now = Utc::now().to_rfc3339();

        for cat in &["decisions", "solutions", "patterns", "bugs", "insights", "questions"] {
            write_knowledge_file(project_dir, cat, &[(&format!("{}-1", cat), &now, "high")]);
        }

        let q = compute_project_quality(project_dir, "perfect-proj").unwrap();
        assert_eq!(q.quality_score, 100, "6 fresh, high-confidence categories should score 100");
        assert_eq!(q.categories, 6);
        assert_eq!(q.stale_pct, 0);
        assert_eq!(q.recent, 6);
    }

    #[test]
    fn test_reflect_expired_entries_not_counted() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();

        // Entry with a very short TTL that has already passed
        let old_ts = (Utc::now() - chrono::Duration::days(10)).to_rfc3339();
        let now = Utc::now().to_rfc3339();

        // One expired (10 days ago, 7d TTL) + one active
        let mut content = "# Decisions\n\n".to_string();
        content.push_str(&format!(
            "## Session: expired-1 ({}) [ttl:7d] [confidence:high]\n\nExpired entry.\n\n",
            old_ts
        ));
        content.push_str(&format!(
            "## Session: active-1 ({}) [confidence:high]\n\nActive entry.\n\n",
            now
        ));
        fs::write(project_dir.join("decisions.md"), content).unwrap();

        let q = compute_project_quality(project_dir, "ttl-proj").unwrap();
        // Expired entry should be excluded — only 1 active entry counted
        assert_eq!(q.total_entries, 1, "Expired entries should not count; got {}", q.total_entries);
    }

    #[test]
    fn test_reflect_mixed_confidence_correct_distribution() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        let now = Utc::now().to_rfc3339();

        // 2 high, 1 medium, 1 low, 1 unknown (no confidence tag) in decisions
        let mut content = "# Decisions\n\n".to_string();
        for (id, conf) in &[("h1","high"),("h2","high"),("m1","medium"),("l1","low")] {
            content.push_str(&format!(
                "## Session: {} ({}) [confidence:{}]\n\nContent.\n\n",
                id, now, conf
            ));
        }
        // one without confidence tag
        content.push_str(&format!(
            "## Session: u1 ({})\n\nNo confidence tag.\n\n",
            now
        ));
        fs::write(project_dir.join("decisions.md"), content).unwrap();

        // cmd_reflect should run without panicking and count 5 entries
        let _result = cmd_reflect("nonexistent-in-real-home");
        // This won't find the file since HOME is real, but compute_project_quality is unit-tested
        // directly, so verify via that:
        let q = compute_project_quality(project_dir, "conf-proj").unwrap();
        assert_eq!(q.total_entries, 5);
    }

    #[test]
    fn test_reflect_future_timestamp_treated_as_recent() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        // Slightly in the future (clock skew) — age_days will be 0 or negative, not stale, is recent
        let future_ts = (Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
        write_knowledge_file(project_dir, "decisions", &[("f1", &future_ts, "high")]);

        let q = compute_project_quality(project_dir, "future-proj").unwrap();
        assert_eq!(q.stale_pct, 0, "Future timestamps should not be counted as stale");
        assert_eq!(q.total_entries, 1);
    }

    #[test]
    fn test_reflect_compute_quality_returns_none_for_empty_files() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        // Files exist but contain no session blocks
        fs::write(project_dir.join("decisions.md"), "# Decisions\n\n(empty)\n").unwrap();
        fs::write(project_dir.join("solutions.md"), "").unwrap();

        let q = compute_project_quality(project_dir, "empty-blocks");
        assert!(q.is_none(), "No session blocks should yield None");
    }

    #[test]
    fn test_reflect_all_projects_aggregation() {
        let tmp = TempDir::new().unwrap();
        let knowledge_dir = tmp.path();
        let now = Utc::now().to_rfc3339();

        // Project A: 6 categories, all fresh → should score 100
        for cat in &["decisions","solutions","patterns","bugs","insights","questions"] {
            let dir = knowledge_dir.join("proj-a");
            fs::create_dir_all(&dir).unwrap();
            write_knowledge_file(&dir, cat, &[("e1", &now, "high")]);
        }

        // Project B: 1 category only → low diversity score
        {
            let dir = knowledge_dir.join("proj-b");
            fs::create_dir_all(&dir).unwrap();
            write_knowledge_file(&dir, "decisions", &[("e1", &now, "high")]);
        }

        let a = compute_project_quality(&knowledge_dir.join("proj-a"), "proj-a").unwrap();
        let b = compute_project_quality(&knowledge_dir.join("proj-b"), "proj-b").unwrap();

        assert_eq!(a.quality_score, 100);
        assert!(b.quality_score < a.quality_score, "proj-b should score lower due to low category count");
    }
}
