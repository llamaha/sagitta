// Repository indexing and syncing functions from repo_helpers.rs will be moved here. 

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::collections::HashSet;
use anyhow::{Context, Result};
use log::{info, warn, error};
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use uuid::Uuid;
use qdrant_client::qdrant::{PointStruct, Filter, Condition, ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector, PointId};
use qdrant_client::Payload;
use crate::constants::{BATCH_SIZE, FIELD_BRANCH, FIELD_CHUNK_CONTENT, FIELD_COMMIT_HASH, FIELD_ELEMENT_TYPE, FIELD_END_LINE, FIELD_FILE_EXTENSION, FIELD_FILE_PATH, FIELD_LANGUAGE, FIELD_START_LINE, MAX_FILE_SIZE_BYTES};
use crate::error::VectorDBError as Error;
use crate::config::{AppConfig, RepositoryConfig};
use crate::embedding::EmbeddingHandler;
use crate::syntax;
use crate::QdrantClientTrait;
use crate::repo_helpers::qdrant_utils::{custom_upsert_batch, ensure_repository_collection_exists};
use std::sync::atomic::{AtomicUsize, Ordering};
use rayon::prelude::*;
use std::cell::RefCell;
use std::thread_local;

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

fn prewarm_thread_local_models(config: &AppConfig) {
    let n_threads = rayon::current_num_threads();
    (0..n_threads).into_par_iter().for_each(|_| {
        THREAD_EMBEDDING_HANDLER.with(|handler_cell| {
            let mut handler_opt = handler_cell.borrow_mut();
            if handler_opt.is_none() {
                *handler_opt = Some(EmbeddingHandler::new(config).unwrap());
            }
        });
    });
}

thread_local! {
    static THREAD_EMBEDDING_HANDLER: RefCell<Option<EmbeddingHandler>> = RefCell::new(None);
}

fn process_files_parallel(
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    branch_name: &str,
    commit_hash: &str,
    pb: &ProgressBar,
) -> Vec<Result<(Vec<PointStruct>, PathBuf), String>> {
    relative_paths.par_iter().map(|relative_path| {
        let full_path = repo_root.join(relative_path);
        let result = THREAD_EMBEDDING_HANDLER.with(|handler_cell| {
            let mut handler_opt = handler_cell.borrow_mut();
            if handler_opt.is_none() {
                *handler_opt = Some(EmbeddingHandler::new(config).map_err(|e| format!("Failed to initialize embedding handler: {}", e))?);
            }
            let handler = handler_opt.as_mut().unwrap();
            // File size check
            match std::fs::metadata(&full_path) {
                Ok(metadata) => {
                    if metadata.len() > MAX_FILE_SIZE_BYTES {
                        Err(format!(
                            "Skipping file larger than {} bytes: {}",
                            MAX_FILE_SIZE_BYTES,
                            full_path.display()
                        ))
                    } else {
                        if !full_path.is_file() {
                            Err(format!("Skipping non-file path found during indexing: {}", full_path.display()))
                        } else {
                            // Parse and embed
                            match crate::syntax::get_chunks(&full_path) {
                                Ok(chunks) => {
                                    if chunks.is_empty() {
                                        Ok((Vec::new(), relative_path.clone()))
                                    } else {
                                        let file_path_str = relative_path.to_string_lossy().to_string();
                                        let file_extension = relative_path.extension()
                                            .unwrap_or_default()
                                            .to_string_lossy()
                                            .to_string();
                                        let mut points = Vec::new();
                                        for chunk in &chunks {
                                            let chunk_content = &chunk.content;
                                            let embedding = match handler.embed(&[chunk_content]) {
                                                Ok(mut result) => {
                                                    if result.is_empty() {
                                                        continue;
                                                    }
                                                    result.remove(0)
                                                }
                                                Err(_) => {
                                                    continue;
                                                }
                                            };
                                            let point_id_uuid = Uuid::new_v4().to_string();
                                            let mut payload = Payload::new();
                                            payload.insert(FIELD_FILE_PATH, file_path_str.clone());
                                            payload.insert(FIELD_START_LINE, chunk.start_line as i64);
                                            payload.insert(FIELD_END_LINE, chunk.end_line as i64);
                                            payload.insert(FIELD_LANGUAGE, chunk.language.clone());
                                            payload.insert(FIELD_CHUNK_CONTENT, chunk.content.clone());
                                            payload.insert(FIELD_BRANCH, branch_name.to_string());
                                            payload.insert(FIELD_COMMIT_HASH, commit_hash.to_string());
                                            payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.to_string());
                                            payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
                                            points.push(PointStruct::new(point_id_uuid, embedding, payload));
                                        }
                                        Ok((points, relative_path.clone()))
                                    }
                                }
                                Err(e) => Err(format!("Failed to parse file {}: {}", full_path.display(), e)),
                            }
                        }
                    }
                }
                Err(e) => {
                    Err(format!(
                        "Failed to get metadata for file {}: {}. Skipping file.",
                        full_path.display(), e
                    ))
                }
            }
        });
        pb.inc(1);
        result
    }).collect()
}

pub async fn index_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: &C,
    onnx_model_path_opt: Option<String>,
    onnx_tokenizer_path_opt: Option<String>,
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
) -> Result<(), Error>
{
    if relative_paths.is_empty() {
        return Ok(());
    }
    prewarm_thread_local_models(config);
    let pb = ProgressBar::new(relative_paths.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}, {eta})")
            .map_err(|e| Error::Other(e.to_string()))?
            .progress_chars("#=-"),
    );
    let mut points_batch = Vec::new();
    let mut errors = Vec::new();
    let results = process_files_parallel(config, repo_root, relative_paths, branch_name, commit_hash, &pb);
    for result in results {
        match result {
            Ok((points, _relative_path)) => {
                for point in points {
                    points_batch.push(point);
                    if points_batch.len() >= BATCH_SIZE {
                        crate::repo_helpers::qdrant_utils::custom_upsert_batch(client, collection_name, points_batch.clone(), &pb).await?;
                        points_batch.clear();
                    }
                }
            }
            Err(e) => {
                errors.push(e);
            }
        }
    }
    if !points_batch.is_empty() {
        crate::repo_helpers::qdrant_utils::custom_upsert_batch(client, collection_name, points_batch, &pb).await?;
    }
    pb.finish_with_message("Indexing complete");
    if !errors.is_empty() {
        for e in errors { log::error!("[index_files] {}", e); }
    }
    Ok(())
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
    embedding_dim: u64, // Pass dimension instead of full handler
) -> Result<RepositoryConfig, Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    // Require either URL or existing local path
    if url.is_empty() && (local_path_opt.is_none() || !local_path_opt.unwrap().exists()) {
        return Err(Error::Other("Either URL or existing local repository path must be provided".to_string()));
    }

    // Handle repository name
    let repo_name = match name_opt {
        Some(name) => name.to_string(),
        None => {
            if !url.is_empty() {
                // If URL is provided, derive name from URL
                PathBuf::from(url)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.trim_end_matches(".git").to_string())
                    .ok_or_else(|| Error::Other("Could not derive repository name from URL".to_string()))?
            } else {
                // If URL is not provided, derive name from local path directory name
                local_path_opt.unwrap() // Safe because we checked is_none() above
                    .file_name()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string())
                    .ok_or_else(|| Error::Other("Could not derive repository name from local path".to_string()))?
            }
        },
    };

    let local_path = local_path_opt.map_or_else(|| base_path_for_new_clones.join(&repo_name), |p| p.clone());

    // Handle URL extraction from existing repository
    let mut final_url = url.to_string();

    // Check if repository path exists
    if local_path.exists() {
        info!("Repository directory '{}' already exists.", local_path.display());
        
        // Attempt to open the repository
        let repo = git2::Repository::open(&local_path)
            .with_context(|| format!("Failed to open existing repository at {}", local_path.display()))?;
        
        // If URL is empty, try to extract it from the repository's remote
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
        
        // Determine branch
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
        
        // Setup Collection
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
    
    // For new clones, URL is required and must not be empty
    if final_url.is_empty() {
        return Err(Error::Other("URL is required when adding a new repository (local directory doesn't exist).".to_string()));
    }
    
    // Clone repository
    info!("Cloning repository '{}' from {}", repo_name, final_url);
    // Create the directory first
    std::fs::create_dir_all(&local_path)
        .with_context(|| format!("Failed to create directory at {}", local_path.display()))?;

    // Execute clone using std::process::Command
    let mut cmd = std::process::Command::new("git");
    cmd.arg("clone").arg(&final_url).arg(&local_path);
    if let Some(branch) = branch_opt {
        cmd.arg("-b").arg(branch);
    }

    // Setup SSH command if keys provided
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
    // Open the repo to determine the initial branch if not specified
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
) -> Result<(), Error> {
    let repo_config = config.repositories.get(repo_config_index)
        .ok_or_else(|| Error::ConfigurationError(format!("Repository index {} out of bounds", repo_config_index)))?;

    // Initialize embedding handler
    let _embedding_handler = EmbeddingHandler::new(config)
        .map_err(|e: Error| Error::Other(format!("Failed to initialize embedding handler for sync: {}", e)))?;

    let _repo_root = PathBuf::from(&repo_config.local_path);

    // TODO: Implement the full sync logic here as needed for your application.
    Ok(())
} 