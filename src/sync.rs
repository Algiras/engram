use crate::error::{MemoryError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// GitHub Gist API client
pub struct GistClient {
    token: String,
    client: reqwest::Client,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Gist {
    pub id: String,
    pub description: String,
    pub files: HashMap<String, GistFile>,
    pub html_url: String,
    pub public: bool,
    pub history: Option<Vec<GistHistory>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GistHistory {
    pub version: String,
    pub committed_at: String,
    pub user: Option<GistUser>,
    pub change_status: Option<GistChangeStatus>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GistUser {
    pub login: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GistChangeStatus {
    pub total: Option<i32>,
    pub additions: Option<i32>,
    pub deletions: Option<i32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GistFile {
    pub filename: String,
    pub content: Option<String>,
}

#[derive(Debug, Serialize)]
struct CreateGistRequest {
    description: String,
    public: bool,
    files: HashMap<String, CreateGistFile>,
}

#[derive(Debug, Serialize)]
struct CreateGistFile {
    content: String,
}

impl GistClient {
    pub fn new(token: String) -> Self {
        Self {
            token,
            client: reqwest::Client::new(),
        }
    }

    /// Get GitHub token from environment or gh CLI
    pub fn from_env() -> Result<Self> {
        // Try environment variables first
        let token = std::env::var("GITHUB_TOKEN")
            .or_else(|_| std::env::var("GH_TOKEN"))
            .or_else(|_| {
                // Try gh CLI auth token
                std::process::Command::new("gh")
                    .args(["auth", "token"])
                    .output()
                    .ok()
                    .and_then(|output| {
                        if output.status.success() {
                            String::from_utf8(output.stdout)
                                .ok()
                                .map(|s| s.trim().to_string())
                        } else {
                            None
                        }
                    })
                    .ok_or(std::env::VarError::NotPresent)
            })
            .map_err(|_| {
                MemoryError::Config(
                    "GitHub token not found. Set GITHUB_TOKEN, or authenticate with 'gh auth login'"
                        .into(),
                )
            })?;
        Ok(Self::new(token))
    }

    /// Create a new private gist
    pub async fn create_gist(
        &self,
        description: &str,
        files: HashMap<String, String>,
    ) -> Result<Gist> {
        let files_map: HashMap<String, CreateGistFile> = files
            .into_iter()
            .map(|(name, content)| (name, CreateGistFile { content }))
            .collect();

        let request = CreateGistRequest {
            description: description.to_string(),
            public: false,
            files: files_map,
        };

        let response = self
            .client
            .post("https://api.github.com/gists")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "engram")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "GitHub API error {}: {}",
                status, text
            )));
        }

        Ok(response.json().await?)
    }

    /// Update an existing gist
    pub async fn update_gist(
        &self,
        gist_id: &str,
        description: Option<&str>,
        files: HashMap<String, String>,
    ) -> Result<Gist> {
        let files_map: HashMap<String, CreateGistFile> = files
            .into_iter()
            .map(|(name, content)| (name, CreateGistFile { content }))
            .collect();

        let mut request_body = serde_json::json!({
            "files": files_map
        });

        if let Some(desc) = description {
            request_body["description"] = serde_json::json!(desc);
        }

        let response = self
            .client
            .patch(format!("https://api.github.com/gists/{}", gist_id))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "engram")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .json(&request_body)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "GitHub API error {}: {}",
                status, text
            )));
        }

        Ok(response.json().await?)
    }

    /// Get a gist by ID
    pub async fn get_gist(&self, gist_id: &str) -> Result<Gist> {
        let response = self
            .client
            .get(format!("https://api.github.com/gists/{}", gist_id))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "engram")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "GitHub API error {}: {}",
                status, text
            )));
        }

        Ok(response.json().await?)
    }

    /// List user's gists (first page)
    pub async fn list_gists(&self) -> Result<Vec<Gist>> {
        let response = self
            .client
            .get("https://api.github.com/gists")
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "engram")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "GitHub API error {}: {}",
                status, text
            )));
        }

        Ok(response.json().await?)
    }

    /// Get gist version history
    pub async fn get_gist_history(&self, gist_id: &str) -> Result<Vec<GistHistory>> {
        // Get gist with full history
        let response = self
            .client
            .get(format!("https://api.github.com/gists/{}", gist_id))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "engram")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "GitHub API error {}: {}",
                status, text
            )));
        }

        let gist: Gist = response.json().await?;
        Ok(gist.history.unwrap_or_default())
    }

    /// Get a specific version of a gist
    pub async fn get_gist_version(&self, gist_id: &str, version: &str) -> Result<Gist> {
        let response = self
            .client
            .get(format!(
                "https://api.github.com/gists/{}/{}",
                gist_id, version
            ))
            .header("Authorization", format!("Bearer {}", self.token))
            .header("User-Agent", "engram")
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            return Err(MemoryError::Config(format!(
                "GitHub API error {}: {}",
                status, text
            )));
        }

        Ok(response.json().await?)
    }
}

/// Read knowledge files for a project with automatic secret redaction
pub fn read_knowledge_files(
    memory_dir: &std::path::Path,
    project: &str,
) -> Result<HashMap<String, String>> {
    use crate::hive::SecretDetector;

    let knowledge_dir = memory_dir.join("knowledge").join(project);
    let mut files = HashMap::new();

    let file_names = ["decisions.md", "solutions.md", "patterns.md", "context.md"];

    // Detect and redact secrets
    let detector = SecretDetector::new()?;

    for file_name in &file_names {
        let path = knowledge_dir.join(file_name);
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            if !content.trim().is_empty() {
                // Redact secrets before uploading
                let redacted = detector.redact_secrets(&content);
                files.insert(file_name.to_string(), redacted);
            }
        }
    }

    // Add metadata
    let metadata = serde_json::json!({
        "project": project,
        "synced_at": chrono::Utc::now().to_rfc3339(),
        "tool": "engram",
        "version": env!("CARGO_PKG_VERSION"),
    });
    files.insert(
        "metadata.json".to_string(),
        serde_json::to_string_pretty(&metadata)?,
    );

    Ok(files)
}

// ── Git Repository Operations ──────────────────────────────────────────

/// Initialize a git repository for knowledge sharing
pub fn init_git_repo(repo_path: &std::path::Path) -> Result<()> {
    std::fs::create_dir_all(repo_path)?;

    let status = std::process::Command::new("git")
        .args(["init"])
        .current_dir(repo_path)
        .status()?;

    if !status.success() {
        return Err(MemoryError::Config(
            "Failed to initialize git repository".into(),
        ));
    }

    // Create .gitignore
    let gitignore = repo_path.join(".gitignore");
    std::fs::write(&gitignore, "*.tmp\n*.lock\n.DS_Store\n")?;

    // Create README
    let readme = repo_path.join("README.md");
    std::fs::write(
        &readme,
        "# Claude Memory Knowledge Repository\n\nShared knowledge base synced by engram.\n",
    )?;

    // Initial commit
    std::process::Command::new("git")
        .args(["add", "."])
        .current_dir(repo_path)
        .status()?;

    std::process::Command::new("git")
        .args([
            "commit",
            "-m",
            "Initial commit: engram knowledge repository",
        ])
        .current_dir(repo_path)
        .status()?;

    Ok(())
}

/// Push knowledge to a git repository
pub fn push_to_git_repo(
    memory_dir: &std::path::Path,
    project: &str,
    repo_path: &std::path::Path,
    commit_message: Option<&str>,
    push_remote: bool,
) -> Result<()> {
    if !repo_path.join(".git").exists() {
        return Err(MemoryError::Config(format!(
            "Not a git repository: {}. Run 'engram sync init-repo' first.",
            repo_path.display()
        )));
    }

    // Create project directory in repo
    let project_dir = repo_path.join(project);
    std::fs::create_dir_all(&project_dir)?;

    // Read knowledge files
    let files = read_knowledge_files(memory_dir, project)?;

    // Write files to repo
    for (filename, content) in &files {
        let target = project_dir.join(filename);
        std::fs::write(target, content)?;
    }

    // Git add
    std::process::Command::new("git")
        .args(["add", project])
        .current_dir(repo_path)
        .status()?;

    // Git commit
    let default_message = format!("Update {} knowledge", project);
    let message = commit_message.unwrap_or(&default_message);
    let status = std::process::Command::new("git")
        .args(["commit", "-m", message])
        .current_dir(repo_path)
        .status()?;

    // Commit might fail if no changes, that's OK
    let has_changes = status.success();

    // Git push if requested and there are changes
    if push_remote && has_changes {
        let status = std::process::Command::new("git")
            .args(["push"])
            .current_dir(repo_path)
            .status()?;

        if !status.success() {
            return Err(MemoryError::Config(
                "Failed to push to remote. Check git remote configuration.".into(),
            ));
        }
    }

    Ok(())
}

/// Pull knowledge from a git repository
pub fn pull_from_git_repo(
    memory_dir: &std::path::Path,
    project: &str,
    repo_path: &std::path::Path,
    fetch_remote: bool,
    branch: &str,
) -> Result<()> {
    if !repo_path.join(".git").exists() {
        return Err(MemoryError::Config(format!(
            "Not a git repository: {}",
            repo_path.display()
        )));
    }

    // Fetch from remote if requested
    if fetch_remote {
        std::process::Command::new("git")
            .args(["fetch"])
            .current_dir(repo_path)
            .status()?;

        std::process::Command::new("git")
            .args(["checkout", branch])
            .current_dir(repo_path)
            .status()?;

        std::process::Command::new("git")
            .args(["pull"])
            .current_dir(repo_path)
            .status()?;
    }

    // Read files from repo
    let project_dir = repo_path.join(project);
    if !project_dir.exists() {
        return Err(MemoryError::Config(format!(
            "Project '{}' not found in repository",
            project
        )));
    }

    let knowledge_dir = memory_dir.join("knowledge").join(project);
    std::fs::create_dir_all(&knowledge_dir)?;

    // Copy knowledge files
    for entry in std::fs::read_dir(&project_dir)? {
        let entry = entry?;
        let filename = entry.file_name();
        let source = entry.path();
        let target = knowledge_dir.join(&filename);

        if source.is_file() {
            std::fs::copy(&source, &target)?;
        }
    }

    Ok(())
}

/// Write knowledge files for a project
pub fn write_knowledge_files(
    memory_dir: &std::path::Path,
    project: &str,
    files: &HashMap<String, GistFile>,
) -> Result<()> {
    let knowledge_dir = memory_dir.join("knowledge").join(project);
    std::fs::create_dir_all(&knowledge_dir)?;

    for (filename, gist_file) in files {
        if filename == "metadata.json" {
            continue; // Skip metadata
        }

        if let Some(content) = &gist_file.content {
            let path = knowledge_dir.join(filename);
            std::fs::write(path, content)?;
        }
    }

    Ok(())
}
