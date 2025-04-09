use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a git repository with utility functions
pub struct GitRepo {
    pub path: PathBuf,
}

impl GitRepo {
    /// Create a new GitRepo instance
    pub fn new(path: PathBuf) -> Result<Self> {
        // Validate that this is a git repository
        let git_dir = path.join(".git");
        if !git_dir.exists() {
            return Err(anyhow!("Not a git repository: {}", path.display()));
        }

        Ok(Self { path })
    }

    /// Get the commit hash for a branch
    pub fn get_commit_hash(&self, branch: &str) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["rev-parse", branch])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?.trim().to_string())
        } else {
            Err(anyhow!(
                "Failed to get commit hash for branch {}: {}",
                branch,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Get files changed between two commits
    pub fn get_changed_files(&self, from_commit: &str, to_commit: &str) -> Result<Vec<PathBuf>> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["diff", "--name-status", from_commit, to_commit])
            .output()?;

        if output.status.success() {
            let files = String::from_utf8(output.stdout)?
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.trim().split_whitespace().collect();
                    if parts.len() >= 2 {
                        // Second part is the file path
                        Some(self.path.join(parts[1]))
                    } else {
                        None
                    }
                })
                .collect();

            Ok(files)
        } else {
            Err(anyhow!(
                "Failed to get changed files between {} and {}: {}",
                from_commit,
                to_commit,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Get the file modification time from git history
    pub fn get_file_mod_time(&self, file_path: &Path) -> Result<DateTime<Utc>> {
        // Get relative path to the repository
        let rel_path = file_path
            .strip_prefix(&self.path)
            .map_err(|_| anyhow!("File path is not in repository"))?;

        let output = Command::new("git")
            .current_dir(&self.path)
            .args([
                "log",
                "-1",
                "--format=%at",
                "--",
                rel_path.to_str().unwrap(),
            ])
            .output()?;

        if output.status.success() {
            let timestamp = String::from_utf8(output.stdout)?.trim().parse::<i64>()?;

            Ok(Utc.timestamp_opt(timestamp, 0).unwrap())
        } else {
            // If git command fails, fall back to file system time
            let metadata = file_path.metadata()?;
            let modified = metadata.modified()?;

            let system_time: DateTime<Utc> = modified.into();
            Ok(system_time)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile;

    // Helper function to set up a temporary git repository for testing
    fn create_test_repo() -> Result<(tempfile::TempDir, GitRepo)> {
        let temp_dir = tempfile::tempdir()?;
        let repo_path = temp_dir.path().to_path_buf();

        // Initialize git repo
        Command::new("git").arg("init").arg(&repo_path).output()?;

        // Configure user name and email for commits
        Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.email", "test@example.com"])
            .output()?;
        Command::new("git")
            .current_dir(&repo_path)
            .args(["config", "user.name", "Test User"])
            .output()?;

        Ok((temp_dir, GitRepo::new(repo_path)?))
    }

    // Helper function to create a commit in the test repository
    fn create_commit(git_repo: &GitRepo, file_name: &str, content: &str) -> Result<String> {
        let file_path = git_repo.path.join(file_name);
        fs::write(&file_path, content)?;
        Command::new("git")
            .current_dir(&git_repo.path)
            .arg("add")
            .arg(&file_path)
            .output()?;
        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["commit", "-m", &format!("Add {}", file_name)])
            .output()?;

        // Get commit hash
        git_repo.get_commit_hash("HEAD")
    }

    // Helper function to create a branch
    fn create_branch(git_repo: &GitRepo, branch_name: &str) -> Result<()> {
        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["checkout", "-b", branch_name])
            .output()?;
        Ok(())
    }

    // Helper function to checkout a branch
    fn checkout_branch(git_repo: &GitRepo, branch_name: &str) -> Result<()> {
        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["checkout", branch_name])
            .output()?;
        Ok(())
    }
}
