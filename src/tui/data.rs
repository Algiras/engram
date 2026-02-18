use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::Deserialize;

#[cfg(unix)]
fn pid_is_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as i32, 0) == 0 }
}

#[cfg(not(unix))]
fn pid_is_alive(_pid: u32) -> bool {
    false
}

/// A browsable item in the right panel.
#[derive(Clone)]
pub enum MemoryItem {
    Session {
        path: PathBuf,
        session_id: String,
        date: DateTime<Utc>,
        size: u64,
    },
    KnowledgeFile {
        path: PathBuf,
        name: String,
        size: u64,
    },
}

impl MemoryItem {
    pub fn display_label(&self) -> String {
        match self {
            MemoryItem::Session {
                session_id,
                date,
                size,
                ..
            } => {
                let sz = humansize::format_size(*size, humansize::BINARY);
                format!("  {} {} {}", session_id, date.format("%Y-%m-%d %H:%M"), sz)
            }
            MemoryItem::KnowledgeFile { name, size, .. } => {
                let sz = humansize::format_size(*size, humansize::BINARY);
                format!("  {} {}", name, sz)
            }
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            MemoryItem::Session { path, .. } => path,
            MemoryItem::KnowledgeFile { path, .. } => path,
        }
    }
}

#[derive(Clone)]
pub struct ProjectEntry {
    pub name: String,
    pub items: Vec<MemoryItem>,
}

pub struct MemoryTree {
    pub projects: Vec<ProjectEntry>,
}

/// Scan the memory directory and build a browsable tree.
pub fn load_tree(memory_dir: &Path) -> MemoryTree {
    let mut project_map: std::collections::BTreeMap<String, Vec<MemoryItem>> =
        std::collections::BTreeMap::new();

    // Scan conversations/
    let conv_dir = memory_dir.join("conversations");
    if conv_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&conv_dir) {
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let project_name = entry.file_name().to_string_lossy().to_string();
                let items = project_map.entry(project_name).or_default();

                if let Ok(sessions) = fs::read_dir(entry.path()) {
                    for session in sessions.flatten() {
                        if !session.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                            continue;
                        }
                        let session_id = session.file_name().to_string_lossy().to_string();
                        let session_path = session.path();

                        // Compute total size & newest mtime in this session dir
                        let (size, date) = dir_stats(&session_path);

                        items.push(MemoryItem::Session {
                            path: session_path,
                            session_id,
                            date,
                            size,
                        });
                    }
                }
            }
        }
    }

    // Scan knowledge/
    let knowledge_dir = memory_dir.join("knowledge");
    if knowledge_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&knowledge_dir) {
            for entry in entries.flatten() {
                if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    continue;
                }
                let project_name = entry.file_name().to_string_lossy().to_string();
                let items = project_map.entry(project_name).or_default();

                if let Ok(files) = fs::read_dir(entry.path()) {
                    for file in files.flatten() {
                        if file.file_type().map(|t| t.is_file()).unwrap_or(false) {
                            let name = file.file_name().to_string_lossy().to_string();
                            let size = file.metadata().map(|m| m.len()).unwrap_or(0);
                            items.push(MemoryItem::KnowledgeFile {
                                path: file.path(),
                                name,
                                size,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort items within each project: sessions by date desc, then knowledge files
    for items in project_map.values_mut() {
        items.sort_by(|a, b| match (a, b) {
            (MemoryItem::Session { date: da, .. }, MemoryItem::Session { date: db, .. }) => {
                db.cmp(da)
            }
            (MemoryItem::Session { .. }, MemoryItem::KnowledgeFile { .. }) => {
                std::cmp::Ordering::Less
            }
            (MemoryItem::KnowledgeFile { .. }, MemoryItem::Session { .. }) => {
                std::cmp::Ordering::Greater
            }
            (
                MemoryItem::KnowledgeFile { name: na, .. },
                MemoryItem::KnowledgeFile { name: nb, .. },
            ) => na.cmp(nb),
        });
    }

    let projects = project_map
        .into_iter()
        .map(|(name, items)| ProjectEntry { name, items })
        .collect();

    MemoryTree { projects }
}

/// Delete a memory entry (file or directory).
pub fn delete_entry(path: &Path) -> io::Result<()> {
    if path.is_dir() {
        fs::remove_dir_all(path)
    } else {
        fs::remove_file(path)
    }
}

/// Compute total size and latest modification time for a directory.
fn dir_stats(dir: &Path) -> (u64, DateTime<Utc>) {
    let mut total_size = 0u64;
    let mut latest: DateTime<Utc> = DateTime::UNIX_EPOCH;

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            if let Ok(meta) = entry.metadata() {
                total_size += meta.len();
                if let Ok(modified) = meta.modified() {
                    let dt: DateTime<Utc> = modified.into();
                    if dt > latest {
                        latest = dt;
                    }
                }
            }
        }
    }

    (total_size, latest)
}

/// Installed pack entry for TUI display
#[derive(Clone)]
pub struct PackEntry {
    pub name: String,
    pub version: String,
    pub description: String,
    pub registry: String,
    pub categories: Vec<String>,
    pub keywords: Vec<String>,
    pub installed_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct InstalledPackStore {
    packs: Vec<InstalledPackMetadata>,
}

#[derive(Deserialize)]
struct InstalledPackMetadata {
    name: String,
    registry: String,
    version: String,
    installed_at: String,
    path: PathBuf,
}

#[derive(Deserialize)]
struct PackManifest {
    name: String,
    version: String,
    description: String,
    categories: Vec<String>,
    keywords: Vec<String>,
}

/// Load installed packs for TUI display
pub fn load_packs(memory_dir: &Path) -> Vec<PackEntry> {
    let installed_packs_path = memory_dir.join("hive/installed_packs.json");

    if !installed_packs_path.exists() {
        return Vec::new();
    }

    let content = match fs::read_to_string(&installed_packs_path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let store: InstalledPackStore = match serde_json::from_str(&content) {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };

    let mut entries = Vec::new();

    for installed in store.packs {
        // Load manifest for full details
        let manifest_path = installed.path.join(".pack/manifest.json");
        if let Ok(manifest_content) = fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<PackManifest>(&manifest_content) {
                // Parse installed_at timestamp
                let installed_at = DateTime::parse_from_rfc3339(&installed.installed_at)
                    .map(|dt| dt.with_timezone(&Utc))
                    .unwrap_or_else(|_| Utc::now());

                entries.push(PackEntry {
                    name: installed.name,
                    version: installed.version,
                    description: manifest.description,
                    registry: installed.registry,
                    categories: manifest.categories,
                    keywords: manifest.keywords,
                    installed_at,
                });
            }
        }
    }

    // Sort by name
    entries.sort_by(|a, b| a.name.cmp(&b.name));

    entries
}

/// Render detailed pack information for TUI display
pub fn render_pack_detail(pack: &PackEntry, memory_dir: &Path) -> String {
    let mut output = String::new();

    // Header
    output.push_str(&format!("# {} v{}\n\n", pack.name, pack.version));
    output.push_str(&format!("{}\n\n", pack.description));

    // Metadata
    output.push_str("## Metadata\n\n");
    output.push_str(&format!("**Registry:** {}\n", pack.registry));
    output.push_str(&format!("**Version:** {}\n", pack.version));
    output.push_str(&format!(
        "**Installed:** {}\n",
        pack.installed_at.format("%Y-%m-%d %H:%M:%S")
    ));
    output.push_str(&format!("**Categories:** {}\n", pack.categories.join(", ")));
    if !pack.keywords.is_empty() {
        output.push_str(&format!("**Keywords:** {}\n", pack.keywords.join(", ")));
    }
    output.push('\n');

    // Load manifest for full details
    let pack_path = memory_dir.join("packs/installed").join(&pack.name);
    let manifest_path = pack_path.join(".pack/manifest.json");

    if let Ok(manifest_content) = fs::read_to_string(&manifest_path) {
        if let Ok(manifest) = serde_json::from_str::<serde_json::Value>(&manifest_content) {
            if let Some(author) = manifest.get("author") {
                output.push_str("## Author\n\n");
                if let Some(name) = author.get("name").and_then(|v| v.as_str()) {
                    output.push_str(&format!("**Name:** {}\n", name));
                }
                if let Some(email) = author.get("email").and_then(|v| v.as_str()) {
                    output.push_str(&format!("**Email:** {}\n", email));
                }
                output.push('\n');
            }

            if let Some(repo) = manifest.get("repository").and_then(|v| v.as_str()) {
                output.push_str(&format!("**Repository:** {}\n\n", repo));
            }

            if let Some(homepage) = manifest.get("homepage").and_then(|v| v.as_str()) {
                output.push_str(&format!("**Homepage:** {}\n\n", homepage));
            }

            if let Some(license) = manifest.get("license").and_then(|v| v.as_str()) {
                output.push_str(&format!("**License:** {}\n\n", license));
            }
        }
    }

    // Knowledge statistics
    output.push_str("## Knowledge Contents\n\n");
    let knowledge_dir = pack_path.join("knowledge");

    if knowledge_dir.exists() {
        for category in &[
            "patterns.md",
            "solutions.md",
            "workflows.md",
            "decisions.md",
            "preferences.md",
        ] {
            let file_path = knowledge_dir.join(category);
            if file_path.exists() {
                if let Ok(content) = fs::read_to_string(&file_path) {
                    let entry_count = content.matches("## Session:").count();
                    let size = content.len();
                    let size_kb = size / 1024;

                    output.push_str(&format!(
                        "**{}:** {} entries ({} KB)\n",
                        category.replace(".md", "").to_uppercase(),
                        entry_count,
                        size_kb
                    ));
                }
            }
        }
    }

    output.push('\n');

    // Preview of first knowledge file
    output.push_str("## Knowledge Preview\n\n");
    for category in &["patterns.md", "solutions.md", "workflows.md"] {
        let file_path = knowledge_dir.join(category);
        if file_path.exists() {
            if let Ok(content) = fs::read_to_string(&file_path) {
                output.push_str(&format!(
                    "### {}\n\n",
                    category.replace(".md", "").to_uppercase()
                ));

                // Show first 500 bytes (clamped to char boundary)
                let preview = if content.len() > 500 {
                    let end = {
                        let mut e = 500;
                        while !content.is_char_boundary(e) {
                            e -= 1;
                        }
                        e
                    };
                    format!("{}...\n\n(Use 'v' to view full content)", &content[..end])
                } else {
                    content
                };

                output.push_str(&preview);
                output.push_str("\n\n");
                break; // Only show first available file
            }
        }
    }

    output
}

/// Load learning dashboard data for a project
pub fn load_learning_dashboard(memory_dir: &Path, project: &str) -> String {
    use crate::learning::progress::load_state;

    let learning_dir = memory_dir.join("learning");

    if !learning_dir.join(project).join("state.json").exists() {
        return format!(
            "No learning data for project '{}'\n\nRun learning simulation to generate data:\n  engram learn simulate --project {}",
            project, project
        );
    }

    match load_state(memory_dir, project) {
        Ok(state) => {
            // Capture the display output as a string
            let mut output = String::new();
            output.push_str(&format!("Learning Progress: {}\n", project));
            output.push_str(&format!(
                "Created: {}\n",
                state.created_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!(
                "Updated: {}\n",
                state.updated_at.format("%Y-%m-%d %H:%M:%S")
            ));
            output.push_str(&format!("Sessions: {}\n\n", state.session_count()));

            let converged = state.has_converged();
            output.push_str(&format!(
                "Status: {}\n\n",
                if converged {
                    "Converged ✓"
                } else {
                    "Learning..."
                }
            ));

            if let Some(latest) = state.metrics_history.last() {
                output.push_str("Current Metrics:\n");
                output.push_str(&format!("  Health Score: {}\n", latest.health_score));
                output.push_str(&format!(
                    "  Avg Query Time: {} ms\n",
                    latest.avg_query_time_ms
                ));
                output.push_str(&format!(
                    "  Stale Knowledge: {:.1}%\n",
                    latest.stale_knowledge_pct
                ));
                output.push_str(&format!(
                    "  Storage Size: {:.1} MB\n",
                    latest.storage_size_mb
                ));
            }

            output.push_str(&format!(
                "\nAdaptation Success Rate: {:.1}%\n",
                state.adaptation_success_rate() * 100.0
            ));
            output.push_str(&format!(
                "Total Adaptations: {}\n",
                state.adaptation_history.len()
            ));

            output
        }
        Err(e) => format!("Error loading learning state: {}", e),
    }
}

/// Load analytics data for a project
pub fn load_analytics(memory_dir: &Path, project: &str, days: u32) -> String {
    use crate::analytics::insights::generate_insights;
    use crate::analytics::tracker::EventTracker;

    let tracker = EventTracker::new(memory_dir);

    match tracker.get_events(Some(project), days) {
        Ok(events) => {
            let insights = generate_insights(&events);

            let mut output = String::new();
            output.push_str(&format!("Analytics: {} (last {} days)\n\n", project, days));
            output.push_str(&format!("Total Events: {}\n", insights.total_events));
            output.push_str(&format!("Unique Projects: {}\n", insights.unique_projects));

            if let Some(most_active) = &insights.most_active_project {
                output.push_str(&format!("Most Active Project: {}\n", most_active));
            }

            output.push_str(&format!(
                "Most Common Event: {}\n",
                insights.most_common_event
            ));
            output.push_str(&format!("Usage Trend: {}\n\n", insights.usage_trend));

            if !insights.top_knowledge.is_empty() {
                output.push_str("Top Knowledge:\n");
                for item in &insights.top_knowledge {
                    output.push_str(&format!("  • {}\n", item));
                }
                output.push('\n');
            }

            if !insights.stale_knowledge.is_empty() {
                output.push_str("Stale Knowledge:\n");
                for item in &insights.stale_knowledge {
                    output.push_str(&format!("  • {}\n", item));
                }
                output.push('\n');
            }

            // Event log
            output.push_str(&format!("Recent Events ({}):\n", events.len().min(20)));
            for event in events.iter().rev().take(20) {
                output.push_str(&format!(
                    "  {} - {:?} [{}]\n",
                    event.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    event.event_type,
                    event.project
                ));
            }

            output
        }
        Err(e) => format!("Error loading analytics: {}", e),
    }
}

/// Load health check report for a project
pub fn load_health_report(memory_dir: &Path, project: &str) -> String {
    use crate::health::{check_project_health, Severity};

    match check_project_health(memory_dir, project) {
        Ok(report) => {
            let mut output = String::new();
            output.push_str(&format!("Health Check: {}\n\n", project));
            output.push_str(&format!(
                "Score: {}/100 ({})\n\n",
                report.score,
                report.health_status()
            ));

            if report.issues.is_empty() {
                output.push_str("✓ No issues found!\n");
            } else {
                output.push_str(&format!("Issues ({}):\n", report.issues.len()));

                // Group by severity
                let critical: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Critical)
                    .collect();
                let warnings: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Warning)
                    .collect();
                let info: Vec<_> = report
                    .issues
                    .iter()
                    .filter(|i| i.severity == Severity::Info)
                    .collect();

                if !critical.is_empty() {
                    output.push_str("\nCRITICAL:\n");
                    for issue in critical {
                        output.push_str(&format!("  ✗ {}\n", issue.description));
                        if let Some(cmd) = &issue.fix_command {
                            output.push_str(&format!("    Fix: {}\n", cmd));
                        }
                    }
                }

                if !warnings.is_empty() {
                    output.push_str("\nWARNINGS:\n");
                    for issue in warnings {
                        output.push_str(&format!("  ! {}\n", issue.description));
                        if let Some(cmd) = &issue.fix_command {
                            output.push_str(&format!("    Fix: {}\n", cmd));
                        }
                    }
                }

                if !info.is_empty() {
                    output.push_str("\nINFO:\n");
                    for issue in info {
                        output.push_str(&format!("  • {}\n", issue.description));
                    }
                }
            }

            if !report.recommendations.is_empty() {
                output.push_str("\nRecommendations:\n");
                for rec in &report.recommendations {
                    output.push_str(&format!("  → {}\n", rec));
                }
            }

            output
        }
        Err(e) => format!("Error running health check: {}", e),
    }
}

pub fn load_daemon_status(memory_dir: &Path) -> String {
    let pid_file = memory_dir.join("daemon.pid");
    let log_file = memory_dir.join("daemon.log");

    let mut output = String::new();
    output.push_str("Engram Daemon\n");
    output.push_str("=============\n\n");

    // Check PID
    let running = if let Ok(contents) = std::fs::read_to_string(&pid_file) {
        let pid: Option<u32> = contents.trim().parse().ok();
        if let Some(pid) = pid {
            let alive = pid_is_alive(pid);
            if alive {
                output.push_str(&format!("Status:  RUNNING (PID {})\n", pid));
                true
            } else {
                output.push_str(&format!("Status:  STOPPED (stale PID {})\n", pid));
                false
            }
        } else {
            output.push_str("Status:  STOPPED\n");
            false
        }
    } else {
        output.push_str("Status:  STOPPED\n");
        false
    };

    output.push_str(&format!("Log:     {}\n", log_file.display()));
    output.push('\n');

    // Show last 20 lines of log
    if log_file.exists() {
        if let Ok(contents) = std::fs::read_to_string(&log_file) {
            let lines: Vec<&str> = contents.lines().collect();
            let start = lines.len().saturating_sub(20);
            output.push_str("Recent Log (last 20 lines):\n");
            output.push_str("---------------------------\n");
            for line in &lines[start..] {
                output.push_str(line);
                output.push('\n');
            }
        }
    } else if running {
        output.push_str("(log file not yet created)\n");
    } else {
        output.push_str("No log file found. Start the daemon to create one.\n");
        output.push_str("\nQuick Start:\n");
        output.push_str("  Press [s] to start daemon (15 min interval)\n");
        output.push_str("  Press [+/-] to adjust interval before starting\n");
        output.push_str("  Or: engram daemon start --interval 30\n");
    }

    output
}
