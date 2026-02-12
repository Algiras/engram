use similar::{ChangeTag, TextDiff};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum DiffType {
    Added,
    Removed,
    Modified,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct LineDiff {
    pub line_num: usize,
    pub diff_type: DiffType,
    pub old_content: Option<String>,
    pub new_content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct KnowledgeDiff {
    pub category: String,
    pub lines: Vec<LineDiff>,
    pub additions: usize,
    pub deletions: usize,
    pub modifications: usize,
}

impl KnowledgeDiff {
    pub fn is_empty(&self) -> bool {
        self.additions == 0 && self.deletions == 0 && self.modifications == 0
    }

    pub fn summary(&self) -> String {
        if self.is_empty() {
            "No changes".to_string()
        } else {
            format!(
                "+{} -{} ~{}",
                self.additions, self.deletions, self.modifications
            )
        }
    }
}

pub fn compute_diff(old: &str, new: &str, category: &str) -> KnowledgeDiff {
    let diff = TextDiff::from_lines(old, new);

    let mut lines = Vec::new();
    let mut additions: usize = 0;
    let mut deletions: usize = 0;
    let mut modifications: usize = 0;
    let mut line_num: usize = 0;

    for change in diff.iter_all_changes() {
        match change.tag() {
            ChangeTag::Equal => {
                line_num += 1;
                lines.push(LineDiff {
                    line_num,
                    diff_type: DiffType::Unchanged,
                    old_content: Some(change.to_string()),
                    new_content: Some(change.to_string()),
                });
            }
            ChangeTag::Delete => {
                line_num += 1;
                deletions += 1;
                lines.push(LineDiff {
                    line_num,
                    diff_type: DiffType::Removed,
                    old_content: Some(change.to_string()),
                    new_content: None,
                });
            }
            ChangeTag::Insert => {
                additions += 1;
                lines.push(LineDiff {
                    line_num,
                    diff_type: DiffType::Added,
                    old_content: None,
                    new_content: Some(change.to_string()),
                });
            }
        }
    }

    // Detect modifications (adjacent delete + insert)
    let mut i = 0;
    while i < lines.len() {
        if i + 1 < lines.len()
            && lines[i].diff_type == DiffType::Removed
            && lines[i + 1].diff_type == DiffType::Added
        {
            lines[i].diff_type = DiffType::Modified;
            lines[i].new_content = lines[i + 1].new_content.clone();
            lines.remove(i + 1);
            modifications += 1;
            additions = additions.saturating_sub(1);
            deletions = deletions.saturating_sub(1);
        }
        i += 1;
    }

    KnowledgeDiff {
        category: category.to_string(),
        lines,
        additions,
        deletions,
        modifications,
    }
}

impl fmt::Display for KnowledgeDiff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use colored::Colorize;

        writeln!(f, "{} {}", "Category:".bold(), self.category.cyan())?;
        writeln!(f, "{}", self.summary().dimmed())?;
        writeln!(f)?;

        for line in &self.lines {
            match line.diff_type {
                DiffType::Added => {
                    if let Some(ref content) = line.new_content {
                        writeln!(f, "{} {}", "+".green(), content.trim_end().green())?;
                    }
                }
                DiffType::Removed => {
                    if let Some(ref content) = line.old_content {
                        writeln!(f, "{} {}", "-".red(), content.trim_end().red())?;
                    }
                }
                DiffType::Modified => {
                    if let Some(ref old) = line.old_content {
                        writeln!(f, "{} {}", "-".red(), old.trim_end().red())?;
                    }
                    if let Some(ref new) = line.new_content {
                        writeln!(f, "{} {}", "+".green(), new.trim_end().green())?;
                    }
                }
                DiffType::Unchanged => {
                    // Skip unchanged lines in output (or show with context)
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_changes() {
        let old = "line 1\nline 2\n";
        let new = "line 1\nline 2\n";
        let diff = compute_diff(old, new, "test");
        assert!(diff.is_empty());
    }

    #[test]
    fn test_addition() {
        let old = "line 1\n";
        let new = "line 1\nline 2\n";
        let diff = compute_diff(old, new, "test");
        assert_eq!(diff.additions, 1);
        assert_eq!(diff.deletions, 0);
    }

    #[test]
    fn test_deletion() {
        let old = "line 1\nline 2\n";
        let new = "line 1\n";
        let diff = compute_diff(old, new, "test");
        assert_eq!(diff.additions, 0);
        assert_eq!(diff.deletions, 1);
    }

    #[test]
    fn test_modification() {
        let old = "line 1\n";
        let new = "line 1 modified\n";
        let diff = compute_diff(old, new, "test");
        assert_eq!(diff.modifications, 1);
    }
}
