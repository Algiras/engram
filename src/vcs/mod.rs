use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::{MemoryError, Result};
use crate::extractor::knowledge::parse_session_blocks;

pub use crate::config::CATEGORIES;

// ── Data types ────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct CommitObject {
    pub hash: String,
    pub parent: Option<String>,
    pub message: String,
    pub author: String,
    pub timestamp: DateTime<Utc>,
    pub session_ids: Vec<String>,
    pub category_hashes: HashMap<String, String>,
    pub branch: String,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct StagingIndex {
    /// session_id → categories it appears in
    pub entries: HashMap<String, Vec<String>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct SessionRef {
    pub session_id: String,
    pub categories: Vec<String>,
    pub timestamp: String,
    pub preview: String,
}

#[derive(Debug)]
pub struct VcsStatus {
    pub current_branch: String,
    pub head_hash: Option<String>,
    pub detached_head: bool,
    pub unstaged_new: Vec<SessionRef>,
    pub unstaged_removed: Vec<String>,
    pub staged: Vec<SessionRef>,
}

#[derive(Debug)]
pub struct BranchRef {
    pub name: String,
    pub hash: Option<String>,
    pub is_current: bool,
}

#[derive(Debug)]
pub struct CheckoutResult {
    pub previous_branch: String,
    pub new_ref: String,
    pub blocks_added: usize,
    pub blocks_removed: usize,
    pub conflicts: Vec<String>,
}

// ── MemoryVcs ─────────────────────────────────────────────────────────────

pub struct MemoryVcs {
    vcs_dir: PathBuf,
    knowledge_dir: PathBuf,
    project: String,
}

impl MemoryVcs {
    pub fn new(memory_dir: &Path, project: &str) -> Self {
        MemoryVcs {
            vcs_dir: memory_dir.join("vcs").join(project),
            knowledge_dir: memory_dir.join("knowledge").join(project),
            project: project.to_string(),
        }
    }

    pub fn init(&self) -> Result<()> {
        std::fs::create_dir_all(self.vcs_dir.join("refs").join("heads"))?;
        std::fs::create_dir_all(self.vcs_dir.join("commits"))?;
        std::fs::create_dir_all(self.vcs_dir.join("snapshots"))?;
        std::fs::create_dir_all(self.vcs_dir.join("staging"))?;
        let head_path = self.vcs_dir.join("HEAD");
        if !head_path.exists() {
            std::fs::write(&head_path, "ref: refs/heads/main")?;
        }
        Ok(())
    }

    pub fn is_initialized(&self) -> bool {
        self.vcs_dir.join("HEAD").exists()
    }

    pub fn vcs_dir(&self) -> &Path {
        &self.vcs_dir
    }

    fn require_init(&self) -> Result<()> {
        if !self.is_initialized() {
            return Err(MemoryError::Vcs(format!(
                "VCS not initialized for project '{}'. Run: engram mem init --project {}",
                self.project, self.project
            )));
        }
        Ok(())
    }

    // ── HEAD management ───────────────────────────────────────────────────

    /// Returns (is_detached, ref_value).
    /// If not detached, ref_value is the branch name.
    /// If detached, ref_value is a commit hash (may be empty before first commit).
    fn read_head(&self) -> Result<(bool, String)> {
        let content = std::fs::read_to_string(self.vcs_dir.join("HEAD"))
            .map_err(|e| MemoryError::Vcs(format!("Cannot read HEAD: {}", e)))?;
        let content = content.trim();
        if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
            Ok((false, branch.to_string()))
        } else {
            Ok((true, content.to_string()))
        }
    }

    fn write_head(&self, detached: bool, value: &str) -> Result<()> {
        let content = if detached {
            value.to_string()
        } else {
            format!("ref: refs/heads/{}", value)
        };
        std::fs::write(self.vcs_dir.join("HEAD"), content)?;
        Ok(())
    }

    fn read_branch_hash(&self, branch: &str) -> Result<Option<String>> {
        let path = self.vcs_dir.join("refs").join("heads").join(branch);
        if !path.exists() {
            return Ok(None);
        }
        let hash = std::fs::read_to_string(&path)
            .map_err(|e| MemoryError::Vcs(format!("Cannot read branch ref: {}", e)))?;
        Ok(Some(hash.trim().to_string()))
    }

    fn write_branch_hash(&self, branch: &str, hash: &str) -> Result<()> {
        let path = self.vcs_dir.join("refs").join("heads").join(branch);
        std::fs::write(&path, hash)?;
        Ok(())
    }

    /// Resolve HEAD to a commit hash (None if no commits yet).
    pub fn head_hash(&self) -> Result<Option<String>> {
        let (detached, value) = self.read_head()?;
        if detached {
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        } else {
            self.read_branch_hash(&value)
        }
    }

    // ── Commit storage ─────────────────────────────────────────────────────

    fn load_commit(&self, hash: &str) -> Result<CommitObject> {
        let path = self.vcs_dir.join("commits").join(format!("{}.json", hash));
        let content = std::fs::read_to_string(&path)
            .map_err(|e| MemoryError::Vcs(format!("Cannot read commit {}: {}", hash, e)))?;
        serde_json::from_str(&content)
            .map_err(|e| MemoryError::Vcs(format!("Cannot parse commit {}: {}", hash, e)))
    }

    fn save_commit(&self, commit: &CommitObject) -> Result<()> {
        let path = self
            .vcs_dir
            .join("commits")
            .join(format!("{}.json", commit.hash));
        let content = serde_json::to_string_pretty(commit)
            .map_err(|e| MemoryError::Vcs(format!("Cannot serialize commit: {}", e)))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    // ── Staging ────────────────────────────────────────────────────────────

    fn load_staging(&self) -> Result<StagingIndex> {
        let path = self.vcs_dir.join("staging").join("index.json");
        if !path.exists() {
            return Ok(StagingIndex::default());
        }
        let content = std::fs::read_to_string(&path)?;
        serde_json::from_str(&content)
            .map_err(|e| MemoryError::Vcs(format!("Cannot parse staging index: {}", e)))
    }

    fn save_staging(&self, index: &StagingIndex) -> Result<()> {
        let path = self.vcs_dir.join("staging").join("index.json");
        let content = serde_json::to_string_pretty(index)
            .map_err(|e| MemoryError::Vcs(format!("Cannot serialize staging: {}", e)))?;
        std::fs::write(&path, content)?;
        Ok(())
    }

    // ── Working-tree helpers ───────────────────────────────────────────────

    /// Collect all sessions currently in knowledge files, deduplicating by session_id.
    pub fn collect_working_sessions(&self) -> Result<Vec<SessionRef>> {
        let mut session_map: HashMap<String, SessionRef> = HashMap::new();
        for cat in CATEGORIES {
            let path = self.knowledge_dir.join(format!("{}.md", cat));
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let (_, blocks) = parse_session_blocks(&content);
            for block in blocks {
                let entry = session_map
                    .entry(block.session_id.clone())
                    .or_insert_with(|| SessionRef {
                        session_id: block.session_id.clone(),
                        categories: vec![],
                        timestamp: block.timestamp.clone(),
                        preview: block.preview.clone(),
                    });
                if !entry.categories.contains(&cat.to_string()) {
                    entry.categories.push(cat.to_string());
                }
            }
        }
        let mut sessions: Vec<SessionRef> = session_map.into_values().collect();
        sessions.sort_by(|a, b| a.session_id.cmp(&b.session_id));
        Ok(sessions)
    }

    fn head_session_ids(&self) -> Result<Vec<String>> {
        match self.head_hash()? {
            None => Ok(vec![]),
            Some(hash) => Ok(self.load_commit(&hash)?.session_ids),
        }
    }

    // ── Status ─────────────────────────────────────────────────────────────

    pub fn status(&self) -> Result<VcsStatus> {
        self.require_init()?;
        let (detached, head_ref) = self.read_head()?;
        let current_branch = head_ref.clone();
        let head_hash = self.head_hash()?;
        let head_ids: std::collections::HashSet<String> =
            self.head_session_ids()?.into_iter().collect();
        let working_sessions = self.collect_working_sessions()?;
        let staging = self.load_staging()?;
        let staged_ids: std::collections::HashSet<String> =
            staging.entries.keys().cloned().collect();
        let working_ids: std::collections::HashSet<String> = working_sessions
            .iter()
            .map(|s| s.session_id.clone())
            .collect();

        let unstaged_new: Vec<SessionRef> = working_sessions
            .iter()
            .filter(|s| !head_ids.contains(&s.session_id) && !staged_ids.contains(&s.session_id))
            .cloned()
            .collect();

        let mut unstaged_removed: Vec<String> = head_ids
            .iter()
            .filter(|id| !working_ids.contains(*id))
            .cloned()
            .collect();
        unstaged_removed.sort();

        let staged: Vec<SessionRef> = working_sessions
            .iter()
            .filter(|s| staged_ids.contains(&s.session_id))
            .cloned()
            .collect();

        Ok(VcsStatus {
            current_branch,
            head_hash,
            detached_head: detached,
            unstaged_new,
            unstaged_removed,
            staged,
        })
    }

    // ── Stage ──────────────────────────────────────────────────────────────

    pub fn stage_sessions(&self, ids: &[&str]) -> Result<usize> {
        self.require_init()?;
        let working_sessions = self.collect_working_sessions()?;
        let working_map: HashMap<String, &SessionRef> = working_sessions
            .iter()
            .map(|s| (s.session_id.clone(), s))
            .collect();
        let mut staging = self.load_staging()?;
        let mut count = 0usize;
        for id in ids {
            let session = working_map.get(*id).ok_or_else(|| {
                MemoryError::Vcs(format!("Session '{}' not found in knowledge files.", id))
            })?;
            if staging
                .entries
                .insert(id.to_string(), session.categories.clone())
                .is_none()
            {
                count += 1;
            }
        }
        staging.updated_at = Some(Utc::now());
        self.save_staging(&staging)?;
        Ok(count)
    }

    pub fn stage_all_new(&self) -> Result<usize> {
        self.require_init()?;
        let head_ids: std::collections::HashSet<String> =
            self.head_session_ids()?.into_iter().collect();
        let working = self.collect_working_sessions()?;
        let new_ids: Vec<&str> = working
            .iter()
            .filter(|s| !head_ids.contains(&s.session_id))
            .map(|s| s.session_id.as_str())
            .collect();
        if new_ids.is_empty() {
            return Ok(0);
        }
        self.stage_sessions(&new_ids)
    }

    // ── Commit ─────────────────────────────────────────────────────────────

    fn compute_commit_hash(&self, session_ids: &[String]) -> Result<String> {
        let mut pairs = Vec::new();
        for id in session_ids {
            let ch = self.content_hash_for_session(id)?;
            pairs.push(format!("{}:{}", id, ch));
        }
        pairs.sort();
        let mut hasher = Sha256::new();
        hasher.update(pairs.join("\n").as_bytes());
        Ok(format!("{:x}", hasher.finalize())[..8].to_string())
    }

    fn content_hash_for_session(&self, session_id: &str) -> Result<String> {
        let mut parts = Vec::new();
        for cat in CATEGORIES {
            let path = self.knowledge_dir.join(format!("{}.md", cat));
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let (_, blocks) = parse_session_blocks(&content);
            if let Some(block) = blocks.iter().find(|b| b.session_id == session_id) {
                parts.push(format!("{}:{}", cat, block.content));
            }
        }
        let mut hasher = Sha256::new();
        hasher.update(parts.join("\n").as_bytes());
        Ok(format!("{:x}", hasher.finalize())[..16].to_string())
    }

    fn category_hash(&self, category: &str) -> Result<Option<String>> {
        let path = self.knowledge_dir.join(format!("{}.md", category));
        if !path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&path)?;
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        Ok(Some(format!("{:x}", hasher.finalize())[..16].to_string()))
    }

    /// Create a commit.
    /// - `explicit_ids`: additive — these sessions merged with HEAD's set
    /// - `all_new`: snapshot mode — commit the *entire* current working state,
    ///   enables `forget` + `commit -a` to record deletions
    /// - neither: staging index (additive)
    pub fn commit(
        &self,
        message: &str,
        explicit_ids: Option<Vec<String>>,
        all_new: bool,
    ) -> Result<CommitObject> {
        self.require_init()?;

        let all_ids: Vec<String> = if all_new {
            // Snapshot mode: exactly the current working sessions.
            // Deletions (via `forget`) are naturally included.
            let mut ids: Vec<String> = self
                .collect_working_sessions()?
                .into_iter()
                .map(|s| s.session_id)
                .collect();
            if ids.is_empty() {
                return Err(MemoryError::Vcs(
                    "No knowledge sessions found — nothing to commit.".to_string(),
                ));
            }
            ids.sort();
            ids
        } else {
            // Additive mode: merge explicit or staged IDs with HEAD.
            let new_ids: Vec<String> = if let Some(ids) = explicit_ids {
                ids
            } else {
                let staging = self.load_staging()?;
                if staging.entries.is_empty() {
                    return Err(MemoryError::Vcs(
                        "Nothing staged. Use 'engram mem stage' first, or pass -a.".to_string(),
                    ));
                }
                staging.entries.keys().cloned().collect()
            };
            if new_ids.is_empty() {
                return Err(MemoryError::Vcs(
                    "Nothing to commit — working tree is clean.".to_string(),
                ));
            }
            let mut merged = self.head_session_ids()?;
            for id in &new_ids {
                if !merged.contains(id) {
                    merged.push(id.clone());
                }
            }
            merged.sort();
            merged
        };

        let hash = self.compute_commit_hash(&all_ids)?;
        let parent = self.head_hash()?;

        // If hash == HEAD, session set is unchanged — nothing to commit
        if parent.as_deref() == Some(hash.as_str()) {
            return Err(MemoryError::Vcs(
                "Nothing to commit — all specified sessions are already in HEAD.".to_string(),
            ));
        }

        let (detached, branch_ref) = self.read_head()?;
        let branch = if detached {
            "detached".to_string()
        } else {
            branch_ref.clone()
        };

        let mut category_hashes = HashMap::new();
        for cat in CATEGORIES {
            if let Some(h) = self.category_hash(cat)? {
                category_hashes.insert(cat.to_string(), h);
            }
        }

        let commit = CommitObject {
            hash: hash.clone(),
            parent,
            message: message.to_string(),
            author: "engram".to_string(),
            timestamp: Utc::now(),
            session_ids: all_ids,
            category_hashes,
            branch: branch.clone(),
        };

        self.save_commit(&commit)?;
        self.save_snapshot(&hash)?;

        if !detached {
            self.write_branch_hash(&branch_ref, &hash)?;
        } else {
            self.write_head(true, &hash)?;
        }

        self.save_staging(&StagingIndex::default())?;
        Ok(commit)
    }

    fn save_snapshot(&self, hash: &str) -> Result<()> {
        let dir = self.vcs_dir.join("snapshots").join(hash);
        std::fs::create_dir_all(&dir)?;
        for cat in CATEGORIES {
            let src = self.knowledge_dir.join(format!("{}.md", cat));
            if src.exists() {
                std::fs::copy(&src, dir.join(format!("{}.md", cat)))?;
            }
        }
        Ok(())
    }

    // ── Log ────────────────────────────────────────────────────────────────

    pub fn log(
        &self,
        from: Option<&str>,
        limit: usize,
        grep: Option<&str>,
    ) -> Result<Vec<CommitObject>> {
        self.require_init()?;
        let start = match from {
            Some(r) => Some(self.resolve_target(r)?),
            None => self.head_hash()?,
        };
        let pattern = grep.map(|g| g.to_lowercase());
        let mut commits = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut current = start;
        while let Some(hash) = current {
            if commits.len() >= limit {
                break;
            }
            if !seen.insert(hash.clone()) {
                break; // cycle guard
            }
            let commit = self.load_commit(&hash)?;
            current = commit.parent.clone();
            if let Some(pat) = &pattern {
                let msg_match = commit.message.to_lowercase().contains(pat.as_str());
                let id_match = commit
                    .session_ids
                    .iter()
                    .any(|id| id.to_lowercase().contains(pat.as_str()));
                if msg_match || id_match {
                    commits.push(commit);
                }
            } else {
                commits.push(commit);
            }
        }
        Ok(commits)
    }

    // ── Branches ───────────────────────────────────────────────────────────

    pub fn list_branches(&self) -> Result<Vec<BranchRef>> {
        self.require_init()?;
        let heads_dir = self.vcs_dir.join("refs").join("heads");
        let (detached, current_ref) = self.read_head()?;
        let mut branches = Vec::new();
        if heads_dir.exists() {
            for entry in std::fs::read_dir(&heads_dir)? {
                let entry = entry?;
                let name = entry.file_name().to_string_lossy().to_string();
                let hash = std::fs::read_to_string(entry.path())
                    .ok()
                    .map(|s| s.trim().to_string());
                branches.push(BranchRef {
                    is_current: !detached && current_ref == name,
                    name,
                    hash,
                });
            }
        }
        branches.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(branches)
    }

    pub fn create_branch(&self, name: &str, from_hash: Option<&str>) -> Result<()> {
        self.require_init()?;
        if name.contains('/') || name.contains(' ') {
            return Err(MemoryError::Vcs(format!(
                "Invalid branch name '{}': cannot contain '/' or spaces.",
                name
            )));
        }
        let path = self.vcs_dir.join("refs").join("heads").join(name);
        if path.exists() {
            return Err(MemoryError::Vcs(format!(
                "Branch '{}' already exists.",
                name
            )));
        }
        let hash = if let Some(h) = from_hash {
            h.to_string()
        } else {
            self.head_hash()?.ok_or_else(|| {
                MemoryError::Vcs(
                    "No commits yet — make an initial commit before creating a branch.".to_string(),
                )
            })?
        };
        self.write_branch_hash(name, &hash)?;
        Ok(())
    }

    pub fn delete_branch(&self, name: &str) -> Result<()> {
        self.require_init()?;
        let (detached, current_ref) = self.read_head()?;
        if !detached && current_ref == name {
            return Err(MemoryError::Vcs(format!(
                "Cannot delete the currently checked-out branch '{}'.",
                name
            )));
        }
        let path = self.vcs_dir.join("refs").join("heads").join(name);
        if !path.exists() {
            return Err(MemoryError::Vcs(format!("Branch '{}' not found.", name)));
        }
        std::fs::remove_file(&path)?;
        Ok(())
    }

    // ── Checkout ───────────────────────────────────────────────────────────

    pub fn resolve_target(&self, target: &str) -> Result<String> {
        if let Ok(Some(hash)) = self.read_branch_hash(target) {
            return Ok(hash);
        }
        let commit_path = self
            .vcs_dir
            .join("commits")
            .join(format!("{}.json", target));
        if commit_path.exists() {
            return Ok(target.to_string());
        }
        Err(MemoryError::Vcs(format!(
            "Unknown ref '{}' — not a branch name or commit hash.",
            target
        )))
    }

    pub fn has_uncommitted_changes(&self) -> Result<bool> {
        let s = self.status()?;
        Ok(!s.unstaged_new.is_empty() || !s.staged.is_empty() || !s.unstaged_removed.is_empty())
    }

    pub fn checkout(&self, target: &str, dry_run: bool, force: bool) -> Result<CheckoutResult> {
        self.require_init()?;

        let (detached, current_ref) = self.read_head()?;
        let previous_branch = if detached {
            let short = &current_ref[..current_ref.len().min(8)];
            format!("(detached:{})", short)
        } else {
            current_ref.clone()
        };

        if !force && self.has_uncommitted_changes()? {
            return Err(MemoryError::Vcs(
                "Uncommitted changes exist. Commit them first, or use --force to proceed \
                 (uncommitted sessions will be preserved)."
                    .to_string(),
            ));
        }

        let target_hash = self.resolve_target(target)?;
        let target_commit = self.load_commit(&target_hash)?;
        let snapshot_dir = self.vcs_dir.join("snapshots").join(&target_hash);
        if !snapshot_dir.exists() {
            return Err(MemoryError::Vcs(format!(
                "Snapshot missing for commit {} — repository may be corrupt.",
                target_hash
            )));
        }

        let head_ids: std::collections::HashSet<String> =
            self.head_session_ids()?.into_iter().collect();
        let target_ids: std::collections::HashSet<String> =
            target_commit.session_ids.iter().cloned().collect();
        let working_sessions = self.collect_working_sessions()?;
        let working_ids: std::collections::HashSet<String> = working_sessions
            .iter()
            .map(|s| s.session_id.clone())
            .collect();

        // Sessions in working tree that have never been committed (to any branch)
        let uncommitted_ids: std::collections::HashSet<String> = working_sessions
            .iter()
            .filter(|s| !head_ids.contains(&s.session_id))
            .map(|s| s.session_id.clone())
            .collect();

        let mut blocks_added = 0usize;
        let mut blocks_removed = 0usize;
        let mut conflicts: Vec<String> = Vec::new();

        if dry_run {
            for id in &target_ids {
                if !working_ids.contains(id) {
                    blocks_added += 1;
                }
            }
            for id in &head_ids {
                if !target_ids.contains(id) && working_ids.contains(id) {
                    blocks_removed += 1;
                }
            }
            return Ok(CheckoutResult {
                previous_branch,
                new_ref: target.to_string(),
                blocks_added,
                blocks_removed,
                conflicts,
            });
        }

        // Real checkout: per-category merge
        std::fs::create_dir_all(&self.knowledge_dir)?;

        for cat in CATEGORIES {
            let snapshot_file = snapshot_dir.join(format!("{}.md", cat));
            let working_file = self.knowledge_dir.join(format!("{}.md", cat));

            // Parse working file once
            let (wc_preamble, wblocks) = if working_file.exists() {
                let wc = std::fs::read_to_string(&working_file)?;
                parse_session_blocks(&wc)
            } else {
                (String::new(), vec![])
            };

            // Count blocks that will be removed (committed in HEAD, not in target)
            for block in &wblocks {
                if head_ids.contains(&block.session_id) && !target_ids.contains(&block.session_id) {
                    blocks_removed += 1;
                }
            }

            // Build uncommitted extra content (not in target, so target can't override)
            let uncommitted_extra: String = wblocks
                .iter()
                .filter(|b| {
                    uncommitted_ids.contains(&b.session_id) && !target_ids.contains(&b.session_id)
                })
                .map(|b| format!("{}{}", b.header, b.content))
                .collect();

            if snapshot_file.exists() {
                let snapshot_content = std::fs::read_to_string(&snapshot_file)?;
                let (_, sblocks) = parse_session_blocks(&snapshot_content);

                // Detect conflicts: uncommitted local version differs from target version
                for sb in &sblocks {
                    if uncommitted_ids.contains(&sb.session_id) {
                        if let Some(wb) = wblocks.iter().find(|w| w.session_id == sb.session_id) {
                            if wb.content != sb.content && !conflicts.contains(&sb.session_id) {
                                conflicts.push(sb.session_id.clone());
                            }
                        }
                    }
                    if !working_ids.contains(&sb.session_id) {
                        blocks_added += 1;
                    }
                }

                // Write: snapshot + uncommitted extra
                let mut new_content = snapshot_content;
                if !uncommitted_extra.is_empty() {
                    if !new_content.ends_with('\n') {
                        new_content.push('\n');
                    }
                    new_content.push_str(&uncommitted_extra);
                }
                std::fs::write(&working_file, new_content)?;
            } else {
                // Target has no file for this category
                if !uncommitted_extra.is_empty() {
                    let preamble = if !wc_preamble.is_empty() {
                        wc_preamble.clone()
                    } else {
                        let title: String = cat
                            .chars()
                            .enumerate()
                            .map(|(i, c)| if i == 0 { c.to_ascii_uppercase() } else { c })
                            .collect();
                        format!("# {}\n\n", title)
                    };
                    std::fs::write(&working_file, format!("{}{}", preamble, uncommitted_extra))?;
                } else if working_file.exists() {
                    // Keep only uncommitted blocks; remove everything committed
                    let remaining: String = wblocks
                        .iter()
                        .filter(|b| !head_ids.contains(&b.session_id))
                        .map(|b| format!("{}{}", b.header, b.content))
                        .collect();
                    if remaining.is_empty() && wc_preamble.trim().is_empty() {
                        std::fs::remove_file(&working_file).ok();
                    } else {
                        std::fs::write(&working_file, format!("{}{}", wc_preamble, remaining))?;
                    }
                }
            }
        }

        // Update HEAD
        if self.read_branch_hash(target).ok().flatten().is_some() {
            self.write_head(false, target)?;
        } else {
            self.write_head(true, &target_hash)?;
        }

        // Invalidate context.md so it gets regenerated on next inject/recall
        let ctx = self.knowledge_dir.join("context.md");
        if ctx.exists() {
            std::fs::remove_file(&ctx).ok();
        }

        Ok(CheckoutResult {
            previous_branch,
            new_ref: target.to_string(),
            blocks_added,
            blocks_removed,
            conflicts,
        })
    }

    // ── Show ───────────────────────────────────────────────────────────────

    /// Display snapshot content for a ref without checking it out.
    pub fn show(&self, target: &str, category_filter: Option<&str>) -> Result<String> {
        self.require_init()?;
        let hash = self.resolve_target(target)?;
        let commit = self.load_commit(&hash)?;
        let snapshot_dir = self.vcs_dir.join("snapshots").join(&hash);
        if !snapshot_dir.exists() {
            return Err(MemoryError::Vcs(format!(
                "Snapshot missing for commit {} — repository may be corrupt.",
                hash
            )));
        }

        let single_buf: [&str; 1];
        let cats: &[&str] = if let Some(cat) = category_filter {
            if !CATEGORIES.contains(&cat) {
                return Err(MemoryError::Vcs(format!(
                    "Unknown category '{}'. Valid: {}",
                    cat,
                    CATEGORIES.join(", ")
                )));
            }
            single_buf = [cat];
            &single_buf
        } else {
            CATEGORIES
        };

        let mut out = String::new();
        out.push_str(&format!("commit {} — branch: {}\n", hash, commit.branch));
        out.push_str(&format!(
            "Date:    {}\n",
            commit.timestamp.format("%Y-%m-%d %H:%M UTC")
        ));
        out.push_str(&format!("Message: {}\n", commit.message));
        out.push_str(&format!("Sessions: {}\n", commit.session_ids.len()));
        if let Some(parent) = &commit.parent {
            out.push_str(&format!("Parent:  {}\n", &parent[..parent.len().min(8)]));
        }
        out.push('\n');

        let mut total_blocks = 0usize;
        for cat in cats {
            let path = snapshot_dir.join(format!("{}.md", cat));
            if !path.exists() {
                continue;
            }
            let content = std::fs::read_to_string(&path)?;
            let (_, blocks) = parse_session_blocks(&content);
            if blocks.is_empty() {
                continue;
            }
            total_blocks += blocks.len();
            out.push_str(&format!(
                "── {} ({} block{})\n",
                cat,
                blocks.len(),
                if blocks.len() == 1 { "" } else { "s" }
            ));
            for block in &blocks {
                let preview: String = block.preview.chars().take(72).collect();
                out.push_str(&format!(
                    "  [{}] ({})  {}\n",
                    block.session_id, block.timestamp, preview
                ));
            }
            out.push('\n');
        }

        if total_blocks == 0 {
            out.push_str("(snapshot is empty)\n");
        }

        Ok(out)
    }

    // ── Diff ───────────────────────────────────────────────────────────────

    pub fn diff(
        &self,
        from: Option<&str>,
        to: Option<&str>,
        category_filter: Option<&str>,
    ) -> Result<String> {
        self.require_init()?;

        let from_hash: Option<String> = match from {
            Some(f) => Some(self.resolve_target(f)?),
            None => self.head_hash()?,
        };
        let to_hash: Option<String> = match to {
            Some(t) => Some(self.resolve_target(t)?),
            None => None,
        };

        let read_content = |hash_opt: Option<&str>, cat: &str| -> Result<String> {
            match hash_opt {
                Some(h) => {
                    let p = self
                        .vcs_dir
                        .join("snapshots")
                        .join(h)
                        .join(format!("{}.md", cat));
                    if p.exists() {
                        Ok(std::fs::read_to_string(&p)?)
                    } else {
                        Ok(String::new())
                    }
                }
                None => {
                    let p = self.knowledge_dir.join(format!("{}.md", cat));
                    if p.exists() {
                        Ok(std::fs::read_to_string(&p)?)
                    } else {
                        Ok(String::new())
                    }
                }
            }
        };

        // Validate category filter and build list
        let single_buf: [&str; 1];
        let cats: &[&str] = if let Some(cat) = category_filter {
            if !CATEGORIES.contains(&cat) {
                return Err(MemoryError::Vcs(format!(
                    "Unknown category '{}'. Valid: {}",
                    cat,
                    CATEGORIES.join(", ")
                )));
            }
            single_buf = [cat];
            &single_buf
        } else {
            CATEGORIES
        };

        let mut output = String::new();
        for cat in cats {
            let from_content = read_content(from_hash.as_deref(), cat)?;
            let to_content = read_content(to_hash.as_deref(), cat)?;
            if from_content == to_content {
                continue;
            }
            let diff_result = TextDiff::from_lines(&from_content, &to_content);
            output.push_str(&format!("--- a/{}\n+++ b/{}\n", cat, cat));
            for change in diff_result.iter_all_changes() {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                output.push_str(&format!("{}{}", sign, change));
            }
            output.push('\n');
        }

        if output.is_empty() {
            output = "No differences found.\n".to_string();
        }

        Ok(output)
    }
}
