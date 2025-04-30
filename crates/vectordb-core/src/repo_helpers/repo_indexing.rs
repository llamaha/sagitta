// Repository indexing and syncing functions from repo_helpers.rs will be moved here. 

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::{HashSet, HashMap};
use crate::config::RepositoryConfig;
use anyhow::{Context, Result};
use tracing::{info, warn, error, debug};
use indicatif::{ProgressBar, ProgressStyle};
use uuid::Uuid;
use qdrant_client::qdrant::{PointStruct, Filter, Condition, ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector, PointId};
use crate::constants::{BATCH_SIZE, FIELD_BRANCH, FIELD_CHUNK_CONTENT, FIELD_COMMIT_HASH, FIELD_ELEMENT_TYPE, FIELD_END_LINE, FIELD_FILE_EXTENSION, FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE, MAX_FILE_SIZE_BYTES};
use crate::config::AppConfig;
use crate::embedding::EmbeddingHandler;
use crate::QdrantClientTrait;
use crate::indexing::{self, index_repo_files, ensure_collection_exists};
use crate::error::VectorDBError as Error;
use crate::repo_helpers::git_utils::create_fetch_options;
use crate::repo_helpers::qdrant_utils::get_collection_name;
use git2::{Repository, FetchOptions, Reference, BranchType, AutotagOption, ErrorCode};
use anyhow::anyhow;
use std::process::{Command, Stdio};

pub async fn update_sync_status_and_languages<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    config: &mut AppConfig,
    repo_config_index: usize,
    branch_name: &str,
    commit_oid_str: &str,
    client: &C,
    collection_name: &str,
) -> Result<(), Error> {
    let repo_config = config.repositories.get_mut(repo_config_index)
        .ok_or_else(|| Error::ConfigurationError(format!("Repository index {} out of bounds", repo_config_index)))?;
    log::debug!("Updating last synced commit for branch '{}' to {}", branch_name, commit_oid_str);
    repo_config.last_synced_commits.insert(branch_name.to_string(), commit_oid_str.to_string());
    log::debug!("Querying Qdrant for distinct languages in collection '{}' for branch '{}'", collection_name, branch_name);
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

pub async fn index_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: Arc<C>,
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
) -> Result<usize, Error>
{
    if relative_paths.is_empty() {
        info!("No files provided for indexing.");
        return Ok(0);
    }

    let embedding_handler = EmbeddingHandler::new(config)
        .context("Failed to initialize embedding handler for repo indexing")?;
    info!("Embedding dimension for repo: {}", embedding_handler.dimension()?);

    let embedding_handler_arc = Arc::new(embedding_handler);

    let pb = ProgressBar::new(relative_paths.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}, {eta})")
            .map_err(|e| Error::Other(e.to_string()))?
            .progress_chars("#=-"),
    );
    pb.enable_steady_tick(std::time::Duration::from_millis(100));

    index_repo_files(
        config,
        repo_root,
        relative_paths,
        collection_name,
        branch_name,
        commit_hash,
        client.clone(),
        embedding_handler_arc,
        Some(&pb),
        config.indexing.max_concurrent_upserts,
    ).await
}

pub async fn prepare_repository<C>(
    url: &str,
    name_opt: Option<&str>,
    local_path_opt: Option<&PathBuf>,
    branch_opt: Option<&str>,
    target_ref_opt: Option<&str>,
    remote_opt: Option<&str>,
    ssh_key_path_opt: Option<&PathBuf>,
    ssh_passphrase_opt: Option<&str>,
    base_path_for_new_clones: &Path,
    client: Arc<C>,
    embedding_dim: u64,
) -> Result<RepositoryConfig, Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    if url.is_empty() && (local_path_opt.is_none() || !local_path_opt.unwrap().exists()) {
        return Err(Error::Other("Either URL or existing local repository path must be provided".to_string()));
    }

    let repo_name = name_opt.unwrap_or_else(|| {
        url.split('/').last().unwrap_or("unknown_repo").trim_end_matches(".git")
    });
    let final_local_path = local_path_opt
        .cloned()
        .unwrap_or_else(|| base_path_for_new_clones.join(repo_name));
    let final_branch = branch_opt.unwrap_or("main"); // Default to main if not specified
    let final_remote = remote_opt.unwrap_or("origin"); // Default to origin if not specified

    let url_str = url.to_string(); // Clone url for closure
    let repo_name_str = repo_name.to_string(); // Clone repo_name for closure
    let collection_name = get_collection_name(&repo_name_str);

    let mut was_cloned = false;
    if !final_local_path.exists() {
        info!("Ensuring Qdrant collection '{collection_name}' exists (dim={embedding_dim})...");
        ensure_collection_exists(client.as_ref(), &collection_name, embedding_dim).await?;
        info!("Qdrant collection ensured for new clone.");

        info!("Cloning repository '{repo_name_str}' from {url_str} into {}...", final_local_path.display());
        let clone_status = Command::new("git")
            .arg("clone")
            .arg("--branch")
            .arg(final_branch)
            .arg(&url_str)
            .arg(&final_local_path)
            .stdout(Stdio::piped()) // Capture stdout
            .stderr(Stdio::piped()) // Capture stderr
            .spawn()
            .context("Failed to spawn git clone command")?
            .wait_with_output()
            .context("Failed to wait for git clone command")?;

        if clone_status.status.success() {
            was_cloned = true;
            let path_str = final_local_path.display().to_string();
            info!("Successfully cloned repository to {path_str}.");
            let stdout = String::from_utf8_lossy(&clone_status.stdout);
            let stderr = String::from_utf8_lossy(&clone_status.stderr);
            if !stdout.is_empty() {
                debug!("git clone stdout:\n{}", stdout);
            }
            if !stderr.is_empty() {
                info!("git clone stderr:\n{}", stderr);
            }
        } else {
            let stderr_cow = String::from_utf8_lossy(&clone_status.stderr);
            let stderr = stderr_cow.as_ref();
            error!("Failed to clone repository: {stderr}");
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
            return Err(Error::GitMessageError(format!("Git clone command failed: {}", stderr)));
        }
    } else {
        let path_str = final_local_path.display().to_string();
        info!("Repository already exists locally at {path_str}, ensuring collection exists...");
        info!("Ensuring Qdrant collection '{collection_name}' exists (dim={embedding_dim}) for existing clone...");
        ensure_collection_exists(client.as_ref(), &collection_name, embedding_dim).await?;
        info!("Qdrant collection ensured for existing clone.");
    }

    // --- Handle target_ref --- 
    let final_active_branch: String;
    if let Some(target_ref) = target_ref_opt {
        info!("Attempting to checkout target ref '{}' for repository '{}'...", target_ref, repo_name);
        
        // Fetch before checkout to ensure the ref is available locally, especially if it's a remote branch/tag
        // Don't prune here, might remove the ref we want if it's only remote
        let fetch_status = Command::new("git")
            .current_dir(&final_local_path)
            .arg("fetch")
            .arg(final_remote)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to spawn git fetch before checkout for {}", repo_name))?
            .wait_with_output()
            .context(format!("Failed to wait for git fetch before checkout for {}", repo_name))?;
        
        if !fetch_status.status.success() {
            let stderr = String::from_utf8_lossy(&fetch_status.stderr);
            warn!("Git fetch before checkout failed for {}: {}. Checkout might still succeed if ref is local.", repo_name, stderr);
        } else {
            info!("Git fetch before checkout successful for {}", repo_name);
        }

        // Now attempt checkout
        let checkout_status = Command::new("git")
            .current_dir(&final_local_path)
            .arg("checkout")
            .arg(target_ref)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .context(format!("Failed to spawn git checkout {} for {}", target_ref, repo_name))?
            .wait_with_output()
            .context(format!("Failed to wait for git checkout {} for {}", target_ref, repo_name))?;

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
             // If we cloned the repo just now and checkout failed, clean up.
            if was_cloned {
                warn!("Attempting to remove repository directory {} due to failed checkout of target ref.", final_local_path.display());
                if let Err(e) = std::fs::remove_dir_all(&final_local_path) {
                     error!("Failed to remove directory {} after failed checkout: {}", final_local_path.display(), e);
                }
            }
            return Err(Error::GitMessageError(format!(
                "Failed to checkout target ref '{}': {}",
                target_ref,
                stderr
            )));
        }
    } else {
        // No target_ref specified, use the initially cloned/existing branch
        final_active_branch = final_branch.to_string();
    }

    Ok(RepositoryConfig {
        name: repo_name.to_string(),
        url: url_str,
        local_path: final_local_path,
        default_branch: final_branch.to_string(),
        tracked_branches: vec![final_branch.to_string()],
        active_branch: Some(final_active_branch),
        remote_name: Some(final_remote.to_string()),
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
        ssh_key_path: ssh_key_path_opt.cloned(),
        ssh_key_passphrase: ssh_passphrase_opt.map(String::from),
        added_as_local_path: local_path_opt.is_some(),
        target_ref: target_ref_opt.map(|s| s.to_string()),
    })
}

pub async fn delete_repository_data<C>(
    repo_config: &RepositoryConfig,
    client: Arc<C>,
) -> Result<(), Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = &repo_config.name;
    let collection_name = get_collection_name(repo_name);
    info!("Attempting to delete Qdrant collection '{collection_name}'...");
    match client.delete_collection(collection_name.clone()).await {
        Ok(deleted) => {
            if deleted {
                info!("Successfully deleted Qdrant collection '{collection_name}'.");
            } else {
                info!("Qdrant collection '{collection_name}' did not exist or was already deleted.");
            }
        }
        Err(e) => {
            let error_str = e.to_string();
            warn!("Failed to delete Qdrant collection '{collection_name}': {error_str}. Continuing removal process.");
        }
    }

    let local_path = &repo_config.local_path;
    if !local_path.exists() {
        info!("Local directory '{}' does not exist. Skipping removal.", local_path.display());
        return Ok(());
    }
    let path_str = local_path.to_string_lossy();
    if path_str.len() < 10 { 
        error!("Path '{path_str}' is suspiciously short. Skipping removal for safety.");
        return Ok(());
    }
    let git_dir = local_path.join(".git");
    if !git_dir.exists() || !git_dir.is_dir() {
        warn!("No .git directory found at '{}'. This may not be a git repository. Skipping removal for safety.", local_path.display());
        return Ok(());
    }
    let dangerous_paths = ["/", "/home", "/usr", "/bin", "/sbin", "/etc", "/var", "/tmp", "/opt", "/boot", "/lib", "/dev", "/proc", "/sys", "/run"];
    if dangerous_paths.iter().any(|p| path_str == *p || path_str.starts_with(&format!("{}/", p))) {
        error!("Path '{path_str}' appears to be a system directory. Refusing to delete for safety.");
        return Ok(());
    }
    let is_in_repos_dir = path_str.contains("/repositories/") || path_str.contains("/vectordb-cli/") || path_str.contains("/repos/");
    if !is_in_repos_dir {
        warn!("Repository path '{path_str}' doesn't appear to be in a standard repositories directory. Skipping automatic removal for safety.");
        warn!("If you want to delete this directory, please do so manually.");
        return Ok(());
    }
    info!("Removing local repository directory '{}'...", local_path.display());
    match std::fs::remove_dir_all(local_path) {
        Ok(()) => info!("Successfully removed local repository directory '{}'.", local_path.display()),
        Err(e) => {
             let error_str = e.to_string();
            error!("Failed to remove local repository directory '{}': {error_str}. Manual cleanup may be required.", local_path.display());
        }
    }

    Ok(())
}

/// Represents basic information about a Git commit.
#[derive(Debug, Clone)]
pub struct CommitInfo {
    pub oid: git2::Oid,
    pub summary: String,
}

/// Synchronizes a specific repository based on its configuration.
///
/// - If `target_ref` is set in the `RepositoryConfig`, this function will:
///   1. Check out the specified static ref (tag, commit, branch).
///   2. Determine the commit hash for that ref.
///   3. Return the commit hash without fetching updates from the remote.
///   (The caller is then responsible for indexing based on this static ref).
/// - If `target_ref` is *not* set, this function will:
///   1. Fetch updates from the remote for the repository's active branch.
///   2. Merge the changes (fast-forward or create merge commit).
///   3. Return the new HEAD commit hash after the merge.
///   (The caller is then responsible for indexing the updated files).
///
/// Note: The actual indexing (`index_files`) is typically called *after* this function
/// by the primary handler (e.g., in MCP server or CLI command) based on the returned commit hash.
pub async fn sync_repository_branch(
    config: &AppConfig,
    repo_config_index: usize,
    _client: Arc<impl QdrantClientTrait + Send + Sync + 'static>, // Keep client for future use maybe
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
            .context(format!("Failed to execute git rev-parse for target ref {}", target_ref))?;

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
        .arg(format!("refs/heads/{}", branch_name)) // Target local branch ref
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context(format!("Failed to execute git rev-parse for local {}", branch_name))?;

    let local_commit_hash = if local_commit_output.status.success() {
        String::from_utf8(local_commit_output.stdout)?.trim().to_string()
    } else {
        let stderr_cow = String::from_utf8_lossy(&local_commit_output.stderr);
        let stderr = stderr_cow.as_ref();
        if stderr.contains("unknown revision or path not in the working tree") {
            warn!("Local branch {branch_name} not found in {repo_name}. It might have been deleted or not checked out.");
            return Ok(format!("Local branch {} not found.", branch_name));
        } else {
            error!("Failed to get local commit hash for {repo_name}/{branch_name}: {stderr}");
            return Err(anyhow!("Failed to get local commit hash for {}/{}: {}", repo_config.name, branch_name, stderr));
        }
    };

    // 3. Get remote commit hash
    let remote_ref = format!("refs/remotes/{}/{}", remote_name, branch_name);
    let remote_commit_output = Command::new("git")
        .current_dir(repo_path)
        .arg("rev-parse")
        .arg(&remote_ref)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .context(format!("Failed to execute git rev-parse for remote {}", remote_ref))?;

    let remote_commit_hash = if remote_commit_output.status.success() {
        String::from_utf8(remote_commit_output.stdout)?.trim().to_string()
    } else {
        let stderr_cow = String::from_utf8_lossy(&remote_commit_output.stderr);
        let stderr = stderr_cow.as_ref();
        if stderr.contains("unknown revision or path not in the working tree") {
            info!("Remote branch {remote_ref} not found for {repo_name} after fetch (likely deleted).");
            return Ok(format!("Remote branch {}/{} not found after fetch.", remote_name, branch_name));
        } else {
            error!("Failed to get remote commit hash for {repo_name}/{remote_ref} even though ref exists: {stderr}");
            return Err(anyhow!("Failed to get remote commit hash for {}/{}: {}", repo_config.name, remote_ref, stderr));
        }
    };

    // 4. Compare local and remote hashes
    if local_commit_hash == remote_commit_hash {
        info!("Branch {branch_name} already up-to-date for {repo_name}.");
        return Ok(format!("Branch {} already up-to-date.", branch_name));
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
            .context(format!("Failed to execute git merge --ff-only for {}", branch_name))?;

        if !merge_output.status.success() {
            let stderr_cow = String::from_utf8_lossy(&merge_output.stderr);
            let stderr = stderr_cow.as_ref();
            error!("Fast-forward merge failed for {repo_name}/{branch_name}: {stderr}");
            return Err(anyhow!("Fast-forward merge failed for {}/{}: {}", repo_config.name, branch_name, stderr));
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
        Ok(format!("Local branch {} is ahead of remote.", branch_name))
    } else {
        // Local and remote have diverged
        warn!("Local branch {branch_name} ({local_commit_hash}) and remote {remote_commit_hash} ({remote_commit_hash}) have diverged (base: {merge_base_hash}) for {repo_name}. Manual merge required.");
        Ok(format!("Branch {} has diverged from remote. Manual merge required.", branch_name))
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
        .context(format!("Failed to execute git checkout {}", branch_name))?;

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