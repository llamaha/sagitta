use anyhow::{anyhow, Result};
use chrono::{DateTime, TimeZone, Utc};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Represents a git repository with utility functions
pub struct GitRepo {
    pub path: PathBuf,
}

/// Represents a git commit
#[derive(Debug, Clone)]
pub struct GitCommit {
    pub hash: String,
    pub author: String,
    pub date: DateTime<Utc>,
    pub message: String,
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

    /// Get the current branch name
    pub fn get_current_branch(&self) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["branch", "--show-current"])
            .output()?;

        if output.status.success() {
            let branch = String::from_utf8(output.stdout)?.trim().to_string();

            // If empty (detached HEAD), try to get from HEAD ref
            if branch.is_empty() {
                let head_output = Command::new("git")
                    .current_dir(&self.path)
                    .args(["rev-parse", "--abbrev-ref", "HEAD"])
                    .output()?;

                if head_output.status.success() {
                    let head_ref = String::from_utf8(head_output.stdout)?.trim().to_string();

                    if head_ref != "HEAD" {
                        return Ok(head_ref);
                    }
                }

                // Fall back to "main" if we can't determine
                Ok("main".to_string())
            } else {
                Ok(branch)
            }
        } else {
            Err(anyhow!(
                "Failed to get current branch: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
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

    /// List all branches in the repository
    pub fn list_branches(&self) -> Result<Vec<String>> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["branch", "--format=%(refname:short)"])
            .output()?;

        if output.status.success() {
            let branches = String::from_utf8(output.stdout)?
                .lines()
                .map(|line| line.trim().to_string())
                .filter(|line| !line.is_empty())
                .collect();

            Ok(branches)
        } else {
            Err(anyhow!(
                "Failed to list branches: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// List all remote branches in the repository
    pub fn list_remote_branches(&self) -> Result<Vec<String>> {
        // First, update remote information
        let _ = Command::new("git")
            .current_dir(&self.path)
            .args(["fetch", "--prune"])
            .output()?;

        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["branch", "-r", "--format=%(refname:short)"])
            .output()?;

        if output.status.success() {
            let branches = String::from_utf8(output.stdout)?
                .lines()
                .map(|line| {
                    // Remove remote part (e.g., "origin/") from branch names
                    let parts: Vec<&str> = line.trim().split('/').collect();
                    if parts.len() > 1 {
                        parts[1..].join("/")
                    } else {
                        line.trim().to_string()
                    }
                })
                .filter(|line| !line.is_empty() && line != "HEAD")
                .collect();

            Ok(branches)
        } else {
            Err(anyhow!(
                "Failed to list remote branches: {}",
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

    /// Get detailed changes between two commits
    pub fn get_change_set(&self, from_commit: &str, to_commit: &str) -> Result<ChangeSet> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["diff", "--name-status", from_commit, to_commit])
            .output()?;

        if output.status.success() {
            let mut added_files = Vec::new();
            let mut modified_files = Vec::new();
            let mut deleted_files = Vec::new();

            String::from_utf8(output.stdout)?.lines().for_each(|line| {
                let parts: Vec<&str> = line.trim().split_whitespace().collect();
                if parts.len() >= 2 {
                    let status = parts[0];
                    let file_path = self.path.join(parts[1]);

                    match status.chars().next() {
                        Some('A') => added_files.push(file_path),
                        Some('M') => modified_files.push(file_path),
                        Some('D') => deleted_files.push(file_path),
                        Some('R') => {
                            // Renamed - treat as delete of old + add of new
                            if parts.len() >= 3 {
                                deleted_files.push(self.path.join(parts[1]));
                                added_files.push(self.path.join(parts[2]));
                            }
                        }
                        _ => {} // Ignore other changes
                    }
                }
            });

            Ok(ChangeSet {
                commit_before: from_commit.to_string(),
                commit_after: to_commit.to_string(),
                added_files,
                modified_files,
                deleted_files,
            })
        } else {
            Err(anyhow!(
                "Failed to get change set between {} and {}: {}",
                from_commit,
                to_commit,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Check if a repository needs reindexing by comparing commits
    pub fn needs_reindexing(&self, branch: &str, last_indexed_commit: &str) -> Result<bool> {
        // Get the latest commit hash
        let current_commit = self.get_commit_hash(branch)?;

        // If the commits are the same, no reindexing needed
        if current_commit == last_indexed_commit {
            return Ok(false);
        }

        // Check if the last indexed commit still exists in the history
        let commit_exists = Command::new("git")
            .current_dir(&self.path)
            .args([
                "cat-file",
                "-e",
                &format!("{}^{{commit}}", last_indexed_commit),
            ])
            .status()?
            .success();

        if !commit_exists {
            // Commit doesn't exist or isn't a commit, need full reindex
            return Ok(true);
        }

        // Get changed files count - if there are changes, reindexing is needed
        let changed_files = self.get_changed_files(last_indexed_commit, &current_commit)?;

        Ok(!changed_files.is_empty())
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

    /// Get commit history for a file
    pub fn get_file_history(&self, file_path: &Path) -> Result<Vec<GitCommit>> {
        // Get relative path to the repository
        let rel_path = file_path
            .strip_prefix(&self.path)
            .map_err(|_| anyhow!("File path is not in repository"))?;

        let output = Command::new("git")
            .current_dir(&self.path)
            .args([
                "log",
                "--format=%H|%an|%at|%s",
                "--",
                rel_path.to_str().unwrap(),
            ])
            .output()?;

        if output.status.success() {
            let commits = String::from_utf8(output.stdout)?
                .lines()
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split('|').collect();
                    if parts.len() >= 4 {
                        // Parse timestamp
                        let timestamp = parts[2].parse::<i64>().ok()?;
                        let date = Utc.timestamp_opt(timestamp, 0).unwrap();

                        Some(GitCommit {
                            hash: parts[0].to_string(),
                            author: parts[1].to_string(),
                            date,
                            message: parts[3].to_string(),
                        })
                    } else {
                        None
                    }
                })
                .collect();

            Ok(commits)
        } else {
            Err(anyhow!(
                "Failed to get file history: {}",
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Find the common ancestor commit between two references (branches or commits)
    pub fn find_common_ancestor(&self, ref1: &str, ref2: &str) -> Result<String> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["merge-base", ref1, ref2])
            .output()?;

        if output.status.success() {
            Ok(String::from_utf8(output.stdout)?.trim().to_string())
        } else {
            Err(anyhow!(
                "Failed to find common ancestor between {} and {}: {}",
                ref1,
                ref2,
                String::from_utf8_lossy(&output.stderr)
            ))
        }
    }

    /// Check if a commit is an ancestor of another commit
    pub fn is_ancestor_of(&self, potential_ancestor: &str, commit: &str) -> Result<bool> {
        let output = Command::new("git")
            .current_dir(&self.path)
            .args(["merge-base", "--is-ancestor", potential_ancestor, commit])
            .status()?;

        // Returns success (0) if potential_ancestor is an ancestor of commit
        Ok(output.success())
    }

    /// Get files changed between two commits or branches, even if they're on different branches
    pub fn get_cross_branch_changes(&self, from_ref: &str, to_ref: &str) -> Result<ChangeSet> {
        // First try to find a common ancestor
        let common_ancestor = self.find_common_ancestor(from_ref, to_ref)?;

        // If the common ancestor is the same as from_ref, we can do a direct diff
        if common_ancestor == from_ref {
            return self.get_change_set(from_ref, to_ref);
        }

        // Otherwise, we need to get changes since the common ancestor
        self.get_change_set(&common_ancestor, to_ref)
    }
}

/// Represents a set of changes between two commits
#[derive(Debug, Clone)]
pub struct ChangeSet {
    pub commit_before: String,
    pub commit_after: String,
    pub added_files: Vec<PathBuf>,
    pub modified_files: Vec<PathBuf>,
    pub deleted_files: Vec<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::tempdir;

    fn create_test_repo() -> Result<(tempfile::TempDir, GitRepo)> {
        let temp_dir = tempdir()?;
        let path = temp_dir.path();

        // Initialize git repo
        Command::new("git")
            .current_dir(path)
            .args(["init"])
            .output()?;

        // Configure git
        Command::new("git")
            .current_dir(path)
            .args(["config", "user.name", "Test User"])
            .output()?;

        Command::new("git")
            .current_dir(path)
            .args(["config", "user.email", "test@example.com"])
            .output()?;

        // Create the git repo object
        let git_repo = GitRepo::new(path.to_path_buf())?;

        Ok((temp_dir, git_repo))
    }

    fn create_commit(git_repo: &GitRepo, file_name: &str, content: &str) -> Result<String> {
        let file_path = git_repo.path.join(file_name);
        std::fs::write(&file_path, content)?;

        // Add and commit
        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["add", file_name])
            .output()?;

        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["commit", "-m", &format!("Add {}", file_name)])
            .output()?;

        // Get the commit hash
        git_repo.get_commit_hash("HEAD")
    }

    fn create_branch(git_repo: &GitRepo, branch_name: &str) -> Result<()> {
        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["checkout", "-b", branch_name])
            .output()?;

        Ok(())
    }

    fn checkout_branch(git_repo: &GitRepo, branch_name: &str) -> Result<()> {
        Command::new("git")
            .current_dir(&git_repo.path)
            .args(["checkout", branch_name])
            .output()?;

        Ok(())
    }

    #[test]
    fn test_find_common_ancestor() -> Result<()> {
        let (temp_dir, git_repo) = create_test_repo()?;

        // Create initial commit on main
        let main_commit = create_commit(&git_repo, "main.txt", "main content")?;

        // Create a feature branch
        create_branch(&git_repo, "feature")?;

        // Add a commit on feature
        let feature_commit = create_commit(&git_repo, "feature.txt", "feature content")?;

        // Back to main
        checkout_branch(&git_repo, "main")?;

        // Add another commit on main
        let main_commit2 = create_commit(&git_repo, "main2.txt", "main content 2")?;

        // Find common ancestor
        let ancestor = git_repo.find_common_ancestor("main", "feature")?;

        // The common ancestor should be the first main commit
        assert_eq!(ancestor, main_commit);

        Ok(())
    }

    #[test]
    fn test_is_ancestor_of() -> Result<()> {
        let (temp_dir, git_repo) = create_test_repo()?;

        // Create initial commit on main
        let main_commit = create_commit(&git_repo, "main.txt", "main content")?;

        // Create a feature branch
        create_branch(&git_repo, "feature")?;

        // Add a commit on feature
        let feature_commit = create_commit(&git_repo, "feature.txt", "feature content")?;

        // Check relationships
        assert!(git_repo.is_ancestor_of(&main_commit, &feature_commit)?);
        assert!(!git_repo.is_ancestor_of(&feature_commit, &main_commit)?);

        Ok(())
    }

    #[test]
    fn test_get_cross_branch_changes() -> Result<()> {
        let (temp_dir, git_repo) = create_test_repo()?;

        // Create initial commit on main
        let main_commit = create_commit(&git_repo, "common.txt", "common content")?;

        // Create a feature branch
        create_branch(&git_repo, "feature")?;

        // Add commits on feature
        create_commit(&git_repo, "feature1.txt", "feature content 1")?;
        let feature_commit = create_commit(&git_repo, "feature2.txt", "feature content 2")?;

        // Back to main
        checkout_branch(&git_repo, "main")?;

        // Add commits on main
        create_commit(&git_repo, "main1.txt", "main content 1")?;
        let main_commit2 = create_commit(&git_repo, "main2.txt", "main content 2")?;

        // Get cross-branch changes from feature to main
        let changes = git_repo.get_cross_branch_changes("feature", "main")?;

        // Should include main1.txt and main2.txt as added files
        assert_eq!(changes.added_files.len(), 2);

        // Get cross-branch changes from main to feature
        let changes = git_repo.get_cross_branch_changes("main", "feature")?;

        // Should include feature1.txt and feature2.txt as added files
        assert_eq!(changes.added_files.len(), 2);

        Ok(())
    }
}
