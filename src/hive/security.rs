// Security - Secret detection and validation for knowledge packs

use crate::error::{MemoryError, Result};
use regex::Regex;
use std::path::Path;

/// Detected secret in knowledge content
#[derive(Debug, Clone)]
pub struct DetectedSecret {
    pub file_path: String,
    pub line_number: usize,
    pub pattern_name: String,
    pub matched_text: String,
}

/// Secret detection patterns
pub struct SecretDetector {
    patterns: Vec<(String, Regex)>,
}

impl SecretDetector {
    /// Create a new secret detector with default patterns
    pub fn new() -> Result<Self> {
        let patterns = vec![
            (
                "API Key",
                r#"(?i)api[_-]?key["']?\s*[:=]\s*["']?[A-Za-z0-9_-]{20,}"#,
            ),
            ("Password", r#"(?i)password["']?\s*[:=]\s*["']?[^\s]{8,}"#),
            ("OpenAI Key", r"sk-[a-zA-Z0-9]{20,}"),
            ("Anthropic Key", r"sk-ant-[a-zA-Z0-9-]{20,}"),
            ("GitHub Token", r"ghp_[A-Za-z0-9]{36}"),
            ("GitHub OAuth", r"gho_[A-Za-z0-9]{36}"),
            ("AWS Access Key", r"AKIA[0-9A-Z]{16}"),
            (
                "Private Key",
                r"-----BEGIN (RSA|OPENSSH|EC|DSA) PRIVATE KEY-----",
            ),
            ("Bearer Token", r"(?i)bearer\s+[A-Za-z0-9_-]{20,}"),
            ("JWT", r"eyJ[A-Za-z0-9_-]*\.eyJ[A-Za-z0-9_-]*\.[A-Za-z0-9_-]*"),
            ("Generic Secret", r#"(?i)secret["']?\s*[:=]\s*["']?[A-Za-z0-9_-]{20,}"#),
            ("Auth Token", r#"(?i)auth[_-]?token["']?\s*[:=]\s*["']?[A-Za-z0-9_-]{20,}"#),
        ];

        let compiled: Result<Vec<_>> = patterns
            .into_iter()
            .map(|(name, pattern)| {
                Regex::new(pattern)
                    .map(|re| (name.to_string(), re))
                    .map_err(|e| MemoryError::Config(format!("Invalid regex for {}: {}", name, e)))
            })
            .collect();

        Ok(Self {
            patterns: compiled?,
        })
    }

    /// Scan a file for secrets
    pub fn scan_file(&self, file_path: &Path) -> Result<Vec<DetectedSecret>> {
        let content = std::fs::read_to_string(file_path)?;
        let mut secrets = Vec::new();

        for (line_num, line) in content.lines().enumerate() {
            for (pattern_name, regex) in &self.patterns {
                if let Some(mat) = regex.find(line) {
                    // Skip common false positives
                    if self.is_false_positive(mat.as_str(), line) {
                        continue;
                    }

                    secrets.push(DetectedSecret {
                        file_path: file_path.to_string_lossy().to_string(),
                        line_number: line_num + 1,
                        pattern_name: pattern_name.clone(),
                        matched_text: self.redact_secret(mat.as_str()),
                    });
                }
            }
        }

        Ok(secrets)
    }

    /// Scan a directory recursively for secrets
    pub fn scan_directory(&self, dir: &Path) -> Result<Vec<DetectedSecret>> {
        let mut all_secrets = Vec::new();

        for entry in walkdir::WalkDir::new(dir)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "md" || ext == "json"))
        {
            let secrets = self.scan_file(entry.path())?;
            all_secrets.extend(secrets);
        }

        Ok(all_secrets)
    }

    /// Check if a match is likely a false positive
    fn is_false_positive(&self, matched: &str, context: &str) -> bool {
        // Common false positives
        let false_positives = [
            "example",
            "placeholder",
            "your-key-here",
            "xxx",
            "***",
            "redacted",
            "<api-key>",
            "sk-...",
            "api_key_here",
        ];

        let matched_lower = matched.to_lowercase();
        false_positives
            .iter()
            .any(|fp| matched_lower.contains(fp))
            || context.contains("Example:")
            || context.contains("example")
            || matched.chars().all(|c| c == 'x' || c == '*')
    }

    /// Redact secret for display (show first/last few chars)
    fn redact_secret(&self, secret: &str) -> String {
        if secret.len() <= 10 {
            "*".repeat(secret.len())
        } else {
            format!(
                "{}***{}",
                &secret[..3],
                &secret[secret.len() - 3..]
            )
        }
    }
}

impl Default for SecretDetector {
    fn default() -> Self {
        Self::new().unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_secret_detection() {
        let detector = SecretDetector::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.md");

        // Write test content with secrets
        std::fs::write(
            &test_file,
            "Some text\napi_key = sk-1234567890abcdefghij\nMore text",
        )
        .unwrap();

        let secrets = detector.scan_file(&test_file).unwrap();
        assert!(!secrets.is_empty());
        assert_eq!(secrets[0].line_number, 2);
    }

    #[test]
    fn test_false_positive_filtering() {
        let detector = SecretDetector::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        let test_file = temp_dir.path().join("test.md");

        // Write content with false positives
        std::fs::write(
            &test_file,
            "Example: api_key = your-key-here\napi_key = placeholder",
        )
        .unwrap();

        let secrets = detector.scan_file(&test_file).unwrap();
        assert_eq!(secrets.len(), 0, "Should filter false positives");
    }

    #[test]
    fn test_directory_scan() {
        let detector = SecretDetector::new().unwrap();

        let temp_dir = TempDir::new().unwrap();
        std::fs::write(temp_dir.path().join("file1.md"), "api_key=sk-real123456789012345678").unwrap();
        std::fs::write(temp_dir.path().join("file2.md"), "No secrets here").unwrap();

        let secrets = detector.scan_directory(temp_dir.path()).unwrap();
        assert!(!secrets.is_empty(), "Should detect at least one secret");
        assert!(secrets.iter().any(|s| s.file_path.contains("file1.md")));
    }
}
