use crate::error::Result;
use colored::Colorize;
use std::path::Path;

#[derive(Debug)]
pub struct HealthReport {
    pub project: String,
    pub score: u8, // 0-100
    pub issues: Vec<Issue>,
    pub recommendations: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Issue {
    pub severity: Severity,
    pub category: IssueCategory,
    pub description: String,
    pub auto_fixable: bool,
    pub fix_command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Critical, // Breaks functionality
    Warning,  // Degrades performance
    Info,     // Could be better
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IssueCategory {
    MissingEmbeddings,
    StaleContext,
    MissingGraph,
    HighDuplication,
    LowCoverage,
    Contradictions,
    UnusedKnowledge,
    LargeFiles,
    ExpiredEntries,
}

impl HealthReport {
    pub fn new(project: String) -> Self {
        Self {
            project,
            score: 100,
            issues: Vec::new(),
            recommendations: Vec::new(),
        }
    }

    pub fn add_issue(&mut self, issue: Issue) {
        let penalty = match issue.severity {
            Severity::Critical => 20,
            Severity::Warning => 10,
            Severity::Info => 5,
        };
        self.score = self.score.saturating_sub(penalty);
        self.issues.push(issue);
    }

    pub fn add_recommendation(&mut self, rec: String) {
        self.recommendations.push(rec);
    }

    pub fn health_status(&self) -> &str {
        match self.score {
            90..=100 => "Excellent",
            75..=89 => "Good",
            50..=74 => "Fair",
            25..=49 => "Poor",
            _ => "Critical",
        }
    }

    pub fn health_color(&self) -> colored::Color {
        match self.score {
            90..=100 => colored::Color::Green,
            75..=89 => colored::Color::Cyan,
            50..=74 => colored::Color::Yellow,
            25..=49 => colored::Color::Magenta,
            _ => colored::Color::Red,
        }
    }
}

/// Run health check on a project
pub fn check_project_health(memory_dir: &Path, project: &str) -> Result<HealthReport> {
    let mut report = HealthReport::new(project.to_string());
    let knowledge_dir = memory_dir.join("knowledge").join(project);

    if !knowledge_dir.exists() {
        report.add_issue(Issue {
            severity: Severity::Critical,
            category: IssueCategory::LowCoverage,
            description: "No knowledge directory found".into(),
            auto_fixable: true,
            fix_command: Some(format!("engram ingest --project {}", project)),
        });
        return Ok(report);
    }

    // Check for context.md
    let context_path = knowledge_dir.join("context.md");
    if !context_path.exists() {
        report.add_issue(Issue {
            severity: Severity::Warning,
            category: IssueCategory::StaleContext,
            description: "No context.md (knowledge not synthesized)".into(),
            auto_fixable: true,
            fix_command: Some(format!("engram regen {}", project)),
        });
    } else {
        // Check if context is stale (older than knowledge files)
        if is_stale(&context_path, &knowledge_dir)? {
            report.add_issue(Issue {
                severity: Severity::Info,
                category: IssueCategory::StaleContext,
                description: "context.md is older than knowledge files".into(),
                auto_fixable: true,
                fix_command: Some(format!("engram regen {}", project)),
            });
        }
    }

    // Check for embeddings
    let embeddings_path = knowledge_dir.join("embeddings.json");
    if !embeddings_path.exists() {
        report.add_issue(Issue {
            severity: Severity::Info,
            category: IssueCategory::MissingEmbeddings,
            description: "No embeddings index (semantic search unavailable)".into(),
            auto_fixable: true,
            fix_command: Some(format!("engram embed {}", project)),
        });
        report.add_recommendation("Generate embeddings for semantic search".into());
    }

    // Check for knowledge graph
    let graph_path = knowledge_dir.join("graph.json");
    if !graph_path.exists() {
        report.add_issue(Issue {
            severity: Severity::Info,
            category: IssueCategory::MissingGraph,
            description: "No knowledge graph (associative search unavailable)".into(),
            auto_fixable: true,
            fix_command: Some(format!("engram graph build {}", project)),
        });
        report.add_recommendation("Build knowledge graph for associative queries".into());
    }

    // Check knowledge file sizes (too large = needs consolidation)
    for file_name in &["decisions.md", "solutions.md", "patterns.md"] {
        let path = knowledge_dir.join(file_name);
        if let Ok(metadata) = std::fs::metadata(&path) {
            let size_mb = metadata.len() as f64 / 1_048_576.0;
            if size_mb > 5.0 {
                report.add_issue(Issue {
                    severity: Severity::Warning,
                    category: IssueCategory::LargeFiles,
                    description: format!(
                        "{} is large ({:.1} MB) - may need consolidation",
                        file_name, size_mb
                    ),
                    auto_fixable: false,
                    fix_command: Some(format!("engram consolidate {} --threshold 0.9", project)),
                });
            }
        }
    }

    // Check for inbox items
    let inbox_path = knowledge_dir.join("inbox.md");
    if inbox_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&inbox_path) {
            let entry_count = content.matches("## Session:").count();
            if entry_count > 10 {
                report.add_issue(Issue {
                    severity: Severity::Info,
                    category: IssueCategory::UnusedKnowledge,
                    description: format!("{} inbox entries pending review", entry_count),
                    auto_fixable: false,
                    fix_command: Some(format!("engram review {}", project)),
                });
                report.add_recommendation("Review and promote inbox entries".into());
            }
        }
    }

    // Check for expired entries accumulating
    let expired_count = count_expired_entries(memory_dir, project)?;
    if expired_count > 0 {
        report.add_issue(Issue {
            severity: Severity::Info,
            category: IssueCategory::ExpiredEntries,
            description: format!("{} expired entries accumulating cruft", expired_count),
            auto_fixable: true,
            fix_command: Some(format!("engram forget {} --expired", project)),
        });
        report.add_recommendation(format!(
            "Run 'engram forget {} --expired' to clean up {} expired entries, or use 'engram inject' (auto-cleans by default)",
            project, expired_count
        ));
    }

    // Overall health recommendations
    if report.score >= 90 {
        report
            .add_recommendation("System is healthy! Consider running doctor periodically.".into());
    } else if report.score >= 75 {
        report.add_recommendation("Fix warnings to improve health score.".into());
    } else {
        report
            .add_recommendation("Multiple issues detected - run with --fix to auto-repair.".into());
    }

    Ok(report)
}

/// Check if context.md is older than knowledge files
fn is_stale(context_path: &Path, knowledge_dir: &Path) -> Result<bool> {
    let context_modified = std::fs::metadata(context_path)?.modified()?;

    for file_name in &["decisions.md", "solutions.md", "patterns.md"] {
        let path = knowledge_dir.join(file_name);
        if let Ok(metadata) = std::fs::metadata(&path) {
            if let Ok(modified) = metadata.modified() {
                if modified > context_modified {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

/// Auto-fix issues that can be repaired automatically
pub async fn auto_fix_issues(
    config: &crate::config::Config,
    project: &str,
    issues: &[Issue],
) -> Result<Vec<String>> {
    let mut fixed = Vec::new();

    for issue in issues {
        if !issue.auto_fixable {
            continue;
        }

        match issue.category {
            IssueCategory::StaleContext => {
                // Regenerate context
                if let Err(e) = regenerate_context(config, project).await {
                    eprintln!("  {} Failed to regen context: {}", "✗".red(), e);
                } else {
                    fixed.push("Regenerated stale context.md".into());
                }
            }
            IssueCategory::MissingEmbeddings => {
                // Generate embeddings
                if let Err(e) = generate_embeddings(config, project).await {
                    eprintln!("  {} Failed to generate embeddings: {}", "✗".red(), e);
                } else {
                    fixed.push("Generated embeddings index".into());
                }
            }
            IssueCategory::MissingGraph => {
                // Build graph
                if let Err(e) = build_graph(config, project).await {
                    eprintln!("  {} Failed to build graph: {}", "✗".red(), e);
                } else {
                    fixed.push("Built knowledge graph".into());
                }
            }
            _ => {}
        }
    }

    Ok(fixed)
}

async fn regenerate_context(config: &crate::config::Config, project: &str) -> Result<()> {
    use crate::extractor::knowledge::{
        parse_session_blocks, partition_by_expiry, reconstruct_blocks,
    };
    use crate::llm::client::LlmClient;

    let knowledge_dir = config.memory_dir.join("knowledge").join(project);

    let read_and_filter = |path: &Path| -> String {
        let raw = std::fs::read_to_string(path).unwrap_or_default();
        let (preamble, blocks) = parse_session_blocks(&raw);
        let (active, _) = partition_by_expiry(blocks);
        reconstruct_blocks(&preamble, &active)
    };

    let decisions = read_and_filter(&knowledge_dir.join("decisions.md"));
    let solutions = read_and_filter(&knowledge_dir.join("solutions.md"));
    let patterns = read_and_filter(&knowledge_dir.join("patterns.md"));

    let client = LlmClient::new(&config.llm);
    let context = client
        .chat(
            crate::llm::prompts::SYSTEM_KNOWLEDGE_EXTRACTOR,
            &crate::llm::prompts::context_prompt(project, &decisions, &solutions, &patterns, ""),
        )
        .await?;

    let context_with_header = format!("# {} - Project Context\n\n{}\n", project, context);
    std::fs::write(knowledge_dir.join("context.md"), &context_with_header)?;

    Ok(())
}

async fn generate_embeddings(_config: &crate::config::Config, _project: &str) -> Result<()> {
    // Placeholder - would call embed command
    Ok(())
}

async fn build_graph(_config: &crate::config::Config, _project: &str) -> Result<()> {
    // Placeholder - would call graph build
    Ok(())
}

/// Count total expired entries across all knowledge files for a project.
fn count_expired_entries(memory_dir: &Path, project: &str) -> Result<usize> {
    use crate::extractor::knowledge::{parse_session_blocks, partition_by_expiry};

    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let global_prefs = memory_dir
        .join("knowledge")
        .join("_global")
        .join("preferences.md");

    let files = [
        knowledge_dir.join("decisions.md"),
        knowledge_dir.join("solutions.md"),
        knowledge_dir.join("patterns.md"),
        knowledge_dir.join("inbox.md"),
        global_prefs,
    ];

    let mut total_expired = 0;

    for file_path in files.iter().filter(|p| p.exists()) {
        let content = std::fs::read_to_string(file_path)?;
        let (_preamble, blocks) = parse_session_blocks(&content);
        let (_active, expired) = partition_by_expiry(blocks);
        total_expired += expired.len();
    }

    Ok(total_expired)
}
