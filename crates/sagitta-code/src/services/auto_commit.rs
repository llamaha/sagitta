use anyhow::{Result, Context};
use git2::{Repository, StatusOptions, Status};
use log::{debug, error, info, warn};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use tokio::sync::{mpsc, RwLock};
use std::sync::Arc;

use crate::config::types::AutoCommitConfig;
use crate::services::commit_generator::CommitMessageGenerator;
use crate::services::file_watcher::FileChangeEvent;

/// Represents a commit operation result
#[derive(Debug, Clone)]
pub struct CommitResult {
    /// Repository path where the commit was made
    pub repo_path: PathBuf,
    /// Commit hash
    pub commit_hash: String,
    /// Commit message
    pub commit_message: String,
    /// Number of files changed
    pub files_changed: usize,
    /// Lines added
    pub lines_added: usize,
    /// Lines deleted
    pub lines_deleted: usize,
    /// Timestamp when commit was made
    pub timestamp: Instant,
}

/// Tracks repository state for auto-commit decisions
#[derive(Debug, Clone)]
pub struct RepositoryState {
    /// Last commit hash we're aware of
    last_known_commit: Option<String>,
    /// Last time we made an auto-commit
    last_auto_commit: Option<Instant>,
    /// Pending changes that haven't been committed yet
    pending_changes: Vec<FileChangeEvent>,
    /// Whether this repository currently has uncommitted changes
    has_uncommitted_changes: bool,
}

/// Service that automatically commits changes to git repositories
pub struct AutoCommitter {
    config: AutoCommitConfig,
    commit_generator: CommitMessageGenerator,
    /// Track repository states for commit decisions
    repository_states: Arc<RwLock<HashMap<PathBuf, RepositoryState>>>,
    /// Channel for receiving commit results
    commit_tx: mpsc::UnboundedSender<CommitResult>,
    commit_rx: Option<mpsc::UnboundedReceiver<CommitResult>>,
}

impl AutoCommitter {
    /// Create a new auto-committer
    pub fn new(config: AutoCommitConfig, commit_generator: CommitMessageGenerator) -> Self {
        let (commit_tx, commit_rx) = mpsc::unbounded_channel();

        Self {
            config,
            commit_generator,
            repository_states: Arc::new(RwLock::new(HashMap::new())),
            commit_tx,
            commit_rx: Some(commit_rx),
        }
    }

    /// Start the auto-committer and return a receiver for commit results
    pub fn start(&mut self) -> mpsc::UnboundedReceiver<CommitResult> {
        self.commit_rx.take().expect("Auto-committer already started")
    }

    /// Process file change events and potentially trigger commits
    pub async fn handle_file_changes(&self, mut change_rx: mpsc::UnboundedReceiver<FileChangeEvent>) {
        info!("Starting auto-committer file change handler");

        while let Some(change_event) = change_rx.recv().await {
            if !self.config.enabled {
                continue;
            }

            if let Err(e) = self.process_file_change(change_event).await {
                error!("Error processing file change: {}", e);
            }
        }

        info!("Auto-committer file change handler stopped");
    }

    /// Process a single file change event
    async fn process_file_change(&self, change_event: FileChangeEvent) -> Result<()> {
        let repo_path = change_event.repo_path.clone(); // Clone to avoid borrow issues
        
        debug!(
            "Processing file change in {}: {} {:?}",
            repo_path.display(),
            change_event.file_path.display(),
            change_event.change_type
        );

        // Update repository state
        {
            let mut states = self.repository_states.write().await;
            let state = states.entry(repo_path.clone()).or_insert_with(|| RepositoryState {
                last_known_commit: None,
                last_auto_commit: None,
                pending_changes: Vec::new(),
                has_uncommitted_changes: false,
            });

            state.pending_changes.push(change_event);
            state.has_uncommitted_changes = true;
        }

        // Check if we should auto-commit this repository
        if self.should_auto_commit(&repo_path).await? {
            self.perform_auto_commit(&repo_path).await?;
        }

        Ok(())
    }

    /// Determine if we should auto-commit for this repository
    async fn should_auto_commit(&self, repo_path: &Path) -> Result<bool> {
        if !self.config.enabled {
            return Ok(false);
        }

        let states = self.repository_states.read().await;
        let state = match states.get(repo_path) {
            Some(state) => state,
            None => return Ok(false),
        };

        // Check cooldown period
        if let Some(last_commit) = state.last_auto_commit {
            let cooldown = Duration::from_secs(self.config.cooldown_seconds);
            if last_commit.elapsed() < cooldown {
                debug!(
                    "Auto-commit cooldown active for {}, {} seconds remaining",
                    repo_path.display(),
                    (cooldown - last_commit.elapsed()).as_secs()
                );
                return Ok(false);
            }
        }

        // Check if there are actually uncommitted changes
        if !state.has_uncommitted_changes {
            return Ok(false);
        }

        // Verify with git status
        self.has_staged_or_modified_files(repo_path)
    }

    /// Check if repository has staged or modified files
    fn has_staged_or_modified_files(&self, repo_path: &Path) -> Result<bool> {
        let repo = Repository::open(repo_path)
            .context("Failed to open git repository")?;

        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        status_options.include_ignored(false);

        let statuses = repo.statuses(Some(&mut status_options))
            .context("Failed to get repository status")?;

        for entry in statuses.iter() {
            let status = entry.status();
            
            // Check for changes that should be committed
            if status.intersects(
                Status::WT_MODIFIED |
                Status::WT_NEW |
                Status::WT_DELETED |
                Status::INDEX_MODIFIED |
                Status::INDEX_NEW |
                Status::INDEX_DELETED
            ) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Perform an auto-commit for the given repository
    async fn perform_auto_commit(&self, repo_path: &Path) -> Result<()> {
        info!("Performing auto-commit for repository: {}", repo_path.display());

        // Get git diff for commit message generation
        let diff_output = self.get_git_diff(repo_path)?;
        let file_stats = self.get_file_stats(repo_path)?;

        // Generate commit message
        let commit_message = if !diff_output.trim().is_empty() {
            self.commit_generator
                .generate_commit_message(repo_path, &diff_output)
                .await
                .unwrap_or_else(|e| {
                    warn!("Failed to generate AI commit message: {}. Using fallback.", e);
                    self.commit_generator.generate_fallback_message(
                        file_stats.files_changed,
                        file_stats.lines_added,
                        file_stats.lines_deleted,
                    )
                })
        } else {
            self.commit_generator.generate_fallback_message(
                file_stats.files_changed,
                file_stats.lines_added,
                file_stats.lines_deleted,
            )
        };

        // Perform the git commit
        let commit_hash = self.execute_git_commit(repo_path, &commit_message)?;

        // Update repository state
        {
            let mut states = self.repository_states.write().await;
            if let Some(state) = states.get_mut(repo_path) {
                state.last_known_commit = Some(commit_hash.clone());
                state.last_auto_commit = Some(Instant::now());
                state.pending_changes.clear();
                state.has_uncommitted_changes = false;
            }
        }

        // Send commit result
        let commit_result = CommitResult {
            repo_path: repo_path.to_path_buf(),
            commit_hash: commit_hash.clone(),
            commit_message: commit_message.clone(),
            files_changed: file_stats.files_changed,
            lines_added: file_stats.lines_added,
            lines_deleted: file_stats.lines_deleted,
            timestamp: Instant::now(),
        };

        if let Err(e) = self.commit_tx.send(commit_result) {
            error!("Failed to send commit result: {}", e);
        }

        info!(
            "Auto-commit completed for {}: {} ({})",
            repo_path.display(),
            commit_message.lines().next().unwrap_or(""),
            &commit_hash[..8]
        );

        Ok(())
    }

    /// Get git diff output for commit message generation
    fn get_git_diff(&self, repo_path: &Path) -> Result<String> {
        let repo = Repository::open(repo_path)?;
        
        let mut diff_opts = git2::DiffOptions::new();
        diff_opts.context_lines(3);
        diff_opts.interhunk_lines(1);

        // Get diff between working directory and index
        let diff = repo.diff_index_to_workdir(None, Some(&mut diff_opts))?;
        
        let mut diff_output = String::new();
        diff.print(git2::DiffFormat::Patch, |_delta, _hunk, line| {
            match line.origin() {
                '+' | '-' | ' ' => {
                    diff_output.push(line.origin());
                    diff_output.push_str(std::str::from_utf8(line.content()).unwrap_or(""));
                }
                _ => {}
            }
            true
        })?;

        Ok(diff_output)
    }

    /// Get file statistics for commit
    fn get_file_stats(&self, repo_path: &Path) -> Result<FileStats> {
        let repo = Repository::open(repo_path)?;
        
        let mut status_options = StatusOptions::new();
        status_options.include_untracked(true);
        
        let statuses = repo.statuses(Some(&mut status_options))?;
        
        let mut files_changed = 0;
        for entry in statuses.iter() {
            let status = entry.status();
            if status.intersects(
                Status::WT_MODIFIED | 
                Status::WT_NEW | 
                Status::WT_DELETED |
                Status::INDEX_MODIFIED |
                Status::INDEX_NEW |
                Status::INDEX_DELETED
            ) {
                files_changed += 1;
            }
        }

        // For simplicity, use basic line counting
        // In a real implementation, you might want more sophisticated diff parsing
        Ok(FileStats {
            files_changed,
            lines_added: 0, // Could be calculated from diff
            lines_deleted: 0, // Could be calculated from diff
        })
    }

    /// Execute the actual git commit
    fn execute_git_commit(&self, repo_path: &Path, commit_message: &str) -> Result<String> {
        let repo = Repository::open(repo_path)?;
        
        // Add all changes to index
        let mut index = repo.index()?;
        index.add_all(&["*"], git2::IndexAddOption::DEFAULT, None)?;
        index.write()?;

        // Create commit
        let signature = repo.signature()?;
        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        
        let parent_commit = match repo.head() {
            Ok(head) => Some(head.peel_to_commit()?),
            Err(_) => None, // Initial commit
        };

        let parents: Vec<&git2::Commit> = parent_commit.iter().collect();
        
        let commit_id = repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            commit_message,
            &tree,
            &parents,
        )?;

        Ok(commit_id.to_string())
    }

    /// Force a commit for a repository (bypassing cooldown and change detection)
    pub async fn force_commit(&self, repo_path: &Path, custom_message: Option<String>) -> Result<CommitResult> {
        info!("Forcing commit for repository: {}", repo_path.display());

        if !self.has_staged_or_modified_files(repo_path)? {
            return Err(anyhow::anyhow!("No changes to commit in repository"));
        }

        let diff_output = self.get_git_diff(repo_path)?;
        let file_stats = self.get_file_stats(repo_path)?;

        let commit_message = match custom_message {
            Some(msg) => msg,
            None => {
                if !diff_output.trim().is_empty() {
                    self.commit_generator
                        .generate_commit_message(repo_path, &diff_output)
                        .await?
                } else {
                    self.commit_generator.generate_fallback_message(
                        file_stats.files_changed,
                        file_stats.lines_added,
                        file_stats.lines_deleted,
                    )
                }
            }
        };

        let commit_hash = self.execute_git_commit(repo_path, &commit_message)?;

        // Update state
        {
            let mut states = self.repository_states.write().await;
            let state = states.entry(repo_path.to_path_buf()).or_insert_with(|| RepositoryState {
                last_known_commit: None,
                last_auto_commit: None,
                pending_changes: Vec::new(),
                has_uncommitted_changes: false,
            });
            
            state.last_known_commit = Some(commit_hash.clone());
            state.last_auto_commit = Some(Instant::now());
            state.pending_changes.clear();
            state.has_uncommitted_changes = false;
        }

        let commit_result = CommitResult {
            repo_path: repo_path.to_path_buf(),
            commit_hash: commit_hash.clone(),
            commit_message: commit_message.clone(),
            files_changed: file_stats.files_changed,
            lines_added: file_stats.lines_added,
            lines_deleted: file_stats.lines_deleted,
            timestamp: Instant::now(),
        };

        info!(
            "Forced commit completed for {}: {} ({})",
            repo_path.display(),
            commit_message.lines().next().unwrap_or(""),
            &commit_hash[..8]
        );

        Ok(commit_result)
    }

    /// Update configuration
    pub fn update_config(&mut self, config: AutoCommitConfig) {
        self.commit_generator.update_config(config.clone());
        self.config = config;
    }

    /// Get current configuration
    pub fn config(&self) -> &AutoCommitConfig {
        &self.config
    }

    /// Get repository states (for debugging/monitoring)
    pub async fn get_repository_states(&self) -> HashMap<PathBuf, RepositoryState> {
        self.repository_states.read().await.clone()
    }
}

#[derive(Debug, Clone)]
struct FileStats {
    files_changed: usize,
    lines_added: usize,
    lines_deleted: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_auto_committer_creation() {
        let config = AutoCommitConfig::default();
        let generator = CommitMessageGenerator::new(
            crate::llm::fast_model::FastModelProvider::new(Default::default()),
            config.clone()
        );
        
        let mut committer = AutoCommitter::new(config, generator);
        let _rx = committer.start();
        
        assert!(committer.config.enabled);
    }

    #[test]
    fn test_has_staged_or_modified_files_no_repo() {
        let temp_dir = TempDir::new().unwrap();
        let config = AutoCommitConfig::default();
        let generator = CommitMessageGenerator::new(
            crate::llm::fast_model::FastModelProvider::new(Default::default()),
            config.clone()
        );
        
        let committer = AutoCommitter::new(config, generator);
        let result = committer.has_staged_or_modified_files(temp_dir.path());
        
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_should_auto_commit_disabled() {
        let mut config = AutoCommitConfig::default();
        config.enabled = false;
        
        let generator = CommitMessageGenerator::new(
            crate::llm::fast_model::FastModelProvider::new(Default::default()),
            config.clone()
        );
        
        let committer = AutoCommitter::new(config, generator);
        let temp_dir = TempDir::new().unwrap();
        
        let result = committer.should_auto_commit(temp_dir.path()).await.unwrap();
        assert!(!result);
    }
}