use crate::mcp::{
    error_codes,
    types::{
        ErrorObject, RepositoryAddParams, RepositoryAddResult, RepositoryInfo, RepositoryListParams,
        RepositoryListResult, RepositoryRemoveParams, RepositoryRemoveResult, RepositorySyncParams,
        RepositorySyncResult,
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
    repo_helpers::{delete_repository_data, get_collection_name, sync_repository_branch},
};
use crate::server::map_core_error_to_user_message; // Import helper from server for now

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
    let commit_hash: String;
    let status_message: String;
    let target_ref_name: Option<String>; // To store the target ref name if it exists

    // --- Sync Stage (Calls core function that handles both static ref and dynamic branch) ---
    {
        // Acquire write lock for sync_repository_branch and config save
        let mut config_write_guard = config.write().await;

        let repo_index = config_write_guard
            .repositories
            .iter()
            .position(|r| r.name == repo_name)
            .ok_or_else(|| ErrorObject {
                code: error_codes::REPO_NOT_FOUND,
                message: format!("Repository '{}' not found during sync", repo_name),
                data: None,
            })?;

        // Store target_ref value before calling sync, as config might change
        target_ref_name = config_write_guard.repositories[repo_index].target_ref.clone();

        // Call the core sync function - it handles static vs dynamic internally
        let sync_result = sync_repository_branch(
            &*config_write_guard,
            repo_index,
            qdrant_client.clone(),
            true, // fetch_and_merge flag (currently unused in core func but kept)
        )
        .await;

        match sync_result {
            Ok(sync_output) => {
                // sync_repository_branch returns the commit hash for static refs
                // or the new HEAD commit hash for successfully merged dynamic branches
                // or a status message string for non-fast-forward cases (ahead, diverged)
                
                // Check if the output looks like a commit hash (e.g., 40 hex chars)
                if sync_output.len() == 40 && sync_output.chars().all(|c| c.is_ascii_hexdigit()) {
                    commit_hash = sync_output;
                    info!(repo_name=%repo_name, commit=%commit_hash, "Sync successful, proceeding to index.");
                    
                    // Save config (sync_repository_branch might update last_synced_commit for dynamic branches)
                    if let Err(e) = save_config(&*config_write_guard, None) {
                        error!(error = %e, "Failed to save config after repository sync");
                        return Err(ErrorObject {
                            code: error_codes::CONFIG_SAVE_FAILED,
                            message: format!("Failed to save configuration after sync: {}", e),
                            data: None,
                        });
                    }
                } else {
                    // Output is likely a status message (up-to-date, ahead, diverged, etc.)
                    info!(repo_name=%repo_name, status=%sync_output, "Sync resulted in status message, no new commit to index.");
                    // Save config anyway, as fetch might have updated things
                    if let Err(e) = save_config(&*config_write_guard, None) {
                        error!(error = %e, "Failed to save config after repository sync resulted in status message");
                        // Don't error out here, just log, as the sync itself didn't fail
                    }
                    // Return a specific result indicating no indexing occurred
                    return Ok(RepositorySyncResult {
                        name: repo_name,
                        status: sync_output, // Pass the status message through
                        commit_hash: "N/A".to_string(), // Indicate no specific commit was indexed now
                    });
                }
            }
            Err(e) => {
                error!(repo_name = %repo_name, error = %e, "Core repository sync failed");
                // Save config even on sync failure
                 if let Err(save_err) = save_config(&*config_write_guard, None) {
                     error!(error = %save_err, "Failed to save config after failed repository sync attempt");
                 }
                 let user_message = map_core_error_to_user_message(&anyhow!(e), "Sync failed");
                return Err(ErrorObject {
                    code: error_codes::CORE_LOGIC_ERROR,
                     message: user_message,
                    data: None,
                });
            }
        }
    } // config_write_guard dropped here

    // Determine status message based on whether it was a static ref sync
    if let Some(ref_name) = target_ref_name {
        status_message = format!("Indexed static ref '{}'", ref_name);
    } else {
        status_message = "Synced and Indexed".to_string();
    }

    // --- Indexing Stage (Only runs if sync resulted in a commit hash) ---
    info!(repo_name = %repo_name, commit = %commit_hash, "Starting indexing stage.");

    // Get fresh clones of config needed for indexing (config might have been updated)
    let repo_config_clone: RepositoryConfig = {
        let config_read_guard = config.read().await;
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
     let app_config_clone: AppConfig = {
         let config_read_guard = config.read().await;
         (*config_read_guard).clone()
     };

    let repo_root = &repo_config_clone.local_path;
    // Use target_ref as branch context if available, otherwise use active/default branch
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
        info!(repo_name = %repo_name, count = files_to_index_rel.len(), "Found files to index");

        match indexing::index_repo_files(
            &app_config_clone, // Use cloned config (safe for both paths)
            repo_root,
            &files_to_index_rel,
            &collection_name,
            context_identifier, // Use target_ref or branch name here
            &commit_hash,
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

    Ok(RepositorySyncResult {
        name: repo_name,
        status: status_message, // Use dynamic status message
        commit_hash,
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
            repositories_base_path: Some(PathBuf::from("/base")),
            repositories: vec![repo1.clone(), repo2.clone()],
            active_repository: Some("repo1".to_string()),
            indexing: IndexingConfig { max_concurrent_upserts: 1 },
            // Other fields can be default or None as they aren't used by list
            onnx_model_path: None,
            onnx_tokenizer_path: None,
            server_api_key_path: None,
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