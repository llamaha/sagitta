use anyhow::{Context, Result};
use colored::*;
use git2::{Repository, CredentialType, FetchOptions, RemoteCallbacks, Cred};
use qdrant_client::qdrant::{Filter, PointId, Condition, PointStruct, PointsSelector, points_selector::PointsSelectorOneOf, PointsIdsList, DeletePointsBuilder, ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector, UpsertPointsBuilder, UpdateStatus};
use qdrant_client::Payload;
use std::collections::{HashSet, HashMap};
use std::path::{PathBuf, Path};
use std::sync::Arc;
use std::time::Duration;
use indicatif::{ProgressBar, ProgressStyle};
use log::{self, info, warn, error};
use uuid::Uuid;
use std::fs;

use crate::cli::commands::{CliArgs, FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, FIELD_LANGUAGE, FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_FILE_EXTENSION, BATCH_SIZE, FIELD_BRANCH, FIELD_COMMIT_HASH};
use crate::config::{AppConfig, RepositoryConfig};
use crate::vectordb::embedding_logic::{EmbeddingHandler};
use crate::vectordb::error::VectorDBError;
use crate::vectordb::qdrant_client_trait::QdrantClientTrait;
use crate::syntax;

// Use a type alias for VectorDBError
type Error = VectorDBError;

const COLLECTION_NAME_PREFIX: &str = "repo_";
pub(crate) const DEFAULT_VECTOR_DIMENSION: u64 = 384;
const MAX_FILE_SIZE_BYTES: u64 = 250 * 1024; // 250 KB limit

/// Helper function to check if a file extension is explicitly supported by a parser
pub(crate) fn is_supported_extension(extension: &str) -> bool {
    matches!(extension.to_lowercase().as_str(), 
        "rs" | "rb" | "go" | "js" | "jsx" | "ts" | "tsx" | "yaml" | "yml" | "md" | "mdx" | "py"
    )
}

/// Helper to create FetchOptions with SSH credential callback
pub(crate) fn create_fetch_options<'a>(
    repo_configs: Vec<RepositoryConfig>,
    repo_url: &'a str,
    ssh_key_path: Option<&'a PathBuf>,
    ssh_key_passphrase: Option<&'a str>,
) -> Result<FetchOptions<'a>> {
    let mut callbacks = RemoteCallbacks::new();
    let relevant_repo_config = repo_configs.iter()
        .find(|r| r.url == repo_url)
        .cloned();
        
    // Check if running in server mode (no interactive prompts allowed)
    let is_server_mode = false;
    
    // Is this an SSH URL? (starts with git@ or ssh://)
    let is_ssh_url = repo_url.starts_with("git@") || repo_url.starts_with("ssh://");
    
    callbacks.credentials(move |_url, username_from_git, allowed_types| {
        log::debug!("Credential callback triggered. URL: {}, Username: {:?}, Allowed: {:?}", _url, username_from_git, allowed_types);
        
        // In server mode, immediately fail for SSH URLs without explicit credentials
        if is_server_mode && is_ssh_url && ssh_key_path.is_none() && 
           !relevant_repo_config.as_ref().and_then(|r| r.ssh_key_path.as_ref()).is_some() {
            log::error!("Server mode detected with SSH URL '{}' but no SSH key configured. Use HTTPS URLs or configure SSH keys explicitly.", _url);
            return Err(git2::Error::from_str("Server mode cannot use interactive authentication. Use HTTPS URLs or configure SSH keys explicitly."));
        }
        
        // First check direct SSH key parameters (for new repositories)
        if allowed_types.contains(CredentialType::SSH_KEY) && ssh_key_path.is_some() {
            let user = username_from_git.unwrap_or("git");
            let key_path = ssh_key_path.unwrap();
            log::debug!("Attempting SSH key authentication from direct parameters. User: '{}', Key Path: {}", user, key_path.display());
            match Cred::ssh_key(user, None, key_path, ssh_key_passphrase) {
                Ok(cred) => {
                    log::info!("SSH key credential created successfully from direct parameters for user '{}'.", user);
                    return Ok(cred);
                }
                Err(e) => {
                    log::error!("Failed to create SSH key credential from direct parameter path {}: {}", key_path.display(), e);
                }
            }
        }
        
        // Then check repository config (for existing repositories)
        if let Some(repo_config) = &relevant_repo_config {
            if allowed_types.contains(CredentialType::SSH_KEY) {
                if let Some(key_path) = &repo_config.ssh_key_path {
                    let user = username_from_git.unwrap_or("git");
                    log::debug!("Attempting SSH key authentication from repo config. User: '{}', Key Path: {}", user, key_path.display());
                    match Cred::ssh_key(user, None, key_path, repo_config.ssh_key_passphrase.as_deref()) {
                        Ok(cred) => {
                            log::info!("SSH key credential created successfully from repo config for user '{}'.", user);
                            return Ok(cred);
                        }
                        Err(e) => {
                            log::error!("Failed to create SSH key credential from repo config path {}: {}", key_path.display(), e);
                        }
                    }
                } else {
                    log::debug!("SSH key requested, but no ssh_key_path configured for repo '{}'", repo_config.name);
                }
            }
        } else {
            log::debug!("No repository configuration found for URL '{}' in credential callback.", _url);
        }
        
        // In server mode, don't try to use default credentials which might prompt for a password
        if is_server_mode && is_ssh_url {
            log::error!("No configured SSH credentials found for URL '{}' in server mode. Unable to authenticate.", _url);
            return Err(git2::Error::from_str("Server mode cannot use interactive authentication. Configure SSH keys explicitly."));
        }
        
        // Finally try default
        if allowed_types.contains(CredentialType::DEFAULT) {
            log::debug!("Attempting default system credentials.");
            match Cred::default() {
                Ok(cred) => {
                    log::info!("Using default system credentials.");
                    return Ok(cred);
                }
                Err(e) => {
                    log::warn!("Failed to get default system credentials: {}", e);
                }
            }
        }
        log::error!("No suitable credentials found or configured for URL '{}', user '{:?}'", _url, username_from_git);
        Err(git2::Error::from_str("Authentication failed: no suitable credentials found"))
    });
    let mut fetch_opts = FetchOptions::new();
    fetch_opts.remote_callbacks(callbacks);
    Ok(fetch_opts)
}


pub(crate) fn get_collection_name(repo_name: &str) -> String {
    format!("{}{}", COLLECTION_NAME_PREFIX, repo_name)
}


/// Perform a fast-forward merge if possible
pub(crate) fn merge_local_branch<'repo>(
    repo: &'repo Repository,
    branch_name: &str,
    target_commit: &git2::Commit<'repo>,
) -> Result<()> {
    log::debug!("Attempting merge for branch '{}' to commit {}", branch_name, target_commit.id());
    let branch_ref_name = format!("refs/heads/{}", branch_name);
    let mut branch_ref = repo.find_reference(&branch_ref_name)
        .with_context(|| format!("Failed to find local branch reference '{}'", branch_ref_name))?;
    let target_annotated_commit = repo.find_annotated_commit(target_commit.id())?;
    let analysis = repo.merge_analysis(&[&target_annotated_commit])?;
    if analysis.0.is_fast_forward() {
        log::info!("Branch '{}' can be fast-forwarded.", branch_name);
        branch_ref.set_target(target_commit.id(), "Fast-forward merge")
            .with_context(|| format!("Failed to fast-forward branch '{}'", branch_name))?;
        log::debug!("Branch '{}' successfully fast-forwarded to {}", branch_name, target_commit.id());
        repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
            .with_context(|| format!("Failed to checkout head after fast-forwarding branch '{}'", branch_name))?;
        log::debug!("Checked out head for branch '{}' after fast-forward.", branch_name);
    } else if analysis.0.is_up_to_date() {
        log::info!("Branch '{}' is already up-to-date with commit {}", branch_name, target_commit.id());
    } else {
        log::warn!("Branch '{}' cannot be fast-forwarded to {}. Merge commit needed (not implemented automatically).", branch_name, target_commit.id());
    }
    Ok(())
}


/// Recursively collect files from a Git tree
pub(crate) fn collect_files_from_tree(
    repo: &Repository,
    tree: &git2::Tree,
    file_list: &mut Vec<PathBuf>,
    current_path: &PathBuf,
) -> Result<()> {
    for entry in tree.iter() {
        let entry_path = current_path.join(entry.name().unwrap_or(""));
        match entry.kind() {
            Some(git2::ObjectType::Blob) => {
                if entry_path.extension().map_or(false, |ext| is_supported_extension(ext.to_str().unwrap_or(""))) {
                     file_list.push(entry_path);
                 } else {
                    log::trace!("Skipping non-supported file: {}", entry_path.display());
                 }
            }
            Some(git2::ObjectType::Tree) => {
                let subtree = repo.find_tree(entry.id())?;
                collect_files_from_tree(repo, &subtree, file_list, &entry_path)?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Update the config file with the last synced commit and detected languages
pub(crate) async fn update_sync_status_and_languages<
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


/// Deletes points associated with specific file paths from a Qdrant collection.
pub(crate) async fn delete_points_for_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: &C,
    collection_name: &str,
    branch_name: &str,
    relative_paths: &[PathBuf],
) -> Result<(), Error> {
    if relative_paths.is_empty() {
        log::debug!("No files provided for deletion in branch '{}'.", branch_name);
        return Ok(());
    }
    log::info!("Deleting points for {} files in branch '{}' from collection '{}'...",
        relative_paths.len(), branch_name, collection_name);
    let mut point_ids_to_delete: Vec<PointId> = Vec::new();
    let filter = Filter::must([
        Condition::matches(FIELD_BRANCH, branch_name.to_string()),
        Filter {
            should: 
                relative_paths.iter().map(|p| {
                    Condition::matches(FIELD_FILE_PATH, p.to_string_lossy().into_owned())
                }).collect::<Vec<_>>(),
            min_should: None,
            must: Vec::new(),
            must_not: Vec::new(),
        }.into()
    ]);
    let mut offset: Option<PointId> = None;
    loop {
        let mut builder = ScrollPointsBuilder::new(collection_name)
            .filter(filter.clone())
            .limit(1000)
            .with_payload(false)
            .with_vectors(false);

        if let Some(o) = offset {
            builder = builder.offset(o);
        }
        
        let scroll_request = builder.into();
        let scroll_result: ScrollResponse = client.scroll(scroll_request).await
            .with_context(|| format!("Failed to scroll points for deletion in collection '{}'", collection_name))?;
        if scroll_result.result.is_empty() {
            break;
        }
        for point in scroll_result.result {
            if let Some(id) = point.id {
                 point_ids_to_delete.push(id);
            } else {
                log::warn!("Found point without ID during scroll for deletion: {:?}", point);
            }
        }
        offset = scroll_result.next_page_offset;
        if offset.is_none() {
            break;
        }
    }
    if point_ids_to_delete.is_empty() {
        log::info!("No points found matching the files to be deleted in branch '{}'.", branch_name);
        return Ok(());
    }
    log::debug!("Found {} points to delete for branch '{}'.", point_ids_to_delete.len(), branch_name);
    for chunk in point_ids_to_delete.chunks(BATCH_SIZE) {
         let _points_selector = PointsSelector {
             points_selector_one_of: Some(PointsSelectorOneOf::Points(
                 PointsIdsList { ids: chunk.to_vec() }
             ))
         };
         let delete_request = DeletePointsBuilder::new(collection_name)
            .points(chunk.to_vec());
         client.delete_points(delete_request.into()).await
             .with_context(|| format!("Failed to delete a batch of points from collection '{}'", collection_name))?;
        log::debug!("Deleted batch of {} points for branch '{}'.", chunk.len(), branch_name);
    }
    log::info!("Successfully deleted {} points for {} files in branch '{}'.",
        point_ids_to_delete.len(), relative_paths.len(), branch_name);
    Ok(())
}

/// Custom implementation of upsert_batch for QdrantClientTrait
async fn custom_upsert_batch<C: QdrantClientTrait>(
    client: &C,
    collection_name: &str,
    points: Vec<PointStruct>,
    progress_bar: &ProgressBar,
) -> Result<(), Error> {
    let _total_points = points.len();
    let mut success_count = 0;
    let mut retry_count = 0;
    const MAX_RETRIES: u32 = 3;
    let mut backoff = Duration::from_millis(100);

    for chunk in points.chunks(BATCH_SIZE) {
        let chunk_len = chunk.len();
        let mut attempts = 0;
        loop {
            let request = UpsertPointsBuilder::new(collection_name, chunk.to_vec()).wait(true);
            match client.upsert_points(request.build()).await {
                Ok(response) => {
                    let status_code = response.result.map(|r| r.status).unwrap_or(-1); // Use -1 for unknown/missing
                    let qdrant_status = UpdateStatus::try_from(status_code);

                    match qdrant_status {
                        Ok(UpdateStatus::Completed) => {
                            success_count += chunk_len;
                            progress_bar.inc(chunk_len as u64);
                            break; // Success for this chunk
                        }
                        Ok(other_status) => {
                            // Handle other known statuses (Acknowledged, etc.) as warnings/failures
                            let msg = format!("Upsert resulted in status {:?}. Response: {:?}", other_status, response);
                            log::warn!("{}", msg);
                            if attempts >= MAX_RETRIES {
                                return Err(Error::Other(msg))
                            }
                        }
                        Err(_) => {
                            // Handle unknown status code
                            let msg = format!("Upsert resulted in unknown status code {}. Response: {:?}", status_code, response);
                            log::warn!("{}", msg);
                             if attempts >= MAX_RETRIES {
                                return Err(Error::Other(msg))
                            }
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("Error upserting points batch: {}", e);
                    log::warn!("{}", msg);
                    if attempts >= MAX_RETRIES {
                       return Err(Error::Other(format!("{}: {}", msg, e)))
                    }
                }
            }

            attempts += 1;
            retry_count += 1;
            log::warn!("Retrying batch (attempt {}/{}), waiting {:?}...", attempts, MAX_RETRIES, backoff);
            tokio::time::sleep(backoff).await;
            backoff = std::cmp::min(backoff * 2, Duration::from_secs(5)); // Exponential backoff capped at 5s
        }
    }
    log::info!("Upserted {} points ({} retries) into '{}'.", success_count, retry_count, collection_name);
    Ok(())
}

/// Indexes a list of files into the specified Qdrant collection.
pub async fn index_files<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: &C,
    cli_args: &CliArgs,
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
) -> Result<(), Error>
{
    if relative_paths.is_empty() {
        log::info!("No files provided for indexing in branch '{}'.", branch_name);
        return Ok(());
    }
    log::info!("Indexing {} files for branch '{}' (commit: {}) into collection '{}'...",
        relative_paths.len(), branch_name, &commit_hash[..7], collection_name);
    
    // Determine model and tokenizer paths using CLI -> Env -> Config priority
    let model_env_var = std::env::var("VECTORDB_ONNX_MODEL").ok();
    let tokenizer_env_var = std::env::var("VECTORDB_ONNX_TOKENIZER_DIR").ok();

    let onnx_model_path_str = cli_args.onnx_model_path_arg.as_deref()
        .or(model_env_var.as_deref())
        .or(config.onnx_model_path.as_deref())
        .ok_or_else(|| Error::Other("ONNX model path must be provided via --onnx-model, VECTORDB_ONNX_MODEL, or config".to_string()))?;
    
    let onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_deref()
        .or(tokenizer_env_var.as_deref())
        .or(config.onnx_tokenizer_path.as_deref())
        .ok_or_else(|| Error::Other("ONNX tokenizer path must be provided via --onnx-tokenizer-dir, VECTORDB_ONNX_TOKENIZER_DIR, or config".to_string()))?;

    let _model_path = PathBuf::from(onnx_model_path_str);
    let _tokenizer_path = PathBuf::from(onnx_tokenizer_dir_str);

    // Construct VdbConfig for the handler
    let embedding_handler = EmbeddingHandler::new(config)
        .map_err(|e| Error::Other(format!("Failed to initialize embedding handler: {}", e)))?;
    
    // Pre-warm the embedding provider cache to load the model upfront
    log::debug!("Pre-warming embedding provider cache...");
    let embedding_dim = embedding_handler.dimension()? as u64;
    log::debug!("Embedding provider cache warmed. Detected dimension: {}", embedding_dim);

    // Ensure collection exists with the correct embedding dimension
    ensure_repository_collection_exists(client, collection_name, embedding_dim).await?;

    let pb = ProgressBar::new(relative_paths.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}, {eta})")
            .map_err(|e| Error::Other(e.to_string()))?
            .progress_chars("#=-"),
    );
    let mut points_batch = Vec::new();
    for relative_path in relative_paths {
        let full_path = repo_root.join(relative_path);

        // --- Add File Size Check --- 
        match std::fs::metadata(&full_path) {
            Ok(metadata) => {
                if metadata.len() > MAX_FILE_SIZE_BYTES {
                    log::warn!(
                        "Skipping file larger than {} bytes: {}",
                        MAX_FILE_SIZE_BYTES,
                        full_path.display()
                    );
                    pb.println(format!(
                        "Skipping large file ({}KB): {}",
                        metadata.len() / 1024,
                        relative_path.display()
                    ).yellow().to_string());
                    pb.inc(1); // Increment progress bar as we are skipping
                    continue; // Skip this file
                }
            }
            Err(e) => {
                 log::error!(
                    "Failed to get metadata for file {}: {}. Skipping file.",
                    full_path.display(), e
                );
                 pb.println(format!(
                    "Error getting metadata for {}, skipping: {}",
                    relative_path.display(), e
                ).red().to_string());
                pb.inc(1); // Increment progress bar
                continue; // Skip file if metadata fails
            }
        }
        // --- End File Size Check ---

        if !full_path.is_file() {
            log::warn!("Skipping non-file path found during indexing: {}", full_path.display());
            pb.inc(1);
            continue;
        }
        match syntax::get_chunks(&full_path) {
            Ok(chunks) => {
                log::debug!("Got {} chunks for file: {}", chunks.len(), relative_path.display());
                let file_path_str = relative_path.to_string_lossy().to_string();
                let file_extension = relative_path.extension().unwrap_or_default().to_string_lossy().to_string();
                for chunk in chunks {
                    let language_str = chunk.language;
                    let point_id_uuid = Uuid::new_v4().to_string();
                    
                    // Handle potential embedding errors gracefully
                    let embedding = match embedding_handler.embed(&[&chunk.content]) {
                        Ok(mut result) => {
                            if result.is_empty() {
                                log::warn!("Embedding returned empty result for chunk in file {} ({}:{})", file_path_str, chunk.start_line, chunk.end_line);
                                pb.println(format!(
                                    "Warning: Skipping chunk in {} ({}:{}) due to empty embedding result.",
                                    relative_path.display(), chunk.start_line, chunk.end_line
                                ).yellow().to_string());
                                continue; // Skip this chunk if embedding is empty
                            }
                            // Assuming embed_batch returns a vec with one element for one input string
                            result.remove(0)
                        }
                        Err(e) => {
                            log::warn!(
                                "Skipping chunk due to embedding error in file '{}' (lines {}..{}): {}",
                                file_path_str,
                                chunk.start_line,
                                chunk.end_line,
                                e
                            );
                            // Also print to console for visibility during progress
                            pb.println(format!(
                                "Warning: Skipping chunk in {} ({}:{}) due to embedding error: {}",
                                relative_path.display(), chunk.start_line, chunk.end_line, e.to_string().chars().take(100).collect::<String>() // Show limited error msg
                            ).yellow().to_string());
                            continue; // Skip this chunk on error
                        }
                    };

                    // Proceed with the valid embedding
                    let mut payload = Payload::new();
                    payload.insert(FIELD_FILE_PATH, file_path_str.clone());
                    payload.insert(FIELD_START_LINE, chunk.start_line as i64);
                    payload.insert(FIELD_END_LINE, chunk.end_line as i64);
                    payload.insert(FIELD_LANGUAGE, language_str.clone());
                    payload.insert(FIELD_CHUNK_CONTENT, chunk.content);
                    payload.insert(FIELD_BRANCH, branch_name.to_string());
                    payload.insert(FIELD_COMMIT_HASH, commit_hash.to_string());
                    payload.insert(FIELD_ELEMENT_TYPE, chunk.element_type.to_string());
                    payload.insert(FIELD_FILE_EXTENSION, file_extension.clone());
                    points_batch.push(PointStruct::new(point_id_uuid, embedding, payload));
                    if points_batch.len() >= BATCH_SIZE {
                        // Use our custom upsert function
                        custom_upsert_batch(client, collection_name, points_batch, &pb).await?;
                        points_batch = Vec::new();
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to get chunks for file {}: {}", full_path.display(), e);
                 pb.println(format!("Error processing {}: {}", relative_path.display(), e).red().to_string());
            }
        }
        pb.inc(1);
    }
    if !points_batch.is_empty() {
        // Use our custom upsert function
        custom_upsert_batch(client, collection_name, points_batch, &pb).await?;
    }
    pb.finish_with_message("Indexing complete");
    Ok(())
}

/// Ensures that a Qdrant collection exists for the repository and has the correct configuration.
/// If the collection does not exist, it will be created.
/// If it exists but has the wrong vector dimension, it will be deleted and recreated.
pub(crate) async fn ensure_repository_collection_exists<
    C: QdrantClientTrait + Send + Sync + 'static,
>(
    client: &C,
    collection_name: &str,
    vector_dimension: u64,
) -> Result<(), Error>
{
    match client.collection_exists(collection_name.to_string()).await {
        Ok(exists) => {
            if exists {
                log::debug!("Collection '{}' already exists.", collection_name);
                // TODO: Optionally verify existing collection parameters?
                Ok(())
            } else {
                log::info!("Collection '{}' does not exist. Creating...", collection_name);
                // Create collection using the trait method
                client.create_collection(collection_name, vector_dimension).await
                    .map_err(|e| Error::Other(format!("Failed to create collection '{}': {}", collection_name, e.to_string())))?;
                println!(
                    "{}",
                    format!(
                        "Created Qdrant collection '{}' with dimension {}.",
                        collection_name.cyan(),
                        vector_dimension
                    ).green()
                );
                Ok(())
            }
        }
        Err(e) => {
            log::error!("Failed to check or create collection '{}': {}", collection_name, e);
            Err(Error::Other(format!("Failed to ensure collection '{}' exists: {}", collection_name, e)))
        }
    }
}

/// Create a filter for a specific branch
pub fn create_branch_filter(branch_name: &str) -> qdrant_client::qdrant::Filter {
    qdrant_client::qdrant::Filter::must([
        qdrant_client::qdrant::Condition::matches(crate::cli::commands::FIELD_BRANCH, branch_name.to_string()),
    ])
}

// ============================================================================
// Shared Repository Management Logic (for CLI and Server)
// ============================================================================

/// Core logic to clone or open a repository and prepare its config.
/// Does not modify AppConfig or save anything.
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
        let repo = Repository::open(&local_path)
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
        let collection_name = get_collection_name(&repo_name);
        info!("Ensuring Qdrant collection '{}' exists (dim={})...", collection_name, embedding_dim);
        if let Err(e) = ensure_repository_collection_exists(client.as_ref(), &collection_name, embedding_dim).await {
            // Map the specific QdrantError or general Error to Error::Other for reporting
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
            last_synced_commits: HashMap::new(),
            indexed_languages: None,
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
    fs::create_dir_all(&local_path)
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
    let repo = Repository::open(&local_path)
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

    let collection_name = get_collection_name(&repo_name);
    info!("Ensuring Qdrant collection '{}' exists (dim={})...", collection_name, embedding_dim);
    if let Err(e) = ensure_repository_collection_exists(client.as_ref(), &collection_name, embedding_dim).await {
        // Map the specific QdrantError or general Error to Error::Other for reporting
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
        last_synced_commits: HashMap::new(),
        indexed_languages: None,
    };

    Ok(new_repo_config)
}


/// Core logic to delete repository data (Qdrant collection, local files).
/// Does not modify AppConfig or save anything.
pub async fn delete_repository_data<C>(
    repo_config: &RepositoryConfig,
    client: Arc<C>,
) -> Result<(), Error>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    let repo_name = &repo_config.name;
    let collection_name = get_collection_name(repo_name);
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
            // Log error but consider it non-fatal for the overall removal
            warn!("Failed to delete Qdrant collection '{}': {}. Continuing removal process.", collection_name, e);
        }
    }

    // Added safety checks for repository path removal
    let local_path = &repo_config.local_path;
    
    // Perform safety checks before deleting the directory
    if !local_path.exists() {
        info!("Local directory '{}' does not exist. Skipping removal.", local_path.display());
        return Ok(());
    }

    // SAFETY CHECK 1: Ensure path is not too short (could be system root, home dir, etc.)
    let path_str = local_path.to_string_lossy();
    if path_str.len() < 10 {  // Arbitrary but reasonable minimum path length for a repo directory
        error!("Path '{}' is suspiciously short. Skipping removal for safety.", path_str);
        return Ok(());
    }

    // SAFETY CHECK 2: Verify .git directory exists (confirming it's likely a git repo)
    let git_dir = local_path.join(".git");
    if !git_dir.exists() || !git_dir.is_dir() {
        warn!("No .git directory found at '{}'. This may not be a git repository. Skipping removal for safety.", local_path.display());
        return Ok(());
    }

    // SAFETY CHECK 3: Check for potentially dangerous paths
    let dangerous_paths = [
        "/", "/home", "/usr", "/bin", "/sbin", "/etc", "/var", "/tmp", "/opt",
        "/boot", "/lib", "/dev", "/proc", "/sys", "/run"
    ];
    
    if dangerous_paths.iter().any(|p| path_str == *p || path_str.starts_with(&format!("{}/", p))) {
        error!("Path '{}' appears to be a system directory. Refusing to delete for safety.", path_str);
        return Ok(());
    }

    // SAFETY CHECK 4: Only delete if we can confirm it's in a repositories directory
    // Look for patterns like .../repositories/repo-name or vectordb-cli/repo-name
    let is_in_repos_dir = path_str.contains("/repositories/") || 
                           path_str.contains("/vectordb-cli/") ||
                           path_str.contains("/repos/");
                           
    if !is_in_repos_dir {
        warn!("Repository path '{}' doesn't appear to be in a standard repositories directory. Skipping automatic removal for safety.", path_str);
        warn!("If you want to delete this directory, please do so manually.");
        return Ok(());
    }

    // If all safety checks pass, proceed with deletion
    info!("Attempting to remove local clone at {}...", local_path.display());
    match fs::remove_dir_all(local_path) {
        Ok(_) => info!("Successfully removed local directory '{}'.", local_path.display()),
        Err(e) => {
            // Log error but consider it non-fatal
             error!("Failed to remove local directory '{}': {}. Please remove it manually.", local_path.display(), e);
             warn!("Failed to remove local directory '{}'. Please remove it manually.", local_path.display());
        }
    }

    Ok(())
}


/// Core logic to switch branch for a repository.
/// Uses git2, potentially blocking.
/// Does not modify AppConfig or save anything.
pub fn switch_repository_branch(
    repo_config: &RepositoryConfig,
    target_branch_name: &str,
) -> Result<(), Error> {
    info!("Attempting to switch to branch '{}' in repository at {:?}", target_branch_name, repo_config.local_path);
    let repo = Repository::open(&repo_config.local_path)
        .with_context(|| format!("Failed to open repository at {}", repo_config.local_path.display()))?;

    let remote_name = repo_config.remote_name.as_deref().unwrap_or("origin");

    if repo.find_branch(target_branch_name, git2::BranchType::Local).is_err() {
        info!(
            "Local branch '{}' not found. Checking remote '{}'...",
            target_branch_name, remote_name
        );
        
        info!("Fetching from remote '{}' to update refs...", remote_name);
        let mut remote = repo.find_remote(remote_name)?;
        
        // Need fetch options potentially with SSH creds
        let mut fetch_opts = create_fetch_options(
            vec![repo_config.clone()], // Hack: Need a way to pass config or creds
            &repo_config.url,
            repo_config.ssh_key_path.as_ref(),
            repo_config.ssh_key_passphrase.as_deref()
        )?;
        remote.fetch(&[] as &[&str], Some(&mut fetch_opts), None)
            .with_context(|| format!("Failed initial fetch from remote '{}' before branch check", remote_name))?;
        info!("Fetch for refs update complete.");

        let remote_branch_ref = format!("{}/{}", remote_name, target_branch_name);
        match repo.find_branch(&remote_branch_ref, git2::BranchType::Remote) {
            Ok(remote_branch) => {
                info!(
                    "Branch '{}' found on remote '{}'. Creating local tracking branch...",
                    target_branch_name, remote_name
                );
                let commit = remote_branch.get().peel_to_commit()
                    .with_context(|| format!("Failed to get commit for remote branch {}", remote_branch_ref))?;
                repo.branch(target_branch_name, &commit, false)
                    .with_context(|| format!("Failed to create local branch '{}'", target_branch_name))?;
                let mut local_branch = repo.find_branch(target_branch_name, git2::BranchType::Local)?;
                local_branch.set_upstream(Some(&remote_branch_ref))
                    .with_context(|| format!("Failed to set upstream for branch '{}' to '{}'", target_branch_name, remote_branch_ref))?;
            }
            Err(_) => {
                return Err(Error::from(anyhow::anyhow!(
                    "Branch '{}' not found locally or on remote '{}'.",
                    target_branch_name,
                    remote_name
                )));
            }
        }
    }

    info!("Checking out branch '{}'...", target_branch_name);
    let ref_name = format!("refs/heads/{}", target_branch_name);
    repo.set_head(&ref_name)
        .with_context(|| format!("Failed to checkout branch '{}'", target_branch_name))?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::new().force()))
        .with_context(|| format!("Failed to force checkout head for branch '{}'", target_branch_name))?;

    info!("Successfully switched to branch '{}'", target_branch_name);
    Ok(())
}

pub async fn sync_repository_branch(
    _cli_args: &CliArgs,
    config: &AppConfig,
    repo_config_index: usize,
    _client: Arc<impl QdrantClientTrait + Send + Sync + 'static>,
    _fetch_and_merge: bool,
) -> Result<(), Error> {
    let repo_config = config.repositories.get(repo_config_index)
        .ok_or_else(|| Error::ConfigurationError(format!("Repository index {} out of bounds", repo_config_index)))?;

    // Initialize embedding handler
    let _embedding_handler = EmbeddingHandler::new(config)
        .map_err(|e: VectorDBError| Error::Other(format!("Failed to initialize embedding handler for sync: {}", e)))?;

    let _repo_root = PathBuf::from(&repo_config.local_path);

    // ... rest of function ...

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::RepositoryConfig;
    use std::path::PathBuf;
    use std::collections::HashMap;

    #[test]
    fn test_create_fetch_options_direct_ssh_params() {
        // Set up test data
        let repo_configs = vec![];
        let repo_url = "git@example.com:user/repo.git";
        let key_path = PathBuf::from("/path/to/key");
        let ssh_key_path = Some(&key_path);
        let ssh_key_passphrase = Some("passphrase");

        // Call the function
        let result = create_fetch_options(
            repo_configs,
            repo_url,
            ssh_key_path,
            ssh_key_passphrase
        );

        // Just verify that it builds the options without errors
        assert!(result.is_ok());
        // We can't easily test the callbacks directly in a unit test
    }

    #[test]
    fn test_create_fetch_options_repo_config_ssh() {
        // Set up test data
        let repo_url = "git@example.com:user/repo.git";
        let repo_configs = vec![
            RepositoryConfig {
                name: "repo".to_string(),
                url: repo_url.to_string(),
                local_path: PathBuf::from("/tmp/repo"),
                default_branch: "main".to_string(),
                tracked_branches: vec!["main".to_string()],
                active_branch: Some("main".to_string()),
                remote_name: Some("origin".to_string()),
                ssh_key_path: Some(PathBuf::from("/path/to/key")),
                ssh_key_passphrase: Some("passphrase".to_string()),
                last_synced_commits: HashMap::new(),
                indexed_languages: None,
            }
        ];

        // Call the function
        let result = create_fetch_options(
            repo_configs,
            repo_url,
            None,
            None
        );

        // Just verify that it builds the options without errors
        assert!(result.is_ok());
        // We can't easily test the callbacks directly in a unit test
    }

    #[test]
    fn test_create_fetch_options_default_credentials() {
        // Set up test data with no SSH keys configured
        let repo_configs = vec![];
        let repo_url = "https://example.com/user/repo.git";

        // Call the function
        let result = create_fetch_options(
            repo_configs,
            repo_url,
            None,
            None
        );

        // Just verify that it builds the options without errors
        assert!(result.is_ok());
        // We can't easily test the callbacks directly in a unit test
    }
    
    #[test]
    fn test_is_ssh_url_detection() {
        // Test SSH URL detection for git@ format
        let repo_url = "git@github.com:user/repo.git";
        assert!(repo_url.starts_with("git@") || repo_url.starts_with("ssh://"));
        
        // Test SSH URL detection for ssh:// format
        let repo_url = "ssh://git@example.com/user/repo.git";
        assert!(repo_url.starts_with("git@") || repo_url.starts_with("ssh://"));
        
        // Test non-SSH URL
        let repo_url = "https://github.com/user/repo.git";
        assert!(!(repo_url.starts_with("git@") || repo_url.starts_with("ssh://")));
    }
    
    #[test]
    fn test_get_collection_name() {
        assert_eq!(get_collection_name("test-repo"), "repo_test-repo");
        assert_eq!(get_collection_name("my_project"), "repo_my_project");
    }
}
