use crate::mcp::{
    error_codes,
    types::{
        ErrorObject, RepositoryAddParams, RepositoryAddResult, RepositoryInfo, RepositoryListParams,
        RepositoryListResult, RepositoryRemoveParams, RepositoryRemoveResult, RepositorySyncParams,
        RepositorySyncResult, RepositorySearchFileParams, RepositorySearchFileResult, RepositoryViewFileParams, RepositoryViewFileResult,
    },
};
use crate::server::map_add_repo_error; // Import helper from server for now
use anyhow::{anyhow, Result};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use vectordb_core::{
    config::{get_repo_base_path, save_config, AppConfig, RepositoryConfig},
    embedding::EmbeddingHandler,
    indexing::{self, gather_files},
    qdrant_client_trait::QdrantClientTrait,
    repo_add::{handle_repo_add, AddRepoArgs},
    repo_helpers::{delete_repository_data, get_collection_name, switch_repository_branch},
    error::{VectorDBError, Result as CoreResult},
    sync::{sync_repository, SyncOptions},
    fs_utils::{find_files_matching_pattern, read_file_range},
};
use crate::server::map_core_error_to_user_message; // Import helper from server for now
use tempdir;
use std::path::PathBuf;
use git2::Repository; // Import git2
use vectordb_core::config; // Add imports

#[instrument(skip(config, qdrant_client, embedding_handler), fields(repo_name = ?params.name, url = ?params.url))]
pub async fn handle_repository_add<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryAddParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    embedding_handler: Arc<EmbeddingHandler>,
) -> Result<RepositoryAddResult, ErrorObject> {
    let initial_base_path = get_repo_base_path(Some(&*config.read().await)).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to determine repository base path: {}", e),
        data: None,
    })?;
    fs::create_dir_all(&initial_base_path).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!(
            "Failed to create repository base path {}: {}",
            initial_base_path.display(),
            e
        ),
        data: None,
    })?;

    let embedding_dim = embedding_handler.dimension().map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to get embedding dimension: {}", e),
        data: None,
    })? as u64;

    // Map MCP params (String paths) to core AddRepoArgs (PathBuf paths)
    let args = AddRepoArgs {
        url: params.url,
        local_path: params.local_path.map(std::path::PathBuf::from), // Convert Option<String> to Option<PathBuf>
        name: Some(params.name), // Core expects Option<String>, MCP has required String
        branch: params.branch,
        remote: None, // Remote name is determined by core logic if not specified
        repositories_base_path: None, // Base path is passed separately
        ssh_key: params.ssh_key.map(std::path::PathBuf::from), // Convert Option<String> to Option<PathBuf>
        ssh_passphrase: params.ssh_passphrase,
        target_ref: params.target_ref, // Pass through the target_ref
    };

    let new_repo_config_result = handle_repo_add(
        args, // Use the mapped args
        initial_base_path,
        embedding_dim,
        qdrant_client.clone(),
    )
    .await;

    match new_repo_config_result {
        Ok(repo_config) => {
            let mut config_write_guard = config.write().await;

            // Check if repo already exists in config (handle_repo_add doesn't check config)
            if config_write_guard
                .repositories
                .iter()
                .any(|r| r.name == repo_config.name)
            {
                return Err(ErrorObject {
                    code: error_codes::REPO_ALREADY_EXISTS,
                    message: format!(
                        "Repository '{}' already exists in configuration.",
                        repo_config.name
                    ),
                    data: None,
                });
            }

            config_write_guard.repositories.push(repo_config.clone());

            if let Err(e) = save_config(&*config_write_guard, None) {
                error!(error = %e, repo_name=%repo_config.name, "Failed to save config after adding repository {} to memory", repo_config.name);
                // Attempt to remove the repo we just added to memory if save fails
                config_write_guard.repositories.pop();
                return Err(ErrorObject {
                    code: error_codes::CONFIG_SAVE_FAILED,
                    message: format!("Failed to save configuration after adding repository: {}", e),
                    data: None,
                });
            }
            info!(repo_name=%repo_config.name, "Successfully added repository and saved config.");

            Ok(RepositoryAddResult {
                name: repo_config.name,
                url: repo_config.url,
                local_path: repo_config.local_path,
                default_branch: repo_config.default_branch,
                active_branch: repo_config.active_branch,
            })
        }
        Err(e) => {
            error!(error = %e, "Core handle_repo_add failed");
            Err(map_add_repo_error(e)) // Use imported helper
        }
    }
}

#[instrument(skip(config))]
pub async fn handle_repository_list(
    _params: RepositoryListParams,
    config: Arc<RwLock<AppConfig>>,
) -> Result<RepositoryListResult, ErrorObject> {
    let config_read_guard = config.read().await;
    let repo_infos: Vec<RepositoryInfo> = config_read_guard
        .repositories
        .iter()
        .map(|r| RepositoryInfo {
            name: r.name.clone(),
            remote: r.url.clone(),
            description: None,
            branch: r.active_branch.clone(),
            last_updated: None,
        })
        .collect();
    Ok(RepositoryListResult {
        repositories: repo_infos,
    })
}

#[instrument(skip(config, qdrant_client), fields(repo_name = %params.name))]
pub async fn handle_repository_remove<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryRemoveParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
) -> Result<RepositoryRemoveResult, ErrorObject> {
    let mut config_write_guard = config.write().await;

    let repo_index = config_write_guard
        .repositories
        .iter()
        .position(|r| r.name == params.name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found for removal", params.name),
            data: None,
        })?;

    let repo_config_to_remove = config_write_guard.repositories[repo_index].clone();

    // Attempt to delete data first
    if let Err(e) = delete_repository_data(&repo_config_to_remove, qdrant_client.clone()).await {
        // Log error but proceed with config removal
        error!(repo_name = %params.name, error = %e, "Failed to delete repository data, proceeding with config removal.");
    }

    // Remove from config
    config_write_guard.repositories.remove(repo_index);

    // Save updated config
    if let Err(e) = save_config(&*config_write_guard, None) {
        error!(error = %e, "Failed to save config after removing repository");
        // TODO: Should we attempt to restore the removed repo in memory?
        return Err(ErrorObject {
            code: error_codes::CONFIG_SAVE_FAILED,
            message: format!("Failed to save configuration after removal: {}", e),
            data: None,
        });
    }

    Ok(RepositoryRemoveResult {
        name: params.name,
        status: "Removed".to_string(),
    })
}

#[instrument(skip(config, qdrant_client, embedding_handler), fields(repo_name = %params.name))]
pub async fn handle_repository_sync<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositorySyncParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    embedding_handler: Arc<EmbeddingHandler>,
) -> Result<RepositorySyncResult, ErrorObject> {
    let repo_name = params.name.clone();
    let mut commit_hash: String; // Make mutable
    let target_ref_name: Option<String>; // To store the target ref name if it exists

    // --- Get Vocabulary Path BEFORE sync ---
    let (vocab_path, vocab_exists_before_sync) = {
        let config_read = config.read().await;
        let collection_name_for_vocab = get_collection_name(&repo_name);
        let path_result = config::get_vocabulary_path(&*config_read, &collection_name_for_vocab);
        match path_result {
            Ok(p) => {
                let exists = p.exists();
                info!(repo_name=%repo_name, vocab_path=%p.display(), vocab_exists=%exists, "Checked vocabulary existence before sync");
                (p, exists)
            },
            Err(e) => {
                error!(error = %e, collection=%collection_name_for_vocab, "Failed to determine vocabulary path before sync");
                // Return error immediately if we can't even determine the path
                return Err(ErrorObject {
                    code: error_codes::CONFIG_LOAD_FAILED,
                    message: format!("Failed to determine vocabulary path: {}", e),
                    data: None,
                });
            }
        }
    };
    // --- End Vocabulary Path Check ---

    // Drop the write lock before calling sync, which might take time
    // drop(config.write().await); // Drop seems incorrect here, should be read lock?

    // Prepare sync options
    let options = SyncOptions {
        force: params.force.unwrap_or(false),
        extensions: params.extensions,
    };
    
    // Clone AppConfig before sync to avoid holding lock across await (like CLI)
    let app_config_clone: AppConfig = {
        let config_read = config.read().await;
        (*config_read).clone()
    };

    let repo_index = app_config_clone // Use the clone to find the index
        .repositories
        .iter()
        .position(|r| r.name == repo_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found during sync", repo_name),
            data: None,
        })?;

    // Store target_ref value before calling sync, as config might change
    target_ref_name = app_config_clone.repositories[repo_index].target_ref.clone();

    // Call core function and map the Result<vectordb_core::sync::SyncResult, VectorDBError> 
    // to Result<RepositorySyncResult, ErrorObject> (Note: Core SyncResult structure differs from MCP)
    let core_sync_result = sync_repository(
        qdrant_client.clone(),
        &app_config_clone.repositories[repo_index], // Pass ref from clone
        options,
        &app_config_clone, // Pass ref to clone
    ).await;
    
    // No longer need to explicitly drop the lock here

    let sync_message: String;

    match core_sync_result {
        Ok(sync_result) => {
            sync_message = sync_result.message.clone(); // Store message for final result
            if sync_result.success {
                if let Some(commit) = sync_result.last_synced_commit {
                    commit_hash = commit; // Store commit for potential indexing stage
                    info!(repo_name=%repo_name, commit=%commit_hash, "Sync successful, proceeding to update config.");
                    // Update config immediately
                    let mut config_write = config.write().await;
                    if let Some(repo_mut) = config_write.repositories.iter_mut().find(|r| r.name == repo_name) {
                        let branch_or_ref = target_ref_name.as_deref().or(repo_mut.active_branch.as_deref()).unwrap_or("main");
                        repo_mut.last_synced_commits.insert(branch_or_ref.to_string(), commit_hash.clone());
                        repo_mut.indexed_languages = Some(sync_result.indexed_languages.clone()); // Clone languages
                        // Save config right after updating it, propagating error
                        vectordb_core::config::save_config(&*config_write, None).map_err(|e| {
                            error!(error = %e, "Failed to save config after repository sync update");
                            // Decide if this should be a hard error for the sync operation? Yes.
                            ErrorObject {
                                code: error_codes::CONFIG_SAVE_FAILED,
                                message: format!("Failed to save config after sync update: {}", e),
                                data: None,
                            }
                        })?; // Use ? to propagate error
                    } else {
                         error!("Failed to find repository '{}' to update sync status after successful sync.", repo_name);
                    }
                } else {
                    // Sync reported success but no commit hash (e.g., up-to-date message)
                    info!(repo_name=%repo_name, status=%sync_result.message, "Sync resulted in status message, no new commit hash from sync.");
                    commit_hash = String::new(); 
                }
            } else {
                // Sync reported failure
                error!(repo_name = %repo_name, error = %sync_result.message, "Core repository sync reported failure");
                return Err(ErrorObject {
                    code: error_codes::CORE_LOGIC_ERROR,
                    message: sync_result.message,
                    data: None,
                });
            }
        },
        Err(core_error) => { // Explicitly map the error
            error!(repo_name= %repo_name, error = %core_error, "Core sync function failed");
            let user_message = map_core_error_to_user_message(&anyhow!(core_error), "Sync failed");
            return Err(ErrorObject { 
                code: error_codes::INTERNAL_ERROR, // Or map core_error type to specific MCP code
                message: user_message, 
                data: None 
            });
        }
    }
    // --- End of Sync Stage Logic ---
    
    // --- Indexing Stage ---
    // Determine if indexing should run:
    // 1. Sync produced a commit hash (even if it's the same as before) OR
    // 2. Vocabulary file didn't exist before the sync started (force initial index)
    let should_index = !commit_hash.is_empty() || !vocab_exists_before_sync;

    if !should_index {
        info!(repo_name = %repo_name, commit = %commit_hash, vocab_exists_before = %vocab_exists_before_sync, "Skipping indexing stage: No new commit and vocabulary already exists.");
    } else {
        // Get fresh repo config clone needed for indexing (config might have been updated)
        let repo_config_clone: RepositoryConfig = {
            let config_read_guard = config.read().await; // Read lock again
            config_read_guard
                .repositories
                .iter()
                .find(|r| r.name == repo_name)
                .cloned()
                .ok_or_else(|| ErrorObject {
                    code: error_codes::REPO_NOT_FOUND,
                    message: format!("Repository '{}' disappeared unexpectedly before indexing", repo_name),
                    data: None,
                })?
        };
        // Clone entire app config needed by index_repo_files
        let app_config_clone: AppConfig = {
            let config_read_guard = config.read().await;
            (*config_read_guard).clone()
        };

        let repo_root = &repo_config_clone.local_path;

        // Determine the commit hash to use for indexing
        let indexing_commit_hash = if commit_hash.is_empty() {
            // If sync reported no change but we force index (vocab missing),
            // get the current HEAD commit of the local repo.
            info!(repo_name=%repo_name, "Fetching current HEAD commit for forced indexing.");
            match Repository::open(repo_root) {
                Ok(repo) => repo.head()
                    .and_then(|head_ref| head_ref.resolve())
                    .and_then(|resolved_ref| resolved_ref.target().ok_or_else(|| git2::Error::from_str("HEAD has no target OID")))
                    .map(|oid| oid.to_string())
                    .map_err(|e| {
                        error!(repo_name=%repo_name, error=%e, path=%repo_root.display(), "Failed to get current commit hash from local repo");
                        e // Propagate git2 error for logging maybe?
                    })
                    .unwrap_or_else(|_| String::new()) // Fallback to empty on error
                ,
                Err(e) => {
                    error!(repo_name=%repo_name, error=%e, path=%repo_root.display(), "Failed to open local git repository");
                    String::new() // Fallback to empty if repo open fails
                }
            }
        } else {
            // Use the commit hash reported by the sync operation
            commit_hash.clone()
        };

        // Ensure we don't try indexing with an empty commit hash if it was derived or passed incorrectly
        if indexing_commit_hash.is_empty() {
             error!(repo_name=%repo_name, "Resolved commit hash for indexing is empty. Aborting indexing stage.");
             // Return an error if we can't get a commit hash when indexing is required
             return Err(ErrorObject {
                 code: error_codes::GIT_OPERATION_FAILED, // Or a new specific code?
                 message: "Failed to determine commit hash required for indexing.".to_string(),
                 data: None,
             });
        } else {
             info!(repo_name = %repo_name, commit = %indexing_commit_hash, "Starting indexing stage.");
        }

        // Now proceed with indexing logic using indexing_commit_hash
        let context_identifier = repo_config_clone
            .target_ref
            .as_deref()
            .or(repo_config_clone.active_branch.as_deref())
            .unwrap_or(&repo_config_clone.default_branch);
        let collection_name = get_collection_name(&repo_name);

        let files_to_index_abs = match gather_files(&[repo_root.clone()], None) {
            Ok(files) => files,
            Err(e) => {
                error!(repo_name = %repo_name, error = %e, "Failed to list files for indexing");
                return Err(ErrorObject {
                    code: error_codes::INTERNAL_ERROR,
                    message: format!("Failed to gather files in {}: {}", repo_root.display(), e),
                    data: None,
                });
            }
        };

        let files_to_index_rel: Vec<std::path::PathBuf> = files_to_index_abs
            .iter()
            .filter_map(|abs_path| abs_path.strip_prefix(repo_root).ok().map(|p| p.to_path_buf()))
            .collect();

        if files_to_index_rel.is_empty() {
            warn!(repo_name = %repo_name, "No supported files found to index in repository.");
        } else {
            info!(repo_name = %repo_name, count = files_to_index_rel.len(), commit=%indexing_commit_hash, "Found files to index, calling index_repo_files");

            match indexing::index_repo_files(
                &app_config_clone, 
                repo_root,
                &files_to_index_rel,
                &collection_name,
                context_identifier, 
                &indexing_commit_hash, // Use the potentially derived commit hash
                qdrant_client.clone(),
                embedding_handler.clone(),
                None,
                app_config_clone.indexing.max_concurrent_upserts,
            )
            .await
            {
                Ok(count) => {
                    info!(repo_name = %repo_name, count = count, "Successfully indexed files.");
                }
                Err(e) => {
                    error!(repo_name = %repo_name, error = %e, "Indexing failed during sync");
                    let user_message = map_core_error_to_user_message(&anyhow!(e), "Indexing failed");
                    return Err(ErrorObject {
                        code: error_codes::CORE_LOGIC_ERROR,
                        message: user_message,
                        data: None,
                    });
                }
            }
        }
    } // End of indexing stage

    // Return the message from the core sync result
    Ok(RepositorySyncResult {
        message: sync_message, // Use the stored message
        // NOTE: MCP SyncResult doesn't have commit hash or name fields like core
        // These were part of the previous draft but not the MCP struct definition
        // name: repo_name, 
        // commit_hash: Some(indexing_commit_hash), // Should we return the hash used for indexing?
    })
}

/// Helper to get RepositoryConfig based on MCP params (name or active)
/// Note: This differs slightly from CLI utils as MCP doesn't have a global active repo state directly.
/// Clients usually specify the repo per request, or it might be implied by context (not handled here yet).
fn get_repo_config_mcp<'a>(
    config: &'a AppConfig,
    repo_name_param: Option<&str>,
) -> Result<&'a RepositoryConfig, ErrorObject> {
    let repo_name = repo_name_param.ok_or_else(|| ErrorObject {
        code: error_codes::INVALID_PARAMS,
        message: "Repository name must be provided in repository_name parameter.".to_string(),
        data: None,
    })?;

    config
        .repositories
        .iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| ErrorObject {
            code: error_codes::REPO_NOT_FOUND,
            message: format!("Repository '{}' not found in configuration.", repo_name),
            data: None,
        })
}

#[instrument(skip(config), fields(repo_name = ?params.repository_name, pattern = %params.pattern))]
pub async fn handle_repository_search_file(
    params: RepositorySearchFileParams,
    config: Arc<RwLock<AppConfig>>,
) -> Result<RepositorySearchFileResult, ErrorObject> {
    let config_read = config.read().await;
    let repo_config = get_repo_config_mcp(&config_read, params.repository_name.as_deref())?;
    let search_path = &repo_config.local_path;
    let case_sensitive = params.case_sensitive.unwrap_or(false);

    let matching_paths = find_files_matching_pattern(search_path, &params.pattern, case_sensitive)
        .map_err(|e| {
            let user_message = map_core_error_to_user_message(&anyhow!(e), "File search failed");
            ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: user_message,
                data: None,
            }
        })?;

    // Convert PathBufs to Strings for JSON response
    let matching_files_str = matching_paths
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    Ok(RepositorySearchFileResult { matching_files: matching_files_str })
}

#[instrument(skip(config), fields(repo_name = ?params.repository_name, file_path = %params.file_path))]
pub async fn handle_repository_view_file(
    params: RepositoryViewFileParams,
    config: Arc<RwLock<AppConfig>>,
) -> Result<RepositoryViewFileResult, ErrorObject> {
    let config_read = config.read().await;
    let repo_config = get_repo_config_mcp(&config_read, params.repository_name.as_deref())?;
    let base_path = &repo_config.local_path;
    let relative_path = PathBuf::from(&params.file_path); // Convert String to PathBuf

    let absolute_path = base_path.join(&relative_path);

    // Canonicalize and check for path traversal
     let canonical_base = base_path.canonicalize()
        .map_err(|e| ErrorObject { 
            code: error_codes::INTERNAL_ERROR, 
            message: format!("Failed to canonicalize base path {}: {}", base_path.display(), e), 
            data: None 
        })?;
    let canonical_target = absolute_path.canonicalize()
        .map_err(|e| ErrorObject { 
            code: error_codes::FILE_NOT_FOUND, // Use file not found if canonicalization fails for target 
            message: format!("File not found or failed to access: {} (from {}): {}", absolute_path.display(), params.file_path, e), 
            data: None 
        })?;

    if !canonical_target.starts_with(&canonical_base) {
         return Err(ErrorObject {
             code: error_codes::INVALID_PARAMS,
             message: "Attempted path traversal detected. Target path is outside the repository root.".to_string(),
             data: None,
         });
    }

    let content = read_file_range(&canonical_target, params.start_line, params.end_line)
         .map_err(|e| {
            let user_message = map_core_error_to_user_message(&anyhow!(e), "File view failed");
            ErrorObject {
                code: error_codes::INTERNAL_ERROR, // Or FILE_NOT_FOUND?
                message: user_message,
                data: None,
            }
        })?;
    
    Ok(RepositoryViewFileResult {
        content,
        repository_name: repo_config.name.clone(),
        relative_path: params.file_path.clone(),
        absolute_path: canonical_target.to_string_lossy().to_string(),
        start_line: params.start_line,
        end_line: params.end_line,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::types::{RepositoryListParams, RepositoryListResult, RepositoryInfo};
    use vectordb_core::config::{AppConfig, RepositoryConfig, IndexingConfig};
    use std::sync::Arc;
    use tokio::sync::RwLock;
    use std::path::PathBuf;
    use std::collections::HashMap;

    #[tokio::test]
    async fn test_handle_repository_list_success() {
        // 1. Setup Mock Config
        let repo1 = RepositoryConfig {
            name: "repo1".to_string(),
            url: "url1".to_string(),
            local_path: PathBuf::from("/tmp/repo1"),
            default_branch: "main".to_string(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("main".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None, // Standard repo tracking a branch
        };
        let repo2 = RepositoryConfig {
            name: "repo2".to_string(),
            url: "url2".to_string(),
            local_path: PathBuf::from("/tmp/repo2"),
            default_branch: "main".to_string(), // Default is main
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            active_branch: Some("v1.0.0".to_string()), // But active is the target ref
            ssh_key_path: None,
            ssh_key_passphrase: None,
            last_synced_commits: HashMap::new(),
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: Some("v1.0.0".to_string()), // Repo tracking a specific tag
        };
        let config = AppConfig {
            qdrant_url: "dummy".to_string(),
            repositories_base_path: Some(PathBuf::from("/base").to_string_lossy().into_owned()),
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1".to_string()),
            indexing: IndexingConfig { max_concurrent_upserts: 1 },
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
            vocabulary_base_path: Some(PathBuf::from("/vocab").to_string_lossy().into_owned()),
        };
        let config_arc = Arc::new(RwLock::new(config));

        // 2. Prepare Params
        let params = RepositoryListParams {};

        // 3. Call Handler
        let result = handle_repository_list(params, config_arc).await;

        // 4. Assertions
        assert!(result.is_ok());
        let list_result = result.unwrap();

        assert_eq!(list_result.repositories.len(), 2);

        // Check repo1 details (standard branch tracking)
        let res_repo1 = list_result.repositories.iter().find(|r| r.name == "repo1").expect("repo1 not found");
        assert_eq!(res_repo1.remote, "url1");
        assert_eq!(res_repo1.branch, Some("main".to_string())); // Branch field shows active branch

        // Check repo2 details (static target ref tracking)
        let res_repo2 = list_result.repositories.iter().find(|r| r.name == "repo2").expect("repo2 not found");
        assert_eq!(res_repo2.remote, "url2");
        assert_eq!(res_repo2.branch, Some("v1.0.0".to_string())); // Branch field correctly shows target_ref as active branch
    }
} 