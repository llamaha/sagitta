// Repository indexing and syncing functions from repo_helpers.rs will be moved here. 

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashSet;
use crate::config::RepositoryConfig;
use anyhow::{Context, Result};
use log::{info, warn, error};
use indicatif::{ProgressBar, ProgressStyle};
use uuid::Uuid;
use qdrant_client::qdrant::{PointStruct, Filter, Condition, ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector, PointId};
use crate::constants::{BATCH_SIZE, FIELD_BRANCH, FIELD_CHUNK_CONTENT, FIELD_COMMIT_HASH, FIELD_ELEMENT_TYPE, FIELD_END_LINE, FIELD_FILE_EXTENSION, FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE, MAX_FILE_SIZE_BYTES};
use crate::config::AppConfig;
use crate::embedding::EmbeddingHandler;
use crate::QdrantClientTrait;
use crate::repo_helpers::qdrant_utils::{ensure_repository_collection_exists};
use crate::indexing::{self, index_repo_files};
use crate::error::VectorDBError as Error;

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
                 log::error!("Failed to scroll points for distinct languages from Qdrant for collection '{}', branch '{}': {}. Language list in config may be incomplete.",
                    collection_name, branch_name, e);
                 repo_config.indexed_languages = None;
                 return Ok(());
            }
        }
    }
    log::info!("Found indexed languages for branch '{}': {:?}", branch_name, languages);
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
        log::info!("No files provided for indexing.");
        return Ok(0);
    }

    let embedding_handler = EmbeddingHandler::new(config)
        .context("Failed to initialize embedding handler for repo indexing")?;
    log::info!("Embedding dimension for repo: {}", embedding_handler.dimension()?);

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

    let repo_name = match name_opt {
        Some(name) => name.to_string(),
        None => {
            if !url.is_empty() {
                PathBuf::from(url)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_end_matches(".git").to_string())
                    .ok_or_else(|| Error::Other("Could not derive repository name from URL".to_string()))?
            } else {
                local_path_opt.unwrap()
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::Other("Could not derive repository name from local path".to_string()))?
            }
        },
    };

    let local_path = local_path_opt.map_or_else(|| base_path_for_new_clones.join(&repo_name), |p| p.clone());

    let mut final_url = url.to_string();

    if local_path.exists() {
        info!("Repository directory '{}' already exists.", local_path.display());
        
        let repo = git2::Repository::open(&local_path)
            .with_context(|| format!("Failed to open existing repository at {}", local_path.display()))?;
        
        if final_url.is_empty() {
            let remote_name = remote_opt.unwrap_or("origin");
            match repo.find_remote(remote_name) {
                Ok(remote) => {
                    if let Some(url) = remote.url() {
                        info!("Found remote URL for '{}': {}", remote_name, url);
                        final_url = url.to_string();
                    } else {
                        return Err(Error::Other(format!("Remote '{}' exists but has no URL.", remote_name)));
                    }
                }
                Err(_) => {
                    return Err(Error::Other(format!("Could not find remote '{}' in existing repository.", remote_name)));
                }
            }
        }
        
        let initial_branch_name = match branch_opt {
            Some(branch_name) => branch_name.to_string(),
            None => {
                let head_ref = repo.find_reference("HEAD")?;
                let head_ref_resolved = head_ref.resolve()?;
                head_ref_resolved.shorthand()
                    .ok_or_else(|| Error::Other("Could not determine default branch name from HEAD".to_string()))?
                    .to_string()
            }
        };
        
        let collection_name = format!("repo_{}", repo_name);
        info!("Ensuring Qdrant collection '{}' exists (dim={})...", collection_name, embedding_dim);
        if let Err(e) = ensure_repository_collection_exists(client.as_ref(), &collection_name, embedding_dim).await {
            match e {
                Error::QdrantError(qe) => {
                    return Err(Error::Other(format!("Failed to ensure collection '{}' exists: {}", collection_name, qe.to_string())))
                },
                _ => {
                    return Err(Error::Other(format!("Failed to ensure collection '{}' exists: {}", collection_name, e.to_string())))
                }
            }
        }
        info!("Qdrant collection ensured for existing repository.");
        
        let new_repo_config = RepositoryConfig {
            name: repo_name.clone(),
            url: final_url,
            local_path: local_path.clone(),
            default_branch: initial_branch_name.clone(),
            tracked_branches: vec![initial_branch_name.clone()],
            active_branch: Some(initial_branch_name.clone()),
            remote_name: Some(remote_opt.unwrap_or("origin").to_string()),
            ssh_key_path: ssh_key_path_opt.cloned(),
            ssh_key_passphrase: ssh_passphrase_opt.map(String::from),
            last_synced_commits: std::collections::HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
        };
        
        return Ok(new_repo_config);
    }
    
    if final_url.is_empty() {
        return Err(Error::Other("URL is required when adding a new repository (local directory doesn't exist).".to_string()));
    }
    
    info!("Cloning repository '{}' from {}", repo_name, final_url);
    std::fs::create_dir_all(&local_path)
        .with_context(|| format!("Failed to create directory at {}", local_path.display()))?;

    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg(&final_url).arg(&local_path);
    if let Some(branch) = branch_opt {
        cmd.arg("-b").arg(branch);
    }

    let git_ssh_command = if let Some(key_path) = ssh_key_path_opt {
        let key_path_str = key_path.to_string_lossy();
        let base_ssh_cmd = format!("ssh -i {} -o IdentitiesOnly=yes -o StrictHostKeyChecking=accept-new", key_path_str);
        if ssh_passphrase_opt.is_some() {
            warn!("SSH key passphrase provided but direct use in GIT_SSH_COMMAND is insecure/unsupported. Relying on ssh-agent.");
        }
        Some(base_ssh_cmd)
    } else {
        None
    };

    if let Some(ssh_cmd) = git_ssh_command {
        cmd.env("GIT_SSH_COMMAND", ssh_cmd);
    }

    let clone_output = cmd.output().context("Failed to execute git clone command")?;

    if !clone_output.status.success() {
        let stderr = String::from_utf8_lossy(&clone_output.stderr);
        log::error!("Git clone command failed with status: {}. Stderr:\n{}", clone_output.status, stderr);
        return Err(Error::Other(format!(
            "Git clone command failed with status: {}. Stderr: {}",
            clone_output.status,
            stderr
        )));
    }
    
    info!("Repository cloned successfully to {}", local_path.display());
    let repo = git2::Repository::open(&local_path)
            .with_context(|| format!("Failed to open newly cloned repository at {}", local_path.display()))?;

    let initial_branch_name = match branch_opt {
        Some(b) => b.to_string(),
        None => {
            let head = repo.head().context("Failed to get HEAD reference after clone")?;
            head.shorthand()
                .ok_or_else(|| Error::Other("Could not determine default branch name from HEAD".to_string()))?
                .to_string()
        }
    };
    log::info!("Default branch determined as: {}", initial_branch_name);

    let collection_name = format!("repo_{}", repo_name);
    info!("Ensuring Qdrant collection '{}' exists (dim={})...", collection_name, embedding_dim);
    if let Err(e) = ensure_repository_collection_exists(client.as_ref(), &collection_name, embedding_dim).await {
        match e {
            Error::QdrantError(qe) => {
                return Err(Error::Other(format!("Failed to ensure collection '{}' exists: {}", collection_name, qe.to_string())))
            },
            _ => {
                return Err(Error::Other(format!("Failed to ensure collection '{}' exists: {}", collection_name, e.to_string())))
            }
        }
    }
    info!("Qdrant collection ensured.");

    let new_repo_config = RepositoryConfig {
        name: repo_name.clone(),
        url: final_url,
        local_path: local_path.clone(),
        default_branch: initial_branch_name.clone(),
        tracked_branches: vec![initial_branch_name.clone()],
        active_branch: Some(initial_branch_name.clone()),
        remote_name: Some(remote_opt.unwrap_or("origin").to_string()),
        ssh_key_path: ssh_key_path_opt.cloned(),
        ssh_key_passphrase: ssh_passphrase_opt.map(String::from),
        last_synced_commits: std::collections::HashMap::new(),
        indexed_languages: None,
        added_as_local_path: false,
    };

    Ok(new_repo_config)
}

pub async fn delete_repository_data<C>(
    repo_config: &RepositoryConfig,
    client: Arc<C>,
) -> Result<(), Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = &repo_config.name;
    let collection_name = format!("repo_{}", repo_name);
    info!("Attempting to delete Qdrant collection '{}'...", collection_name);
    match client.delete_collection(collection_name.clone()).await {
        Ok(deleted) => {
            if deleted {
                info!("Successfully deleted Qdrant collection '{}'.", collection_name);
            } else {
                info!("Qdrant collection '{}' did not exist or was already deleted.", collection_name);
            }
        }
        Err(e) => {
            warn!("Failed to delete Qdrant collection '{}': {}. Continuing removal process.", collection_name, e);
        }
    }

    let local_path = &repo_config.local_path;
    if !local_path.exists() {
        info!("Local directory '{}' does not exist. Skipping removal.", local_path.display());
        return Ok(());
    }
    let path_str = local_path.to_string_lossy();
    if path_str.len() < 10 { 
        error!("Path '{}' is suspiciously short. Skipping removal for safety.", path_str);
        return Ok(());
    }
    let git_dir = local_path.join(".git");
    if !git_dir.exists() || !git_dir.is_dir() {
        warn!("No .git directory found at '{}'. This may not be a git repository. Skipping removal for safety.", local_path.display());
        return Ok(());
    }
    let dangerous_paths = ["/", "/home", "/usr", "/bin", "/sbin", "/etc", "/var", "/tmp", "/opt", "/boot", "/lib", "/dev", "/proc", "/sys", "/run"];
    if dangerous_paths.iter().any(|p| path_str == *p || path_str.starts_with(&format!("{}/", p))) {
        error!("Path '{}' appears to be a system directory. Refusing to delete for safety.", path_str);
        return Ok(());
    }
    let is_in_repos_dir = path_str.contains("/repositories/") || path_str.contains("/vectordb-cli/") || path_str.contains("/repos/");
    if !is_in_repos_dir {
        warn!("Repository path '{}' doesn't appear to be in a standard repositories directory. Skipping automatic removal for safety.", path_str);
        warn!("If you want to delete this directory, please do so manually.");
        return Ok(());
    }
    info!("Attempting to remove local clone at {}...", local_path.display());
    match std::fs::remove_dir_all(local_path) {
        Ok(_) => info!("Successfully removed local directory '{}'.", local_path.display()),
        Err(e) => {
             error!("Failed to remove local directory '{}': {}. Please remove it manually.", local_path.display(), e);
             warn!("Failed to remove local directory '{}'. Please remove it manually.", local_path.display());
        }
    }

    Ok(())
}

pub async fn sync_repository_branch(
    config: &AppConfig,
    repo_config_index: usize,
    _client: Arc<impl QdrantClientTrait + Send + Sync + 'static>,
    _fetch_and_merge: bool,
) -> Result<String, Error> {
    let repo_config = config.repositories.get(repo_config_index)
        .ok_or_else(|| Error::ConfigurationError(format!("Repository index {} out of bounds", repo_config_index)))?;

    let _embedding_handler = EmbeddingHandler::new(config)
        .map_err(|e: Error| Error::Other(format!("Failed to initialize embedding handler for sync: {}", e)))?;

    let _repo_root = PathBuf::from(&repo_config.local_path);

    // TODO: Implement the full sync logic here as needed for your application.
    // For now, return a placeholder commit hash to satisfy the type checker.
    Ok("placeholder_commit_hash".to_string())
} 