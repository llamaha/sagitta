use anyhow::{anyhow, Context, Result};
use git2::{Repository, CredentialType, FetchOptions, RemoteCallbacks, Cred};
use qdrant_client::qdrant::{CreateCollectionBuilder, Distance, FieldType, VectorParamsBuilder, Filter, PointId, Condition, PointStruct, PointsSelector, points_selector::PointsSelectorOneOf, PointsIdsList, DeletePointsBuilder, ScrollPointsBuilder, ScrollResponse, PayloadIncludeSelector};
use qdrant_client::{Qdrant, Payload};
use std::collections::{HashSet};
use std::path::{PathBuf};
use indicatif::{ProgressBar, ProgressStyle};
use log;
use colored::Colorize;
use uuid::Uuid;

use crate::cli::commands::{
    ensure_payload_index, upsert_batch, CliArgs, FIELD_FILE_PATH, FIELD_START_LINE, FIELD_END_LINE, 
    FIELD_LANGUAGE, FIELD_CHUNK_CONTENT, FIELD_ELEMENT_TYPE, FIELD_FILE_EXTENSION, BATCH_SIZE, FIELD_BRANCH, FIELD_COMMIT_HASH
};
use crate::{syntax, vectordb::{embedding::EmbeddingModelType, embedding_logic::EmbeddingHandler}, config::{AppConfig, RepositoryConfig}};
use crate::cli::repo_commands::COLLECTION_NAME_PREFIX;

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
    #[cfg(feature = "server")]
    let is_server_mode = true;
    #[cfg(not(feature = "server"))]
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
pub(crate) async fn update_sync_status_and_languages(
    config: &mut AppConfig,
    repo_config_index: usize,
    branch_name: &str,
    commit_oid_str: &str,
    client: &Qdrant,
    collection_name: &str,
) -> Result<()> {
    log::debug!("Updating last synced commit for branch '{}' to {}", branch_name, commit_oid_str);
    config.repositories[repo_config_index]
        .last_synced_commits
        .insert(branch_name.to_string(), commit_oid_str.to_string());
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

        let scroll_request = builder;
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
                 config.repositories[repo_config_index].indexed_languages = None;
                 return Ok(());
            }
        }
    }
    log::info!("Found indexed languages for branch '{}': {:?}", branch_name, languages);
    let mut sorted_languages: Vec<String> = languages.into_iter().collect();
    sorted_languages.sort();
    config.repositories[repo_config_index].indexed_languages = Some(sorted_languages);
    Ok(())
}


/// Deletes points associated with specific file paths from a Qdrant collection.
pub(crate) async fn delete_points_for_files(
    client: &Qdrant,
    collection_name: &str,
    branch_name: &str,
    relative_paths: &[PathBuf],
) -> Result<()> {
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
        
        let scroll_request = builder;
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
         client.delete_points(delete_request).await
             .with_context(|| format!("Failed to delete a batch of points from collection '{}'", collection_name))?;
        log::debug!("Deleted batch of {} points for branch '{}'.", chunk.len(), branch_name);
    }
    log::info!("Successfully deleted {} points for {} files in branch '{}'.",
        point_ids_to_delete.len(), relative_paths.len(), branch_name);
    Ok(())
}

/// Indexes a list of files into the specified Qdrant collection.
pub(crate) async fn index_files(
    client: &Qdrant,
    cli_args: &CliArgs,
    config: &AppConfig,
    repo_root: &PathBuf,
    relative_paths: &[PathBuf],
    collection_name: &str,
    branch_name: &str,
    commit_hash: &str,
) -> Result<()> {
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
        .ok_or_else(|| anyhow!("ONNX model path must be provided via --onnx-model, VECTORDB_ONNX_MODEL, or config"))?;
    
    let onnx_tokenizer_dir_str = cli_args.onnx_tokenizer_dir_arg.as_deref()
        .or(tokenizer_env_var.as_deref())
        .or(config.onnx_tokenizer_path.as_deref())
        .ok_or_else(|| anyhow!("ONNX tokenizer path must be provided via --onnx-tokenizer-dir, VECTORDB_ONNX_TOKENIZER_DIR, or config"))?;

    let model_path = Some(PathBuf::from(onnx_model_path_str));
    let tokenizer_path = Some(PathBuf::from(onnx_tokenizer_dir_str));

    let embedding_handler = EmbeddingHandler::new(
        EmbeddingModelType::Onnx, 
        model_path,
        tokenizer_path
    )
        .context("Failed to initialize embedding handler")?;
    
    // Pre-warm the embedding provider cache to load the model upfront
    log::debug!("Pre-warming embedding provider cache...");
    let embedding_dim = embedding_handler.dimension()? as u64;
    log::debug!("Embedding provider cache warmed. Detected dimension: {}", embedding_dim);
    
    // Ensure collection exists with the correct embedding dimension
    ensure_repository_collection_exists(client, collection_name, embedding_dim).await?;

    let pb = ProgressBar::new(relative_paths.len() as u64);
    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({per_sec}, {eta})")?
        .progress_chars("#>-."));
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
                        upsert_batch(client, collection_name, points_batch, 0, 0, &pb).await?;
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
        upsert_batch(client, collection_name, points_batch, 0, 0, &pb).await?;
    }
    pb.finish_with_message("Indexing complete");
    Ok(())
}

/// Ensures that a Qdrant collection exists for the repository, creating it if necessary.
pub(crate) async fn ensure_repository_collection_exists(
    client: &Qdrant,
    collection_name: &str,
    embedding_dimension: u64,
) -> Result<()> {
    log::debug!("Checking existence of collection: {}", collection_name);
    match client.collection_info(collection_name).await {
        Ok(_) => {
            log::info!("Collection '{}' already exists.", collection_name);
            Ok(())
        }
        Err(e) => {
            if e.to_string().contains("Not found") || e.to_string().contains("doesn't exist") {
                 log::info!("Collection '{}' not found. Creating...", collection_name);
                println!("Creating Qdrant collection '{}'...", collection_name);
                let create_request = CreateCollectionBuilder::new(collection_name)
                        .vectors_config(VectorParamsBuilder::new(embedding_dimension, Distance::Cosine));
                client
                    .create_collection(create_request)
                    .await
                    .with_context(|| format!("Failed to create collection '{}'", collection_name))?;
                 log::info!("Collection '{}' created successfully.", collection_name);
                 log::debug!("Ensuring payload indexes exist for collection '{}'", collection_name);
                 ensure_payload_index(client, collection_name, FIELD_FILE_PATH, FieldType::Keyword, true, None).await?;
                 ensure_payload_index(client, collection_name, FIELD_LANGUAGE, FieldType::Keyword, true, None).await?;
                 ensure_payload_index(client, collection_name, FIELD_BRANCH, FieldType::Keyword, true, None).await?;
                 ensure_payload_index(client, collection_name, FIELD_COMMIT_HASH, FieldType::Keyword, true, None).await?;
                 ensure_payload_index(client, collection_name, FIELD_ELEMENT_TYPE, FieldType::Keyword, true, None).await?;
                 ensure_payload_index(client, collection_name, FIELD_FILE_EXTENSION, FieldType::Keyword, true, None).await?;
                 log::info!("Payload indexes ensured for collection '{}'.", collection_name);
                Ok(())
            } else {
                Err(anyhow!("Failed to check collection '{}': {}", collection_name, e))
            }
        }
    }
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
