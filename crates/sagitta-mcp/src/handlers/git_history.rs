use crate::mcp::{
    error_codes,
    types::{
        ErrorObject, RepositoryGitHistoryParams, RepositoryGitHistoryResult, GitCommit,
    },
};
use anyhow::Result;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument};
use sagitta_search::{
    config::{AppConfig, get_repo_base_path},
    qdrant_client_trait::QdrantClientTrait,
};
use git2::{Repository, Oid, Time};
use chrono::{DateTime, Utc, TimeZone};
use crate::middleware::auth_middleware::AuthenticatedUser;
use axum::Extension;

#[instrument(skip(config, _qdrant_client), fields(repo_name = ?params.repository_name))]
pub async fn handle_repository_git_history<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryGitHistoryParams,
    config: Arc<RwLock<AppConfig>>,
    _qdrant_client: Arc<C>,
    _auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositoryGitHistoryResult, ErrorObject> {
    info!("Handling repository/git_history request");

    let config_guard = config.read().await;
    
    // Find the repository in config
    let repo_config = config_guard
        .repositories
        .iter()
        .find(|r| r.name == params.repository_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::INVALID_PARAMS,
            message: format!("Repository '{}' not found", params.repository_name),
            data: None,
        })?;

    let repo_base_path = get_repo_base_path(Some(&*config_guard)).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to determine repository base path: {e}"),
        data: None,
    })?;

    let repo_path = repo_base_path.join(&repo_config.name);
    
    // Drop the config guard before potentially long git operations
    drop(config_guard);

    // Open the git repository
    let repo = Repository::open(&repo_path).map_err(|e| {
        error!(error = %e, "Failed to open repository at {:?}", repo_path);
        ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to open repository: {e}"),
            data: None,
        }
    })?;

    // Get current branch
    let current_branch = get_current_branch(&repo)?;

    // Create a revwalk
    let mut revwalk = repo.revwalk().map_err(|e| {
        error!(error = %e, "Failed to create revwalk");
        ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to initialize git history walk: {e}"),
            data: None,
        }
    })?;

    // Configure the revwalk based on parameters
    if let Some(branch_name) = &params.branch_name {
        // Start from specific branch
        let branch_ref = if branch_name.contains('/') {
            // Assume it's already a full reference like "origin/main"
            format!("refs/remotes/{branch_name}")
        } else {
            // Try local branch first
            format!("refs/heads/{branch_name}")
        };
        
        match repo.revparse_single(&branch_ref) {
            Ok(obj) => {
                revwalk.push(obj.id()).map_err(|e| ErrorObject {
                    code: error_codes::INTERNAL_ERROR,
                    message: format!("Failed to start from branch {branch_name}: {e}"),
                    data: None,
                })?;
            }
            Err(_) => {
                // Try as a remote branch
                let remote_ref = format!("refs/remotes/origin/{branch_name}");
                match repo.revparse_single(&remote_ref) {
                    Ok(obj) => {
                        revwalk.push(obj.id()).map_err(|e| ErrorObject {
                            code: error_codes::INTERNAL_ERROR,
                            message: format!("Failed to start from branch {branch_name}: {e}"),
                            data: None,
                        })?;
                    }
                    Err(_) => {
                        return Err(ErrorObject {
                            code: error_codes::INVALID_PARAMS,
                            message: format!("Branch '{branch_name}' not found"),
                            data: None,
                        });
                    }
                }
            }
        }
    } else {
        // Start from HEAD
        revwalk.push_head().map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to start from HEAD: {e}"),
            data: None,
        })?;
    }

    // Set sorting to time order (newest first)
    revwalk.set_sorting(git2::Sort::TIME).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to set sorting: {e}"),
        data: None,
    })?;

    // Apply path filter if specified
    if let Some(path) = &params.path {
        // Note: git2 doesn't directly support path filtering in revwalk,
        // so we'll need to check each commit manually
        info!("Path filtering requested for: {}", path);
    }

    // Limit commits
    let max_commits = params.max_commits.min(1000) as usize;
    let mut commits = Vec::new();
    let mut total_walked = 0;

    // Parse date filters if provided
    let since_time = params.since.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp())
    });
    let until_time = params.until.as_ref().and_then(|s| {
        DateTime::parse_from_rfc3339(s).ok().map(|dt| dt.timestamp())
    });

    for oid_result in revwalk {
        if commits.len() >= max_commits {
            break;
        }
        total_walked += 1;

        let oid = oid_result.map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to walk commit history: {e}"),
            data: None,
        })?;

        let commit = repo.find_commit(oid).map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to find commit: {e}"),
            data: None,
        })?;

        // Apply time filters
        let commit_time = commit.time().seconds();
        if let Some(since) = since_time {
            if commit_time < since {
                continue;
            }
        }
        if let Some(until) = until_time {
            if commit_time > until {
                continue;
            }
        }

        // Apply author filter
        if let Some(author_filter) = &params.author {
            let author = commit.author();
            let author_name = author.name().unwrap_or("");
            let author_email = author.email().unwrap_or("");
            
            if !author_name.contains(author_filter) && !author_email.contains(author_filter) {
                continue;
            }
        }

        // Apply path filter by checking if commit touches the specified path
        if let Some(path_filter) = &params.path {
            if !commit_touches_path(&repo, &commit, path_filter)? {
                continue;
            }
        }

        // Convert commit to our format
        let git_commit = convert_commit(&repo, &commit)?;
        commits.push(git_commit);
    }

    let truncated = total_walked > max_commits;
    let total_commits = commits.len() as u64;

    Ok(RepositoryGitHistoryResult {
        commits,
        current_branch,
        total_commits,
        truncated,
    })
}

fn get_current_branch(repo: &Repository) -> Result<String, ErrorObject> {
    match repo.head() {
        Ok(head) => {
            if head.is_branch() {
                // We're on a branch
                if let Some(branch_name) = head.shorthand() {
                    Ok(branch_name.to_string())
                } else {
                    // Shouldn't happen for a branch
                    let oid = head.target().ok_or_else(|| ErrorObject {
                        code: error_codes::INTERNAL_ERROR,
                        message: "HEAD has no target".to_string(),
                        data: None,
                    })?;
                    Ok(format!("detached-{}", &oid.to_string()[..7]))
                }
            } else {
                // Detached HEAD
                let oid = head.target().ok_or_else(|| ErrorObject {
                    code: error_codes::INTERNAL_ERROR,
                    message: "HEAD has no target".to_string(),
                    data: None,
                })?;
                Ok(format!("detached-{}", &oid.to_string()[..7]))
            }
        }
        Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
            // Repository has no commits yet
            Ok("main".to_string())
        }
        Err(e) => Err(ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to get current branch: {e}"),
            data: None,
        }),
    }
}

fn convert_commit(repo: &Repository, commit: &git2::Commit) -> Result<GitCommit, ErrorObject> {
    let id = commit.id().to_string();
    let short_id = id.chars().take(7).collect();
    let message = commit.message().unwrap_or("<no message>").to_string();
    let author = commit.author();
    let author_name = author.name().unwrap_or("<unknown>").to_string();
    let email = author.email().unwrap_or("<unknown>").to_string();
    
    // Convert git2::Time to RFC3339 timestamp
    let timestamp = format_git_time(&commit.time());
    
    let parents = commit.parent_ids()
        .map(|oid| oid.to_string())
        .collect();
    
    // Get branch/tag refs pointing to this commit
    let refs = get_refs_for_commit(repo, commit.id())?;
    
    Ok(GitCommit {
        id,
        short_id,
        message,
        author: author_name,
        email,
        timestamp,
        parents,
        refs,
    })
}

fn format_git_time(time: &Time) -> String {
    match Utc.timestamp_opt(time.seconds(), 0).single() {
        Some(dt) => dt.to_rfc3339(),
        None => {
            // Fallback to current time if timestamp is invalid
            Utc::now().to_rfc3339()
        }
    }
}

fn get_refs_for_commit(repo: &Repository, oid: Oid) -> Result<Vec<String>, ErrorObject> {
    let mut refs = Vec::new();
    
    // Check all references
    match repo.references() {
        Ok(references) => {
            for reference in references.flatten() {
                if let Some(target) = reference.target() {
                    if target == oid {
                        if let Some(name) = reference.shorthand() {
                            refs.push(name.to_string());
                        }
                    }
                }
            }
        }
        Err(e) => {
            // Log but don't fail - refs are optional information
            info!("Failed to get references: {}", e);
        }
    }
    
    Ok(refs)
}

fn commit_touches_path(repo: &Repository, commit: &git2::Commit, path: &str) -> Result<bool, ErrorObject> {
    // For the first commit (no parents), check if the path exists in the tree
    if commit.parent_count() == 0 {
        let tree = commit.tree().map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to get commit tree: {e}"),
            data: None,
        })?;
        
        // Check if path exists in tree
        return Ok(tree.get_path(std::path::Path::new(path)).is_ok());
    }
    
    // For commits with parents, check diff
    for parent_id in commit.parent_ids() {
        let parent = repo.find_commit(parent_id).map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to find parent commit: {e}"),
            data: None,
        })?;
        
        let parent_tree = parent.tree().map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to get parent tree: {e}"),
            data: None,
        })?;
        
        let commit_tree = commit.tree().map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to get commit tree: {e}"),
            data: None,
        })?;
        
        let diff = repo.diff_tree_to_tree(Some(&parent_tree), Some(&commit_tree), None)
            .map_err(|e| ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: format!("Failed to create diff: {e}"),
                data: None,
            })?;
        
        // Check if any delta involves our path
        let mut touches_path = false;
        diff.foreach(
            &mut |delta, _| {
                if let Some(old_file) = delta.old_file().path() {
                    if old_file.to_string_lossy().contains(path) {
                        touches_path = true;
                        return false; // Stop iteration
                    }
                }
                if let Some(new_file) = delta.new_file().path() {
                    if new_file.to_string_lossy().contains(path) {
                        touches_path = true;
                        return false; // Stop iteration
                    }
                }
                true // Continue iteration
            },
            None,
            None,
            None,
        ).map_err(|e| ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to iterate diff: {e}"),
            data: None,
        })?;
        
        if touches_path {
            return Ok(true);
        }
    }
    
    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_params() {
        let params = RepositoryGitHistoryParams {
            repository_name: "test-repo".to_string(),
            ..Default::default()
        };
        
        assert_eq!(params.max_commits, 100);
        assert!(params.branch_name.is_none());
        assert!(params.since.is_none());
        assert!(params.until.is_none());
        assert!(params.author.is_none());
        assert!(params.path.is_none());
    }
}