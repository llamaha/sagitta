use anyhow::{Result, Context};
use log::{debug, info};
use std::path::Path;
use crate::config::types::AutoCommitConfig;
use crate::llm::fast_model::FastModelProvider;

/// Generates commit messages using AI based on git diffs
pub struct CommitMessageGenerator {
    fast_model: FastModelProvider,
    config: AutoCommitConfig,
}

impl CommitMessageGenerator {
    /// Create a new commit message generator
    pub fn new(fast_model: FastModelProvider, config: AutoCommitConfig) -> Self {
        Self {
            fast_model,
            config,
        }
    }

    /// Generate a commit message from git diff
    pub async fn generate_commit_message(
        &self,
        repo_path: &Path,
        diff_output: &str,
    ) -> Result<String> {
        if diff_output.trim().is_empty() {
            return Ok("Auto-commit: No changes detected".to_string());
        }

        let prompt = self.build_commit_prompt(diff_output)?;
        
        debug!("Generating commit message for repository: {}", repo_path.display());
        debug!("Diff size: {} characters", diff_output.len());

        let commit_message = self.fast_model.generate_simple_text(&prompt)
            .await
            .context("Failed to generate commit message using fast model")?;

        let final_message = self.format_commit_message(&commit_message)?;
        
        info!("Generated commit message: {}", final_message.lines().next().unwrap_or(""));
        
        Ok(final_message)
    }

    /// Build the prompt for commit message generation
    fn build_commit_prompt(&self, diff_output: &str) -> Result<String> {
        let system_prompt = r#"You are an expert developer assistant. Generate a clear, concise commit message based on the provided git diff.

Guidelines for commit messages:
- First line: Brief summary (50 chars or less) following conventional commit format if applicable
- Use present tense, imperative mood ("Add feature" not "Added feature")
- Focus on WHAT changed and WHY, not HOW
- If multiple types of changes, prioritize the most significant
- Common prefixes: feat:, fix:, refactor:, docs:, style:, test:, chore:
- Keep the first line focused and specific

Examples:
- "feat: add user authentication system"
- "fix: resolve memory leak in file processor"
- "refactor: simplify database connection logic"
- "docs: update API documentation for v2.0"

If the changes are extensive or include multiple unrelated changes, create a descriptive summary.
Only respond with the commit message, no additional text or explanation."#;

        let user_prompt = format!(
            "Generate a commit message for these changes:\n\n```diff\n{}\n```",
            // Limit diff size to prevent prompt overflow
            if diff_output.len() > 4000 {
                &diff_output[..4000]
            } else {
                diff_output
            }
        );

        Ok(format!("{}\n\n{}", system_prompt, user_prompt))
    }

    /// Format the final commit message with attribution
    fn format_commit_message(&self, generated_message: &str) -> Result<String> {
        let clean_message = generated_message.trim();
        
        // Split into title and body if the generated message has multiple lines
        let lines: Vec<&str> = clean_message.lines().collect();
        let title = lines.first().unwrap_or(&"Auto-commit").trim();
        
        // Apply template if configured
        let formatted_message = if !self.config.commit_message_template.is_empty() 
            && self.config.commit_message_template.contains("{summary}") {
            
            let body = if lines.len() > 1 {
                lines[1..].join("\n").trim().to_string()
            } else {
                String::new()
            };

            self.config.commit_message_template
                .replace("{summary}", title)
                .replace("{details}", &body)
        } else {
            clean_message.to_string()
        };

        // Add attribution if configured
        let final_message = if !self.config.attribution.is_empty() {
            if formatted_message.contains('\n') {
                format!("{}\n\n{}", formatted_message, self.config.attribution)
            } else {
                format!("{}\n\n{}", formatted_message, self.config.attribution)
            }
        } else {
            formatted_message
        };

        Ok(final_message)
    }

    /// Generate a simple commit message without AI (fallback)
    pub fn generate_fallback_message(&self, file_count: usize, additions: usize, deletions: usize) -> String {
        let summary = match file_count {
            0 => "Auto-commit: No changes detected".to_string(),
            1 => "Auto-commit: Update 1 file".to_string(),
            n => format!("Auto-commit: Update {} files", n),
        };

        let details = if additions > 0 || deletions > 0 {
            format!("\n+{} -{} lines", additions, deletions)
        } else {
            String::new()
        };

        let message = format!("{}{}", summary, details);

        if !self.config.attribution.is_empty() {
            format!("{}\n\n{}", message, self.config.attribution)
        } else {
            message
        }
    }

    /// Update configuration
    pub fn update_config(&mut self, config: AutoCommitConfig) {
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &AutoCommitConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::types::AutoCommitConfig;

    #[test]
    fn test_format_commit_message_simple() {
        let config = AutoCommitConfig {
            enabled: true,
            commit_message_template: String::new(),
            attribution: "Co-authored-by: Test AI".to_string(),
            skip_hooks: false,
            cooldown_seconds: 30,
        };

        let generator = CommitMessageGenerator {
            fast_model: FastModelProvider::new(Default::default()),
            config,
        };

        let result = generator.format_commit_message("feat: add new feature").unwrap();
        assert!(result.contains("feat: add new feature"));
        assert!(result.contains("Co-authored-by: Test AI"));
    }

    #[test]
    fn test_format_commit_message_with_template() {
        let config = AutoCommitConfig {
            enabled: true,
            commit_message_template: "Auto: {summary}\n\nDetails: {details}".to_string(),
            attribution: "Co-authored-by: Test AI".to_string(),
            skip_hooks: false,
            cooldown_seconds: 30,
        };

        let generator = CommitMessageGenerator {
            fast_model: FastModelProvider::new(Default::default()),
            config,
        };

        let input = "feat: add feature\n\nThis adds a new feature";
        let result = generator.format_commit_message(input).unwrap();
        
        assert!(result.contains("Auto: feat: add feature"));
        assert!(result.contains("Details: This adds a new feature"));
        assert!(result.contains("Co-authored-by: Test AI"));
    }

    #[test]
    fn test_generate_fallback_message() {
        let config = AutoCommitConfig {
            enabled: true,
            commit_message_template: String::new(),
            attribution: "Co-authored-by: Test AI".to_string(),
            skip_hooks: false,
            cooldown_seconds: 30,
        };

        let generator = CommitMessageGenerator {
            fast_model: FastModelProvider::new(Default::default()),
            config,
        };

        let result = generator.generate_fallback_message(3, 25, 10);
        assert!(result.contains("Update 3 files"));
        assert!(result.contains("+25 -10 lines"));
        assert!(result.contains("Co-authored-by: Test AI"));
    }

    #[test]
    fn test_build_commit_prompt() {
        let config = AutoCommitConfig::default();
        let generator = CommitMessageGenerator {
            fast_model: FastModelProvider::new(Default::default()),
            config,
        };

        let diff = "--- a/file.txt\n+++ b/file.txt\n@@ -1,3 +1,4 @@\n line1\n+line2\n line3";
        let prompt = generator.build_commit_prompt(diff).unwrap();
        
        assert!(prompt.contains("conventional commit format"));
        assert!(prompt.contains(diff));
        assert!(prompt.contains("Generate a commit message"));
    }
}