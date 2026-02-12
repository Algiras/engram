use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};

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
                format!(
                    "  {} {} {}",
                    session_id,
                    date.format("%Y-%m-%d %H:%M"),
                    sz
                )
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
