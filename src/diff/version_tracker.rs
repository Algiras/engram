use crate::error::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static VERSION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgeVersion {
    pub version_id: String,
    pub timestamp: DateTime<Utc>,
    pub category: String,
    pub content_hash: String,
    pub size_bytes: usize,
}

pub struct VersionTracker {
    versions_dir: PathBuf,
}

impl VersionTracker {
    pub fn new(memory_dir: &Path, project: &str) -> Self {
        let versions_dir = memory_dir.join("versions").join(project);
        Self { versions_dir }
    }

    pub fn track_version(&self, category: &str, content: &str) -> Result<KnowledgeVersion> {
        fs::create_dir_all(&self.versions_dir)?;

        let timestamp = Utc::now();
        let content_hash = Self::hash_content(content);
        let counter = VERSION_COUNTER.fetch_add(1, Ordering::Relaxed);
        let version_id = format!("{}-{}-{}", category, timestamp.timestamp(), counter);

        let version = KnowledgeVersion {
            version_id: version_id.clone(),
            timestamp,
            category: category.to_string(),
            content_hash: content_hash.clone(),
            size_bytes: content.len(),
        };

        // Save version metadata
        let meta_file = self.versions_dir.join(format!("{}.json", version_id));
        let meta_json = serde_json::to_string_pretty(&version)?;
        fs::write(&meta_file, meta_json)?;

        // Save content snapshot
        let content_file = self.versions_dir.join(format!("{}.md", version_id));
        fs::write(&content_file, content)?;

        Ok(version)
    }

    pub fn get_versions(&self, category: &str) -> Result<Vec<KnowledgeVersion>> {
        if !self.versions_dir.exists() {
            return Ok(Vec::new());
        }

        let mut versions = Vec::new();

        for entry in fs::read_dir(&self.versions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            if let Ok(version) = serde_json::from_str::<KnowledgeVersion>(&content) {
                if version.category == category {
                    versions.push(version);
                }
            }
        }

        versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(versions)
    }

    pub fn get_version_content(&self, version_id: &str) -> Result<String> {
        let content_file = self.versions_dir.join(format!("{}.md", version_id));
        Ok(fs::read_to_string(&content_file)?)
    }

    pub fn get_latest_version(&self, category: &str) -> Result<Option<KnowledgeVersion>> {
        let versions = self.get_versions(category)?;
        Ok(versions.into_iter().next())
    }

    pub fn cleanup_old_versions(&self, keep_count: usize) -> Result<usize> {
        if !self.versions_dir.exists() {
            return Ok(0);
        }

        let mut category_versions: HashMap<String, Vec<KnowledgeVersion>> = HashMap::new();

        // Group versions by category
        for entry in fs::read_dir(&self.versions_dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) != Some("json") {
                continue;
            }

            let content = fs::read_to_string(&path)?;
            if let Ok(version) = serde_json::from_str::<KnowledgeVersion>(&content) {
                category_versions
                    .entry(version.category.clone())
                    .or_default()
                    .push(version);
            }
        }

        let mut removed = 0;

        // Remove old versions, keeping only keep_count most recent per category
        for (_, mut versions) in category_versions {
            versions.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

            for version in versions.iter().skip(keep_count) {
                let meta_file = self
                    .versions_dir
                    .join(format!("{}.json", version.version_id));
                let content_file = self.versions_dir.join(format!("{}.md", version.version_id));

                if meta_file.exists() {
                    fs::remove_file(&meta_file)?;
                }
                if content_file.exists() {
                    fs::remove_file(&content_file)?;
                }

                removed += 1;
            }
        }

        Ok(removed)
    }

    fn hash_content(content: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        content.hash(&mut hasher);
        format!("{:x}", hasher.finish())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_track_version() {
        let temp = TempDir::new().unwrap();
        let tracker = VersionTracker::new(temp.path(), "test-project");

        let content = "Test knowledge content";
        let version = tracker.track_version("decisions", content).unwrap();

        assert_eq!(version.category, "decisions");
        assert_eq!(version.size_bytes, content.len());
    }

    #[test]
    fn test_get_versions() {
        let temp = TempDir::new().unwrap();
        let tracker = VersionTracker::new(temp.path(), "test-project");

        tracker.track_version("decisions", "Version 1").unwrap();
        tracker.track_version("decisions", "Version 2").unwrap();
        tracker.track_version("patterns", "Pattern 1").unwrap();

        let versions = tracker.get_versions("decisions").unwrap();
        assert_eq!(versions.len(), 2);
    }

    #[test]
    fn test_cleanup_old_versions() {
        let temp = TempDir::new().unwrap();
        let tracker = VersionTracker::new(temp.path(), "test-project");

        for i in 0..5 {
            tracker
                .track_version("decisions", &format!("Version {}", i))
                .unwrap();
        }

        let removed = tracker.cleanup_old_versions(2).unwrap();
        assert_eq!(removed, 3);

        let versions = tracker.get_versions("decisions").unwrap();
        assert_eq!(versions.len(), 2);
    }
}
