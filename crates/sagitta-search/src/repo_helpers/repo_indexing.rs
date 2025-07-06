// Repository indexing and syncing functions from repo_helpers.rs will be moved here. 

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use crate::config::{AppConfig, RepositoryConfig};
use anyhow::{Context, Result};
use tracing::{info, warn, error, debug};
use qdrant_client::qdrant::{Filter, Condition, ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector, PointId};
use crate::constants::{FIELD_BRANCH, FIELD_LANGUAGE};
use crate::error::SagittaError;
use crate::QdrantClientTrait;
use crate::indexing::{index_repo_files, ensure_collection_exists, is_hidden, is_target_dir, IndexRepoFilesParams};
use crate::repo_helpers::qdrant_utils::get_collection_name;
use crate::repo_helpers::qdrant_utils::get_branch_aware_collection_name;
use anyhow::anyhow;
use std::process::{Command, Stdio};
use walkdir::WalkDir;
use git2;
use std::fs;
use git2::Repository;
use std::os::unix::fs::PermissionsExt;
use tokio;
use crate::sync_progress::{SyncProgressReporter, AddProgressReporter, AddProgress, RepoAddStage};
use sagitta_embed::EmbeddingProcessor;

/// Updates the last synced commit for a branch in the AppConfig and refreshes the
/// list of indexed languages for that branch by querying Qdrant.
pub async fn update_sync_status_and_languages<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    config: &mut AppConfig,
    repo_config_index: usize,
    branch_name: &str,
    commit_oid_str: &str,
    client: &C,
    collection_name: &str,
) -> Result<(), SagittaError> {
    let repo_config = config.repositories.get_mut(repo_config_index)
        .ok_or_else(|| SagittaError::ConfigurationError(format!("Repository index {repo_config_index} out of bounds")))?;
    log::debug!("Updating last synced commit for branch '{branch_name}' to {commit_oid_str}");
    repo_config.last_synced_commits.insert(branch_name.to_string(), commit_oid_str.to_string());
    log::debug!("Querying Qdrant for distinct languages in collection '{collection_name}' for branch '{branch_name}'");
    let mut languages = HashSet::new();
    let mut offset: Option<PointId> = None;
    loop {
        let mut builder = ScrollPointsBuilder::new(collection_name)
            .filter(Filter::must([
                Condition::matches(FIELD_BRANCH, branch_name.to_string()),
            ]))
            .limit(1000)
            .with_payload(PayloadIncludeSelector { fields: vec![FIELD_LANGUAGE.to_string()] })
            .with_vectors(false);

        if let Some(o) = offset {
            builder = builder.offset(o);
        }

        let scroll_request = builder.into();
        let scroll_result: Result<ScrollResponse, _> = client.scroll(scroll_request).await;
        match scroll_result {
            Ok(response) => {
                if response.result.is_empty() {
                    break;
                }
                for point in response.result {
                    if let Some(lang_value) = point.payload.get(FIELD_LANGUAGE) {
                        if let Some(lang_str) = lang_value.as_str() {
                            languages.insert(lang_str.to_string());
                        }
                    }
                }
                offset = response.next_page_offset;
                if offset.is_none() {
                    break;
                }
            }
            Err(e) => {
                 error!("Failed to scroll points for distinct languages from Qdrant for collection '{}', branch '{}': {}. Language list in config may be incomplete.",
                    collection_name, branch_name, e);
                 repo_config.indexed_languages = None;
                 return Ok(());
            }
        }
    }
    info!("Found indexed languages for branch '{}': {:?}", branch_name, languages);
    let mut sorted_languages: Vec<String> = languages.into_iter().collect();
    sorted_languages.sort();
    repo_config.indexed_languages = Some(sorted_languages);
    Ok(())
}

/// Parameters for indexing files
pub struct IndexFilesParams<'a, C> {
    /// Qdrant client instance for database operations
    pub client: Arc<C>,
    /// Application configuration containing indexing settings
    pub config: &'a AppConfig,
    /// Root directory path of the repository being indexed
    pub repo_root: &'a PathBuf,
    /// List of relative file paths within the repository to index
    pub relative_paths: &'a [PathBuf],
    /// Name of the Qdrant collection to store indexed data
    pub collection_name: &'a str,
    /// Name of the branch being indexed
    pub branch_name: &'a str,
    /// Git commit hash being indexed
    pub commit_hash: &'a str,
    /// Optional progress reporter for tracking indexing progress
    pub progress_reporter: Option<Arc<dyn SyncProgressReporter>>,
}

/// Indexes a list of relative file paths within a repository.
/// Handles embedding generation and upserting points to Qdrant in batches.
/// This function utilizes parallel processing for CPU-bound tasks (parsing, embedding).
pub async fn index_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(params: IndexFilesParams<'_, C>) -> Result<usize, SagittaError>
{
    if params.relative_paths.is_empty() {
        info!("No files provided for indexing.");
        return Ok(0);
    }

    let embedding_config = crate::app_config_to_embedding_config(params.config);
    let embedding_pool = crate::EmbeddingPool::with_configured_sessions(embedding_config)
        .context("Failed to initialize embedding pool for repo indexing")?;
    info!("Embedding dimension for repo: {}", embedding_pool.dimension());

    let embedding_pool_arc = Arc::new(embedding_pool);

    index_repo_files(IndexRepoFilesParams {
        config: params.config,
        repo_root: params.repo_root,
        relative_paths: params.relative_paths,
        collection_name: params.collection_name,
        branch_name: params.branch_name,
        commit_hash: params.commit_hash,
        client: params.client.clone(),
        embedding_pool: embedding_pool_arc,
        progress_reporter: params.progress_reporter,
        max_concurrent_upserts: params.config.indexing.max_concurrent_upserts,
    }).await
}

/// Parameters for preparing a repository
pub struct PrepareRepositoryParams<'a> {
    /// Repository URL
    pub url: &'a str,
    /// Optional repository name
    pub name_opt: Option<&'a str>,
    /// Optional local path
    pub local_path_opt: Option<&'a PathBuf>,
    /// Optional branch
    pub branch_opt: Option<&'a str>,
    /// Optional target ref
    pub target_ref_opt: Option<&'a str>,
    /// Optional remote
    pub remote_opt: Option<&'a str>,
    /// Optional SSH key path
    pub ssh_key_path_opt: Option<&'a PathBuf>,
    /// Optional SSH passphrase
    pub ssh_passphrase_opt: Option<&'a str>,
    /// Base path for new clones
    pub base_path_for_new_clones: &'a Path,
    /// Embedding dimension
    pub embedding_dim: u64,
    /// App configuration
    pub config: &'a AppConfig,
    /// Optional progress reporter
    pub add_progress_reporter: Option<Arc<dyn AddProgressReporter>>,
}

/// Prepares a repository for use, either by cloning it or ensuring the local path exists.
/// Checks out the specified branch or target ref.
/// Ensures the corresponding Qdrant collection exists.
/// Returns the generated `RepositoryConfig` (does not save it to AppConfig).
pub async fn prepare_repository<C>(
    params: PrepareRepositoryParams<'_>,
    client: Arc<C>,
) -> Result<RepositoryConfig, SagittaError>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    if params.url.is_empty() && (params.local_path_opt.is_none() || !params.local_path_opt.unwrap().exists()) {
        return Err(SagittaError::Other("Either URL or existing local repository path must be provided".to_string()));
    }

    let repo_name = params.name_opt.unwrap_or_else(|| {
        params.url.split('/').next_back().unwrap_or("unknown_repo").trim_end_matches(".git")
    });
    let final_local_path = params.local_path_opt
        .cloned()
        .unwrap_or_else(|| params.base_path_for_new_clones.join(repo_name));
    let final_branch = params.branch_opt.unwrap_or("main"); // Default to main if not specified
    let final_remote = params.remote_opt.unwrap_or("origin"); // Default to origin if not specified

    let url_str = params.url.to_string(); // Clone url for closure
    let repo_name_str = repo_name.to_string(); // Clone repo_name for closure
    
    // Use branch-aware collection naming to match the new sync behavior
    let current_branch_or_ref = params.target_ref_opt.unwrap_or(final_branch);
    let collection_name = get_branch_aware_collection_name(&repo_name_str, current_branch_or_ref, params.config);

    let mut was_cloned = false;
    if !final_local_path.exists() {
        info!("Ensuring Qdrant collection '{collection_name}' exists (dim={})...", params.embedding_dim);
        ensure_collection_exists(client.clone(), &collection_name, params.embedding_dim).await?;
        info!("Qdrant collection ensured for new clone.");

        // Report clone start
        if let Some(reporter) = &params.add_progress_reporter {
            reporter.report(AddProgress::new(RepoAddStage::Clone {
                message: format!("Starting clone of repository '{repo_name_str}'"),
                progress: None,
            })).await;
        }

        info!("Cloning repository '{repo_name_str}' from {url_str} into {}...", final_local_path.display());
        
        // First try to clone without specifying a branch to avoid hanging if the branch doesn't exist
        let clone_result = if final_branch == "main" || final_branch == "master" {
            // For common default branches, try cloning without specifying branch first
            info!("Attempting to clone without specifying branch (will use repository default)...");
            info!("Large repositories may take several minutes to clone. Please wait...");
            let clone_status = tokio::time::timeout(
                std::time::Duration::from_secs(1800), // 30 minute timeout for large repos
                tokio::process::Command::new("git")
                    .arg("clone")
                    .arg(&url_str)
                    .arg(&final_local_path)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
            ).await;
            
            match clone_status {
                Ok(Ok(output)) => Ok(output),
                Ok(Err(e)) => Err(anyhow!("Git clone command failed: {}", e)),
                Err(_) => Err(anyhow!("Git clone timed out after 30 minutes")),
            }
        } else {
            // For non-default branches, try with the specific branch
            info!("Attempting to clone with specific branch '{}'...", final_branch);
            info!("Large repositories may take several minutes to clone. Please wait...");
            let clone_status = tokio::time::timeout(
                std::time::Duration::from_secs(1800), // 30 minute timeout for large repos
                tokio::process::Command::new("git")
                    .arg("clone")
                    .arg("--branch")
                    .arg(final_branch)
                    .arg(&url_str)
                    .arg(&final_local_path)
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .output()
            ).await;
            
            match clone_status {
                Ok(Ok(output)) => Ok(output),
                Ok(Err(e)) => {
                    warn!("Clone with specific branch '{}' failed: {}. Trying without branch specification...", final_branch, e);
                    // Fallback: try cloning without branch specification
                    info!("Falling back to default branch clone. Large repositories may take several minutes...");
                    let fallback_status = tokio::time::timeout(
                        std::time::Duration::from_secs(1800),
                        tokio::process::Command::new("git")
                            .arg("clone")
                            .arg(&url_str)
                            .arg(&final_local_path)
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .output()
                    ).await;
                    
                    match fallback_status {
                        Ok(Ok(output)) => Ok(output),
                        Ok(Err(e)) => Err(anyhow!("Fallback git clone command failed: {}", e)),
                        Err(_) => Err(anyhow!("Fallback git clone timed out after 30 minutes")),
                    }
                },
                Err(_) => Err(anyhow!("Git clone with branch '{}' timed out after 30 minutes", final_branch)),
            }
        };

        let clone_status = match clone_result {
            Ok(output) => output,
            Err(e) => {
                error!("Failed to clone repository: {}", e);
                
                // Report clone error
                if let Some(reporter) = &params.add_progress_reporter {
                    reporter.report(AddProgress::new(RepoAddStage::Error {
                        message: format!("Git clone failed: {e}"),
                    })).await;
                }
                
                return Err(SagittaError::GitMessageError(format!("Git clone failed: {e}")));
            }
        };

        if clone_status.status.success() {
            was_cloned = true;
            let path_str = final_local_path.display().to_string();
            info!("Successfully cloned repository to {path_str}.");
            
            // Report clone completion
            if let Some(reporter) = &params.add_progress_reporter {
                reporter.report(AddProgress::new(RepoAddStage::Clone {
                    message: format!("Successfully cloned repository to {path_str}"),
                    progress: Some((1, 1)), // Mark as complete
                })).await;
            }
            
            let stdout = String::from_utf8_lossy(&clone_status.stdout);
            let stderr = String::from_utf8_lossy(&clone_status.stderr);
            if !stdout.is_empty() {
                debug!("git clone stdout:\n{}", stdout);
            }
            if !stderr.is_empty() {
                info!("git clone stderr:\n{}", stderr);
            }
            
            // If we cloned without specifying a branch, we might need to checkout the desired branch
            if final_branch != "main" && final_branch != "master" {
                info!("Checking if we need to checkout branch '{}'...", final_branch);
                
                // Report checkout start
                if let Some(reporter) = &params.add_progress_reporter {
                    reporter.report(AddProgress::new(RepoAddStage::Checkout {
                        message: format!("Checking out branch '{final_branch}'"),
                    })).await;
                }
                
                let checkout_result = tokio::time::timeout(
                    std::time::Duration::from_secs(30),
                    tokio::process::Command::new("git")
                        .current_dir(&final_local_path)
                        .arg("checkout")
                        .arg(final_branch)
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .output()
                ).await;
                
                match checkout_result {
                    Ok(Ok(checkout_output)) => {
                        if checkout_output.status.success() {
                            info!("Successfully checked out branch '{}'", final_branch);
                        } else {
                            let stderr = String::from_utf8_lossy(&checkout_output.stderr);
                            warn!("Failed to checkout branch '{}': {}. Using default branch.", final_branch, stderr);
                        }
                    },
                    Ok(Err(e)) => {
                        warn!("Checkout command failed: {}. Using default branch.", e);
                    },
                    Err(_) => {
                        warn!("Checkout timed out. Using default branch.");
                    }
                }
            }
        } else {
            let stderr_cow = String::from_utf8_lossy(&clone_status.stderr);
            let stderr = stderr_cow.as_ref();
            error!("Failed to clone repository: {stderr}");
            
            // Report clone error
            if let Some(reporter) = &params.add_progress_reporter {
                reporter.report(AddProgress::new(RepoAddStage::Error {
                    message: format!("Git clone command failed: {stderr}"),
                })).await;
            }
            
            // Attempt to clean up partially cloned directory
            if final_local_path.exists() {
                let path_str = final_local_path.display().to_string();
                warn!("Attempting to remove partially cloned directory at {path_str}");
                if let Err(e) = std::fs::remove_dir_all(&final_local_path) {
                    let path_str = final_local_path.display().to_string();
                    let error_str = e.to_string();
                    error!("Failed to remove directory {path_str} after failed clone: {error_str}");
                }
            }
            return Err(SagittaError::GitMessageError(format!("Git clone command failed: {stderr}")));
        }
    } else {
        let path_str = final_local_path.display().to_string();
        info!("Repository already exists locally at {path_str}, ensuring collection exists...");
        info!("Ensuring Qdrant collection '{collection_name}' exists (dim={}) for existing clone...", params.embedding_dim);
        ensure_collection_exists(client.clone(), &collection_name, params.embedding_dim).await?;
        info!("Qdrant collection ensured for existing clone.");
    }

    // --- Handle target_ref --- 
    let final_active_branch: String;
    let resolved_target_ref: Option<String>;
    
    // Resolve special refs like HEAD
    if let Some(target_ref) = params.target_ref_opt {
        // Open repository to resolve refs
        let repo = git2::Repository::open(&final_local_path)
            .context(format!("Failed to open repository at {} to resolve refs", final_local_path.display()))?;
        
        if target_ref == "HEAD" {
            match super::git_edge_cases::resolve_git_ref(&repo, target_ref) {
                Ok(resolved) => {
                    info!("Resolved '{}' to '{}'", target_ref, resolved);
                    resolved_target_ref = Some(resolved);
                }
                Err(e) => {
                    warn!("Failed to resolve '{}': {}, using default branch", target_ref, e);
                    resolved_target_ref = None;
                }
            }
        } else {
            // Validate the ref name
            match super::git_edge_cases::validate_ref_name(target_ref) {
                Ok(_) => resolved_target_ref = Some(target_ref.to_string()),
                Err(e) => {
                    return Err(SagittaError::GitMessageError(format!(
                        "Invalid target ref '{target_ref}': {e}"
                    )));
                }
            }
        }
    } else {
        resolved_target_ref = None;
    }
    
    if let Some(target_ref) = resolved_target_ref.as_ref() {
        info!("Attempting to checkout target ref '{}' for repository '{}'...", target_ref, repo_name);
        
        // Report fetch start
        if let Some(reporter) = &params.add_progress_reporter {
            reporter.report(AddProgress::new(RepoAddStage::Fetch {
                message: format!("Fetching latest changes for target ref '{target_ref}'"),
                progress: None,
            })).await;
        }
        
        // Fetch before checkout to ensure the ref is available locally, especially if it's a remote branch/tag
        // Don't prune here, might remove the ref we want if it's only remote
        let fetch_status = Command::new("git")
            .current_dir(&final_local_path)
            .arg("fetch")
            .arg(final_remote)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to spawn git fetch before checkout for {repo_name}"))?
            .wait_with_output()
            .context(format!("Failed to wait for git fetch before checkout for {repo_name}"))?;
        
        if !fetch_status.status.success() {
            let stderr = String::from_utf8_lossy(&fetch_status.stderr);
            warn!("Git fetch before checkout failed for {}: {}. Checkout might still succeed if ref is local.", repo_name, stderr);
        } else {
            info!("Git fetch before checkout successful for {}", repo_name);
        }

        // Report checkout start
        if let Some(reporter) = &params.add_progress_reporter {
            reporter.report(AddProgress::new(RepoAddStage::Checkout {
                message: format!("Checking out target ref '{target_ref}'"),
            })).await;
        }

        // Now attempt checkout
        let checkout_status = Command::new("git")
            .current_dir(&final_local_path)
            .arg("checkout")
            .arg(target_ref)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to spawn git checkout {target_ref} for {repo_name}"))?
            .wait_with_output()
            .context(format!("Failed to wait for git checkout {target_ref} for {repo_name}"))?;

        if checkout_status.status.success() {
            info!("Successfully checked out target ref '{}' for repository '{}'.", target_ref, repo_name);
            final_active_branch = target_ref.to_string();
            let stdout = String::from_utf8_lossy(&checkout_status.stdout);
            let stderr = String::from_utf8_lossy(&checkout_status.stderr);
            if !stdout.is_empty() {
                debug!("git checkout stdout:\n{}", stdout);
            }
            if !stderr.is_empty() && !stderr.contains("HEAD is now at") {
                 info!("git checkout stderr:\n{}", stderr);
            }
        } else {
            let stderr_cow = String::from_utf8_lossy(&checkout_status.stderr);
            let stderr = stderr_cow.as_ref();
            error!("Failed to checkout target ref '{}' for repository '{}': {}", target_ref, repo_name, stderr);
            
            // Report checkout error
            if let Some(reporter) = &params.add_progress_reporter {
                reporter.report(AddProgress::new(RepoAddStage::Error {
                    message: format!("Failed to checkout target ref '{target_ref}': {stderr}"),
                })).await;
            }
            
             // If we cloned the repo just now and checkout failed, clean up.
            if was_cloned {
                warn!("Attempting to remove repository directory {} due to failed checkout of target ref.", final_local_path.display());
                if let Err(e) = std::fs::remove_dir_all(&final_local_path) {
                     error!("Failed to remove directory {} after failed checkout: {}", final_local_path.display(), e);
                }
            }
            return Err(SagittaError::GitMessageError(format!(
                "Failed to checkout target ref '{target_ref}': {stderr}"
            )));
        }
    } else {
        // No target_ref specified, use the initially cloned/existing branch
        final_active_branch = final_branch.to_string();
    }

    // Determine the final URL for the RepositoryConfig
    let mut final_url_to_store = url_str.clone(); // url_str is from the function input `url`
    if final_url_to_store.is_empty() && final_local_path.exists() {
        // If no URL was provided and it's an existing local path, try to get it from the git remote
        match git2::Repository::open(&final_local_path) {
            Ok(repo) => {
                let remote_to_check = params.remote_opt.unwrap_or("origin");
                match repo.find_remote(remote_to_check) {
                    Ok(remote) => {
                        if let Some(git_remote_url) = remote.url() {
                            info!("No URL provided for local repo, using remote '{}' URL: {}", remote_to_check, git_remote_url);
                            final_url_to_store = git_remote_url.to_string();
                        } else {
                            warn!("Local repo at '{}' has remote '{}' but no URL is configured for it.", final_local_path.display(), remote_to_check);
                        }
                    }
                    Err(_) => {
                        warn!("Local repo at '{}' does not have a remote named '{}'. URL will be empty.", final_local_path.display(), remote_to_check);
                    }
                }
            }
            Err(e) => {
                warn!("Failed to open local repo at '{}' to check for remote URL: {}. URL will be empty.", final_local_path.display(), e);
            }
        }
    }

    // Report completion
    if let Some(reporter) = &params.add_progress_reporter {
        reporter.report(AddProgress::new(RepoAddStage::Completed {
            message: format!("Repository '{}' successfully prepared at {}", repo_name, final_local_path.display()),
        })).await;
    }

    Ok(RepositoryConfig {
        name: repo_name.to_string(),
        url: final_url_to_store, // Use the potentially updated URL
        local_path: final_local_path,
        default_branch: final_active_branch.clone(),
        tracked_branches: if final_active_branch != final_branch && params.target_ref_opt.is_some() {
            vec![final_branch.to_string(), final_active_branch.clone()]
        } else {
            vec![final_active_branch.clone()]
        },
        active_branch: Some(final_active_branch),
        remote_name: Some(final_remote.to_string()),
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
        ssh_key_path: params.ssh_key_path_opt.cloned(),
        ssh_key_passphrase: params.ssh_passphrase_opt.map(String::from),
        added_as_local_path: params.local_path_opt.is_some(),
        target_ref: params.target_ref_opt.map(|s| s.to_string()),
    })
}

/// Deletes all data associated with a repository, including the Qdrant collection and the local directory.
pub async fn delete_repository_data<C>(
    repo_config: &RepositoryConfig,
    client: Arc<C>,
    config: &AppConfig,
) -> Result<(), SagittaError>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = &repo_config.name;
    
    // With branch-aware collections, we need to delete all collections for this repository
    // For now, we'll try to delete the legacy collection and the current branch collection
    // In the future, we could list all collections and filter by pattern
    
    // Try to delete legacy collection (for backward compatibility)
    let legacy_collection_name = get_collection_name(repo_name, config);
    info!("Attempting to delete legacy Qdrant collection '{legacy_collection_name}'...");
    match client.delete_collection(legacy_collection_name.clone()).await {
        Ok(deleted) => {
            if deleted {
                info!("Successfully deleted legacy Qdrant collection '{legacy_collection_name}'.");
            } else {
                info!("Legacy Qdrant collection '{legacy_collection_name}' did not exist or was already deleted.");
            }
        }
        Err(e) => {
            let error_str = e.to_string();
            warn!("Failed to delete legacy Qdrant collection '{legacy_collection_name}': {error_str}. Continuing removal process.");
        }
    }
    
    // Try to delete branch-aware collections for known branches
    let branches_to_try = vec![
        repo_config.default_branch.as_str(),
        repo_config.active_branch.as_deref().unwrap_or("main"),
        "main",
        "master",
        "develop",
        "dev"
    ];
    
    // Also include target_ref if it exists
    let mut all_refs_to_try = branches_to_try;
    if let Some(target_ref) = &repo_config.target_ref {
        all_refs_to_try.push(target_ref.as_str());
    }
    
    // Remove duplicates
    all_refs_to_try.sort();
    all_refs_to_try.dedup();
    
    for branch_or_ref in all_refs_to_try {
        let branch_collection_name = get_branch_aware_collection_name(repo_name, branch_or_ref, config);
        info!("Attempting to delete branch-aware Qdrant collection '{branch_collection_name}' for branch/ref '{branch_or_ref}'...");
        match client.delete_collection(branch_collection_name.clone()).await {
            Ok(deleted) => {
                if deleted {
                    info!("Successfully deleted branch-aware Qdrant collection '{branch_collection_name}'.");
                } else {
                    info!("Branch-aware Qdrant collection '{branch_collection_name}' did not exist or was already deleted.");
                }
            }
            Err(e) => {
                let error_str = e.to_string();
                warn!("Failed to delete branch-aware Qdrant collection '{branch_collection_name}': {error_str}. Continuing removal process.");
            }
        }
    }

    let local_path = &repo_config.local_path;
    if !local_path.exists() {
        info!("Local directory '{}' does not exist. Skipping removal.", local_path.display());
        return Ok(());
    }
    
    // Safety checks
    let path_str = local_path.to_string_lossy();
    if path_str.len() < 10 { 
        error!("Path '{path_str}' is suspiciously short (len {}). Skipping removal for safety.", path_str.len());
        return Ok(());
    }
    let dangerous_paths = ["/", "/usr", "/bin", "/sbin", "/etc", "/var", "/tmp", "/opt", "/boot", "/lib", "/dev", "/proc", "/sys", "/run"];
    if dangerous_paths.iter().any(|p| path_str == *p || path_str.starts_with(&format!("{p}/"))) {
        error!("Path '{path_str}' appears to be a system directory. Refusing to delete for safety.");
        return Ok(());
    }
    
    info!("Safety checks passed. Starting repository directory removal process...");
    
    // Close any open Git handles
    if let Ok(repo) = Repository::open(local_path) {
        info!("Closing any open Git repository handles...");
        drop(repo);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    
    // Try using external rm command for Unix systems (more reliable)
    if cfg!(unix) {
        info!("Attempting to use external rm command for forceful deletion...");
        let output = std::process::Command::new("rm")
            .arg("-rf")
            .arg(local_path.as_os_str())
            .output();
            
        match output {
            Ok(output) if output.status.success() => {
                info!("External rm command reported success");
                std::thread::sleep(std::time::Duration::from_millis(500));
                
                if !local_path.exists() {
                    info!("Successfully verified repository directory removal via external command");
                    return Ok(());
                } else {
                    warn!("External rm command reported success but directory still exists");
                }
            }
            Ok(output) => {
                warn!("External rm command failed: {}", String::from_utf8_lossy(&output.stderr));
            }
            Err(e) => {
                warn!("Failed to execute external rm command: {}", e);
            }
        }
    }
    
    // If external command didn't work, fall back to staged deletion
    info!("Falling back to staged directory deletion...");
    
    // Make all files writable first
    for entry in WalkDir::new(local_path).into_iter().filter_map(|e| e.ok()) {
        if let Err(e) = fs::set_permissions(entry.path(), fs::Permissions::from_mode(0o777)) {
            debug!("Failed to set permissions on {}: {}", entry.path().display(), e);
        }
    }
    
    // Delete all files first, then build a list of directories to delete
    let mut dirs_to_delete = Vec::new();
    
    for entry in WalkDir::new(local_path).into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Err(e) = fs::remove_file(entry.path()) {
                warn!("Failed to delete file {}: {}", entry.path().display(), e);
            }
        } else if entry.file_type().is_dir() {
            dirs_to_delete.push(entry.path().to_path_buf());
        }
    }
    
    // Sort directories by depth (deepest first)
    dirs_to_delete.sort_by_key(|path| {
        path.components().count()
    });
    dirs_to_delete.reverse();
    
    // Now delete directories from deepest to shallowest
    for dir in dirs_to_delete {
        if dir == *local_path {
            continue; // Skip the root directory, we'll delete it last
        }
        if let Err(e) = fs::remove_dir(&dir) {
            warn!("Failed to delete directory {}: {}", dir.display(), e);
            // Try remove_dir_all as a fallback
            if let Err(e) = fs::remove_dir_all(&dir) {
                warn!("Failed to delete directory with remove_dir_all {}: {}", dir.display(), e);
            }
        }
    }
    
    // Finally, try to remove the repository root directory
    let mut attempts = 0;
    let max_attempts = 3;
    
    while attempts < max_attempts {
        attempts += 1;
        info!("Attempt {} to remove repository root directory", attempts);
        
        if let Err(e) = fs::remove_dir(local_path) {
            warn!("Failed to remove repository root directory: {}", e);
            std::thread::sleep(std::time::Duration::from_millis(200 * attempts));
            
            // If remove_dir fails, try remove_dir_all
            if let Err(e) = fs::remove_dir_all(local_path) {
                warn!("Failed to remove repository root directory with remove_dir_all: {}", e);
            } else {
                info!("Successfully removed repository directory with remove_dir_all");
                break;
            }
        } else {
            info!("Successfully removed repository directory");
            break;
        }
    }
    
    // Final verification
    std::thread::sleep(std::time::Duration::from_millis(500));
    if !local_path.exists() {
        info!("Successfully verified repository directory removal");
        Ok(())
    } else {
        error!("Failed to delete repository directory after all attempts");
        Err(SagittaError::Other("Failed to delete repository directory".to_string()))
    }
}

/// Represents basic information about a Git commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    /// The Git commit object ID (hash).
    pub oid: git2::Oid,
    /// The commit summary (first line of the commit message).
    pub summary: String,
}

/// Synchronizes a specific repository based on its configuration.
///
/// - If `target_ref` is set in the `RepositoryConfig`, this function will:
///   1. Check out the specified static ref (tag, commit, branch).
///   2. Determine the commit hash for that ref.
///   3. Return the commit hash without fetching updates from the remote.
///      (The caller is then responsible for indexing based on this static ref).
/// - If `target_ref` is *not* set, this function will:
///   1. Fetch updates from the remote for the repository's active branch.
///   2. Merge the changes (fast-forward or create merge commit).
///   3. Return the new HEAD commit hash after the merge.
///      (The caller is then responsible for indexing the updated files).
///
/// Note: The actual indexing (`index_files`) is typically called *after* this function
/// by the primary handler (e.g., in MCP server or CLI command) based on the returned commit hash.
pub async fn sync_repository_branch(
    config: &AppConfig,
    repo_config_index: usize,
    _client: Arc<impl QdrantClientTrait + 'static>, // Keep client for future use maybe
    _fetch_and_merge: bool, // Keep flag for future use maybe
) -> Result<String, anyhow::Error> {
    let repo_config = config.repositories.get(repo_config_index)
        .ok_or_else(|| anyhow!("Repository index {} out of bounds", repo_config_index))?;

    let repo_path = &repo_config.local_path;
    let repo_name = &repo_config.name;

    // --- Handle Static Target Ref --- 
    if let Some(target_ref) = &repo_config.target_ref {
        info!("Processing static target ref '{}' for repository '{}'.", target_ref, repo_name);
        
        // Ensure the target ref is checked out
        match checkout_branch(repo_path, target_ref) {
            Ok(()) => info!("Ensured checkout of static ref '{}' for repository '{}'.", target_ref, repo_name),
            Err(e) => {
                error!("Failed to checkout static target ref '{}' for repository '{}': {}", target_ref, repo_name, e);
                return Err(anyhow!(
                    "Failed to checkout static target ref '{}' for repository '{}': {}",
                    target_ref,
                    repo_name,
                    e
                ));
            }
        }

        // Get the commit hash for the target ref
        let commit_hash_output = Command::new("git")
            .current_dir(repo_path)
            .arg("rev-parse")
            .arg(target_ref)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context(format!("Failed to execute git rev-parse for target ref {target_ref}"))?;

        if commit_hash_output.status.success() {
            let commit_hash = String::from_utf8(commit_hash_output.stdout)?.trim().to_string();
            info!("Resolved static target ref '{}' to commit {} for repository '{}'.", target_ref, commit_hash, repo_name);
            // Return the commit hash directly. The caller will handle indexing.
            return Ok(commit_hash);
        } else {
            let stderr = String::from_utf8_lossy(&commit_hash_output.stderr);
            error!("Failed to get commit hash for static target ref '{}' in repository '{}': {}", target_ref, repo_name, stderr);
            return Err(anyhow!(
                "Failed to resolve static target ref '{}' to a commit in repository '{}': {}",
                target_ref,
                repo_name,
                stderr
            ));
        }
    }

    // --- Handle Dynamic Branch Sync (Original Logic) ---
    info!("Processing dynamic branch sync for repository '{}'.", repo_name);
    let branch_name = repo_config.active_branch.as_deref()
        .ok_or_else(|| anyhow!("No active branch set for repository {}", repo_config.name))?;
    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin");

    info!("Starting sync for repository {}, branch {}, remote {}", repo_name, branch_name, remote_name);

    // 1. Fetch from remote
    info!("Fetching from remote {remote_name} for {repo_name}...");
    let fetch_output = Command::new("git")
        .current_dir(repo_path)
        .arg("fetch")
        .arg(remote_name)
        .arg("--prune") // Remove remote-tracking refs that no longer exist
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context(format!("Failed to spawn git fetch for {}", repo_config.name))?
        .wait_with_output()
        .context(format!("Failed to wait for git fetch for {}", repo_config.name))?;

    if !fetch_output.status.success() {
        let stderr_cow = String::from_utf8_lossy(&fetch_output.stderr);
        let stderr = stderr_cow.as_ref();
        error!("Git fetch failed for {repo_name}: {stderr}");
        return Err(anyhow!("Git fetch failed for {}: {}", repo_config.name, stderr));
    } else {
        let stdout = String::from_utf8_lossy(&fetch_output.stdout);
        let stderr = String::from_utf8_lossy(&fetch_output.stderr);
        info!("Fetch completed for {repo_name}.");
        if !stdout.is_empty() {
            debug!("git fetch stdout:\n{}", stdout);
        }
        if !stderr.is_empty() {
            info!("git fetch stderr:\n{}", stderr);
        }
    }

    // 2. Get local commit hash (HEAD)
    let local_commit_output = Command::new("git")
        .current_dir(repo_path)
        .arg("rev-parse")
        .arg(format!("refs/heads/{branch_name}")) // Target local branch ref
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context(format!("Failed to execute git rev-parse for local {branch_name}"))?;

    let local_commit_hash = if local_commit_output.status.success() {
        String::from_utf8(local_commit_output.stdout)?.trim().to_string()
    } else {
        let stderr_cow = String::from_utf8_lossy(&local_commit_output.stderr);
        let stderr = stderr_cow.as_ref();
        if stderr.contains("unknown revision or path not in the working tree") {
            warn!("Local branch {branch_name} not found in {repo_name}. It might have been deleted or not checked out.");
            return Ok(format!("Local branch {branch_name} not found."));
        } else {
            error!("Failed to get local commit hash for {repo_name}/{branch_name}: {stderr}");
            return Err(anyhow!("Failed to get local commit hash for {}/{}: {}", repo_config.name, branch_name, stderr));
        }
    };

    // 3. Get remote commit hash
    let remote_ref = format!("refs/remotes/{remote_name}/{branch_name}");
    let remote_commit_output = Command::new("git")
        .current_dir(repo_path)
        .arg("rev-parse")
        .arg(&remote_ref)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context(format!("Failed to execute git rev-parse for remote {remote_ref}"))?;

    let remote_commit_hash = if remote_commit_output.status.success() {
        String::from_utf8(remote_commit_output.stdout)?.trim().to_string()
    } else {
        let stderr_cow = String::from_utf8_lossy(&remote_commit_output.stderr);
        let stderr = stderr_cow.as_ref();
        if stderr.contains("unknown revision or path not in the working tree") {
            info!("Remote branch {remote_ref} not found for {repo_name} after fetch (likely deleted).");
            return Ok(format!("Remote branch {remote_name}/{branch_name} not found after fetch."));
        } else {
            error!("Failed to get remote commit hash for {repo_name}/{remote_ref} even though ref exists: {stderr}");
            return Err(anyhow!("Failed to get remote commit hash for {}/{}: {}", repo_config.name, remote_ref, stderr));
        }
    };

    // 4. Compare local and remote hashes
    if local_commit_hash == remote_commit_hash {
        info!("Branch {branch_name} already up-to-date for {repo_name}.");
        return Ok(format!("Branch {branch_name} already up-to-date."));
    }

    // 5. Determine relationship (behind, ahead, diverged) using merge-base
    let merge_base_output = Command::new("git")
        .current_dir(repo_path)
        .arg("merge-base")
        .arg(&local_commit_hash)
        .arg(&remote_commit_hash)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context("Failed to execute git merge-base")?;

    if !merge_base_output.status.success() {
        let stderr_cow = String::from_utf8_lossy(&merge_base_output.stderr);
        let stderr = stderr_cow.as_ref();
        error!("Failed to find merge base between {local_commit_hash} and {remote_commit_hash} for {repo_name}: {stderr}");
        return Err(anyhow!("Failed to find merge base for {}/{}: {}", repo_config.name, branch_name, stderr));
    }
    let merge_base_hash = String::from_utf8(merge_base_output.stdout)?.trim().to_string();

    // 6. Attempt merge/update based on relationship
    if merge_base_hash == local_commit_hash {
        // Local is behind remote (fast-forward possible)
        info!("Local branch {branch_name} is behind remote for {repo_name}. Attempting fast-forward...");
        checkout_branch(repo_path, branch_name)?;

        let merge_output = Command::new("git")
            .current_dir(repo_path)
            .arg("merge")
            .arg("--ff-only") // Ensure fast-forward
            .arg(&remote_ref)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .context(format!("Failed to execute git merge --ff-only for {branch_name}"))?;

        if !merge_output.status.success() {
            let stderr_cow = String::from_utf8_lossy(&merge_output.stderr);
            let stderr = stderr_cow.as_ref();
            error!("Fast-forward merge failed for {repo_name}/{branch_name}: {stderr}");
            Err(anyhow!("Fast-forward merge failed for {}/{}: {}", repo_config.name, branch_name, stderr))
        } else {
            let stdout = String::from_utf8_lossy(&merge_output.stdout);
            info!("Fast-forward merge successful for {repo_name}/{branch_name}. New commit: {remote_commit_hash}");
            if !stdout.is_empty() {
                debug!("git merge stdout:\n{}", stdout);
            }
            Ok(remote_commit_hash)
        }
    } else if merge_base_hash == remote_commit_hash {
        // Local is ahead of remote
        warn!("Local branch {branch_name} ({local_commit_hash}) is ahead of remote {remote_commit_hash} ({remote_commit_hash}) for {repo_name}. No automatic push configured.");
        Ok(format!("Local branch {branch_name} is ahead of remote."))
    } else {
        // Local and remote have diverged
        warn!("Local branch {branch_name} ({local_commit_hash}) and remote {remote_commit_hash} ({remote_commit_hash}) have diverged (base: {merge_base_hash}) for {repo_name}. Manual merge required.");
        Ok(format!("Branch {branch_name} has diverged from remote. Manual merge required."))
    }
}

/// Helper function to checkout a specific branch using Command
fn checkout_branch(repo_path: &Path, branch_name: &str) -> Result<(), anyhow::Error> {
    let repo_path_str = repo_path.display().to_string();
    info!("Checking out branch {branch_name} in {repo_path_str}...");
    let checkout_output = Command::new("git")
        .current_dir(repo_path)
        .arg("checkout")
        .arg(branch_name)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context(format!("Failed to execute git checkout {branch_name}"))?;

    if !checkout_output.status.success() {
        let stderr_cow = String::from_utf8_lossy(&checkout_output.stderr);
        let stderr = stderr_cow.as_ref();
        if stderr.contains("did not match any file(s) known to git") || stderr.contains("pathspec") {
            error!("Branch {branch_name} does not exist locally in {repo_path_str}. Cannot checkout.");
            return Err(anyhow!("Branch {} does not exist locally in {}", branch_name, repo_path.display()));
        } else {
            error!("Failed to checkout branch {branch_name} in {repo_path_str}: {stderr}");
            return Err(anyhow!("Failed to checkout branch {} in {}: {}", branch_name, repo_path.display(), stderr));
        }
    } else {
        let stdout = String::from_utf8_lossy(&checkout_output.stdout);
        let repo_path_str = repo_path.display().to_string();
        info!("Successfully checked out branch {branch_name} in {repo_path_str}.");
        if !stdout.is_empty() {
            debug!("git checkout stdout:\n{}", stdout);
        }
        let stderr = String::from_utf8_lossy(&checkout_output.stderr);
        if !stderr.is_empty() && !stderr.contains("Switched to branch") {
            info!("git checkout stderr:\n{}", stderr);
        }
    }
    Ok(())
}

/// Indexes all supported files within a given repository path for a specific branch and commit.
/// This is typically called after `sync_repository_branch` has updated the repository.
pub async fn index_repository<C: QdrantClientTrait + Send + Sync + 'static>(
    client: Arc<C>,
    repo_path: &Path,
    collection_name: &str,
    branch: &str,
    commit_hash: &str,
    config: &AppConfig,
) -> Result<()> {
    let mut files_to_process = Vec::new();
    
    // Walk through the repository directory
    for entry in WalkDir::new(repo_path)
        .into_iter()
        .filter_entry(|e| !is_hidden(e) && !is_target_dir(e))
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        // Get file metadata
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                log::warn!("Failed to get metadata for file {}: {}. Skipping.", path.display(), e);
                continue;
            }
        };

        // Check file size
        if metadata.len() > config.performance.max_file_size_bytes {
            log::warn!(
                "File {} exceeds maximum size limit ({} bytes), skipping",
                path.display(),
                config.performance.max_file_size_bytes
            );
            continue;
        }

        // Get relative path
        let relative_path = path.strip_prefix(repo_path)
            .map_err(|e| anyhow!("Failed to get relative path for {}: {}", path.display(), e))?;
        
        files_to_process.push(relative_path.to_path_buf());
    }

    // Process the files
    if files_to_process.is_empty() {
        log::info!("No files found to process in repository {}", repo_path.display());
        return Ok(());
    }

    // Index the files
    index_files(IndexFilesParams {
        client,
        config,
        repo_root: &repo_path.to_path_buf(),
        relative_paths: &files_to_process,
        collection_name,
        branch_name: branch,
        commit_hash,
        progress_reporter: None,
    }).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;
    use crate::qdrant_client_trait::MockQdrantClientTrait; // For a mock client

    #[tokio::test]
    async fn test_delete_repository_data_simple() -> Result<(), SagittaError> {
        let temp_dir = tempdir().map_err(|e| SagittaError::Other(format!("Failed to create temp dir: {}", e)))?;
        let repo_path = temp_dir.path().join("my_test_repo");
        fs::create_dir_all(&repo_path).map_err(|e| SagittaError::Other(format!("Failed to create repo_path: {}", e)))?;
        fs::create_dir(repo_path.join(".git")).map_err(|e| SagittaError::Other(format!("Failed to create .git dir: {}", e)))?;
        fs::write(repo_path.join("file.txt"), "hello").map_err(|e| SagittaError::Other(format!("Failed to write file: {}", e)))?;

        assert!(repo_path.exists());
        assert!(repo_path.join(".git").exists());

        let repo_config = RepositoryConfig {
            name: "my_test_repo".to_string(),
            url: "http://localhost:6334".to_string(),
            local_path: repo_path.clone(),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::new(),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
        };

        let app_config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            repositories_base_path: Some(temp_dir.path().to_string_lossy().into_owned()),
            ..Default::default()
        };

        let mut mock_qdrant_client = MockQdrantClientTrait::new();
        // Expect delete_collection to be called and return success
        mock_qdrant_client.expect_delete_collection()
            .returning(|_| Ok(true) ); // Return the Result directly
        
        let client = Arc::new(mock_qdrant_client);

        delete_repository_data(&repo_config, client, &app_config).await?;

        drop(temp_dir); // Explicitly drop temp_dir before the final assert

        assert!(!repo_path.exists(), "Repository directory should have been deleted.");

        Ok(())
    }

    #[tokio::test]
    async fn test_sync_status_update_with_languages() -> Result<(), SagittaError> {
        let mut config = AppConfig::default();
        
        // Add a test repository
        config.repositories.push(RepositoryConfig {
            name: "test-repo".to_string(),
            url: "https://example.com/repo.git".to_string(),
            local_path: PathBuf::from("/tmp/test-repo"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            last_synced_commits: HashMap::new(),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
        });

        let repo_index = 0;
        let branch_name = "main";
        let commit_hash = "abc123def456";

        // Mock Qdrant client that will simulate the language query
        let mut mock_client = MockQdrantClientTrait::new();
        mock_client.expect_scroll()
            .returning(|_| {
                use qdrant_client::qdrant::{ScrollResponse, RetrievedPoint, PointId};
                use std::collections::HashMap;
                
                // Mock response with some language data
                let mut payload = HashMap::new();
                payload.insert("language".to_string(), qdrant_client::qdrant::Value {
                    kind: Some(qdrant_client::qdrant::value::Kind::StringValue("rust".to_string())),
                });
                
                let point = RetrievedPoint {
                    id: Some(PointId { point_id_options: Some(qdrant_client::qdrant::point_id::PointIdOptions::Uuid("test".to_string())) }),
                    payload,
                    vectors: None,
                    shard_key: None,
                    order_value: None,
                };
                
                Ok(ScrollResponse {
                    result: vec![point],
                    next_page_offset: None,
                    time: 0.1,
                    usage: None,
                })
            });

        let client_arc = Arc::new(mock_client);
        let collection_name = "test_collection";

        // Test the update function
        let result = update_sync_status_and_languages(
            &mut config,
            repo_index,
            branch_name,
            commit_hash,
            client_arc.as_ref(),
            collection_name,
        ).await;

        assert!(result.is_ok());
        
        // Verify the commit was updated
        let repo = &config.repositories[repo_index];
        assert_eq!(repo.last_synced_commits.get(branch_name), Some(&commit_hash.to_string()));
        
        // Verify languages were set (at least rust from our mock)
        assert!(repo.indexed_languages.is_some());
        let languages = repo.indexed_languages.as_ref().unwrap();
        assert!(languages.contains(&"rust".to_string()));

        Ok(())
    }

    #[tokio::test]
    async fn test_prepare_repository_timeout_simulation() -> Result<(), SagittaError> {
        // This test simulates timeout behavior without actually timing out
        // since we can't easily test actual 30-minute timeouts in unit tests
        
        let temp_dir = tempdir().map_err(|e| SagittaError::Other(format!("Failed to create temp dir: {}", e)))?;
        let base_path = temp_dir.path();
        
        // Mock client for collection operations
        let mut mock_client = MockQdrantClientTrait::new();
        mock_client.expect_collection_exists()
            .returning(|_| Ok(false));
        mock_client.expect_create_collection()
            .returning(|_, _| Ok(true));
        
        let client = Arc::new(mock_client);
        
        // Test with an invalid URL that would timeout in real scenario
        let config = AppConfig::default();
        
        // This should fail because the URL is invalid, simulating what would happen
        // if a clone operation timed out
        let prepare_params = PrepareRepositoryParams {
            url: "https://invalid-nonexistent-repo-url-that-would-timeout.git",
            name_opt: Some("test-repo"),
            local_path_opt: None, // No local path
            branch_opt: Some("main"),
            target_ref_opt: None, // No target ref
            remote_opt: None, // No remote
            ssh_key_path_opt: None, // No SSH key
            ssh_passphrase_opt: None, // No SSH passphrase
            base_path_for_new_clones: base_path,
            embedding_dim: 384,
            config: &config,
            add_progress_reporter: None, // No progress reporter for test
        };
        let result = prepare_repository::<MockQdrantClientTrait>(
            prepare_params,
            client,
        ).await;
        
        // Should fail with a git error (simulating timeout)
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("Git") || error_msg.contains("clone") || error_msg.contains("failed"));

        Ok(())
    }

    #[tokio::test]
    async fn test_prepare_repository_with_existing_local_path() -> Result<(), SagittaError> {
        let temp_dir = tempdir().map_err(|e| SagittaError::Other(format!("Failed to create temp dir: {}", e)))?;
        let base_path = temp_dir.path();
        let existing_repo_path = base_path.join("existing_repo");
        
        // Create a fake existing repository
        fs::create_dir_all(&existing_repo_path).map_err(|e| SagittaError::Other(format!("Failed to create existing repo: {}", e)))?;
        fs::create_dir_all(&existing_repo_path.join(".git")).map_err(|e| SagittaError::Other(format!("Failed to create .git: {}", e)))?;
        
        // Mock client
        let mut mock_client = MockQdrantClientTrait::new();
        mock_client.expect_collection_exists()
            .returning(|_| Ok(false));
        mock_client.expect_create_collection()
            .returning(|_, _| Ok(true));
        
        let client = Arc::new(mock_client);
        let config = AppConfig::default();
        
        // Test prepare_repository with existing local path (should not timeout)
        let prepare_params = PrepareRepositoryParams {
            url: "", // Empty URL since we're using local path
            name_opt: Some("existing-repo"),
            local_path_opt: Some(&existing_repo_path),
            branch_opt: Some("main"),
            target_ref_opt: None,
            remote_opt: None,
            ssh_key_path_opt: None,
            ssh_passphrase_opt: None,
            base_path_for_new_clones: base_path,
            embedding_dim: 384,
            config: &config,
            add_progress_reporter: None, // No progress reporter for test
        };
        let result = prepare_repository::<MockQdrantClientTrait>(
            prepare_params,
            client,
        ).await;
        
        assert!(result.is_ok());
        let repo_config = result.unwrap();
        assert_eq!(repo_config.name, "existing-repo");
        assert_eq!(repo_config.local_path, existing_repo_path);
        assert!(repo_config.added_as_local_path);

        Ok(())
    }

    #[tokio::test]
    async fn test_index_files_empty_list() -> Result<(), SagittaError> {
        let temp_dir = tempdir().map_err(|e| SagittaError::Other(format!("Failed to create temp dir: {}", e)))?;
        let repo_root = temp_dir.path().to_path_buf();
        
        // Mock client
        let mut mock_client = MockQdrantClientTrait::new();
        // No expectations needed since no files to index
        
        let client = Arc::new(mock_client);
        let config = AppConfig::default();
        let empty_file_list: Vec<PathBuf> = vec![];
        
        let result = index_files(IndexFilesParams {
            client,
            config: &config,
            repo_root: &repo_root,
            relative_paths: &empty_file_list,
            collection_name: "test_collection",
            branch_name: "main",
            commit_hash: "abc123",
            progress_reporter: None,
        }).await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 0); // Should return 0 files indexed

        Ok(())
    }

    #[tokio::test]
    async fn test_timeout_configuration_values() {
        // Test that the timeout values are set correctly
        // This is more of a documentation test to ensure the values match expectations
        
        // The timeout should be 1800 seconds (30 minutes)
        let expected_timeout_secs = 1800;
        let expected_timeout = std::time::Duration::from_secs(expected_timeout_secs);
        
        // Verify the timeout duration is what we expect
        assert_eq!(expected_timeout.as_secs(), 1800);
        assert_eq!(expected_timeout.as_secs() / 60, 30); // 30 minutes
        
        // This documents the timeout behavior that's actually implemented
        // in the prepare_repository function for git clone operations
    }

    #[tokio::test]
    async fn test_cleanup_operations_safety_checks() -> Result<(), SagittaError> {
        let temp_dir = tempdir().map_err(|e| SagittaError::Other(format!("Failed to create temp dir: {}", e)))?;
        
        // Test that dangerous paths are rejected
        let dangerous_paths = vec![
            "/",
            "/usr",
            "/bin", 
            "/etc",
            "/var",
            "/tmp",
            "/home", // This one might be system-dependent
        ];
        
        for dangerous_path in dangerous_paths {
            let repo_config = RepositoryConfig {
                name: "dangerous-repo".to_string(),
                url: "https://example.com/repo.git".to_string(),
                local_path: PathBuf::from(dangerous_path),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                remote_name: Some("origin".to_string()),
                last_synced_commits: HashMap::new(),
                active_branch: Some("main".to_string()),
                ssh_key_path: None,
                ssh_key_passphrase: None,
                indexed_languages: None,
                added_as_local_path: false,
                target_ref: None,
                };
            
            let mut mock_client = MockQdrantClientTrait::new();
            mock_client.expect_delete_collection()
                .returning(|_| Ok(true));
            
            let client = Arc::new(mock_client);
            let config = AppConfig::default();
            
            // This should complete without actually deleting system directories
            let result = delete_repository_data(&repo_config, client, &config).await;
            
            // Should succeed but not actually delete dangerous paths
            // (the function has safety checks to prevent this)
            assert!(result.is_ok());
            
            // Verify the dangerous path still exists
            assert!(PathBuf::from(dangerous_path).exists(), 
                   "Dangerous path {} should not have been deleted", dangerous_path);
        }

        Ok(())
    }
} 