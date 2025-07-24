use crate::mcp::{
    error_codes,
    types::{
        ErrorObject, RepositoryAddParams, RepositoryAddResult, RepositoryInfo, RepositoryListParams,
        RepositoryListResult, RepositoryRemoveParams, RepositoryRemoveResult, RepositorySyncParams,
        RepositorySyncResult, RepositorySearchFileParams, RepositorySearchFileResult, RepositoryViewFileParams, RepositoryViewFileResult,
        RepositorySwitchBranchParams, RepositorySwitchBranchResult, RepositoryListBranchesParams, RepositoryListBranchesResult, SyncDetails,
        RepositoryRipgrepParams, RepositoryRipgrepResult, RipgrepMatch,
    },
};
use anyhow::{anyhow, Result};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};
use sagitta_search::{
    config::{self, get_repo_base_path, save_config, AppConfig, RepositoryConfig},
    EmbeddingPool, EmbeddingProcessor, // Use re-export
    indexing::{self, gather_files, IndexRepoFilesParams},
    qdrant_client_trait::QdrantClientTrait,
    repo_add::{handle_repo_add, AddRepoArgs},
    repo_helpers::{delete_repository_data, get_branch_aware_collection_name},
    sync::SyncOptions,
    fs_utils::{find_files_matching_pattern, read_file_range},
};
use crate::server::{map_add_repo_error, create_error_data};
use std::path::PathBuf;
use git2::Repository;
use crate::middleware::auth_middleware::AuthenticatedUser;
use axum::Extension;
// For creating JSON content
#[cfg(test)]
use serde_json::json;
use git_manager::GitManager;
use crate::progress::LoggingProgressReporter; // Added LoggingProgressReporter

/// Helper function to save config with proper test isolation
/// During tests, this will save to a temporary location if SAGITTA_TEST_CONFIG_PATH is set
fn save_config_with_test_isolation(config: &AppConfig) -> Result<(), sagitta_search::error::SagittaError> {
    if cfg!(test) {
        // During tests, try to use the test path from environment variable
        if let Ok(test_path) = std::env::var("SAGITTA_TEST_CONFIG_PATH") {
            let test_path_buf = std::path::PathBuf::from(test_path);
            
            // Ensure the parent directory exists
            if let Some(parent) = test_path_buf.parent() {
                std::fs::create_dir_all(parent).map_err(|e| {
                    sagitta_search::error::SagittaError::ConfigurationError(
                        format!("Failed to create test config directory {}: {}", parent.display(), e)
                    )
                })?;
            }
            
            return save_config(config, Some(&test_path_buf)).map_err(|e| sagitta_search::error::SagittaError::ConfigurationError(e.to_string()));
        }
        
        // Fallback: if no test path is set, create a temporary config file in /tmp
        let fallback_path = std::path::PathBuf::from("/tmp/sagitta_test_config.toml");
        if let Some(parent) = fallback_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                sagitta_search::error::SagittaError::ConfigurationError(
                    format!("Failed to create fallback test config directory {}: {}", parent.display(), e)
                )
            })?;
        }
        return save_config(config, Some(&fallback_path)).map_err(|e| sagitta_search::error::SagittaError::ConfigurationError(e.to_string()));
    }
    // Normal operation: save to default location
    save_config(config, None).map_err(|e| sagitta_search::error::SagittaError::ConfigurationError(e.to_string()))
}

#[instrument(skip(config, qdrant_client), fields(repo_name = ?params.name, url = ?params.url))]
pub async fn handle_repository_add<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryAddParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositoryAddResult, ErrorObject> {
    // Log the received target_ref immediately
    info!(received_target_ref = ?params.target_ref, "Handling repository/add request");


    let config_read_guard = config.read().await;

    // Create EmbeddingPool instance locally for this operation
    let embedding_config = sagitta_search::app_config_to_embedding_config(&config_read_guard);
    let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config).map_err(|e| {
        error!(error = %e, "Failed to create embedding pool for repo_add");
        ErrorObject {
            code: error_codes::INTERNAL_ERROR,
            message: format!("Failed to initialize embedding pool: {e}"),
            data: None,
        }
    })?;
    let embedding_dim = embedding_pool.dimension();

    info!("Local embedding pool created for repository_add: {}", params.name);
    drop(config_read_guard); // Release read lock before potentially long operations or acquiring write lock

    let initial_base_path = get_repo_base_path(Some(&*config.read().await)).map_err(|e| ErrorObject {
        code: error_codes::INTERNAL_ERROR,
        message: format!("Failed to determine repository base path: {e}"),
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

    // Map MCP params (String paths) to core AddRepoArgs (PathBuf paths)
    let args = AddRepoArgs {
        url: params.url,
        local_path: params.local_path.map(std::path::PathBuf::from), // Convert Option<String> to Option<PathBuf>
        name: Some(params.name), // Core expects Option<String>, MCP has required String
        branch: params.branch, // Pass branch as well, core logic might use it if target_ref is None
        remote: None, // Remote name is determined by core logic if not specified
        repositories_base_path: None, // Base path is passed separately
        ssh_key: params.ssh_key.map(std::path::PathBuf::from), // Convert Option<String> to Option<PathBuf>
        ssh_passphrase: params.ssh_passphrase,
        target_ref: params.target_ref, // Pass through the target_ref from MCP params
    };

    // Get a clone of the config for handle_repo_add
    let config_clone = {
        let config_read_guard = config.read().await;
        config_read_guard.clone()
    };

    let new_repo_config_result = handle_repo_add(
        args, // Use the mapped args
        initial_base_path,
        embedding_dim as u64,
        qdrant_client.clone(),
        &config_clone, // Use the cloned config instead of the guard
        Some(Arc::new(LoggingProgressReporter)), // Add MCP progress reporter
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

            if let Err(e) = save_config_with_test_isolation(&config_write_guard) {
                error!(error = %e, repo_name=%repo_config.name, "Failed to save config after adding repository {} to memory", repo_config.name);
                // Attempt to remove the repo we just added to memory if save fails
                config_write_guard.repositories.pop();
                return Err(ErrorObject {
                    code: error_codes::CONFIG_SAVE_FAILED,
                    message: format!("Failed to save configuration after adding repository: {e}"),
                    data: None,
                });
            }
            info!(repo_name=%repo_config.name, "Successfully added repository and saved config.");

            // Explicitly drop the local embedding pool before returning from success path
            drop(embedding_pool);
            info!("Explicitly dropped local_embedding_pool in handle_repository_add for repo: {}", repo_config.name);

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
            // Explicitly drop here too in case of early error after its creation, though it should also go out of scope
            drop(embedding_pool); 
            Err(map_add_repo_error(e)) // Use imported helper
        }
    }
}

#[instrument(skip(config))]
pub async fn handle_repository_list(
    _params: RepositoryListParams,
    config: Arc<RwLock<AppConfig>>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositoryListResult, ErrorObject> {
    let config_guard = config.read().await;

    // Get all repositories
    let filtered_config = config_guard.clone();

    // Drop the read lock before calling async function
    drop(config_guard);

    // Get enhanced repository listing
    let enhanced_list = sagitta_search::get_enhanced_repository_list(&filtered_config)
        .await
        .map_err(|e| {
            error!("Failed to get enhanced repository list: {}", e);
            ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: format!("Failed to get repository list: {e}"),
                data: None,
            }
        })?;

    // Convert enhanced repositories to RepositoryInfo format for compatibility
    let repo_infos: Vec<RepositoryInfo> = enhanced_list
        .repositories
        .into_iter()
        .map(|enhanced_repo| {
            // Determine branch information
            let branch = enhanced_repo.active_branch
                .or(Some(enhanced_repo.default_branch));

            // Create description with enhanced information
            let mut description_parts = Vec::new();
            
            // Add filesystem status
            if enhanced_repo.filesystem_status.exists {
                if enhanced_repo.filesystem_status.is_git_repository {
                    description_parts.push("Git repository".to_string());
                } else {
                    description_parts.push("Directory (no git)".to_string());
                }
            } else {
                description_parts.push("Missing from filesystem".to_string());
            }

            // Add sync status
            match enhanced_repo.sync_status.state {
                sagitta_search::SyncState::UpToDate => description_parts.push("synced".to_string()),
                sagitta_search::SyncState::NeedsSync => description_parts.push("needs sync".to_string()),
                sagitta_search::SyncState::NeverSynced => description_parts.push("never synced".to_string()),
                sagitta_search::SyncState::Unknown => description_parts.push("sync status unknown".to_string()),
            }

            // Add file information
            if let Some(file_count) = enhanced_repo.filesystem_status.total_files {
                if let Some(size) = enhanced_repo.filesystem_status.size_bytes {
                    description_parts.push(format!("{} files ({})", file_count, format_bytes(size)));
                } else {
                    description_parts.push(format!("{file_count} files"));
                }
            }

            // Add languages
            if let Some(languages) = &enhanced_repo.indexed_languages {
                if !languages.is_empty() {
                    description_parts.push(format!("Languages: {}", languages.join(", ")));
                }
            }

            // Add file extensions (top 3)
            if !enhanced_repo.file_extensions.is_empty() {
                let top_exts: Vec<String> = enhanced_repo.file_extensions
                    .iter()
                    .take(3)
                    .map(|ext| format!("{} ({})", ext.extension, ext.count))
                    .collect();
                description_parts.push(format!("Extensions: {}", top_exts.join(", ")));
            }

            let description = if description_parts.is_empty() {
                None
            } else {
                Some(description_parts.join(" | "))
            };

            // TODO: Add last_updated timestamp from enhanced info when available
            let last_updated = enhanced_repo.last_sync_time
                .map(|dt| dt.to_rfc3339());

            // Convert dependencies to DependencyInfo
            let dependencies: Vec<crate::mcp::types::DependencyInfo> = enhanced_repo.dependencies.iter()
                .map(|dep| {
                    // Get additional info about the dependency repository
                    let dep_repo = filtered_config.repositories.iter()
                        .find(|r| r.name == dep.repository_name);
                    
                    crate::mcp::types::DependencyInfo {
                        repository_name: dep.repository_name.clone(),
                        target_ref: dep.target_ref.clone(),
                        purpose: dep.purpose.clone(),
                        is_available: dep_repo.is_some(),
                        local_path: dep_repo.map(|r| r.local_path.to_string_lossy().to_string()),
                        current_ref: dep_repo.and_then(|r| r.active_branch.clone()),
                    }
                })
                .collect();

            RepositoryInfo {
                name: enhanced_repo.name,
                remote: enhanced_repo.url, // Corrected: enhanced_repo.url is already a String
                description,
                branch,
                last_updated,
                dependencies,
            }
        })
        .collect();

    info!(
        listed_repo_count = repo_infos.len(),
        "Enhanced repository list generated with comprehensive information."
    );

    Ok(RepositoryListResult {
        repositories: repo_infos,
    })
}

// Helper function to format bytes
fn format_bytes(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;
    
    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }
    
    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[instrument(skip(config, qdrant_client), fields(repo_name = %params.name))]
pub async fn handle_repository_remove<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositoryRemoveParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
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

    info!(repo_name = %params.name, "Proceeding with repository removal");

    // Attempt to delete data first
    if let Err(e) = delete_repository_data(&repo_config_to_remove, qdrant_client.clone(), &config_write_guard).await {
        // Log error but proceed with config removal
        error!(repo_name = %params.name, error = %e, "Failed to delete repository data, proceeding with config removal.");
    }

    // Remove from config
    config_write_guard.repositories.remove(repo_index);

    // Save updated config
    if let Err(e) = save_config_with_test_isolation(&config_write_guard) {
        error!(error = %e, "Failed to save config after removing repository");
        // TODO: Should we attempt to restore the removed repo in memory?
        return Err(ErrorObject {
            code: error_codes::CONFIG_SAVE_FAILED,
            message: format!("Failed to save configuration after removal: {e}"),
            data: None,
        });
    }

    Ok(RepositoryRemoveResult {
        name: params.name,
        status: "Removed".to_string(),
    })
}

#[instrument(skip(config, qdrant_client), fields(repo_name = %params.name))]
pub async fn handle_repository_sync<C: QdrantClientTrait + Send + Sync + 'static>(
    params: RepositorySyncParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositorySyncResult, ErrorObject> {
    info!("Handling repository/sync request");
    let repo_name = params.name;
    let config_guard = config.read().await;

    let repo_config = config_guard
        .repositories
        .iter()
        .find(|r| r.name == repo_name)
        .ok_or_else(|| {
            error!("Repository '{}' not found for sync", repo_name);
            ErrorObject {
                code: error_codes::REPO_NOT_FOUND,
                message: format!("Repository '{repo_name}' not found for sync"),
                data: None,
            }
        })?
        .clone();

    info!(repo_name = %repo_name, "Processing repository sync");

    // Drop the read lock before long-running operations
    let app_config_clone = config_guard.clone(); // config_guard is AppConfig, not RwLockReadGuard anymore
    drop(config_guard); // Not strictly needed as it's not a lock guard now, but good for clarity

    let branch_to_sync_str = repo_config.target_ref.as_deref()
        .or(repo_config.active_branch.as_deref())
        .unwrap_or("main");

    let options = SyncOptions {
        force: params.force.unwrap_or(false),
        extensions: params.extensions.clone(),
        // progress_reporter: Some(Arc::new(LoggingProgressReporter {})), // As per compiler error
    };

    // app_config_clone is already defined and holds config_guard.clone()
    // let app_config_clone = config_read_guard.clone(); // REMOVE THIS LINE
    // drop(config_read_guard); // REMOVE THIS LINE

    let core_sync_result = sagitta_search::sync::sync_repository(
        qdrant_client.clone(),
        &repo_config, // repo_config is already a clone
        options,
        &app_config_clone, // This is already available from earlier scope
        Some(Arc::new(LoggingProgressReporter {})), // Add as 5th arg as per compiler error
    )
    .await;

    let sync_message: String;
    let actual_synced_commit: Option<String>;
    let indexed_languages_from_sync: Vec<String>;

    match core_sync_result { // Match on the direct result
        Ok(core_success_result) => {
            sync_message = core_success_result.message.clone();
            actual_synced_commit = core_success_result.last_synced_commit.clone();
            indexed_languages_from_sync = core_success_result.indexed_languages.clone();

            if core_success_result.success {
                if let Some(commit) = &actual_synced_commit {
                    info!(repo_name=%repo_name, commit=%commit, "Sync successful, proceeding to update config.");
                    let mut config_write = config.write().await;
                    if let Some(repo_mut) = config_write.repositories.iter_mut().find(|r| r.name == repo_name) {
                        // Update the new last_synced_commit field
                        repo_mut.last_synced_commit = Some(commit.clone());
                        // Also update deprecated field for backward compatibility
                        repo_mut.last_synced_commits.insert(branch_to_sync_str.to_string(), commit.clone());
                        repo_mut.indexed_languages = Some(indexed_languages_from_sync.clone());
                        save_config_with_test_isolation(&config_write).map_err(|e| {
                            error!(error = %e, "Failed to save config after repository sync update");
                            ErrorObject {
                                code: error_codes::CONFIG_SAVE_FAILED,
                                message: format!("Failed to save config after sync update: {e}"),
                                data: None,
                            }
                        })?;
                    } else {
                         error!("Failed to find repository '{}' to update sync status after successful sync.", repo_name);
                    }
                } else {
                    info!(repo_name=%repo_name, status=%sync_message, "Sync resulted in status message, no new commit hash from sync.");
                }
            } else {
                error!(repo_name = %repo_name, error = %sync_message, "Core repository sync reported failure");
                return Err(ErrorObject {
                    code: error_codes::CORE_LOGIC_ERROR,
                    message: sync_message,
                    data: None,
                });
            }
        },
        Err(core_error) => { 
            error!(repo_name= %repo_name, error = %core_error, "Core sync function failed");
            let error_data = create_error_data(&anyhow!(core_error));
            return Err(ErrorObject { 
                code: error_codes::INTERNAL_ERROR, 
                message: "Core sync function failed.".to_string(),  
                data: Some(error_data), 
            });
        }
    }
    
    // --- Indexing Stage ---

    let vocab_exists_before_sync = {
        let context_identifier = repo_config.target_ref.as_deref()
            .or(repo_config.active_branch.as_deref())
            .unwrap_or(&repo_config.default_branch);
        let collection_name_for_vocab = get_branch_aware_collection_name(&repo_name, context_identifier, &app_config_clone);
        config::get_vocabulary_path(&app_config_clone, &collection_name_for_vocab)
            .map(|p| p.exists())
            .unwrap_or(false)
    };

    let should_index = actual_synced_commit.as_ref().is_some_and(|s| !s.is_empty()) || !vocab_exists_before_sync;

    if !should_index {
        info!(repo_name = %repo_name, commit = ?actual_synced_commit, vocab_exists_before = %vocab_exists_before_sync, "Skipping indexing stage: No new commit and vocabulary already exists.");
    } else {
        // >>>>>>>>>> Moved EmbeddingPool creation here <<<<<<<<<<
        let config_read_for_embedding = config.read().await;
        let embedding_config = sagitta_search::app_config_to_embedding_config(&config_read_for_embedding);
        let embedding_pool = EmbeddingPool::with_configured_sessions(embedding_config).map_err(|e| {
            error!(error = %e, "Failed to create embedding pool for indexing stage");
            ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: format!("Failed to create embedding pool for indexing: {e}"),
                data: None,
            }
        })?;
        let _embedding_dim = embedding_pool.dimension();
        // >>>>>>>>>> End moved block <<<<<<<<<<

        let repo_root = &repo_config.local_path; // Use the initially cloned repo_config

        let indexing_commit_hash = if actual_synced_commit.as_ref().is_none_or(|s| s.is_empty()) {
            info!(repo_name=%repo_name, "Fetching current HEAD commit for forced indexing (sync returned no commit or vocab missing).");
            match Repository::open(repo_root) {
                Ok(repo) => repo.head()
                    .and_then(|head_ref| head_ref.resolve())
                    .and_then(|resolved_ref| resolved_ref.target().ok_or_else(|| git2::Error::from_str("HEAD has no target OID")))
                    .map(|oid| oid.to_string())
                    .unwrap_or_else(|e| {
                        error!(repo_name=%repo_name, error=%e, path=%repo_root.display(), "Failed to get current commit hash from local repo for indexing");
                        String::new()
                    })
                ,
                Err(e) => {
                    error!(repo_name=%repo_name, error=%e, path=%repo_root.display(), "Failed to open local git repository for indexing commit hash retrieval");
                    String::new()
                }
            }
        } else {
            actual_synced_commit.clone().unwrap() // Known to be Some and not empty if this branch is taken due to `should_index` logic
        };

        if indexing_commit_hash.is_empty() {
             error!(repo_name=%repo_name, "Resolved commit hash for indexing is empty. Aborting indexing stage.");
             return Err(ErrorObject {
                 code: error_codes::GIT_OPERATION_FAILED, 
                 message: "Failed to determine commit hash required for indexing.".to_string(),
                 data: None,
             });
        } else {
             info!(repo_name = %repo_name, commit = %indexing_commit_hash, "Starting indexing stage.");
        }

        let context_identifier = repo_config.target_ref.as_deref()
            .or(repo_config.active_branch.as_deref())
            .unwrap_or(&repo_config.default_branch);
        let collection_name = get_branch_aware_collection_name(&repo_name, context_identifier, &app_config_clone);

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

            match indexing::index_repo_files(IndexRepoFilesParams {
                config: &app_config_clone, 
                repo_root,
                relative_paths: &files_to_index_rel,
                collection_name: &collection_name,
                branch_name: context_identifier, 
                commit_hash: &indexing_commit_hash, // Use the potentially derived commit hash
                client: qdrant_client.clone(),
                embedding_pool: Arc::new(embedding_pool),
                progress_reporter: None,
                max_concurrent_upserts: app_config_clone.indexing.max_concurrent_upserts,
            })
            .await
            {
                Ok(count) => {
                    info!(repo_name = %repo_name, count = count, "Successfully indexed files.");
                }
                Err(e) => {
                    error!(repo_name = %repo_name, error = %e, "Indexing failed during sync");
                    let error_data = create_error_data(&anyhow!(e));
                    return Err(ErrorObject {
                        code: error_codes::CORE_LOGIC_ERROR,
                        message: "Indexing failed during sync.".to_string(), // Concise message
                        data: Some(error_data), // Add detailed data
                    });
                }
            }
        }
    } // End of indexing stage

    // Explicitly drop local_embedding_pool if it was created
    // This is tricky now, it's only created in the `else` block. 
    // It will be dropped automatically when it goes out of scope if `should_index` was true.
    // Consider if manual drop is still needed or how to structure.
    // For now, relying on scope drop.

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
            message: format!("Repository '{repo_name}' not found in configuration."),
            data: None,
        })
}

/// Validates search patterns to prevent overly broad searches that could return excessive results
fn validate_search_pattern(pattern: &str) -> Result<(), ErrorObject> {
    // Disallow standalone wildcards that would match everything
    let dangerous_patterns = ["*", "**", "**/*", "*/*", "**/**"];
    
    if dangerous_patterns.contains(&pattern) {
        return Err(ErrorObject {
            code: error_codes::INVALID_PARAMS,
            message: format!(
                "Pattern '{}' is too broad and could return excessive results. Please use more specific patterns like '*.rs', 'src/*.ts', or '*config*'. Patterns must contain at least 2 non-wildcard characters.",
                pattern
            ),
            data: None,
        });
    }
    
    // Require at least some non-wildcard characters
    let non_wildcard_chars = pattern.chars()
        .filter(|c| *c != '*' && *c != '?' && *c != '/')
        .count();
    
    if non_wildcard_chars < 2 {
        return Err(ErrorObject {
            code: error_codes::INVALID_PARAMS,
            message: format!(
                "Pattern '{}' must contain at least 2 non-wildcard characters. Examples: '*.rs', 'test*', '*config*', 'src/*.js'",
                pattern
            ),
            data: None,
        });
    }
    
    Ok(())
}

#[instrument(skip(config), fields(repo_name = ?params.repository_name, pattern = %params.pattern))]
pub async fn handle_repository_search_file(
    params: RepositorySearchFileParams,
    config: Arc<RwLock<AppConfig>>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositorySearchFileResult, ErrorObject> {
    // Validate the search pattern to prevent overly broad searches
    validate_search_pattern(&params.pattern)?;
    
    let config_guard = config.read().await;
    let repo_config = get_repo_config_mcp(&config_guard, params.repository_name.as_deref())?;

    info!(repo_name = %repo_config.name, "Processing search_file");

    let search_path = &repo_config.local_path;
    let case_sensitive = params.case_sensitive.unwrap_or(false);

    let mut matching_paths = find_files_matching_pattern(search_path, &params.pattern, case_sensitive)
        .map_err(|e| {
            let error_data = create_error_data(&anyhow!(e));
            ErrorObject {
                code: error_codes::INTERNAL_ERROR,
                message: "File search failed.".to_string(), // Concise message
                data: Some(error_data), // Add detailed data
            }
        })?;
    
    // Check file count before processing
    const MAX_SEARCH_RESULTS: usize = 500;
    if matching_paths.len() > MAX_SEARCH_RESULTS {
        return Err(ErrorObject {
            code: error_codes::INVALID_REQUEST,
            message: format!(
                "Too many results: {} files found (limit: {}). Try with a more specific pattern. Examples: 'src/*.rs' instead of '*.rs', 'test_*.py' instead of '*.py'",
                matching_paths.len(),
                MAX_SEARCH_RESULTS
            ),
            data: None,
        });
    }

    // Convert PathBufs to Strings for JSON response
    let matching_files_str: Vec<String> = matching_paths
        .into_iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect();

    // Check total output size
    let total_size: usize = matching_files_str.iter().map(|s| s.len() + 1).sum(); // +1 for newline
    if total_size > 5000 {
        return Err(ErrorObject {
            code: error_codes::INVALID_REQUEST,
            message: format!(
                "Output too large: {} characters (limit: 5000). Try with a more specific pattern.",
                total_size
            ),
            data: None,
        });
    }

    Ok(RepositorySearchFileResult { matching_files: matching_files_str })
}

pub async fn handle_repository_ripgrep(
    params: RepositoryRipgrepParams,
    config: Arc<RwLock<AppConfig>>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositoryRipgrepResult, ErrorObject> {
    use grep::regex::RegexMatcherBuilder;
    use grep::searcher::{BinaryDetection, SearcherBuilder};
    use grep::matcher::Matcher;
    use ignore::WalkBuilder;
    use std::io::BufReader;
    
    let config_guard = config.read().await;
    let repo_config = get_repo_config_mcp(&config_guard, params.repository_name.as_deref())?;

    info!(repo_name = %repo_config.name, pattern = %params.pattern, "Processing ripgrep search");

    let search_path = &repo_config.local_path;
    let case_sensitive = params.case_sensitive.unwrap_or(false);
    let context_lines = params.context_lines.unwrap_or(2);
    let max_results = params.max_results.unwrap_or(100);
    
    // Build the regex matcher
    let matcher = RegexMatcherBuilder::new()
        .case_insensitive(!case_sensitive)
        .build(&params.pattern)
        .map_err(|e| {
            ErrorObject {
                code: error_codes::INVALID_PARAMS,
                message: format!("Invalid regex pattern: {}", e),
                data: None,
            }
        })?;
    
    // Build the file walker with optional file pattern filter
    let mut walker = WalkBuilder::new(search_path);
    walker.standard_filters(true); // Respect .gitignore
    
    if let Some(file_pattern) = &params.file_pattern {
        walker.add(file_pattern);
    }
    
    let mut matches = Vec::new();
    let mut total_matches = 0;
    let mut files_matched = std::collections::HashSet::new();
    
    // Walk through files and search
    for entry in walker.build() {
        if matches.len() >= max_results {
            break;
        }
        
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        
        // Get relative path for the result
        let relative_path = path.strip_prefix(search_path)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        
        // Search the file
        let file = match fs::File::open(path) {
            Ok(f) => f,
            Err(_) => continue,
        };
        
        let mut searcher = SearcherBuilder::new()
            .binary_detection(BinaryDetection::quit(b'\0'))
            .line_number(true)
            .build();
        
        let mut line_matches = Vec::new();
        let result = searcher.search_reader(
            &matcher,
            BufReader::new(file),
            grep::searcher::sinks::UTF8(|line_num, line| {
                if matches.len() + line_matches.len() >= max_results {
                    return Ok(false); // Stop searching
                }
                
                // Store the match
                line_matches.push((line_num as usize, line.to_string()));
                total_matches += 1;
                Ok(true)
            }),
        );
        
        if result.is_ok() && !line_matches.is_empty() {
            files_matched.insert(relative_path.clone());
            
            // Read the file to get context lines
            let file_content = fs::read_to_string(path).unwrap_or_default();
            let lines: Vec<&str> = file_content.lines().collect();
            
            for (line_num, line_content) in line_matches {
                if matches.len() >= max_results {
                    break;
                }
                
                let mut before_context = None;
                let mut after_context = None;
                
                if context_lines > 0 {
                    // Get before context
                    let start = line_num.saturating_sub(context_lines + 1);
                    let before: Vec<String> = lines[start..line_num.saturating_sub(1)]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    if !before.is_empty() {
                        before_context = Some(before);
                    }
                    
                    // Get after context
                    let end = (line_num + context_lines).min(lines.len());
                    let after: Vec<String> = lines[line_num..end]
                        .iter()
                        .map(|s| s.to_string())
                        .collect();
                    if !after.is_empty() {
                        after_context = Some(after);
                    }
                }
                
                matches.push(RipgrepMatch {
                    file_path: relative_path.clone(),
                    line_number: line_num,
                    line_content: line_content.trim_end().to_string(),
                    before_context,
                    after_context,
                });
            }
        }
    }
    
    // Calculate total content size
    let mut total_content_size = 0;
    for match_item in &matches {
        total_content_size += match_item.line_content.len();
        if let Some(ref before) = match_item.before_context {
            for line in before {
                total_content_size += line.len() + 1; // +1 for newline
            }
        }
        if let Some(ref after) = match_item.after_context {
            for line in after {
                total_content_size += line.len() + 1; // +1 for newline
            }
        }
    }
    
    // Check if content exceeds limit
    if total_content_size > 5000 {
        return Err(ErrorObject {
            code: error_codes::INVALID_REQUEST,
            message: format!(
                "Output too large: {} characters (limit: 5000). Found {} matches across {} files. Try with stricter filters: more specific pattern, file_pattern (e.g., '*.rs'), or lower max_results.",
                total_content_size,
                total_matches,
                files_matched.len()
            ),
            data: None,
        });
    }
    
    Ok(RepositoryRipgrepResult {
        matches,
        total_matches,
        files_matched: files_matched.len(),
    })
}

#[instrument(skip(config), fields(repo_name = ?params.repository_name, file_path = %params.file_path))]
pub async fn handle_repository_view_file(
    params: RepositoryViewFileParams,
    config: Arc<RwLock<AppConfig>>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositoryViewFileResult, ErrorObject> {
    let config_guard = config.read().await;
    let repo_config = get_repo_config_mcp(&config_guard, params.repository_name.as_deref())?;

    info!(repo_name = %repo_config.name, "Processing view_file");

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
            let error_data = create_error_data(&anyhow!(e));
            ErrorObject {
                code: error_codes::INTERNAL_ERROR, // Or FILE_NOT_FOUND?
                message: "File view failed.".to_string(), // Concise message
                data: Some(error_data), // Add detailed data
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

/// Handle repository branch switching with automatic resync
#[instrument(skip(config, qdrant_client), fields(repo_name = %params.repository_name))]
pub async fn handle_repository_switch_branch<C>(
    params: RepositorySwitchBranchParams,
    config: Arc<RwLock<AppConfig>>,
    qdrant_client: Arc<C>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositorySwitchBranchResult, ErrorObject>
where
    C: QdrantClientTrait + Send + Sync + 'static,
{
    info!("Handling repository/switch_branch request");
    let repo_name = &params.repository_name;
    
    // Determine the target reference - either from branch_name or target_ref
    let (target_ref_to_checkout, is_target_ref) = match (&params.branch_name, &params.target_ref) {
        (Some(branch_name), None) => (branch_name.clone(), false),
        (None, Some(target_ref)) => (target_ref.clone(), true),
        (Some(_), Some(_)) => {
            return Err(ErrorObject {
                code: error_codes::INVALID_PARAMS,
                message: "Cannot specify both branch_name and target_ref. Use one or the other.".to_string(),
                data: None,
            });
        }
        (None, None) => {
            return Err(ErrorObject {
                code: error_codes::INVALID_PARAMS,
                message: "Must specify either branch_name or target_ref.".to_string(),
                data: None,
            });
        }
    };
    
    info!("Target ref to checkout: '{}' (is_target_ref: {})", target_ref_to_checkout, is_target_ref);
    
    let config_guard = config.read().await;

    // Find repository configuration
    let repo_config = config_guard
        .repositories
        .iter()
        .find(|r| r.name == *repo_name)
        .ok_or_else(|| {
            error!("Repository '{}' not found for branch switch", repo_name);
            ErrorObject {
                code: error_codes::REPO_NOT_FOUND,
                message: format!("Repository '{repo_name}' not found for branch switch"),
                data: None,
            }
        })?
        .clone();

    info!(repo_name = %repo_name, "Processing branch switch");

    let repo_path = PathBuf::from(&repo_config.local_path);
    let current_branch = repo_config.active_branch.clone()
        .unwrap_or_else(|| repo_config.default_branch.clone());
    
    // Drop the read lock before git operations
    let _app_config_clone = config_guard.clone();
    drop(config_guard);

    // Initialize git manager and perform branch switch
    let git_manager = GitManager::new();
    
    // Check if target branch exists (only for branches, not for target refs)
    if !is_target_ref {
    let branches = git_manager.list_all_references(&repo_path)
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to list branches: {e}"),
            data: None,
        })?;
    
        if !branches.contains(&target_ref_to_checkout) {
        return Err(ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
                message: format!("Branch '{target_ref_to_checkout}' not found in repository"),
            data: None,
        });
        }
    }

    // Perform branch switch with git-manager
    // Note: GitManager switch_branch is synchronous, not async
    let repo = git_manager::GitRepository::open(&repo_path)
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to open repository: {e}"),
            data: None,
        })?;
    
    let previous_branch = repo.switch_branch(&target_ref_to_checkout)
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to switch branch: {e}"),
            data: None,
        })?;

    info!(
        repo_name = %repo_name,
        previous_branch = %current_branch,
        new_branch = %target_ref_to_checkout,
        "Branch switch completed successfully"
    );

    // Perform automatic resync if enabled
    let (sync_performed, files_changed, sync_details) = if !params.no_auto_resync {
        info!("Performing automatic resync after branch switch");
        
        // Use the existing sync handler with force=false
        let sync_params = RepositorySyncParams {
            name: repo_name.clone(),
            force: Some(false),
            extensions: None,
        };
        
        match handle_repository_sync(sync_params, config.clone(), qdrant_client, auth_user_ext).await {
            Ok(sync_result) => {
                info!("Automatic resync completed successfully");
                // Parse sync message to extract file counts (simplified)
                let files_changed = if sync_result.message.contains("files") {
                    // Try to extract number from message like "Synced 5 files"
                    sync_result.message
                        .split_whitespace()
                        .find_map(|s| s.parse::<usize>().ok())
                        .unwrap_or(0)
                } else {
                    0
                };
                
                (true, files_changed, Some(SyncDetails {
                    files_added: files_changed, // Simplified - we don't have detailed breakdown
                    files_updated: 0,
                    files_removed: 0,
                }))
            }
            Err(e) => {
                warn!("Automatic resync failed: {}", e.message);
                // Don't fail the branch switch if resync fails
                (false, 0, None)
            }
        }
    } else {
        info!("Automatic resync disabled");
        (false, 0, None)
    };

    // Update repository configuration
    {
        let mut config_write_guard = config.write().await;
        if let Some(repo_mut) = config_write_guard.repositories.iter_mut()
            .find(|r| r.name == *repo_name) {
            
            if is_target_ref {
                // If using target_ref, update the target_ref field and set active_branch to the ref
                repo_mut.target_ref = Some(target_ref_to_checkout.clone());
                repo_mut.active_branch = Some(target_ref_to_checkout.clone());
            } else {
                // If using branch name, clear target_ref and set active_branch
                repo_mut.target_ref = None;
                repo_mut.active_branch = Some(target_ref_to_checkout.clone());
                if !repo_mut.tracked_branches.contains(&target_ref_to_checkout) {
                    repo_mut.tracked_branches.push(target_ref_to_checkout.clone());
                }
            }
        }
        
        // Save configuration
        save_config_with_test_isolation(&config_write_guard)
            .map_err(|e| ErrorObject {
                code: error_codes::CONFIG_SAVE_FAILED,
                message: format!("Failed to save configuration: {e}"),
                data: None,
            })?;
    }

    Ok(RepositorySwitchBranchResult {
        previous_branch,
        new_branch: target_ref_to_checkout.clone(),
        sync_performed,
        files_changed,
        sync_details,
    })
}

/// Handle listing repository branches
#[instrument(skip(config), fields(repo_name = %params.repository_name))]
pub async fn handle_repository_list_branches(
    params: RepositoryListBranchesParams,
    config: Arc<RwLock<AppConfig>>,
    auth_user_ext: Option<Extension<AuthenticatedUser>>,
) -> Result<RepositoryListBranchesResult, ErrorObject> {
    info!("Handling repository/list_branches request");
    let repo_name = &params.repository_name;
    
    let config_guard = config.read().await;

    // Find repository configuration
    let repo_config = config_guard
        .repositories
        .iter()
        .find(|r| r.name == *repo_name)
        .ok_or_else(|| {
            error!("Repository '{}' not found for list branches", repo_name);
            ErrorObject {
                code: error_codes::REPO_NOT_FOUND,
                message: format!("Repository '{repo_name}' not found for list branches"),
                data: None,
            }
        })?
        .clone();

    info!(repo_name = %repo_name, "Processing list branches");

    let repo_path = PathBuf::from(&repo_config.local_path);
    let current_branch = repo_config.active_branch.clone()
        .unwrap_or_else(|| repo_config.default_branch.clone());
    
    drop(config_guard);

    // List branches using git manager with filtering
    let git_manager = GitManager::new();
    let repo = git_manager::GitRepository::open(&repo_path)
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to open repository: {e}"),
            data: None,
        })?;

    let mut all_refs = Vec::new();
    
    // Add local branches
    let local_branches = repo.list_branches(Some(git2::BranchType::Local))
        .map_err(|e| ErrorObject {
            code: error_codes::GIT_OPERATION_FAILED,
            message: format!("Failed to list local branches: {e}"),
            data: None,
        })?;
    all_refs.extend(local_branches);
    
    // Add remote branches if requested
    if params.include_remote {
        let remote_branches = repo.list_branches(Some(git2::BranchType::Remote))
            .map_err(|e| ErrorObject {
                code: error_codes::GIT_OPERATION_FAILED,
                message: format!("Failed to list remote branches: {e}"),
                data: None,
            })?;
        
        // Strip remote prefix for display (e.g., "origin/feature" -> "feature")
        for remote_branch in remote_branches {
            if let Some(branch_name) = remote_branch.split('/').nth(1) {
                if !all_refs.contains(&branch_name.to_string()) {
                    all_refs.push(branch_name.to_string());
                }
            }
        }
    }
    
    // Add tags if requested
    if params.include_tags {
        let tags = git_manager.list_tags(&repo_path)
            .map_err(|e| ErrorObject {
                code: error_codes::GIT_OPERATION_FAILED,
                message: format!("Failed to list tags: {e}"),
                data: None,
            })?;
        all_refs.extend(tags);
    }
    
    // Apply filter if provided
    if let Some(filter_pattern) = &params.filter {
        all_refs.retain(|ref_name| {
            // Simple glob-like matching (can be enhanced with proper glob crate later)
            if filter_pattern.contains('*') {
                let pattern = filter_pattern.replace('*', "");
                if filter_pattern.starts_with('*') && filter_pattern.ends_with('*') {
                    // *pattern* - contains
                    ref_name.contains(&pattern)
                } else if filter_pattern.starts_with('*') {
                    // *pattern - ends with
                    ref_name.ends_with(&pattern)
                } else if filter_pattern.ends_with('*') {
                    // pattern* - starts with
                    ref_name.starts_with(&pattern)
                } else {
                    // Fallback to contains for complex patterns
                    ref_name.contains(&pattern)
                }
            } else {
                // Exact match
                ref_name == filter_pattern
            }
        });
    }
    
    // Apply limit (max 200 to prevent overwhelming output)
    let limit = std::cmp::min(params.limit, 200);
    all_refs.truncate(limit);
    
    // Sort for consistent output
    all_refs.sort();

    info!(
        repo_name = %repo_name,
        branch_count = all_refs.len(),
        current_branch = %current_branch,
        "Successfully listed branches"
    );

    Ok(RepositoryListBranchesResult {
        branches: all_refs,
        current_branch,
    })
}

#[cfg(test)]
mod tests {
    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    // WARNING: NEVER EVER CALL save_config() IN TESTS!
    // 
    // This has been a recurring issue where tests overwrite user configuration files.
    // See the warnings in tests/dependency_integration_test.rs for full details.
    // 
    // ALWAYS use in-memory configs. NEVER save to disk in tests.
    // !!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
    
    use super::*;
    use sagitta_search::config::{AppConfig, IndexingConfig, PerformanceConfig, RepositoryConfig};
    use tempfile::tempdir;
    use axum::Extension;
    use crate::middleware::auth_middleware::AuthenticatedUser;
    use sagitta_search::qdrant_client_trait::QdrantClientTrait;
    use async_trait::async_trait;
    use qdrant_client::qdrant::{
        PointsOperationResponse, CollectionInfo, CollectionStatus, OptimizerStatus,
        HealthCheckReply, CreateCollection, SearchPoints, CountPoints, 
        CountResponse, SearchResponse, PointsSelector, DeletePoints, 
        ScrollPoints, ScrollResponse, UpsertPoints, QueryPoints, QueryResponse
    };
    use sagitta_search::error::SagittaError;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[derive(Clone, Debug)]
    struct MockQdrantClient {
        fail_delete_collection: bool,
        collection_exists_response: bool,
    }

    impl Default for MockQdrantClient {
        fn default() -> Self {
            MockQdrantClient { 
                fail_delete_collection: false, 
                collection_exists_response: false, 
            }
        }
    }

    #[async_trait]
    impl QdrantClientTrait for MockQdrantClient {
        async fn health_check(&self) -> Result<HealthCheckReply, SagittaError> {
            Ok(HealthCheckReply { title: "mock".to_string(), version: "mock".to_string(), commit: None })
        }

        async fn delete_collection(&self, _collection_name: String) -> Result<bool, SagittaError> {
            if self.fail_delete_collection {
                Err(SagittaError::Other("Mock: Failed to delete collection".to_string()))
            } else {
                Ok(true)
            }
        }

        async fn search_points(&self, _request: SearchPoints) -> Result<SearchResponse, SagittaError> {
            unimplemented!("MockQdrantClient search_points not implemented for these tests")
        }

        async fn get_collection_info(&self, collection_name: String) -> Result<CollectionInfo, SagittaError> {
            if self.collection_exists_response {
                Ok(CollectionInfo {
                    status: CollectionStatus::Green as i32,
                    optimizer_status: Some(OptimizerStatus { ok: true, error: String::new() }),
                    vectors_count: Some(0u64),
                    indexed_vectors_count: Some(0u64),
                    points_count: Some(0u64),
                    segments_count: 0u64,
                    config: None, 
                    payload_schema: HashMap::new(),
                })
            } else {
                Err(SagittaError::RepositoryNotFound(format!("Mock: Collection {} not found", collection_name)))
            }
        }

        async fn count(&self, _request: CountPoints) -> Result<CountResponse, SagittaError> {
            unimplemented!("MockQdrantClient count not implemented")
        }

        async fn collection_exists(&self, _collection_name: String) -> Result<bool, SagittaError> {
            Ok(self.collection_exists_response)
        }

        async fn delete_points_blocking(&self, _collection_name: &str, _points_selector: &PointsSelector) -> Result<(), SagittaError> {
            unimplemented!("MockQdrantClient delete_points_blocking not implemented")
        }

        async fn scroll(&self, _request: ScrollPoints) -> Result<ScrollResponse, SagittaError> {
            unimplemented!("MockQdrantClient scroll not implemented")
        }

        async fn upsert_points(&self, _request: UpsertPoints) -> Result<PointsOperationResponse, SagittaError> {
            unimplemented!("MockQdrantClient upsert_points not implemented")
        }

        async fn create_collection(&self, _collection_name: &str, _vector_dimension: u64) -> Result<bool, SagittaError> {
            Ok(true)
        }

        async fn create_collection_detailed(&self, _request: CreateCollection) -> Result<bool, SagittaError> {
            Ok(true)
        }

        async fn delete_points(&self, _request: DeletePoints) -> Result<PointsOperationResponse, SagittaError> {
            unimplemented!("MockQdrantClient delete_points not implemented")
        }

        async fn query_points(&self, _request: QueryPoints) -> Result<QueryResponse, SagittaError> {
            unimplemented!("MockQdrantClient query_points not implemented")
        }
        async fn query(&self, _request: QueryPoints) -> Result<QueryResponse, SagittaError> {
            unimplemented!("MockQdrantClient query not implemented")
        }
        
        async fn list_collections(&self) -> Result<Vec<String>, SagittaError> {
            unimplemented!("MockQdrantClient list_collections not implemented")
        }
    }

    fn create_test_repo_config(name: &str) -> RepositoryConfig {
        RepositoryConfig {
            name: name.to_string(),
            url: format!("file:///tmp/test_repo_{}", name),
            local_path: PathBuf::from(format!("/tmp/test_repo_{}", name)), 
            default_branch: "main".to_string(),
            active_branch: Some("main".to_string()),
            last_synced_commits: HashMap::new(),
            tracked_branches: vec!["main".to_string()],
            remote_name: Some("origin".to_string()),
            ssh_key_path: None,
            ssh_key_passphrase: None,
            indexed_languages: None,
            added_as_local_path: false,
            target_ref: None,
            dependencies: Vec::new(),
            last_synced_commit: None,
        }
    }

    fn create_test_app_config(repositories: Vec<RepositoryConfig>, temp_dir_path_str: String) -> Arc<RwLock<AppConfig>> {
        // Create the directory structure first
        let temp_dir_path = PathBuf::from(&temp_dir_path_str);
        std::fs::create_dir_all(&temp_dir_path).expect("Failed to create temp directory");
        
        let model_path = temp_dir_path.join("model.onnx");
        let tokenizer_path = temp_dir_path.join("tokenizer.json");
        let test_config_path = temp_dir_path.join("test_config.toml");

        // Set up test isolation to prevent overwriting user's real config
        std::env::set_var("SAGITTA_TEST_CONFIG_PATH", test_config_path.to_str().unwrap());

        // Create dummy ONNX model file (content doesn't matter as much as existence for some basic checks)
        fs::write(&model_path, "dummy model content").expect("Failed to write dummy model file");
        
        // Create a minimal, structurally valid tokenizer.json
        let min_tokenizer_content = json!({
          "version": "1.0",
          "truncation": null,
          "padding": null,
          "added_tokens": [],
          "normalizer": null,
          "pre_tokenizer": null,
          "post_processor": null,
          "decoder": null,
          "model": {
            "type": "WordPiece",
            "unk_token": "[UNK]",
            "continuing_subword_prefix": "##",
            "max_input_chars_per_word": 100,
            "vocab": {
              "[UNK]": 0,
              "[CLS]": 1,
              "[SEP]": 2
            }
          }
        });
        fs::write(&tokenizer_path, min_tokenizer_content.to_string()).expect("Failed to write dummy tokenizer file");

        Arc::new(RwLock::new(AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: None,
            server_api_key_path: None,
            repositories_base_path: Some(temp_dir_path_str.clone()),
            vocabulary_base_path: Some(temp_dir_path_str.clone()),
            repositories,
            active_repository: None,
            indexing: sagitta_search::config::IndexingConfig::default(),
            performance: sagitta_search::config::PerformanceConfig::default(),
            embedding: sagitta_search::config::EmbeddingEngineConfig::default(),
        }))
    }
    
    fn is_access_denied(err: &ErrorObject) -> bool {
        err.code == error_codes::ACCESS_DENIED
    }

    #[tokio::test]
    async fn test_handle_repository_list() {
        let temp_dir = tempdir().unwrap();
        let repo_base = temp_dir.path().join("repos");
        let vocab_base = temp_dir.path().join("vocab");
        let _model_base = temp_dir.path().join("model");

        let test_config = AppConfig {
            qdrant_url: "http://localhost:6334".to_string(),
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            embed_model: None,
            server_api_key_path: None,
            repositories_base_path: Some(repo_base.to_string_lossy().into_owned()),
            vocabulary_base_path: Some(vocab_base.to_string_lossy().into_owned()),
            repositories: Vec::new(),
            active_repository: None,
            indexing: IndexingConfig::default(),
            performance: PerformanceConfig {
                vector_dimension: 128,
                collection_name_prefix: "test_collection_".to_string(),
                ..PerformanceConfig::default()
            },
            embedding: sagitta_search::config::EmbeddingEngineConfig::default(),
        };

        let config = Arc::new(RwLock::new(test_config));
        let result = handle_repository_list(RepositoryListParams {}, config, None).await.unwrap();
        assert!(result.repositories.is_empty());
    }

    /*
    #[cfg(feature = "multi_tenant")]
    #[tokio::test]
    async fn test_handle_repository_remove_tenant_isolation() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path_str = temp_dir.path().to_string_lossy().into_owned();
        let qdrant_client = Arc::new(MockQdrantClient::default());

        let tenant_a_id = "tenant_a";
        let tenant_b_id = "tenant_b";
        let repo_for_a_name = "repo_for_a";
        let repo_for_b_name = "repo_for_b";
        let global_repo_name = "global_repo";

        let repo_for_a = create_test_repo_config(repo_for_a_name, Some(tenant_a_id.to_string()));
        let repo_for_b = create_test_repo_config(repo_for_b_name, Some(tenant_b_id.to_string()));
        let global_repo = create_test_repo_config(global_repo_name, Some("default_mcp_instance_tenant_for_tests".to_string()));

        // Scenario 1: User Tenant A tries to remove Repo Tenant A (SUCCESS - auth passes)
        let config_s1 = create_test_app_config(vec![repo_for_a.clone()], format!("{}/s1", temp_dir_path_str));
        let auth_user_s1 = create_test_auth_user(Some(tenant_a_id));
        let params_s1 = RepositoryRemoveParams { name: repo_for_a_name.to_string() };
        let result_s1 = handle_repository_remove(params_s1, config_s1.clone(), qdrant_client.clone(), auth_user_s1).await;
        // In test environment, we might get CONFIG_SAVE_FAILED which is acceptable
        match &result_s1 {
            Ok(_) => {}, // Success is good
            Err(e) if e.code == error_codes::CONFIG_SAVE_FAILED => {}, // Expected in test env
            Err(e) => {
                assert!(!is_access_denied(&e), "S1: Should not be ACCESS_DENIED, got error code: {}", e.code);
            }
        }

        // Scenario 2: User Tenant A tries to remove Repo Tenant B (FAIL - ACCESS_DENIED)
        let config_s2 = create_test_app_config(vec![repo_for_b.clone()], format!("{}/s2", temp_dir_path_str));
        let auth_user_s2 = create_test_auth_user(Some(tenant_a_id));
        let params_s2 = RepositoryRemoveParams { name: repo_for_b_name.to_string() };
        let result_s2 = handle_repository_remove(params_s2, config_s2.clone(), qdrant_client.clone(), auth_user_s2).await;
        assert!(result_s2.is_err(), "S2: Expected error");
        assert!(result_s2.err().map_or(false, |e| is_access_denied(&e)), "S2: Expected ACCESS_DENIED");

        // Scenario 3: User Tenant A tries to remove Global Repo (FAIL - ACCESS_DENIED)
        let config_s3 = create_test_app_config(vec![global_repo.clone()], format!("{}/s3", temp_dir_path_str));
        let auth_user_s3 = create_test_auth_user(Some(tenant_a_id));
        let params_s3 = RepositoryRemoveParams { name: global_repo_name.to_string() };
        let result_s3 = handle_repository_remove(params_s3, config_s3.clone(), qdrant_client.clone(), auth_user_s3).await;
        assert!(result_s3.is_err(), "S3: Expected error");
        assert!(result_s3.err().map_or(false, |e| is_access_denied(&e)), "S3: Expected ACCESS_DENIED");

        // Scenario 4: Global User tries to remove Repo Tenant A (FAIL - ACCESS_DENIED)
        let config_s4 = create_test_app_config(vec![repo_for_a.clone()], format!("{}/s4", temp_dir_path_str));
        let auth_user_s4 = create_test_auth_user(None); 
        let params_s4 = RepositoryRemoveParams { name: repo_for_a_name.to_string() };
        let result_s4 = handle_repository_remove(params_s4, config_s4.clone(), qdrant_client.clone(), auth_user_s4).await;
        assert!(result_s4.is_err(), "S4: Expected error");
        assert!(result_s4.err().map_or(false, |e| is_access_denied(&e)), "S4: Expected ACCESS_DENIED");

        // Scenario 5: Global User tries to remove Global Repo (SUCCESS - auth passes)
        let config_s5 = create_test_app_config(vec![global_repo.clone()], format!("{}/s5", temp_dir_path_str));
        let auth_user_s5 = create_test_auth_user(None); 
        let params_s5 = RepositoryRemoveParams { name: global_repo_name.to_string() };
        let result_s5 = handle_repository_remove(params_s5, config_s5.clone(), qdrant_client.clone(), auth_user_s5).await;
        assert!(result_s5.is_ok(), "S5: Expected Ok or non-ACCESS_DENIED error, got {:?}", result_s5.err().map(|e| e.code));
         if let Err(e) = result_s5 {
             assert!(!is_access_denied(&e), "S5: Should not be ACCESS_DENIED");
        }
    }

    #[cfg(feature = "multi_tenant")]
    #[tokio::test]
    async fn test_handle_repository_sync_tenant_isolation() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path_str = temp_dir.path().to_string_lossy().into_owned();
        let qdrant_client = Arc::new(MockQdrantClient::default());

        let tenant_a_id = "tenant_a_sync";
        let tenant_b_id = "tenant_b_sync";
        let repo_for_a_name = "repo_a_sync";
        let repo_for_b_name = "repo_b_sync";
        let global_repo_name = "global_repo_sync";

        let repo_for_a = create_test_repo_config(repo_for_a_name, Some(tenant_a_id.to_string()));
        let repo_for_b = create_test_repo_config(repo_for_b_name, Some(tenant_b_id.to_string()));
        let global_repo = create_test_repo_config(global_repo_name, Some("default_mcp_instance_tenant_for_tests".to_string()));

        // Scenario 1: User Tenant A syncs Repo Tenant A (Auth Pass)
        let config_s1 = create_test_app_config(vec![repo_for_a.clone()], temp_dir_path_str.clone());
        let auth_user_s1 = create_test_auth_user(Some(tenant_a_id));
        let params_s1 = RepositorySyncParams { name: repo_for_a_name.to_string(), force: None, extensions: None };
        let result_s1 = handle_repository_sync(params_s1, config_s1.clone(), qdrant_client.clone(), auth_user_s1).await;
        if let Err(e) = &result_s1 {
            assert!(!is_access_denied(e), "S1 Sync: Should not be ACCESS_DENIED, got code: {}", e.code);
        }

        // Scenario 2: User Tenant A attempts to sync Repo Tenant B (FAIL - ACCESS_DENIED)
        let config_s2 = create_test_app_config(vec![repo_for_b.clone()], temp_dir_path_str.clone());
        let auth_user_s2 = create_test_auth_user(Some(tenant_a_id));
        let params_s2 = RepositorySyncParams { name: repo_for_b_name.to_string(), force: None, extensions: None };
        let result_s2 = handle_repository_sync(params_s2, config_s2.clone(), qdrant_client.clone(), auth_user_s2).await;
        assert!(result_s2.is_err(), "S2 Sync: Expected error");
        if let Err(e) = &result_s2 {
            assert!(is_access_denied(e), 
                "S2 Sync: Expected ACCESS_DENIED, but got error code: {} ({})", 
                e.code, e.message
            );
        } else {
            // This case should not be reached if the first assert passes, but good for completeness
            panic!("S2 Sync: Expected error, but got Ok. This should be unreachable."); 
        }

        // Scenario 3: User Tenant A attempts to sync Global Repo (FAIL - ACCESS_DENIED)
        let config_s3 = create_test_app_config(vec![global_repo.clone()], temp_dir_path_str.clone());
        let auth_user_s3 = create_test_auth_user(Some(tenant_a_id));
        let params_s3 = RepositorySyncParams { name: global_repo_name.to_string(), force: None, extensions: None };
        let result_s3 = handle_repository_sync(params_s3, config_s3.clone(), qdrant_client.clone(), auth_user_s3).await;
        assert!(result_s3.is_err(), "S3 Sync: Expected error");
        assert!(result_s3.err().map_or(false, |e| is_access_denied(&e)), "S3 Sync: Expected ACCESS_DENIED");

        // Scenario 4: Global User attempts to sync Repo Tenant A (FAIL - ACCESS_DENIED)
        let config_s4 = create_test_app_config(vec![repo_for_a.clone()], temp_dir_path_str.clone());
        let auth_user_s4 = create_test_auth_user(None);
        let params_s4 = RepositorySyncParams { name: repo_for_a_name.to_string(), force: None, extensions: None };
        let result_s4 = handle_repository_sync(params_s4, config_s4.clone(), qdrant_client.clone(), auth_user_s4).await;
        assert!(result_s4.is_err(), "S4 Sync: Expected error");
        assert!(result_s4.err().map_or(false, |e| is_access_denied(&e)), "S4 Sync: Expected ACCESS_DENIED");

        // Scenario 5: Global User syncs Global Repo (Auth Pass, but will fail later in sync logic)
        let config_s5 = create_test_app_config(vec![global_repo.clone()], temp_dir_path_str.clone());
        let auth_user_s5 = create_test_auth_user(None);
        let params_s5 = RepositorySyncParams { name: global_repo_name.to_string(), force: None, extensions: None };
        let result_s5 = handle_repository_sync(params_s5, config_s5.clone(), qdrant_client.clone(), auth_user_s5).await;
        assert!(result_s5.is_err(), "S5 Sync: Expected an error from core sync logic for global repo, not OK");
        if let Err(e) = &result_s5 {
            assert!(!is_access_denied(e), "S5 Sync: Should not be ACCESS_DENIED, but an internal error. Got: {}", e.code);
            assert_eq!(e.code, error_codes::INTERNAL_ERROR, "S5 Sync: Expected INTERNAL_ERROR for global repo sync, got {}", e.code);
        }
    }

    #[cfg(feature = "multi_tenant")]
    #[tokio::test]
    async fn test_handle_repository_search_file_tenant_isolation() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path_str = temp_dir.path().to_string_lossy().into_owned();

        let tenant_a_id = "tenant_a_search";
        let tenant_b_id = "tenant_b_search";
        let repo_a_name = "repo_a_search";
        let repo_b_name = "repo_b_search";
        let global_repo_name = "global_repo_search";

        let repo_a_config = create_test_repo_config(repo_a_name, Some(tenant_a_id.to_string()));
        std::fs::create_dir_all(&repo_a_config.local_path).expect("Failed to create dummy repo path for repo_a_search");
        let repo_b_config = create_test_repo_config(repo_b_name, Some(tenant_b_id.to_string()));
        let global_repo_config = create_test_repo_config(global_repo_name, Some("default_mcp_instance_tenant_for_tests".to_string()));
        std::fs::create_dir_all(&global_repo_config.local_path).expect("Failed to create dummy repo path for global_repo_search");

        // Scenario 1: User Tenant A searches Repo Tenant A (Auth Pass)
        let config_s1 = create_test_app_config(vec![repo_a_config.clone()], temp_dir_path_str.clone());
        let auth_user_s1 = create_test_auth_user(Some(tenant_a_id));
        let params_s1 = RepositorySearchFileParams { repository_name: Some(repo_a_name.to_string()), pattern: "*.rs".to_string(), case_sensitive: None };
        let result_s1 = handle_repository_search_file(params_s1, config_s1.clone(), auth_user_s1).await;
        assert!(result_s1.is_ok(), "S1 Search: Expected Ok, got {:?}", result_s1.err().map(|e| e.code));

        // Scenario 2: User Tenant A attempts to search Repo Tenant B (FAIL - ACCESS_DENIED)
        let config_s2 = create_test_app_config(vec![repo_b_config.clone()], temp_dir_path_str.clone());
        let auth_user_s2 = create_test_auth_user(Some(tenant_a_id));
        let params_s2 = RepositorySearchFileParams { repository_name: Some(repo_b_name.to_string()), pattern: "*.rs".to_string(), case_sensitive: None };
        let result_s2 = handle_repository_search_file(params_s2, config_s2.clone(), auth_user_s2).await;
        assert!(result_s2.is_err(), "S2 Search: Expected error");
        assert!(result_s2.err().map_or(false, |e| is_access_denied(&e)), "S2 Search: Expected ACCESS_DENIED");

        // Scenario 3: User Tenant A attempts to search Global Repo (FAIL - ACCESS_DENIED)
        let config_s3 = create_test_app_config(vec![global_repo_config.clone()], temp_dir_path_str.clone());
        let auth_user_s3 = create_test_auth_user(Some(tenant_a_id));
        let params_s3 = RepositorySearchFileParams { repository_name: Some(global_repo_name.to_string()), pattern: "*.rs".to_string(), case_sensitive: None };
        let result_s3 = handle_repository_search_file(params_s3, config_s3.clone(), auth_user_s3).await;
        assert!(result_s3.is_err(), "S3 Search: Expected error");
        assert!(result_s3.err().map_or(false, |e| is_access_denied(&e)), "S3 Search: Expected ACCESS_DENIED");

        // Scenario 4: Global User attempts to search Repo Tenant A (FAIL - ACCESS_DENIED)
        let config_s4 = create_test_app_config(vec![repo_a_config.clone()], temp_dir_path_str.clone());
        let auth_user_s4 = create_test_auth_user(None); 
        let params_s4 = RepositorySearchFileParams { repository_name: Some(repo_a_name.to_string()), pattern: "*.rs".to_string(), case_sensitive: None };
        let result_s4 = handle_repository_search_file(params_s4, config_s4.clone(), auth_user_s4).await;
        assert!(result_s4.is_err(), "S4 Search: Expected error");
        assert!(result_s4.err().map_or(false, |e| is_access_denied(&e)), "S4 Search: Expected ACCESS_DENIED");

        // Scenario 5: Global User searches Global Repo (Auth Pass)
        let config_s5 = create_test_app_config(vec![global_repo_config.clone()], temp_dir_path_str.clone());
        let auth_user_s5 = create_test_auth_user(None); 
        let params_s5 = RepositorySearchFileParams { repository_name: Some(global_repo_name.to_string()), pattern: "*.rs".to_string(), case_sensitive: None };
        let result_s5 = handle_repository_search_file(params_s5, config_s5.clone(), auth_user_s5).await;
        assert!(result_s5.is_ok(), "S5 Search: Expected Ok, got {:?}", result_s5.err().map(|e| e.code));
    }

    #[cfg(feature = "multi_tenant")]
    #[tokio::test]
    async fn test_handle_repository_view_file_tenant_isolation() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path_str = temp_dir.path().to_string_lossy().into_owned();

        let tenant_a_id = "tenant_a_view";
        let tenant_b_id = "tenant_b_view";
        let repo_a_name = "repo_a_view";
        let repo_b_name = "repo_b_view";
        let global_repo_name = "global_repo_view";
        let dummy_file_name = "file.txt";

        let repo_a_config = create_test_repo_config(repo_a_name, Some(tenant_a_id.to_string()));
        std::fs::create_dir_all(&repo_a_config.local_path).expect("Failed to create dummy repo path for repo_a_view");
        std::fs::write(repo_a_config.local_path.join(dummy_file_name), "content").expect("Failed to write dummy file");

        let repo_b_config = create_test_repo_config(repo_b_name, Some(tenant_b_id.to_string()));
        let global_repo_config = create_test_repo_config(global_repo_name, Some("default_mcp_instance_tenant_for_tests".to_string()));
        std::fs::create_dir_all(&global_repo_config.local_path).expect("Failed to create dummy repo path for global_repo_view");
        std::fs::write(global_repo_config.local_path.join(dummy_file_name), "global content").expect("Failed to write dummy global file");

        // Scenario 1: User Tenant A views file in Repo Tenant A (Auth Pass)
        let config_s1 = create_test_app_config(vec![repo_a_config.clone()], temp_dir_path_str.clone());
        let auth_user_s1 = create_test_auth_user(Some(tenant_a_id));
        let params_s1 = RepositoryViewFileParams { repository_name: Some(repo_a_name.to_string()), file_path: dummy_file_name.to_string(), start_line: None, end_line: None };
        let result_s1 = handle_repository_view_file(params_s1, config_s1.clone(), auth_user_s1).await;
        assert!(result_s1.is_ok(), "S1 View: Expected Ok, got {:?}", result_s1.err().map(|e| e.code));

        // Scenario 2: User Tenant A attempts to view file in Repo Tenant B (FAIL - ACCESS_DENIED)
        let config_s2 = create_test_app_config(vec![repo_b_config.clone()], temp_dir_path_str.clone());
        let auth_user_s2 = create_test_auth_user(Some(tenant_a_id));
        let params_s2 = RepositoryViewFileParams { repository_name: Some(repo_b_name.to_string()), file_path: dummy_file_name.to_string(), start_line: None, end_line: None };
        let result_s2 = handle_repository_view_file(params_s2, config_s2.clone(), auth_user_s2).await;
        assert!(result_s2.is_err(), "S2 View: Expected error");
        assert!(result_s2.err().map_or(false, |e| is_access_denied(&e)), "S2 View: Expected ACCESS_DENIED");

        // Scenario 3: User Tenant A attempts to view file in Global Repo (FAIL - ACCESS_DENIED)
        let config_s3 = create_test_app_config(vec![global_repo_config.clone()], temp_dir_path_str.clone());
        let auth_user_s3 = create_test_auth_user(Some(tenant_a_id));
        let params_s3 = RepositoryViewFileParams { repository_name: Some(global_repo_name.to_string()), file_path: dummy_file_name.to_string(), start_line: None, end_line: None };
        let result_s3 = handle_repository_view_file(params_s3, config_s3.clone(), auth_user_s3).await;
        assert!(result_s3.is_err(), "S3 View: Expected error");
        assert!(result_s3.err().map_or(false, |e| is_access_denied(&e)), "S3 View: Expected ACCESS_DENIED");

        // Scenario 4: Global User attempts to view file in Repo Tenant A (FAIL - ACCESS_DENIED)
        let config_s4 = create_test_app_config(vec![repo_a_config.clone()], temp_dir_path_str.clone());
        let auth_user_s4 = create_test_auth_user(None); 
        let params_s4 = RepositoryViewFileParams { repository_name: Some(repo_a_name.to_string()), file_path: dummy_file_name.to_string(), start_line: None, end_line: None };
        let result_s4 = handle_repository_view_file(params_s4, config_s4.clone(), auth_user_s4).await;
        assert!(result_s4.is_err(), "S4 View: Expected error");
        assert!(result_s4.err().map_or(false, |e| is_access_denied(&e)), "S4 View: Expected ACCESS_DENIED");

        // Scenario 5: Global User views file in Global Repo (Auth Pass)
        let config_s5 = create_test_app_config(vec![global_repo_config.clone()], temp_dir_path_str.clone());
        let auth_user_s5 = create_test_auth_user(None); 
        let params_s5 = RepositoryViewFileParams { repository_name: Some(global_repo_name.to_string()), file_path: dummy_file_name.to_string(), start_line: None, end_line: None };
        let result_s5 = handle_repository_view_file(params_s5, config_s5.clone(), auth_user_s5).await;
        assert!(result_s5.is_ok(), "S5 View: Expected Ok, got {:?}", result_s5.err().map(|e| e.code));
    }

    #[tokio::test]
    async fn test_handle_repository_list_branches_repo_not_found() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path_str = temp_dir.path().to_string_lossy().into_owned();

        let config = create_test_app_config(vec![], temp_dir_path_str);
        let auth_user = create_test_auth_user(Some("test_tenant"));

        let params = RepositoryListBranchesParams {
            repository_name: "nonexistent_repo".to_string(),
            filter: None,
            include_remote: true,
            include_tags: true,
            limit: 50,
        };

        let result = handle_repository_list_branches(params, config, auth_user).await;
        assert!(result.is_err(), "Expected error for nonexistent repository");
        
        let error = result.unwrap_err();
        assert_eq!(error.code, error_codes::REPO_NOT_FOUND, "Expected REPO_NOT_FOUND error");
    }

    #[tokio::test]
    async fn test_handle_repository_switch_branch_repo_not_found() {
        let temp_dir = tempdir().unwrap();
        let temp_dir_path_str = temp_dir.path().to_string_lossy().into_owned();
        let qdrant_client = Arc::new(MockQdrantClient::default());

        let config = create_test_app_config(vec![], temp_dir_path_str);
        let auth_user = create_test_auth_user(Some("test_tenant"));

        let params = RepositorySwitchBranchParams {
            repository_name: "nonexistent_repo".to_string(),
            branch_name: Some("main".to_string()),
            target_ref: None,
            force: false,
            no_auto_resync: false,
        };

        let result = handle_repository_switch_branch(params, config, qdrant_client, auth_user).await;
        assert!(result.is_err(), "Expected error for nonexistent repository");
        
        let error = result.unwrap_err();
        assert_eq!(error.code, error_codes::REPO_NOT_FOUND, "Expected REPO_NOT_FOUND error");
    }
    */
    
    #[tokio::test]
    async fn test_search_file_pattern_validation() {
        let temp_dir = tempdir().unwrap();
        let mut repo_config = create_test_repo_config("test_repo");
        repo_config.local_path = temp_dir.path().join("test_repo");
        
        // Create the repository directory
        std::fs::create_dir_all(&repo_config.local_path).unwrap();
        
        let config = create_test_app_config(vec![repo_config], temp_dir.path().to_str().unwrap().to_string());
        
        // Test overly broad patterns that should be rejected
        let dangerous_patterns = vec!["*", "**", "**/*", "*/*", "**/**"];
        
        for pattern in dangerous_patterns {
            let params = RepositorySearchFileParams {
                repository_name: Some("test_repo".to_string()),
                pattern: pattern.to_string(),
                case_sensitive: Some(false),
            };
            
            let result = handle_repository_search_file(params, config.clone(), None).await;
            assert!(result.is_err(), "Pattern '{}' should be rejected", pattern);
            
            let error = result.unwrap_err();
            assert_eq!(error.code, error_codes::INVALID_PARAMS);
            assert!(error.message.contains("too broad"), 
                "Error message for pattern '{}' should mention it's too broad", pattern);
        }
        
        // Test patterns with too few non-wildcard characters
        let insufficient_patterns = vec!["?", "*?", "?*", "*.?", "?.?"];
        
        for pattern in insufficient_patterns {
            let params = RepositorySearchFileParams {
                repository_name: Some("test_repo".to_string()),
                pattern: pattern.to_string(),
                case_sensitive: Some(false),
            };
            
            let result = handle_repository_search_file(params, config.clone(), None).await;
            assert!(result.is_err(), "Pattern '{}' should be rejected", pattern);
            
            let error = result.unwrap_err();
            assert_eq!(error.code, error_codes::INVALID_PARAMS);
            assert!(error.message.contains("at least 2 non-wildcard characters"), 
                "Error message for pattern '{}' should mention character requirement", pattern);
        }
        
        // Test valid patterns that should be accepted
        let valid_patterns = vec!["*.rs", "test*", "*config*", "src/*.js", "**/*.md"];
        
        for pattern in valid_patterns {
            let params = RepositorySearchFileParams {
                repository_name: Some("test_repo".to_string()),
                pattern: pattern.to_string(),
                case_sensitive: Some(false),
            };
            
            // We expect these to succeed (though they may return empty results)
            let result = handle_repository_search_file(params, config.clone(), None).await;
            assert!(result.is_ok(), "Pattern '{}' should be accepted", pattern);
        }
    }
    
    #[tokio::test]
    async fn test_search_file_result_limit() {
        use std::fs;
        
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path().join("test_repo");
        fs::create_dir_all(&repo_path).unwrap();
        
        // Create more than 1000 test files
        for i in 0..1100 {
            let file_path = repo_path.join(format!("test_file_{}.txt", i));
            fs::write(&file_path, format!("Test content {}", i)).unwrap();
        }
        
        let mut repo_config = create_test_repo_config("test_repo");
        repo_config.local_path = repo_path;
        
        let config = create_test_app_config(vec![repo_config], temp_dir.path().to_str().unwrap().to_string());
        
        let params = RepositorySearchFileParams {
            repository_name: Some("test_repo".to_string()),
            pattern: "test_file_*.txt".to_string(),
            case_sensitive: Some(false),
        };
        
        let result = handle_repository_search_file(params, config, None).await;
        assert!(result.is_ok(), "Search should succeed");
        
        let search_result = result.unwrap();
        assert_eq!(search_result.matching_files.len(), 1000, 
            "Result should be limited to 1000 files");
        
        // Verify we got the expected files (they should be sorted somehow)
        for file in &search_result.matching_files {
            assert!(file.contains("test_file_"), "File should match pattern");
            assert!(file.ends_with(".txt"), "File should have .txt extension");
        }
    }
} 