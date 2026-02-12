// Knowledge Pack - Distributable knowledge units
//
// A knowledge pack is a shareable collection of extracted knowledge
// that can be distributed via Git repositories.

use crate::error::{MemoryError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A shareable knowledge pack (similar to Claude Code plugin)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KnowledgePack {
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Author,
    pub license: String,
    pub keywords: Vec<String>,
    pub categories: Vec<PackCategory>,
    pub homepage: Option<String>,
    pub repository: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub privacy: PrivacyPolicy,
    pub min_claude_memory_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PackCategory {
    Patterns,
    Solutions,
    Decisions,
    Workflows,
    Preferences,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrivacyPolicy {
    pub share_patterns: bool,
    pub share_solutions: bool,
    pub share_decisions: bool,
    pub share_preferences: bool,
    pub redact_secrets: bool,
    pub require_review: bool,
}

impl Default for PrivacyPolicy {
    fn default() -> Self {
        Self {
            share_patterns: true,
            share_solutions: true,
            share_decisions: false,
            share_preferences: false,
            redact_secrets: true,
            require_review: true,
        }
    }
}

impl KnowledgePack {
    /// Create a new knowledge pack with default values
    pub fn new(name: String, description: String, author: Author, repository: String) -> Self {
        Self {
            name: name.clone(),
            version: "0.1.0".to_string(),
            description,
            author,
            license: "MIT".to_string(),
            keywords: Vec::new(),
            categories: Vec::new(),
            homepage: None,
            repository,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            privacy: PrivacyPolicy::default(),
            min_claude_memory_version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }

    /// Load a pack manifest from a .pack/manifest.json file
    pub fn load(pack_dir: &Path) -> Result<Self> {
        let manifest_path = pack_dir.join(".pack").join("manifest.json");
        if !manifest_path.exists() {
            return Err(MemoryError::Config(format!(
                "Pack manifest not found: {}",
                manifest_path.display()
            )));
        }

        let content = std::fs::read_to_string(&manifest_path)?;
        let pack: KnowledgePack = serde_json::from_str(&content)
            .map_err(|e| MemoryError::Config(format!("Invalid pack manifest: {}", e)))?;

        pack.validate()?;
        Ok(pack)
    }

    /// Save pack manifest to .pack/manifest.json
    pub fn save(&self, pack_dir: &Path) -> Result<()> {
        self.validate()?;

        let pack_metadata_dir = pack_dir.join(".pack");
        std::fs::create_dir_all(&pack_metadata_dir)?;

        let manifest_path = pack_metadata_dir.join("manifest.json");
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&manifest_path, content)?;

        Ok(())
    }

    /// Validate pack manifest
    pub fn validate(&self) -> Result<()> {
        if self.name.is_empty() {
            return Err(MemoryError::Config("Pack name cannot be empty".into()));
        }

        if self.version.is_empty() {
            return Err(MemoryError::Config("Pack version cannot be empty".into()));
        }

        // Validate semantic version format (basic check)
        if !self.version.contains('.') {
            return Err(MemoryError::Config(format!(
                "Invalid version format: {}. Expected semantic version (e.g., 1.0.0)",
                self.version
            )));
        }

        if self.repository.is_empty() {
            return Err(MemoryError::Config(
                "Pack repository cannot be empty".into(),
            ));
        }

        if self.author.name.is_empty() {
            return Err(MemoryError::Config("Author name cannot be empty".into()));
        }

        Ok(())
    }

    /// Get the knowledge directory path for this pack
    pub fn knowledge_dir(&self, pack_dir: &Path) -> std::path::PathBuf {
        pack_dir.join("knowledge")
    }

    /// Get the graph directory path for this pack
    pub fn graph_dir(&self, pack_dir: &Path) -> std::path::PathBuf {
        pack_dir.join("graph")
    }

    /// Check if pack has a specific category
    pub fn has_category(&self, category: &PackCategory) -> bool {
        self.categories.contains(category)
    }

    /// Check if pack matches a keyword search
    pub fn matches_keyword(&self, query: &str) -> bool {
        let query_lower = query.to_lowercase();
        self.keywords
            .iter()
            .any(|k| k.to_lowercase().contains(&query_lower))
            || self.name.to_lowercase().contains(&query_lower)
            || self.description.to_lowercase().contains(&query_lower)
    }
}

impl Author {
    pub fn new(name: String) -> Self {
        Self { name, email: None }
    }

    pub fn with_email(name: String, email: String) -> Self {
        Self {
            name,
            email: Some(email),
        }
    }
}

impl std::fmt::Display for PackCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PackCategory::Patterns => write!(f, "patterns"),
            PackCategory::Solutions => write!(f, "solutions"),
            PackCategory::Decisions => write!(f, "decisions"),
            PackCategory::Workflows => write!(f, "workflows"),
            PackCategory::Preferences => write!(f, "preferences"),
        }
    }
}

impl std::str::FromStr for PackCategory {
    type Err = MemoryError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "patterns" => Ok(PackCategory::Patterns),
            "solutions" => Ok(PackCategory::Solutions),
            "decisions" => Ok(PackCategory::Decisions),
            "workflows" => Ok(PackCategory::Workflows),
            "preferences" => Ok(PackCategory::Preferences),
            _ => Err(MemoryError::Config(format!("Invalid category: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_pack_creation() {
        let author = Author::new("Test User".to_string());
        let pack = KnowledgePack::new(
            "test-pack".to_string(),
            "A test pack".to_string(),
            author,
            "https://github.com/test/pack".to_string(),
        );

        assert_eq!(pack.name, "test-pack");
        assert_eq!(pack.version, "0.1.0");
        assert!(pack.validate().is_ok());
    }

    #[test]
    fn test_pack_validation() {
        let author = Author::new("Test User".to_string());
        let mut pack = KnowledgePack::new(
            "test-pack".to_string(),
            "A test pack".to_string(),
            author,
            "https://github.com/test/pack".to_string(),
        );

        // Valid pack
        assert!(pack.validate().is_ok());

        // Invalid: empty name
        pack.name = "".to_string();
        assert!(pack.validate().is_err());
        pack.name = "test-pack".to_string();

        // Invalid: empty version
        pack.version = "".to_string();
        assert!(pack.validate().is_err());
        pack.version = "1.0.0".to_string();

        // Invalid: malformed version
        pack.version = "abc".to_string();
        assert!(pack.validate().is_err());
    }

    #[test]
    fn test_pack_save_load() {
        let temp_dir = TempDir::new().unwrap();
        let pack_dir = temp_dir.path();

        let author = Author::with_email("Test User".to_string(), "test@example.com".to_string());
        let pack = KnowledgePack::new(
            "test-pack".to_string(),
            "A test pack".to_string(),
            author,
            "https://github.com/test/pack".to_string(),
        );

        // Save
        pack.save(pack_dir).unwrap();

        // Verify file exists
        assert!(pack_dir.join(".pack/manifest.json").exists());

        // Load
        let loaded_pack = KnowledgePack::load(pack_dir).unwrap();
        assert_eq!(loaded_pack.name, pack.name);
        assert_eq!(loaded_pack.version, pack.version);
        assert_eq!(loaded_pack.author.email, pack.author.email);
    }

    #[test]
    fn test_category_matching() {
        let author = Author::new("Test User".to_string());
        let mut pack = KnowledgePack::new(
            "test-pack".to_string(),
            "A test pack".to_string(),
            author,
            "https://github.com/test/pack".to_string(),
        );

        pack.categories.push(PackCategory::Patterns);
        assert!(pack.has_category(&PackCategory::Patterns));
        assert!(!pack.has_category(&PackCategory::Solutions));
    }

    #[test]
    fn test_keyword_search() {
        let author = Author::new("Test User".to_string());
        let mut pack = KnowledgePack::new(
            "rust-patterns".to_string(),
            "Rust best practices and patterns".to_string(),
            author,
            "https://github.com/test/pack".to_string(),
        );

        pack.keywords.push("rust".to_string());
        pack.keywords.push("patterns".to_string());

        assert!(pack.matches_keyword("rust"));
        assert!(pack.matches_keyword("RUST"));
        assert!(pack.matches_keyword("pattern"));
        assert!(pack.matches_keyword("best"));
        assert!(!pack.matches_keyword("javascript"));
    }

    #[test]
    fn test_category_display() {
        assert_eq!(PackCategory::Patterns.to_string(), "patterns");
        assert_eq!(PackCategory::Solutions.to_string(), "solutions");
        assert_eq!(PackCategory::Decisions.to_string(), "decisions");
    }

    #[test]
    fn test_category_from_str() {
        assert_eq!(
            "patterns".parse::<PackCategory>().unwrap(),
            PackCategory::Patterns
        );
        assert_eq!(
            "SOLUTIONS".parse::<PackCategory>().unwrap(),
            PackCategory::Solutions
        );
        assert!("invalid".parse::<PackCategory>().is_err());
    }
}
