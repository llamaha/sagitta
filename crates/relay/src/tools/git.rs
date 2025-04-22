use crate::chain::action::Action;
use crate::chain::state::ChainState;
use crate::utils::error::{RelayError, Result};
use async_trait::async_trait;
use git2::{Repository, StatusOptions, Signature, IndexAddOption, Oid};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, warn, info};
use crate::context::AppContext;

// --- Git Status Action ---

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct GitStatusParams {
    pub repo_path: Option<String>, // Made public
}

#[derive(Debug)]
pub struct GitStatusAction {
    params: GitStatusParams,
}

impl GitStatusAction {
    pub fn new(repo_path: Option<String>) -> Self {
        Self { params: GitStatusParams { repo_path } }
    }
}

#[async_trait]
impl Action for GitStatusAction {
    fn name(&self) -> &'static str {
        "git_status"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        // Determine the repository path
        let repo_path_str = self.params.repo_path.as_deref()
            .or(state.current_directory.as_deref())
            .ok_or_else(|| RelayError::ToolError("Repository path not specified and not found in state".to_string()))?;

        debug!(repo_path = %repo_path_str, "Executing GitStatusAction");

        let repo = Repository::open(Path::new(repo_path_str))
            .map_err(|e| RelayError::ToolError(format!("Failed to open repository at '{}': {}", repo_path_str, e)))?;

        let mut opts = StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);

        let statuses = repo.statuses(Some(&mut opts))
            .map_err(|e| RelayError::ToolError(format!("Failed to get repository status: {}", e)))?;

        let mut status_summary = String::new();
        if statuses.is_empty() {
            status_summary.push_str("Working tree clean.");
        } else {
            for entry in statuses.iter() {
                let path = entry.path().unwrap_or("[invalid path]");
                let status = entry.status();
                // Simple summary, could be more detailed
                status_summary.push_str(&format!("- {}: {:?}\n", path, status));
            }
        }

        // Store the status summary in the context
        state.set_context("git_status_summary".to_string(), status_summary)
             .map_err(|e| RelayError::ToolError(format!("Failed to set context for git status: {}", e)))?;

        Ok(())
    }
}

// --- Git Add Action ---
// Needed before commit

#[derive(Debug, Serialize, Deserialize)]
pub struct GitAddParams {
    pub repo_path: Option<String>, // Made public
    pub paths: Vec<String>, // Made public
}

#[derive(Debug)]
pub struct GitAddAction {
    params: GitAddParams,
}

impl GitAddAction {
    pub fn new(paths: Vec<String>, repo_path: Option<String>) -> Self {
        Self { params: GitAddParams { repo_path, paths } }
    }
}

#[async_trait]
impl Action for GitAddAction {
    fn name(&self) -> &'static str {
        "git_add"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let repo_path_str = self.params.repo_path.as_deref()
            .or(state.current_directory.as_deref())
            .ok_or_else(|| RelayError::ToolError("Repository path not specified and not found in state for git add".to_string()))?;
        
        debug!(repo_path = %repo_path_str, paths = ?self.params.paths, "Executing GitAddAction");

        let repo = Repository::open(Path::new(repo_path_str))
            .map_err(|e| RelayError::ToolError(format!("Failed to open repository at '{}': {}", repo_path_str, e)))?;
        
        let mut index = repo.index()
             .map_err(|e| RelayError::ToolError(format!("Failed to get repository index: {}", e)))?;
        
        // Add specified paths
        index.add_all(self.params.paths.iter(), IndexAddOption::DEFAULT, None)
             .map_err(|e| RelayError::ToolError(format!("Failed to add paths to index: {}", e)))?;

        // Write the index changes to disk
        index.write()
             .map_err(|e| RelayError::ToolError(format!("Failed to write index changes: {}", e)))?;
             
        info!(repo_path = %repo_path_str, paths = ?self.params.paths, "Files added to git index.");
        state.set_context(format!("git_add_result_{:?}", self.params.paths), "Success".to_string())
             .map_err(|e| RelayError::ToolError(format!("Failed to set context for git add: {}", e)))?;

        Ok(())
    }
}

// --- Git Commit Action ---

#[derive(Debug, Serialize, Deserialize)]
pub struct GitCommitParams {
    pub repo_path: Option<String>, // Made public
    pub message: String, // Made public
    pub author_name: Option<String>, // Made public
    pub author_email: Option<String>, // Made public
}

#[derive(Debug)]
pub struct GitCommitAction {
    params: GitCommitParams,
}

impl GitCommitAction {
    pub fn new(message: String, repo_path: Option<String>, author_name: Option<String>, author_email: Option<String>) -> Self {
        Self { params: GitCommitParams { repo_path, message, author_name, author_email } }
    }
}

// Helper to get the last commit Oid
fn find_last_commit(repo: &Repository) -> std::result::Result<Option<Oid>, git2::Error> {
    match repo.head() {
        Ok(head) => head.target().map(|oid| Ok(Some(oid))).unwrap_or_else(|| Ok(None)),
        Err(ref e) if e.code() == git2::ErrorCode::UnbornBranch => Ok(None), // No commits yet
        Err(e) => Err(e),
    }
}

#[async_trait]
impl Action for GitCommitAction {
    fn name(&self) -> &'static str {
        "git_commit"
    }

    async fn execute(&self, _context: &AppContext, state: &mut ChainState) -> Result<()> {
        let repo_path_str = self.params.repo_path.as_deref()
            .or(state.current_directory.as_deref())
            .ok_or_else(|| RelayError::ToolError("Repository path not specified and not found in state for git commit".to_string()))?;
        
        debug!(repo_path = %repo_path_str, message = %self.params.message, "Executing GitCommitAction");

        let repo = Repository::open(Path::new(repo_path_str))
            .map_err(|e| RelayError::ToolError(format!("Failed to open repository at '{}': {}", repo_path_str, e)))?;
        
        // Get signature - use provided params or fallback to git config
        let signature = match (&self.params.author_name, &self.params.author_email) {
            (Some(name), Some(email)) => Signature::now(name, email),
            _ => repo.signature(), // Fallback to repository config or defaults
        }.map_err(|e| RelayError::ToolError(format!("Failed to create or get git signature: {}", e)))?;
        
        // Get the index tree
        let mut index = repo.index()
            .map_err(|e| RelayError::ToolError(format!("Failed to get repository index: {}", e)))?;
        let oid = index.write_tree()
            .map_err(|e| RelayError::ToolError(format!("Failed to write index tree: {}", e)))?;
        let tree = repo.find_tree(oid)
            .map_err(|e| RelayError::ToolError(format!("Failed to find written tree: {}", e)))?;
            
        // Find parent commit(s)
        let parents = match find_last_commit(&repo) {
            Ok(Some(parent_oid)) => {
                match repo.find_commit(parent_oid) {
                    Ok(commit) => vec![commit],
                    Err(e) => return Err(RelayError::ToolError(format!("Failed to find parent commit '{}': {}", parent_oid, e))),
                }
            }
            Ok(None) => vec![], // No parents for the first commit
            Err(e) => return Err(RelayError::ToolError(format!("Failed to find last commit: {}", e))),
        };
        let parent_refs: Vec<&git2::Commit> = parents.iter().collect();

        // Create the commit
        let commit_oid = repo.commit(
            Some("HEAD"),         // Update the HEAD reference
            &signature,           // Author
            &signature,           // Committer
            &self.params.message, // Commit message
            &tree,                // Tree 
            &parent_refs,         // Parent commits
        ).map_err(|e| RelayError::ToolError(format!("Failed to create commit: {}", e)))?;

        info!(repo_path = %repo_path_str, commit_oid = %commit_oid, "Commit created successfully.");
        state.set_context(format!("git_commit_oid"), commit_oid.to_string())
             .map_err(|e| RelayError::ToolError(format!("Failed to set context for git commit: {}", e)))?;

        Ok(())
    }
}

// TODO: Add GitLogAction, GitBranchAction, etc. 