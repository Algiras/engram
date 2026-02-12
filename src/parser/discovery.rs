use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use walkdir::WalkDir;

use crate::error::Result;

#[derive(Debug, Clone)]
pub struct Project {
    /// Decoded project name (e.g., "claudius")
    pub name: String,
    /// Original directory name
    #[allow(dead_code)]
    pub dir_name: String,
    /// Full path to the project directory
    #[allow(dead_code)]
    pub path: PathBuf,
    /// JSONL session files in this project
    pub sessions: Vec<SessionFile>,
}

#[derive(Debug, Clone)]
pub struct SessionFile {
    /// Session UUID extracted from filename
    pub session_id: String,
    /// Full path to the JSONL file
    pub path: PathBuf,
    /// File size in bytes
    pub size: u64,
    /// Last modified time
    pub modified: DateTime<Utc>,
}

/// Discover all Claude projects and their session files
pub fn discover_projects(projects_dir: &Path) -> Result<Vec<Project>> {
    if !projects_dir.exists() {
        return Err(crate::error::MemoryError::NoProjectsDir(
            projects_dir.display().to_string(),
        ));
    }

    let mut projects = Vec::new();

    for entry in std::fs::read_dir(projects_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }

        let dir_name = entry.file_name().to_string_lossy().to_string();
        let name = decode_project_name(&dir_name);
        let path = entry.path();

        // Find JSONL files
        let mut sessions = Vec::new();
        for file_entry in WalkDir::new(&path)
            .max_depth(1)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
        {
            let file_path = file_entry.path().to_path_buf();
            let metadata = file_entry.metadata()?;

            let session_id = file_path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let modified = metadata
                .modified()
                .map(DateTime::<Utc>::from)
                .unwrap_or_default();

            sessions.push(SessionFile {
                session_id,
                path: file_path,
                size: metadata.len(),
                modified,
            });
        }

        // Sort sessions by modified time (newest first)
        sessions.sort_by(|a, b| b.modified.cmp(&a.modified));

        if !sessions.is_empty() {
            projects.push(Project {
                name,
                dir_name,
                path,
                sessions,
            });
        }
    }

    // Sort projects alphabetically
    projects.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(projects)
}

/// Decode project directory name to human-readable name
/// e.g., "-Users-algimantask-Projects-claudius" -> "claudius"
pub fn decode_project_name(dir_name: &str) -> String {
    // Split by '-', take the last meaningful segment
    let parts: Vec<&str> = dir_name.split('-').filter(|s| !s.is_empty()).collect();

    if parts.is_empty() {
        return dir_name.to_string();
    }

    // Try to find the project name — it's usually the last path component
    // But handle nested paths like "nile-cag-packages-nile-setup-cli"
    // Heuristic: find the segment after "Projects" or "Personal" or use the last one
    let project_markers = ["Projects", "Personal"];

    for (i, part) in parts.iter().enumerate() {
        if project_markers.contains(part) && i + 1 < parts.len() {
            // Join everything after the marker
            return parts[i + 1..].join("-");
        }
    }

    // Fallback: if path has common prefixes, skip "Users-username"
    if parts.len() >= 2 && parts[0] == "Users" {
        if parts.len() == 2 {
            // e.g., -Users-algimantask -> "~"
            return "home".to_string();
        }
        // Skip Users-username, join the rest
        return parts[2..].join("-");
    }

    // Last resort: join all parts
    parts.join("-")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_project_name() {
        assert_eq!(
            decode_project_name("-Users-algimantask-Projects-claudius"),
            "claudius"
        );
        assert_eq!(
            decode_project_name("-Users-algimantask-Personal-memory-palace"),
            "memory-palace"
        );
        assert_eq!(
            decode_project_name("-Users-algimantask-Projects-nile-cag-packages-nile-setup-cli"),
            "nile-cag-packages-nile-setup-cli"
        );
        assert_eq!(decode_project_name("-Users-algimantask-sandbox"), "sandbox");
        assert_eq!(decode_project_name("-Users-algimantask"), "home");
        assert_eq!(decode_project_name("-private-tmp-sm"), "private-tmp-sm");
    }
}
